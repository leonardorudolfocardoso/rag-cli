use clap::Parser;
use rag_cli::{
    Chunk, Embedding, Error, SYSTEM, TextEmbedding3Small, VectorStore, build, chat, chunks, embed,
};
use std::{
    env,
    path::{Path, PathBuf},
    process::ExitCode,
    sync::Arc,
};
use tokio::{sync::Semaphore, task::JoinSet, try_join};

type Model = TextEmbedding3Small;
type Index = VectorStore<Model, Chunk>;

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
    /// Files to ingest (e.g. docs/*.md)
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

/// Reads the given files. Returns (path, contents) in the order given.
async fn read_docs(files: &[PathBuf]) -> std::io::Result<Vec<(PathBuf, String)>> {
    let mut docs = Vec::with_capacity(files.len());
    for path in files {
        let text = tokio::fs::read_to_string(path).await?;
        docs.push((path.clone(), text));
    }
    Ok(docs)
}

/// Chunks every doc and embeds the chunks, with at most 4 requests in flight.
async fn ingest(
    client: &reqwest::Client,
    key: &Arc<str>,
    docs: &[(PathBuf, String)],
    chunk_size: usize,
    overlap: usize,
) -> Result<Index, Error> {
    let semaphore = Arc::new(Semaphore::new(4));
    let mut set = JoinSet::new();

    for (path, text) in docs {
        let source: Arc<Path> = path.as_path().into();
        for text in chunks(text, chunk_size, overlap) {
            let chunk = Chunk {
                source: Arc::clone(&source),
                text: text.to_owned(),
            };
            let client = client.clone();
            let semaphore = Arc::clone(&semaphore);
            let key = Arc::clone(key);
            set.spawn(async move {
                // the semaphore is never closed, so acquire can't fail
                let _perm = semaphore.acquire().await.unwrap();
                let embedding = embed::<Model>(&client, &key, &chunk.text).await?;
                Ok::<_, Error>((chunk, embedding))
            });
        }
    }

    let mut index = Index::new();
    // returning early drops the set, which aborts the chunks still in flight
    while let Some(res) = set.join_next().await {
        let (chunk, embedding) = res.expect("embed task panicked")?;
        index.insert(chunk, embedding);
    }
    Ok(index)
}

/// Asks the model to answer `question` from the `k` chunks closest to its embedding `q`.
async fn answer(
    client: &reqwest::Client,
    key: &str,
    index: &Index,
    question: &str,
    q: &Embedding<Model>,
    k: usize,
) -> Result<String, Error> {
    let top = index.top_k(q, k);
    chat(client, key, SYSTEM, &build(question, &top)).await
}

async fn run(args: Args) -> Result<(), Box<dyn std::error::Error>> {
    let Args {
        files,
        question,
        k,
        chunk_size,
        overlap,
    } = args;
    let chunk_size = chunk_size.try_into()?;

    let docs = read_docs(&files).await?;
    let key: Arc<str> = env::var("OPENAI_API_KEY")
        .map_err(|_| "OPENAI_API_KEY is not set")?
        .into();
    let client = reqwest::Client::new(); // per-request timeout is set in openai::post

    match question {
        Some(question) => {
            // the question embeds while the docs do, instead of after them
            let (index, q) = try_join!(
                ingest(&client, &key, &docs, chunk_size, overlap),
                embed::<Model>(&client, &key, &question),
            )?;
            println!("{}", answer(&client, &key, &index, &question, &q, k).await?);
        }
        None => {
            let index = ingest(&client, &key, &docs, chunk_size, overlap).await?;
            for (question, expected) in QUESTIONS {
                let q = embed::<Model>(&client, &key, question).await?;
                let got = answer(&client, &key, &index, question, &q, k).await?;
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
