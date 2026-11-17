// tests/eval.rs   — run with: cargo test --test eval -- --ignored --nocapture
use rag_cli::{ask, ingest};

struct Case {
    q: &'static str,
    source: Option<&'static str>, // file that must be in top-k (None = not in docs)
    keywords: &'static [&'static str], // all must appear in the answer (lowercased)
}

const CASES: &[Case] = &[
    Case {
        q: "Why should vectors be normalised when they're stored?",
        source: Some("RAG and Agents.md"),
        keywords: &["once"],
    },
    Case {
        q: "What goes wrong when data arrives faster than the consumer can process it, with nothing slowing the sender?",
        source: Some("backpressure.md"),
        keywords: &["memory", "latency"],
    },
    Case {
        q: "Where does caipora sit in the MCP picture?",
        source: Some("RAG and Agents.md"),
        keywords: &["host", "server"],
    },
    Case {
        q: "My RAG gave a wrong answer even though the right chunk was retrieved. What should I fix?",
        source: Some("RAG and Agents.md"),
        keywords: &["prompt"],
    },
    Case {
        q: "What is PostgreSQL's default port?",
        source: None,
        keywords: &["don't know"],
    },
];

#[tokio::test]
#[ignore]
async fn eval() {
    let files: Vec<_> = std::fs::read_dir("tests/fixtures")
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    let rag = ingest(&files).await.unwrap();
    let (mut hits, mut correct) = (0, 0);
    for c in CASES {
        let a = ask(&rag, c.q, 3).await.unwrap();
        let hit = c
            .source
            .is_none_or(|s| a.sources.iter().any(|p| p.ends_with(s)));
        let ok = c.keywords.iter().all(|k| a.text.to_lowercase().contains(k));
        hits += hit as u32;
        correct += ok as u32;
        println!(
            "{} {} | {}\n   → {}",
            if hit { "R✓" } else { "R✗" },
            if ok { "A✓" } else { "A✗" },
            c.q,
            a.text
        );
    }
    println!("recall@3 = {hits}/5, accuracy = {correct}/5");
    assert!(hits == 5, "retrieval should always find the right file");
    assert!(correct >= 4, "answer accuracy below threshold");
}
