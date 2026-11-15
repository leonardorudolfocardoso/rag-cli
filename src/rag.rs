use std::{
    env,
    path::{Path, PathBuf},
    sync::Arc,
};

use tokio::{sync::Semaphore, task::JoinSet};

use crate::{Chunk, Error, SYSTEM, TextEmbedding3Small, VectorStore, build, chat, chunks, embed};

type Model = TextEmbedding3Small;
type Index = VectorStore<Model, Chunk>;

/// How docs are split before they're embedded.
#[derive(Debug, Clone, Copy)]
pub struct ChunkOpts {
    /// Maximum chunk size, in chars
    pub size: usize,
    /// Longest paragraph repeated across a chunk boundary, in chars
    pub overlap: usize,
}

impl Default for ChunkOpts {
    fn default() -> Self {
        ChunkOpts {
            size: 1500,
            overlap: 200,
        }
    }
}

/// Embedded docs, ready to answer questions about them.
pub struct Rag {
    client: reqwest::Client,
    key: Arc<str>,
    index: Index,
}

/// The model's answer, and where the chunks it was given came from.
#[derive(Debug)]
pub struct Answer {
    pub text: String,
    /// Source of each chunk in the prompt, best match first (a file can repeat)
    pub sources: Vec<Arc<Path>>,
}

/// Reads, chunks and embeds `files` with the default [`ChunkOpts`].
pub async fn ingest(files: &[PathBuf]) -> Result<Rag, Error> {
    ingest_with(files, ChunkOpts::default()).await
}

/// Reads, chunks and embeds `files`. Needs `OPENAI_API_KEY` set.
pub async fn ingest_with(files: &[PathBuf], opts: ChunkOpts) -> Result<Rag, Error> {
    let docs = read_docs(files).await?;
    let key: Arc<str> = env::var("OPENAI_API_KEY")
        .map_err(|_| Error::Permanent("OPENAI_API_KEY is not set".into()))?
        .into();
    let client = reqwest::Client::new(); // per-request timeout is set in openai::post
    let index = embed_chunks(&client, &key, &docs, opts).await?;
    Ok(Rag { client, key, index })
}

/// Asks the model to answer `question` from the `k` chunks closest to it.
pub async fn ask(rag: &Rag, question: &str, k: usize) -> Result<Answer, Error> {
    let q = embed::<Model>(&rag.client, &rag.key, question).await?;
    let top = rag.index.top_k(&q, k);
    let text = chat(&rag.client, &rag.key, SYSTEM, &build(question, &top)).await?;
    let sources = top.iter().map(|(_, c)| Arc::clone(&c.source)).collect();
    Ok(Answer { text, sources })
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
async fn embed_chunks(
    client: &reqwest::Client,
    key: &Arc<str>,
    docs: &[(PathBuf, String)],
    opts: ChunkOpts,
) -> Result<Index, Error> {
    let semaphore = Arc::new(Semaphore::new(4));
    let mut set = JoinSet::new();

    for (path, text) in docs {
        let source: Arc<Path> = path.as_path().into();
        for text in chunks(text, opts.size, opts.overlap) {
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
