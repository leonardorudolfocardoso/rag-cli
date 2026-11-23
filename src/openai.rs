use std::{sync::Arc, time::Duration};

use serde::{Serialize, de::DeserializeOwned};

use crate::Error;

const BASE: &str = "https://api.openai.com/v1";
const ATTEMPTS: u32 = 4;
const BACKOFF: Duration = Duration::from_millis(500);

/// HTTP client for the OpenAI API, holding a reusable `reqwest::Client` and API key.
#[derive(Clone)]
pub struct OpenAiClient {
    http: reqwest::Client,
    key: Arc<str>,
}

impl OpenAiClient {
    /// Creates a client with the given OpenAI API key.
    pub fn new(key: impl Into<Arc<str>>) -> Self {
        Self {
            http: reqwest::Client::new(), // per-request timeout is set in post_once
            key: key.into(),
        }
    }

    /// Creates a client using the `OPENAI_API_KEY` environment variable.
    pub fn from_env() -> Result<Self, Error> {
        let key = std::env::var("OPENAI_API_KEY")
            .map_err(|_| Error::Permanent("OPENAI_API_KEY is not set".into()))?;
        Ok(Self::new(key))
    }

    /// POSTs `body` to the OpenAI API, retrying rate limits, server errors and timeouts.
    pub(crate) async fn post<T: DeserializeOwned>(
        &self,
        path: &str,
        body: &impl Serialize,
    ) -> Result<T, Error> {
        retry(|| self.post_once(path, body)).await
    }

    /// Sends a single POST request without retrying, classifying HTTP failures as
    /// retryable or permanent errors. Called by [`Self::post`], which adds retries.
    async fn post_once<T: DeserializeOwned>(
        &self,
        path: &str,
        body: &impl Serialize,
    ) -> Result<T, Error> {
        let url = format!("{BASE}/{path}");
        let response = self
            .http
            .post(url)
            .timeout(Duration::from_secs(30))
            .bearer_auth(&*self.key)
            .json(body)
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(Error::from_status(status, body));
        }
        Ok(response.json().await?)
    }
}

/// Runs `f` until it succeeds, fails permanently, or uses up `ATTEMPTS`,
/// doubling the wait after each retryable failure.
async fn retry<T, F, Fut>(mut f: F) -> Result<T, Error>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, Error>>,
{
    let mut wait = BACKOFF;
    for _ in 1..ATTEMPTS {
        match f().await {
            Err(e) if e.is_retryable() => {
                tokio::time::sleep(wait).await;
                wait *= 2;
            }
            done => return done,
        }
    }
    f().await
}

pub(crate) fn first<T>(items: Vec<T>, what: &str) -> Result<T, Error> {
    items
        .into_iter()
        .next()
        .ok_or_else(|| Error::Permanent(format!("empty {what}")))
}

#[cfg(test)]
mod test {
    use super::{ATTEMPTS, first, retry};
    use crate::Error;

    #[tokio::test(start_paused = true)]
    async fn retry_retries_until_success() {
        let mut calls = 0;
        let res = retry(|| {
            calls += 1;
            let n = calls;
            async move {
                if n < 3 {
                    Err(Error::Retryable("busy".into()))
                } else {
                    Ok(n)
                }
            }
        })
        .await;
        assert_eq!(res.unwrap(), 3);
    }

    #[tokio::test(start_paused = true)]
    async fn retry_stops_on_permanent_error() {
        let mut calls = 0;
        let res: Result<(), _> = retry(|| {
            calls += 1;
            async { Err(Error::Permanent("bad key".into())) }
        })
        .await;
        assert!(!res.unwrap_err().is_retryable());
        assert_eq!(calls, 1);
    }

    #[tokio::test(start_paused = true)]
    async fn retry_gives_up_after_attempts() {
        let mut calls = 0;
        let res: Result<(), _> = retry(|| {
            calls += 1;
            async { Err(Error::Retryable("busy".into())) }
        })
        .await;
        assert!(res.unwrap_err().is_retryable());
        assert_eq!(calls, ATTEMPTS);
    }

    #[test]
    fn first_returns_first_item() {
        assert_eq!(first(vec![1, 2, 3], "items").unwrap(), 1);
    }

    #[test]
    fn first_on_empty_is_permanent_and_names_the_field() {
        let err = first(Vec::<i32>::new(), "choices").unwrap_err();
        assert!(
            matches!(&err, Error::Permanent(m) if m == "empty choices"),
            "got {err}"
        );
    }
}
