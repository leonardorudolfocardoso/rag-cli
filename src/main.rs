use clap::Parser;
use rag_cli::{ChunkOpts, ask, ingest_with};
use std::{path::PathBuf, process::ExitCode};

const QUESTIONS: [(&str, &str); 5] = [
    (
        "Why should vectors be normalised when they're stored?",
        "3Nd → Nd per search, paid once at ingest",
    ),
    (
        "What goes wrong when data arrives faster than the consumer can process it, with nothing slowing the sender?",
        "memory, latency, GC, degradation",
    ),
    (
        "Where does caipora sit in the MCP picture?",
        "between host and tool servers",
    ),
    (
        "My RAG gave a wrong answer even though the right chunk was retrieved. What should I fix?",
        "the prompt (model problem)",
    ),
    ("What is PostgreSQL's default port?", "I don't know"),
];

/// Answer questions about Markdown notes.
#[derive(Parser)]
struct Args {
    /// Files to ingest (e.g. notes/*.md)
    #[arg(required = true)]
    files: Vec<PathBuf>,

    /// The question. If omitted, runs the 5 eval questions
    #[arg(short, long)]
    question: Option<String>,

    /// How many chunks go into the prompt
    #[arg(short, long, default_value_t = 3)]
    k: usize,

    /// Maximum chunk size, in chars
    #[arg(short, long, default_value_t = 1500, value_parser = clap::value_parser!(u64).range(1..))]
    chunk_size: u64,

    /// Longest paragraph repeated across a chunk boundary, in chars
    #[arg(short, long, default_value_t = 200)]
    overlap: usize,
}

async fn run(args: Args) -> Result<(), Box<dyn std::error::Error>> {
    let Args {
        files,
        question,
        k,
        chunk_size,
        overlap,
    } = args;
    let opts = ChunkOpts {
        size: chunk_size.try_into()?,
        overlap,
    };
    let rag = ingest_with(&files, opts).await?;

    match question {
        Some(question) => println!("{}", ask(&rag, &question, k).await?.text),
        None => {
            for (question, expected) in QUESTIONS {
                let got = ask(&rag, question, k).await?.text;
                println!("Q: {question}\nexpected: {expected}\ngot: {got}\n");
            }
        }
    }
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
