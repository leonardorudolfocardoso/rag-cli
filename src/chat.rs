use serde::Deserialize;
use serde_json::json;

use crate::{Error, openai};

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

pub async fn chat(
    client: &reqwest::Client,
    key: &str,
    system: &str,
    user: &str,
) -> Result<String, Error> {
    let res: ChatResponse = openai::post(
        client,
        key,
        "chat/completions",
        &json!({
            "model": "gpt-4o-mini",
            "temperature": 0,                  // same question → same answer, so runs are comparable
            "messages": [
                {"role": "system", "content": system},
                {"role": "user",   "content": user}
            ]
        }),
    )
    .await?;

    Ok(openai::first(res.choices, "choices")?.message.content)
}

#[cfg(test)]
mod test {
    use tokio::time::Instant;

    use super::{ChatResponse, chat};

    #[test]
    fn parses_chat_response() {
        let raw = r#"{"choices": [{"message": {"role": "assistant", "content": "pong"}}]}"#;
        let res: ChatResponse = serde_json::from_str(raw).unwrap();
        assert_eq!(res.choices[0].message.content, "pong");
    }

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
