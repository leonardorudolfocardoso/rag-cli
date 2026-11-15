use std::time::Duration;

use serde::{Serialize, de::DeserializeOwned};

use crate::Error;

const BASE: &str = "https://api.openai.com/v1";
const ATTEMPTS: u32 = 4;
const BACKOFF: Duration = Duration::from_millis(500);

/// POSTs `body` to the OpenAI API, retrying rate limits, server errors and timeouts.
pub(crate) async fn post<T: DeserializeOwned>(
    client: &reqwest::Client,
    key: &str,
    path: &str,
    body: &impl Serialize,
) -> Result<T, Error> {
    retry(|| post_once(client, key, path, body)).await
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

async fn post_once<T: DeserializeOwned>(
    client: &reqwest::Client,
    key: &str,
    path: &str,
    body: &impl Serialize,
) -> Result<T, Error> {
    let url = format!("{BASE}/{path}");
    let response = client
        .post(url)
        .timeout(Duration::from_secs(30))
        .bearer_auth(key)
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
