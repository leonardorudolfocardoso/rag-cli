use std::{path::PathBuf, sync::Arc};

mod error;
pub use error::*;

mod openai;
pub use openai::OpenAiClient;

mod embed;
pub use embed::*;

mod store;
pub use store::*;

mod chat;
pub use chat::*;

mod prompt;
pub use prompt::{SYSTEM, build};

mod chunk;
pub use chunk::*;

mod document;
pub use document::*;

mod rag;
pub use rag::*;

/// Splits each of `docs` with `chunker`, tagging every chunk with its source and
/// keeping the documents' order.
pub fn chunk_documents(docs: &[Document], chunker: &impl Chunker) -> Chunks {
    docs.iter()
        .flat_map(|doc| {
            chunker.chunk(&doc.text).into_iter().map(|text| Chunk {
                source: Arc::clone(&doc.source),
                text: text.into(),
            })
        })
        .collect()
}

/// Reads `files`, splits them with `chunker` and embeds the chunks into a [`Rag`].
pub async fn index<M: Model + 'static, E: Embedder, C: Chat>(
    embedder: E,
    chat: C,
    files: &[PathBuf],
    chunker: &impl Chunker,
) -> Result<Rag<M, E, C>, Error> {
    let docs = Document::from_files(files).await?;
    Rag::ingest(embedder, chat, chunk_documents(&docs, chunker)).await
}
