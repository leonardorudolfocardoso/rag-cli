use std::{marker::PhantomData, sync::Arc};

use serde::Deserialize;
use serde_json::json;
use tokio::{sync::Semaphore, task::JoinSet};

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

impl<M: Model> EmbedResponse<M> {
    /// Validates and orders the returned embeddings by `index`, ensuring exact length match.
    fn into_embeddings(mut self, expected: usize) -> Result<Vec<Embedding<M>>, Error> {
        if self.data.len() != expected {
            return Err(Error::Permanent(format!(
                "expected {expected} embeddings, got {}",
                self.data.len()
            )));
        }
        self.data.sort_by_key(|d| d.index);
        for (i, item) in self.data.iter().enumerate() {
            if item.index != i {
                return Err(Error::Permanent(format!(
                    "missing embedding at index {i} (got {})",
                    item.index
                )));
            }
        }
        Ok(self.data.into_iter().map(|d| d.embedding).collect())
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(bound = "")]
struct EmbedData<M: Model> {
    #[serde(default)]
    index: usize,
    embedding: Embedding<M>,
}

/// An embedding backend capable of encoding one or many strings into [`Embedding<M>`].
pub trait Embedder {
    fn embed<M: Model>(
        &self,
        text: &str,
    ) -> impl Future<Output = Result<Embedding<M>, Error>> + Send;

    fn embed_batch<M: Model + 'static>(
        &self,
        texts: &[&str],
    ) -> impl Future<Output = Result<Vec<Embedding<M>>, Error>> + Send;
}

impl Embedder for OpenAiClient {
    async fn embed<M: Model>(&self, text: &str) -> Result<Embedding<M>, Error> {
        let res: EmbedResponse<M> = self
            .post("embeddings", &json!({ "model": M::NAME, "input": text }))
            .await?;

        Ok(openai::first(res.data, "data")?.embedding)
    }

    /// Embeds all `texts` in input order, splitting into batches of up to 32
    /// with at most 4 requests in flight.
    async fn embed_batch<M: Model + 'static>(
        &self,
        texts: &[&str],
    ) -> Result<Vec<Embedding<M>>, Error> {
        /// Max inputs per `/v1/embeddings` request (stays well under OpenAI's 300k token limit).
        const MAX_BATCH: usize = 32;
        /// Max concurrent `/v1/embeddings` requests in flight.
        const MAX_IN_FLIGHT: usize = 4;

        if texts.is_empty() {
            return Ok(Vec::new());
        }
        if texts.len() <= MAX_BATCH {
            let res: EmbedResponse<M> = self
                .post("embeddings", &json!({ "model": M::NAME, "input": texts }))
                .await?;
            return res.into_embeddings(texts.len());
        }

        let semaphore = Arc::new(Semaphore::new(MAX_IN_FLIGHT));
        let mut set = JoinSet::new();
        let batch_count = texts.len().div_ceil(MAX_BATCH);

        for (batch_idx, slice) in texts.chunks(MAX_BATCH).enumerate() {
            let batch: Vec<String> = slice.iter().map(|s| (*s).to_owned()).collect();
            let client = self.clone();
            let semaphore = Arc::clone(&semaphore);
            set.spawn(async move {
                // the semaphore is never closed, so acquire can't fail
                let _perm = semaphore.acquire().await.unwrap();
                let res: EmbedResponse<M> = client
                    .post("embeddings", &json!({ "model": M::NAME, "input": &batch }))
                    .await?;
                let embeddings = res.into_embeddings(batch.len())?;
                Ok::<_, Error>((batch_idx, embeddings))
            });
        }

        let mut ordered: Vec<Option<Vec<Embedding<M>>>> =
            std::iter::repeat_with(|| None).take(batch_count).collect();
        // returning early drops the set, which aborts batches still in flight
        while let Some(res) = set.join_next().await {
            let (batch_idx, embeddings) = res.expect("embed task panicked")?;
            ordered[batch_idx] = Some(embeddings);
        }

        Ok(ordered.into_iter().flatten().flatten().collect())
    }
}

#[cfg(test)]
mod test {
    use std::sync::Arc;

    use tokio::{sync::Semaphore, time::Instant};

    use super::{EmbedResponse, Embedder, Model, TextEmbedding3Small};
    use crate::OpenAiClient;

    #[derive(Debug)]
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

    #[test]
    fn into_embeddings_orders_by_index() {
        let body =
            r#"{"data":[{"index":1,"embedding":[0.0,1.0]},{"index":0,"embedding":[1.0,0.0]}]}"#;
        let res: EmbedResponse<Tiny> = serde_json::from_str(body).unwrap();
        let embs = res.into_embeddings(2).unwrap();
        let x = super::Embedding::<Tiny>::new(vec![1.0, 0.0]).unwrap();
        assert!((embs[0].cosine(&x) - 1.0).abs() < 1e-6);
        assert!(embs[1].cosine(&x).abs() < 1e-6);
    }

    #[test]
    fn into_embeddings_rejects_wrong_count_or_duplicate_index() {
        let body = r#"{"data":[{"index":0,"embedding":[1.0,0.0]}]}"#;
        let res: EmbedResponse<Tiny> = serde_json::from_str(body).unwrap();
        let err = res.into_embeddings(2).unwrap_err();
        assert!(
            err.to_string().contains("expected 2 embeddings, got 1"),
            "got {err}"
        );

        let dup =
            r#"{"data":[{"index":0,"embedding":[1.0,0.0]},{"index":0,"embedding":[0.0,1.0]}]}"#;
        let res: EmbedResponse<Tiny> = serde_json::from_str(dup).unwrap();
        let err = res.into_embeddings(2).unwrap_err();
        assert!(
            err.to_string().contains("missing embedding at index 1"),
            "got {err}"
        );
    }

    #[tokio::test]
    #[ignore]
    async fn embed_returns_1536_dims() {
        let client = OpenAiClient::from_env().expect("set OPENAI_API_KEY");
        let t0 = Instant::now();
        let v = client.embed::<TextEmbedding3Small>("hello").await.unwrap();
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
            client
                .embed::<TextEmbedding3Small>("hello world")
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
                client
                    .embed::<TextEmbedding3Small>("hello world")
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
