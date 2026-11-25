use clap::{Parser, ValueEnum};
use rag_cli::{Chunks, Document, Markdown, OpenAiClient, Plain, Rag};
use std::{num::NonZeroUsize, path::PathBuf, process::ExitCode};

#[derive(Clone, Copy, ValueEnum)]
enum ChunkerKind {
    /// Splits on headings, keeps fenced code whole (also Logseq)
    Markdown,
    /// Packs paragraphs, no structure
    Plain,
}

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

    /// How files are split into chunks
    #[arg(long, value_enum, default_value_t = ChunkerKind::Plain)]
    chunker: ChunkerKind,
}

async fn run(args: Args) -> Result<(), Box<dyn std::error::Error>> {
    let Args {
        files,
        question,
        k,
        chunk_size,
        chunker,
    } = args;
    let max_chars = chunk_size.map_or(Rag::DEFAULT_MAX_CHUNK_CHARS, NonZeroUsize::get);
    let client = OpenAiClient::from_env()?;
    let docs = Document::from_files(&files).await?;
    let chunks = match chunker {
        ChunkerKind::Markdown => Chunks::from_documents(&docs, &Markdown { max_chars }),
        ChunkerKind::Plain => Chunks::from_documents(&docs, &Plain { max_chars }),
    };
    let rag: Rag = Rag::ingest(client.clone(), client, chunks).await?;

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
