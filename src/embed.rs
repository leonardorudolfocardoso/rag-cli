use std::{marker::PhantomData, time::Duration};

use serde::{Deserialize, Serialize};
use serde_json::json;

pub trait Model {
    const NAME: &'static str;
    const DIMS: usize;
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TextEmbedding3Small;
impl Model for TextEmbedding3Small {
    const NAME: &'static str = "text-embedding-3-small";
    const DIMS: usize = 1536;
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Embedding<M: Model> {
    v: Vec<f32>,
    _m: PhantomData<M>,
}

fn normalise(vec: &mut [f32]) {
    let norm = vec.iter().map(|v| v * v).sum::<f32>().sqrt();
    if norm > 0_f32 {
        for item in vec.iter_mut() {
            *item /= norm
        }
    }
}

impl<M: Model> Embedding<M> {
    pub fn new(mut v: Vec<f32>) -> Result<Self, String> {
        if v.len() != M::DIMS {
            return Err(format!(
                "{}: expected {} dims, got {}",
                M::NAME,
                M::DIMS,
                v.len()
            ));
        }
        normalise(&mut v);
        Ok(Self { v, _m: PhantomData })
    }

    pub fn len(&self) -> usize {
        self.v.len()
    }

    pub fn is_empty(&self) -> bool {
        self.v.is_empty()
    }

    pub fn cosine(&self, other: &Self) -> f32 {
        self.v.iter().zip(&other.v).map(|(a, b)| a * b).sum()
    }
}

#[derive(Debug, Clone, Deserialize)]
struct EmbedResponse<M: Model> {
    data: Vec<EmbedData<M>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct EmbedData<M: Model> {
    embedding: Embedding<M>,
}

#[derive(Debug, thiserror::Error)]
pub enum EmbedError {
    #[error("retryable: {0}")]
    Retryable(String),
    #[error("permanent: {0}")]
    Permanent(String),
}

impl From<reqwest::Error> for EmbedError {
    fn from(e: reqwest::Error) -> Self {
        if e.is_timeout() || e.is_connect() || e.is_body() {
            EmbedError::Retryable(e.to_string())
        } else {
            EmbedError::Permanent(e.to_string())
        }
    }
}
pub async fn embed<M: Model + for<'a> Deserialize<'a>>(
    client: &reqwest::Client,
    key: &str,
    text: &str,
) -> Result<Embedding<M>, EmbedError> {
    let response = client
        .post("https://api.openai.com/v1/embeddings")
        .timeout(Duration::from_secs(30))
        .bearer_auth(key)
        .json(&json!({
           "model": "text-embedding-3-small", "input": text
        }))
        .send()
        .await?;

    let status = response.status();

    if status.is_success() {
        response.json::<EmbedResponse<M>>().await.map(|res| {
            res.data
                .into_iter()
                .next()
                .map(|e| e.embedding)
                .ok_or(EmbedError::Permanent("empty data".to_string()))
        })?
    } else {
        let body = response.text().await.unwrap_or_default();
        let msg = format!("{status}: {body}");

        Err(if status == 429 || status.is_server_error() {
            EmbedError::Retryable(msg)
        } else {
            EmbedError::Permanent(msg)
        })
    }
}

#[cfg(test)]
mod test {
    use std::sync::Arc;

    use tokio::{sync::Semaphore, time::Instant};

    use super::TextEmbedding3Small;

    use super::embed;

    #[tokio::test]
    #[ignore]
    async fn embed_returns_1536_dims() {
        let key = std::env::var("OPENAI_API_KEY").expect("set OPENAI_API_KEY");
        let client = reqwest::Client::new();
        let t0 = Instant::now();
        let v = embed::<TextEmbedding3Small>(&client, &key, "hello")
            .await
            .unwrap();
        let w = t0.elapsed().as_millis();
        println!("elapsed: {w}");
        assert_eq!(v.len(), 1536);
    }

    #[tokio::test]
    #[ignore]
    async fn embed_latency_measurement() {
        let key = std::env::var("OPENAI_API_KEY").expect("set OPENAI_API_KEY");
        let client = reqwest::Client::new(); // created once, reused
        let mut ws = Vec::new();
        for i in 0..26 {
            let t0 = Instant::now();
            embed::<TextEmbedding3Small>(&client, &key, "hello world")
                .await
                .unwrap();
            let w = t0.elapsed();
            println!("call {i}: {w:?}");
            ws.push(w);
        }
        ws.remove(0); // drop the warm-up call
        ws.sort();
        println!(
            "median W = {:?}, max = {:?}",
            ws[ws.len() / 2],
            ws.last().unwrap()
        );
    }

    #[tokio::test]
    #[ignore]
    async fn embed_latency_semaphore_measurement() {
        let key = std::env::var("OPENAI_API_KEY").expect("set OPENAI_API_KEY");
        let client = reqwest::Client::new(); // created once, reused
        let start = Instant::now();

        const N: usize = 26;
        let mut tasks = tokio::task::JoinSet::new();
        let sem = Arc::new(Semaphore::new(N));

        for i in 0..26 {
            let sem = Arc::clone(&sem);
            let client = client.clone();
            let key = key.clone();
            tasks.spawn(async move {
                let _permit = sem.acquire_owned().await.unwrap();
                let t0 = Instant::now();
                embed::<TextEmbedding3Small>(&client, &key, "hello world")
                    .await
                    .unwrap();
                let w = t0.elapsed();
                println!("call {i}: {w:?}");
                w
            });
        }

        let mut ws = tasks.join_all().await;
        ws.sort();

        println!(
            "median W = {:?}, max = {:?}",
            ws[ws.len() / 2],
            ws.last().unwrap()
        );
        println!("T({}) = {:?}", N, start.elapsed());
    }
}
