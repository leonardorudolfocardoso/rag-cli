use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

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
