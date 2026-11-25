use std::path::Path;

use crate::{
    Chat, Chunk, Chunks, Embedder, Error, Model, OpenAiClient, SYSTEM, TextEmbedding3Small,
    VectorStore, build,
};

/// Embedded docs, ready to answer questions about them.
pub struct Rag<M: Model = TextEmbedding3Small, E: Embedder = OpenAiClient, C: Chat = OpenAiClient> {
    embedder: E,
    chat: C,
    index: VectorStore<M, Chunk>,
}

impl Rag {
    /// Default ceiling on chunk length used by the CLI, in chars.
    pub const DEFAULT_MAX_CHUNK_CHARS: usize = 1100;

    /// Default number of chunks put into the prompt by the CLI.
    pub const DEFAULT_K: usize = 5;
}

impl<M: Model + 'static, E: Embedder, C: Chat> Rag<M, E, C> {
    /// Indexes `chunks` by embedding them with `embedder`; `chat` answers questions in
    /// [`Rag::ask`]. Build chunks with [`Chunks::from_documents`], and an OpenAI client for
    /// both roles with [`OpenAiClient::from_env`].
    pub async fn ingest(embedder: E, chat: C, chunks: Chunks) -> Result<Self, Error> {
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
