use std::fs;
use std::path::PathBuf;

use clap::Parser;
use heed::types::{Bytes, DecodeIgnore};
use heed::{Database, EnvOpenOptions};

/// A program that displays the size of the documents in a Meilisearch database.
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// The path to the database.
    #[arg(default_value = "data.ms")]
    path: PathBuf,

    /// Only displays the sum of the indexes documents.
    #[arg(long)]
    sum_only: bool,
}

fn main() -> anyhow::Result<()> {
    let Args { path, sum_only } = Args::parse();

    let mut total_number_of_entries = 0;
    let mut total_documents_size = 0;

    for (i, result) in fs::read_dir(path.join("indexes"))?.enumerate() {
        let entry = result?;
        let env = EnvOpenOptions::new().max_dbs(1).open(entry.path())?;

        let rtxn = env.read_txn()?;
        if let Some(db) = env.open_database(&rtxn, Some("documents"))? {
            let db: Database<DecodeIgnore, Bytes> = db;
            let mut number_of_entries = 0;
            let mut documents_size = 0;

            for result in db.iter(&rtxn)? {
                let ((), value) = result?;
                number_of_entries += 1;
                documents_size += value.len() as u64;
            }

            if !sum_only {
                let prefix = if i == 0 { "   " } else { " + " };
                println!(
                    "{prefix} number of documents: {number_of_entries}, \
                              documents size: {documents_size}B"
                );
            }

            total_number_of_entries += number_of_entries;
            total_documents_size += documents_size;
        }

        drop(rtxn);

        // We close the envs because it can take a lot of memory at some point.
        env.prepare_for_closing().wait();
    }

    if sum_only {
        println!("{total_documents_size}");
    } else {
        println!(
            "total number of documents: {total_number_of_entries}, \
             total documents size: {total_documents_size}B"
        );
    }

    Ok(())
}
