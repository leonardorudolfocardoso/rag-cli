use clap::{Parser, ValueEnum};
use rag_cli::{Markdown, OpenAiClient, Plain, Rag, Rfc, index};
use std::{num::NonZeroUsize, path::PathBuf, process::ExitCode};

#[derive(Clone, Copy, ValueEnum)]
enum ChunkerKind {
    /// Splits on headings, keeps fenced code whole (also Logseq)
    Markdown,
    /// Packs paragraphs, no structure
    Plain,
    /// Splits IETF RFC plaintext on its section headings, found from the layout
    Rfc,
}

/// Answer questions about text files: Markdown notes, plaintext, IETF RFCs.
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

    /// Maximum chunk size, in chars
    #[arg(short, long, default_value_t = NonZeroUsize::new(Rag::DEFAULT_MAX_CHUNK_CHARS).unwrap())]
    chunk_size: NonZeroUsize,

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
    let max_chars = chunk_size.get();
    let client = OpenAiClient::from_env()?;
    let rag: Rag = match chunker {
        ChunkerKind::Markdown => {
            index(client.clone(), client, &files, &Markdown { max_chars }).await?
        }
        ChunkerKind::Plain => index(client.clone(), client, &files, &Plain { max_chars }).await?,
        ChunkerKind::Rfc => index(client.clone(), client, &files, &Rfc { max_chars }).await?,
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
