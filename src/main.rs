use clap::Parser;
use rag_cli::Rag;
use std::{num::NonZeroUsize, path::PathBuf, process::ExitCode};

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
    #[arg(short, long, default_value_t = Rag::DEFAULT_K)]
    k: usize,

    /// Maximum chunk size, in chars [default: 1100]
    #[arg(short, long)]
    chunk_size: Option<NonZeroUsize>,
}

async fn run(args: Args) -> Result<(), Box<dyn std::error::Error>> {
    let Args {
        files,
        question,
        k,
        chunk_size,
    } = args;
    let rag: Rag = match chunk_size {
        Some(n) => Rag::ingest_with(&files, n.get()).await?,
        None => Rag::ingest(&files).await?,
    };

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
