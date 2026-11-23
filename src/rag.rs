use std::{
    env,
    path::{Path, PathBuf},
    sync::Arc,
};

use tokio::{sync::Semaphore, task::JoinSet};

use crate::{Chunk, Error, SYSTEM, TextEmbedding3Small, VectorStore, build, chat, chunks, embed};

type Model = TextEmbedding3Small;
type Index = VectorStore<Model, Chunk>;

/// Maximum chunk size [`Rag::ingest`] uses, in chars.
pub const CHUNK_SIZE: usize = 1500;

/// Embedded docs, ready to answer questions about them.
pub struct Rag {
    client: reqwest::Client,
    key: Arc<str>,
    index: Index,
}

impl Rag {
    /// Reads, chunks and embeds `files` in chunks of up to [`CHUNK_SIZE`] chars.
    pub async fn ingest(files: &[PathBuf]) -> Result<Self, Error> {
        Self::ingest_with(files, CHUNK_SIZE).await
    }

    /// Reads, chunks and embeds `files` in chunks of up to `chunk_size` chars.
    /// Needs `OPENAI_API_KEY` set.
    pub async fn ingest_with(files: &[PathBuf], chunk_size: usize) -> Result<Self, Error> {
        let docs = read_docs(files).await?;
        let key: Arc<str> = env::var("OPENAI_API_KEY")
            .map_err(|_| Error::Permanent("OPENAI_API_KEY is not set".into()))?
            .into();
        let client = reqwest::Client::new(); // per-request timeout is set in openai::post
        let index = embed_chunks(&client, &key, &docs, chunk_size).await?;
        Ok(Self { client, key, index })
    }

    /// Number of chunks stored in the index.
    pub fn chunk_count(&self) -> usize {
        self.index.len()
    }

    /// Embeds `question` and returns the `k` closest chunks, best match first.
    pub async fn retrieve(&self, question: &str, k: usize) -> Result<Vec<Hit>, Error> {
        let q = embed::<Model>(&self.client, &self.key, question).await?;
        Ok(self
            .index
            .top_k(&q, k)
            .into_iter()
            .map(|(score, chunk)| Hit {
                score,
                chunk: chunk.clone(),
            })
            .collect())
    }

    /// Asks the model to answer `question` from the `k` chunks closest to it.
    pub async fn ask(&self, question: &str, k: usize) -> Result<Answer, Error> {
        let hits = self.retrieve(question, k).await?;
        let top: Vec<(f32, &Chunk)> = hits.iter().map(|h| (h.score, &h.chunk)).collect();
        let text = chat(&self.client, &self.key, SYSTEM, &build(question, &top)).await?;
        Ok(Answer { text, hits })
    }
}

/// A retrieved chunk along with its cosine similarity to the query.
#[derive(Debug, Clone)]
pub struct Hit {
    pub score: f32,
    pub chunk: Chunk,
}

/// The model's answer, and the chunks it was given.
#[derive(Debug)]
pub struct Answer {
    pub text: String,
    /// Chunks placed in the prompt, best match first (a file can repeat).
    pub hits: Vec<Hit>,
}

impl Answer {
    /// Source path of each retrieved chunk, best match first.
    pub fn sources(&self) -> Vec<&Path> {
        self.hits.iter().map(|h| h.chunk.source.as_ref()).collect()
    }
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
    chunk_size: usize,
) -> Result<Index, Error> {
    let semaphore = Arc::new(Semaphore::new(4));
    let mut set = JoinSet::new();

    for (path, text) in docs {
        let source: Arc<Path> = path.as_path().into();
        for text in chunks(text, chunk_size) {
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
