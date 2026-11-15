use std::fmt::Write;

pub const SYSTEM: &str = "Answer using only the context below. \
If the answer isn't in the context, say \"I don't know\". \
Cite the chunk numbers you used, like [1].";

/// Builds the user message: numbered context chunks, then the question.
pub fn build(question: &str, chunks: &[(f32, String)]) -> String {
    let mut s = String::from("Context:\n");
    for (i, (_score, text)) in chunks.iter().enumerate() {
        // writing to a String can't fail, so unwrap is safe here
        writeln!(s, "[{}] {}", i + 1, text.trim()).unwrap();
    }
    write!(s, "\nQuestion: {question}").unwrap();
    s
}

#[cfg(test)]
mod tests {
    use super::build;

    #[test]
    fn numbers_chunks_and_ends_with_question() {
        let chunks = vec![
            (0.9, "first chunk".to_string()),
            (0.8, "second chunk\n".to_string()),
        ];
        let p = build("What is backpressure?", &chunks);
        assert!(p.starts_with("Context:\n"));
        assert!(p.contains("[1] first chunk\n"));
        assert!(p.contains("[2] second chunk\n")); // trailing newline trimmed, no blank line
        assert!(p.ends_with("Question: What is backpressure?"));
    }

    #[test]
    fn no_chunks_still_has_question() {
        let p = build("Anything?", &[]);
        assert_eq!(p, "Context:\n\nQuestion: Anything?");
    }
}
