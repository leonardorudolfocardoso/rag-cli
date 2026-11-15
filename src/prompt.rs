use std::fmt::Write;

use crate::Chunk;

pub const SYSTEM: &str = "Answer using only the context below. \
If the answer isn't in the context, say \"I don't know\". \
Cite the chunk numbers you used, like [1].";

/// Builds the user message: numbered context chunks with their source file, then the question.
pub fn build(question: &str, chunks: &[(f32, &Chunk)]) -> String {
    let mut s = String::from("Context:\n");
    for (i, (_score, chunk)) in chunks.iter().enumerate() {
        // writing to a String can't fail, so unwrap is safe here
        let (source, text) = (chunk.source.display(), chunk.text.trim());
        writeln!(s, "[{}] ({source}) {text}", i + 1).unwrap();
    }
    write!(s, "\nQuestion: {question}").unwrap();
    s
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::build;
    use crate::Chunk;

    fn chunk(source: &str, text: &str) -> Chunk {
        Chunk {
            source: Path::new(source).into(),
            text: text.into(),
        }
    }

    #[test]
    fn numbers_chunks_and_ends_with_question() {
        let (a, b) = (
            chunk("a.md", "first chunk"),
            chunk("b.md", "second chunk\n"),
        );
        let p = build("What is backpressure?", &[(0.9, &a), (0.8, &b)]);
        assert!(p.starts_with("Context:\n"));
        assert!(p.contains("[1] (a.md) first chunk\n"));
        assert!(p.contains("[2] (b.md) second chunk\n")); // trailing newline trimmed, no blank line
        assert!(p.ends_with("Question: What is backpressure?"));
    }

    #[test]
    fn no_chunks_still_has_question() {
        let p = build("Anything?", &[]);
        assert_eq!(p, "Context:\n\nQuestion: Anything?");
    }
}
