use clap::Parser;
use rag_cli::{TextEmbedding3Small, build, chat, chunks, embed};
use std::{env, path::PathBuf, sync::Arc, time::Duration};
use tokio::{join, sync::Semaphore};

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

/// Answer questions about Markdown notes.
#[derive(Parser)]
struct Args {
    /// Files to ingest (e.g. docs/*.md)
    #[arg(required = true)]
    files: Vec<PathBuf>,

    /// The question. If omitted, runs the 5 eval questions
    #[arg(short, long, required = true)]
    question: String,

    /// How many chunks go into the prompt
    #[arg(short, long, default_value_t = 3)]
    k: usize,

    /// Chunk size
    #[arg(short, long, default_value_t = 1500)]
    chunk_size: usize,
}

/// Reads the given files. Returns (path, contents) in the order given.
pub async fn read_docs(files: &[PathBuf]) -> std::io::Result<Vec<(PathBuf, String)>> {
    let mut docs = Vec::with_capacity(files.len());
    for path in files {
        let text = tokio::fs::read_to_string(path).await?;
        docs.push((path.clone(), text));
    }
    Ok(docs)
}

fn main() {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .enable_time()
        .build()
        .unwrap();
    let Args {
        files,
        question,
        k,
        chunk_size,
    } = Args::parse();

    rt.block_on(async {
        let docs = read_docs(&files).await.unwrap();
        let texts: Vec<_> = docs.into_iter().map(|doc| doc.1).collect();
        let client = reqwest::ClientBuilder::new()
            .timeout(Duration::from_secs(30))
            .build()
            .unwrap();
        let key = Arc::new(env::var("OPENAI_API_KEY").expect("missing API_KEY"));
        let semaphore = Arc::new(Semaphore::new(4));
        let mut set = tokio::task::JoinSet::new();

        for text in &texts {
            for chunk in chunks(text, chunk_size) {
                let chunk = chunk.to_owned();
                let client = client.clone();
                let semaphore = Arc::clone(&semaphore);
                let key = Arc::clone(&key);
                set.spawn(async move {
                    let _perm = semaphore.acquire().await.unwrap();
                    let embedding = embed::<TextEmbedding3Small>(&client, &key, &chunk)
                        .await
                        .unwrap();
                    (chunk, embedding)
                });
            }
        }

        let embedded_question = {
            let key = Arc::clone(&key);
            let client = client.clone();
            let question = question.clone();
            rt.spawn(async move {
                let _perm = semaphore.acquire().await.unwrap();
                embed::<TextEmbedding3Small>(&client, &key, &question)
                    .await
                    .unwrap()
            })
        };

        let (docs, embeded_question) = join!(set.join_all(), embedded_question);
        let embedded_question = embeded_question.unwrap();

        let mut scored: Vec<_> = docs
            .into_iter()
            .map(|doc| (doc.1.cosine(&embedded_question), doc.0))
            .collect();
        scored.sort_by(|a, b| b.0.total_cmp(&a.0));
        scored.truncate(k);

        match chat(&client, &key, &build(&question, &scored), &question).await {
            Ok(response) => println!("{}", response),
            Err(_) => todo!(),
        }
    });
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
