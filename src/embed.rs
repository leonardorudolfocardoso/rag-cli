use std::marker::PhantomData;

use serde::Deserialize;
use serde_json::json;

use crate::{Error, OpenAiClient, openai};

pub trait Model: Send + Sync {
    const NAME: &'static str;
    const DIMS: usize;
}

#[derive(Debug)]
pub struct TextEmbedding3Small;
impl Model for TextEmbedding3Small {
    const NAME: &'static str = "text-embedding-3-small";
    const DIMS: usize = 1536;
}

#[derive(Debug, Clone, Deserialize)]
#[serde(try_from = "Vec<f32>", bound = "")]
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

impl<M: Model> TryFrom<Vec<f32>> for Embedding<M> {
    type Error = String;

    fn try_from(v: Vec<f32>) -> Result<Self, String> {
        Self::new(v)
    }
}

impl<M: Model> Embedding<M> {
    /// Builds an embedding for model `M`, checking that `v` has exactly `M::DIMS`
    /// elements and L2-normalising it so [`cosine`](Self::cosine) is a plain dot product.
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
#[serde(bound = "")]
struct EmbedResponse<M: Model> {
    data: Vec<EmbedData<M>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(bound = "")]
struct EmbedData<M: Model> {
    embedding: Embedding<M>,
}

pub async fn embed<M: Model>(client: &OpenAiClient, text: &str) -> Result<Embedding<M>, Error> {
    let res: EmbedResponse<M> = client
        .post("embeddings", &json!({ "model": M::NAME, "input": text }))
        .await?;

    Ok(openai::first(res.data, "data")?.embedding)
}

#[cfg(test)]
mod test {
    use std::sync::Arc;

    use tokio::{sync::Semaphore, time::Instant};

    use super::{EmbedResponse, Model, TextEmbedding3Small, embed};
    use crate::OpenAiClient;

    struct Tiny;
    impl Model for Tiny {
        const NAME: &'static str = "tiny";
        const DIMS: usize = 2;
    }

    #[test]
    fn decodes_api_shape_through_new() {
        let body = r#"{"data":[{"embedding":[3.0,4.0]}]}"#;
        let res: EmbedResponse<Tiny> = serde_json::from_str(body).unwrap();
        let e = &res.data[0].embedding;
        assert!((e.cosine(e) - 1.0).abs() < 1e-6); // normalised on decode
    }

    #[test]
    fn decode_rejects_wrong_dimension() {
        let body = r#"{"data":[{"embedding":[1.0]}]}"#;
        let err = serde_json::from_str::<EmbedResponse<Tiny>>(body)
            .err()
            .unwrap();
        assert!(
            err.to_string().contains("expected 2 dims, got 1"),
            "got {err}"
        );
    }

    #[tokio::test]
    #[ignore]
    async fn embed_returns_1536_dims() {
        let client = OpenAiClient::from_env().expect("set OPENAI_API_KEY");
        let t0 = Instant::now();
        let v = embed::<TextEmbedding3Small>(&client, "hello")
            .await
            .unwrap();
        let w = t0.elapsed().as_millis();
        println!("elapsed: {w}");
        assert_eq!(v.len(), 1536);
    }

    #[tokio::test]
    #[ignore]
    async fn embed_latency_measurement() {
        let client = OpenAiClient::from_env().expect("set OPENAI_API_KEY");
        let mut ws = Vec::new();
        for i in 0..26 {
            let t0 = Instant::now();
            embed::<TextEmbedding3Small>(&client, "hello world")
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
        let client = OpenAiClient::from_env().expect("set OPENAI_API_KEY");
        let start = Instant::now();

        const N: usize = 26;
        let mut tasks = tokio::task::JoinSet::new();
        let sem = Arc::new(Semaphore::new(N));

        for i in 0..26 {
            let sem = Arc::clone(&sem);
            let client = client.clone();
            tasks.spawn(async move {
                let _permit = sem.acquire_owned().await.unwrap();
                let t0 = Instant::now();
                embed::<TextEmbedding3Small>(&client, "hello world")
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
