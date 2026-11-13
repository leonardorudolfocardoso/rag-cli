mod embed;
pub use embed::*;

mod store;
pub use store::*;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("retryable: {0}")]
    Retryable(String),
    #[error("permanent: {0}")]
    Permanent(String),
}
impl From<reqwest::Error> for Error {
    fn from(e: reqwest::Error) -> Self {
        if e.is_timeout() || e.is_connect() || e.is_body() {
            Error::Retryable(e.to_string())
        } else {
            Error::Permanent(e.to_string())
        }
    }
}

mod chat;
pub use chat::*;
