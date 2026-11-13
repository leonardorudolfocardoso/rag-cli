use std::time::Duration;

use serde::Deserialize;
use serde_json::json;

use crate::Error;

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
}
#[derive(Deserialize)]
struct Choice {
    message: Message,
}
#[derive(Deserialize)]
struct Message {
    content: String,
}

async fn chat(
    client: &reqwest::Client,
    key: &str,
    system: &str,
    user: &str,
) -> Result<String, Error> {
    let response = client
        .post("https://api.openai.com/v1/chat/completions")
        .timeout(Duration::from_secs(30))
        .bearer_auth(key)
        .json(&json!({
            "model": "gpt-4o-mini",
            "temperature": 0,                  // same question → same answer, so runs are comparable
            "messages": [
                {"role": "system", "content": system},
                {"role": "user",   "content": user}
            ]
        }))
        .send()
        .await?;

    let status = response.status();

    if status.is_success() {
        response.json::<ChatResponse>().await.map(|res| {
            res.choices
                .into_iter()
                .next()
                .map(|c| c.message.content)
                .ok_or(Error::Permanent("empty data".to_string()))
        })?
    } else {
        let body = response.text().await.unwrap_or_default();
        let msg = format!("{status}: {body}");

        Err(if status == 429 || status.is_server_error() {
            Error::Retryable(msg)
        } else {
            Error::Permanent(msg)
        })
    }
}

#[cfg(test)]
mod test {
    use tokio::time::Instant;

    use super::chat;

    const SYSTEM: &str = "You are a terse assistant.";
    const USER: &str = "Reply with exactly one word: pong";

    #[tokio::test]
    #[ignore]
    async fn chat_returns_answer() {
        let key = std::env::var("OPENAI_API_KEY").expect("set OPENAI_API_KEY");
        let client = reqwest::Client::new();
        let t0 = Instant::now();
        let answer = chat(&client, &key, SYSTEM, USER).await.unwrap();
        let w = t0.elapsed().as_millis();
        println!("elapsed: {w}");
        println!("answer: {answer}");
        assert!(answer.to_lowercase().contains("pong"), "got {answer}");
    }

    #[tokio::test]
    #[ignore]
    async fn chat_latency_measurement() {
        let key = std::env::var("OPENAI_API_KEY").expect("set OPENAI_API_KEY");
        let client = reqwest::Client::new(); // created once, reused
        let mut ws = Vec::new();
        for i in 0..26 {
            let t0 = Instant::now();
            chat(&client, &key, SYSTEM, USER).await.unwrap();
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
}
