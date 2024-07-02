use std::fs;
use std::path::PathBuf;

use clap::Parser;
use heed::types::{Bytes, DecodeIgnore};
use heed::{Database, EnvOpenOptions};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use rayon::iter::{IntoParallelIterator, ParallelIterator};

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

    let m = MultiProgress::new();
    let style = ProgressStyle::with_template("{msg} {wide_bar} {pos}/{len} {eta}").unwrap();

    let indexes: anyhow::Result<Vec<_>> = fs::read_dir(path.join("indexes"))?
        .enumerate()
        .map(|(i, result)| {
            result
                .map(|entry| {
                    let pb = m.add(ProgressBar::new(0).with_style(style.clone()));
                    (i, entry, pb)
                })
                .map_err(Into::into)
        })
        .collect();

    let stats: Stats = indexes?
        .into_par_iter()
        .map(|(i, entry, pb)| {
            let env = unsafe { EnvOpenOptions::new().max_dbs(1).open(entry.path())? };

            pb.set_message("Opening read transaction...");
            let rtxn = env.read_txn()?;
            pb.set_message("");

            let stats = if let Some(db) = env.open_database(&rtxn, Some("documents"))? {
                let db: Database<DecodeIgnore, Bytes> = db;
                let mut stats = Stats::default();

                let len = db.len(&rtxn)?;
                pb.set_length(len);

                for result in db.iter(&rtxn)? {
                    let ((), value) = result?;
                    stats.number_of_entries += 1;
                    stats.documents_size += value.len() as u64;
                    pb.inc(1);
                }

                if !sum_only {
                    let Stats { number_of_entries, documents_size } = stats;
                    let avg = documents_size as f32 / number_of_entries as f32;
                    let prefix = if i == 0 { "   " } else { " + " };
                    pb.println(format!(
                        "{prefix} number of documents: {number_of_entries}, \
                              documents size: {documents_size}B (average {avg:.02}B by document)"
                    ));
                }

                stats
            } else {
                Stats::default()
            };

            drop(rtxn);

            // We close the envs because it can take a lot of memory at some point.
            env.prepare_for_closing().wait();

            Ok(stats) as anyhow::Result<_>
        })
        .reduce(
            || Ok(Stats::default()),
            |a, b| match (a, b) {
                (Ok(a), Ok(b)) => Ok(Stats {
                    number_of_entries: a.number_of_entries + b.number_of_entries,
                    documents_size: a.documents_size + b.documents_size,
                }),
                (Ok(_), Err(e)) | (Err(e), Ok(_)) | (Err(e), Err(_)) => Err(e),
            },
        )?;

    let Stats { number_of_entries, documents_size } = stats;
    if sum_only {
        println!("{documents_size}");
    } else {
        let avg = documents_size as f32 / number_of_entries as f32;
        println!(
            "total number of documents: {number_of_entries}, \
             total documents size: {documents_size}B, \
             average document size: {avg:.02}B"
        );
    }

    Ok(())
}

#[derive(Debug, Default)]
struct Stats {
    number_of_entries: u64,
    documents_size: u64,
}
