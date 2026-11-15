mod error;
pub use error::*;

mod openai;

mod embed;
pub use embed::*;

mod store;
pub use store::*;

mod chat;
pub use chat::*;

mod prompt;
pub use prompt::build;

pub fn chunks(text: &str, size: usize) -> impl Iterator<Item = &str> {
    let mut rest = text;
    std::iter::from_fn(move || {
        if rest.is_empty() {
            return None;
        }

        let cut = rest
            .char_indices()
            .nth(size)
            .map(|(i, _)| i)
            .unwrap_or_else(|| rest.len());

        let (chunk, tail) = rest.split_at(cut);
        rest = tail;
        Some(chunk)
    })
}
