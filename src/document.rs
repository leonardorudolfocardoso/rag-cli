use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use crate::{Chunk, Chunker};

/// A loaded document, pairing its source path with its contents.
#[derive(Debug, Clone)]
pub struct Document {
    pub source: Arc<Path>,
    pub text: String,
}

impl Document {
    /// Reads a document from disk.
    pub async fn from_file(path: &Path) -> std::io::Result<Self> {
        let text = tokio::fs::read_to_string(path).await?;
        Ok(Self {
            source: path.into(),
            text,
        })
    }

    /// Reads `files` from disk, in order.
    pub async fn from_files(files: &[PathBuf]) -> std::io::Result<Vec<Self>> {
        let mut docs = Vec::with_capacity(files.len());
        for path in files {
            docs.push(Self::from_file(path).await?);
        }
        Ok(docs)
    }
}

/// Chunks split from one or more documents, in document order.
#[derive(Debug, Clone, Default)]
pub struct Chunks(Vec<Chunk>);

impl Chunks {
    /// Splits each of `docs` with `chunker`, tagging every chunk with its source and
    /// keeping the documents' order.
    pub fn from_documents(docs: &[Document], chunker: &impl Chunker) -> Self {
        Self(
            docs.iter()
                .flat_map(|doc| {
                    chunker.chunk(&doc.text).into_iter().map(|text| Chunk {
                        source: Arc::clone(&doc.source),
                        text: text.into(),
                    })
                })
                .collect(),
        )
    }
}

impl std::ops::Deref for Chunks {
    type Target = [Chunk];

    fn deref(&self) -> &[Chunk] {
        &self.0
    }
}

impl IntoIterator for Chunks {
    type Item = Chunk;
    type IntoIter = std::vec::IntoIter<Chunk>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}
