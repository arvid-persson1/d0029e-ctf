use clap::Parser;
use reqwest::{Client, Url, redirect::Policy};
use std::sync::{Arc, atomic::AtomicUsize};
use tokio::{spawn, sync::mpsc::channel};

mod scan;

use scan::*;

const BUFFER_SIZE: usize = 16;
const NUM_THREADS: usize = 64;

#[derive(Parser)]
struct Cli {
    /// The URL to the index page.
    index_url: Url,
    #[arg(short, long)]
    /// Prints information about progress.
    verbose: bool,
}

#[tokio::main]
async fn main() -> Result<(), ScanError> {
    let Cli { index_url, verbose } = Cli::parse();
    let client = Client::builder()
        .cookie_store(true)
        .redirect(Policy::limited(1))
        .build()
        .expect("Failed to initialize client.");

    // The username isn't relevant, but has to be nonempty.
    client
        .post(index_url.clone())
        .form(&[("username", "name")])
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
