use regex::Regex;
use reqwest::{Client, Error as ReqwestError, Url};
use scraper::{ElementRef, Html, Selector};
use std::{num::ParseIntError, sync::OnceLock};
use thiserror::Error;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Scan {
    Flag(Box<str>),
    NotFound(Vec<usize>),
}

#[derive(Debug, Error)]
pub enum ScanError {
    #[error("{0}")]
    Io(#[from] ReqwestError),
    #[error("Failed to match selector.")]
    ElementNotFound,
    #[error("Element was not in the expected format")]
    UnexpectedFormat,
    #[error("{0}")]
    TicketId(#[from] ParseIntError),
}

macro_rules! selector {
    ($name:ident, $sel:expr) => {
        fn $name() -> &'static Selector {
            static S: OnceLock<Selector> = OnceLock::new();
            S.get_or_init(|| Selector::parse($sel).unwrap())
        }
    };
}

selector!(selector_meta, ".ticket-card > .ticket-meta");
selector!(selector_ticket, ".ticket-list > .ticket");
selector!(selector_ticket_id, ".ticket-header > .ticket-id");
selector!(selector_ticket_header, "h3");
selector!(selector_ticket_description, "p");

macro_rules! regex {
    ($name:ident, $pat:expr) => {
        fn $name() -> &'static Regex {
            static R: OnceLock<Regex> = OnceLock::new();
            R.get_or_init(|| Regex::new($pat).unwrap())
        }
    };
}

regex!(regex_username_header, r"^\s*User:\s*$");
// WARN: we can't distinguish between leading/trailing whitespace as part of a username or just
// included in the HTML. In one test case, there was a leading space and no trailing whitespace, so
// we take this as the format.
regex!(regex_username_field, r"^ (.*)$");
// We don't know the exact format of the flag contents, but we assume it at least doesn't contain
// any '}' characters.
regex!(regex_flag, r"flag\{(.*?)\}");
regex!(regex_ticket_id, r"^\s*Ticket #(\d+)\s*$");

fn capture<'a>(pattern: &Regex, haystack: &'a str) -> Option<&'a str> {
    pattern
        .captures(haystack)
        .map(|c| c.get(1).unwrap().as_str())
}

// `&Url` does not implement `IntoUrl`, and cloning is likely cheaper than parsing.
// See #412 in Reqwest.
pub async fn scan(client: &Client, index_url: Url, id: usize) -> Result<Scan, ScanError> {
    let ticket_page_url = index_url.join(&format!("ticket/{id}")).unwrap();
    let ticket_page = client.get(ticket_page_url).send().await?.text().await?;
    let username = get_username(&Html::parse_document(&ticket_page))?;

    let user_page = client
        .post(index_url)
        .form(&[("username", username)])
        .send()
        .await?
        .text()
        .await?;
    process_tickets(&Html::parse_document(&user_page))
}

fn get_username(html: &Html) -> Result<String, ScanError> {
    let name_field = html
        .select(selector_meta())
        .next()
        .ok_or(ScanError::ElementNotFound)?
        .text()
        .skip_while(|h| !regex_username_header().is_match(h))
        .nth(1)
        .ok_or(ScanError::UnexpectedFormat)?;

    capture(regex_username_field(), name_field)
        .map(ToOwned::to_owned)
        .ok_or(ScanError::UnexpectedFormat)
}

fn process_tickets(html: &Html) -> Result<Scan, ScanError> {
    // TODO: parallelize?
    let tickets = html.select(selector_ticket()).map(|e| parse_ticket(&e));

    let mut ids = Vec::new();
    for ticket in tickets {
        let Ticket {
            id,
            header,
            description,
        } = ticket?;
        let pat = regex_flag();
        if let Some(flag) = capture(pat, &header).or_else(|| capture(pat, &description)) {
            return Ok(Scan::Flag(flag.into()));
        } else {
            ids.push(id);
        }
    }

    Ok(Scan::NotFound(ids))
}

struct Ticket {
    id: usize,
    header: String,
    description: String,
}

fn parse_ticket(ticket: &ElementRef) -> Result<Ticket, ScanError> {
    let id_inner = ticket
        .select(selector_ticket_id())
        .next()
        .ok_or(ScanError::ElementNotFound)?
        .inner_html();
    let id = capture(regex_ticket_id(), &id_inner)
        .ok_or(ScanError::UnexpectedFormat)?
        .parse()?;

    let header = ticket
        .select(selector_ticket_header())
        .next()
        .ok_or(ScanError::ElementNotFound)?
        .inner_html();
    let description = ticket
        .select(selector_ticket_description())
        .next()
        .ok_or(ScanError::ElementNotFound)?
        .inner_html();

    Ok(Ticket {
        id,
        header,
        description,
    })
}
