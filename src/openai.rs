use std::time::Duration;

use serde::{Serialize, de::DeserializeOwned};

use crate::Error;

const BASE: &str = "https://api.openai.com/v1";

pub(crate) async fn post<T: DeserializeOwned>(
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
    use super::first;
    use crate::Error;

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
