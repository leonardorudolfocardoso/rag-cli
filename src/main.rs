fn chunks(doc: &str, size: usize) -> impl Iterator<Item = &str> {
    let mut rest = doc;
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

const QUESTIONS: [(&str, &str); 5] = [
    (
        "Why should vectors be normalised when they're stored?",
        "3Nd → Nd per search, paid once at ingest",
    ),
    (
        "What goes wrong when data arrives faster than the consumer can process it, with nothing slowing the sender?",
        "memory, latency, GC, degradation",
    ),
    (
        "Where does caipora sit in the MCP picture?",
        "between host and tool servers",
    ),
    (
        "My RAG gave a wrong answer even though the right chunk was retrieved. What should I fix?",
        "the prompt (model problem)",
    ),
    ("What is PostgreSQL's default port?", "I don't know"),
];

fn main() {
    println!("Hello, world!");
}

#[cfg(test)]
mod test {
    use super::chunks;

    #[test]
    fn test_chunks() {
        let v: Vec<&str> = chunks("ação é ótima", 4).collect();
        assert_eq!(v, ["ação", " é ó", "tima"]); // 4 chars each, never cut inside "ç"
        assert_eq!(v.concat(), "ação é ótima"); // nothing lost
        assert_eq!(chunks("", 4).count(), 0);
    }
}
