use clap::Parser;
use rag_cli::Rag;
use std::{path::PathBuf, process::ExitCode};

/// Answer questions about Markdown notes.
#[derive(Parser)]
struct Args {
    /// Files to ingest (e.g. notes/*.md)
    #[arg(required = true)]
    files: Vec<PathBuf>,

    /// The question to answer
    #[arg(short, long)]
    question: String,

    /// How many chunks go into the prompt
    #[arg(short, long, default_value_t = 3)]
    k: usize,

    /// Maximum chunk size, in chars
    #[arg(short, long, default_value_t = 1500, value_parser = clap::value_parser!(u64).range(1..))]
    chunk_size: u64,
}

async fn run(args: Args) -> Result<(), Box<dyn std::error::Error>> {
    let Args {
        files,
        question,
        k,
        chunk_size,
    } = args;
    let rag = Rag::ingest_with(&files, chunk_size.try_into()?).await?;

    println!("{}", rag.ask(&question, k).await?.text);
    Ok(())
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    match run(Args::parse()).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}
