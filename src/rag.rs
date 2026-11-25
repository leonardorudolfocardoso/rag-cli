use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use crate::{
    Chat, Chunk, Embedder, Error, Model, OpenAiClient, SYSTEM, TextEmbedding3Small, VectorStore,
    build, chunk_markdown,
};

/// A loaded Markdown document, pairing its source path with its contents.
#[derive(Debug, Clone)]
pub struct Document {
    pub source: Arc<Path>,
    pub text: String,
}

impl Document {
    /// Reads a Markdown document from disk.
    pub async fn from_file(path: &Path) -> std::io::Result<Self> {
        let text = tokio::fs::read_to_string(path).await?;
        Ok(Self {
            source: path.into(),
            text,
        })
    }

    /// Splits this document into chunks of at most `max_chars` chars, each tagged with its source.
    pub fn chunks(&self, max_chars: usize) -> Vec<Chunk> {
        chunk_markdown(&self.text, max_chars)
            .into_iter()
            .map(|text| Chunk {
                source: Arc::clone(&self.source),
                text: text.into(),
            })
            .collect()
    }
}

/// Embedded docs, ready to answer questions about them.
pub struct Rag<M: Model = TextEmbedding3Small, E: Embedder = OpenAiClient, C: Chat = OpenAiClient> {
    embedder: E,
    chat: C,
    index: VectorStore<M, Chunk>,
}

impl Rag {
    /// Default ceiling on chunk length used by [`Rag::ingest`], in chars.
    pub const DEFAULT_MAX_CHUNK_CHARS: usize = 1100;

    /// Default number of chunks put into the prompt by the CLI.
    pub const DEFAULT_K: usize = 5;
}

impl<M: Model + 'static> Rag<M, OpenAiClient, OpenAiClient> {
    /// Reads, chunks and embeds `files` in chunks of up to [`Rag::DEFAULT_MAX_CHUNK_CHARS`] chars.
    pub async fn ingest(files: &[PathBuf]) -> Result<Self, Error> {
        Self::ingest_with(files, Rag::DEFAULT_MAX_CHUNK_CHARS).await
    }

    /// Reads, chunks and embeds `files` in chunks of up to `max_chunk_chars` chars.
    /// Needs `OPENAI_API_KEY` set.
    pub async fn ingest_with(files: &[PathBuf], max_chunk_chars: usize) -> Result<Self, Error> {
        let client = OpenAiClient::from_env()?;
        Self::ingest_with_client(client.clone(), client, files, max_chunk_chars).await
    }
}

impl<M: Model + 'static, E: Embedder, C: Chat> Rag<M, E, C> {
    /// Reads, chunks and embeds `files` using explicit embedding and chat clients.
    pub async fn ingest_with_client(
        embedder: E,
        chat: C,
        files: &[PathBuf],
        max_chunk_chars: usize,
    ) -> Result<Self, Error> {
        let mut chunks = Vec::new();
        for path in files {
            let doc = Document::from_file(path).await?;
            chunks.extend(doc.chunks(max_chunk_chars));
        }

        let texts: Vec<&str> = chunks.iter().map(|c| c.text.as_ref()).collect();
        let embeddings = embedder.embed_batch::<M>(&texts).await?;

        let mut index = VectorStore::new();
        for (chunk, embedding) in chunks.into_iter().zip(embeddings) {
            index.insert(chunk, embedding);
        }

        Ok(Self {
            embedder,
            chat,
            index,
        })
    }
}

impl<M: Model, E: Embedder, C: Chat> Rag<M, E, C> {
    /// Replaces the chat backend while keeping the embedded index intact.
    pub fn with_chat<C2: Chat>(self, chat: C2) -> Rag<M, E, C2> {
        Rag {
            embedder: self.embedder,
            chat,
            index: self.index,
        }
    }

    /// Number of chunks stored in the index.
    pub fn chunk_count(&self) -> usize {
        self.index.len()
    }

    /// Embeds `question` and returns the `k` closest chunks, best match first.
    pub async fn retrieve(&self, question: &str, k: usize) -> Result<Vec<Hit>, Error> {
        let q = self.embedder.embed::<M>(question).await?;
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
        let text = self.chat.chat(SYSTEM, &build(question, &top)).await?;
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
