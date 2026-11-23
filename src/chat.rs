use serde::Deserialize;
use serde_json::json;

use crate::{Error, OpenAiClient, openai};

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

/// A chat model capable of answering a user message given a system prompt.
pub trait Chat {
    fn chat(&self, system: &str, user: &str) -> impl Future<Output = Result<String, Error>> + Send;
}

impl Chat for OpenAiClient {
    async fn chat(&self, system: &str, user: &str) -> Result<String, Error> {
        let res: ChatResponse = self
            .post(
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
}

#[cfg(test)]
mod test {
    use tokio::time::Instant;

    use super::{Chat, ChatResponse};
    use crate::OpenAiClient;

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
        let client = OpenAiClient::from_env().expect("set OPENAI_API_KEY");
        let t0 = Instant::now();
        let answer = client.chat(SYSTEM, USER).await.unwrap();
        let w = t0.elapsed().as_millis();
        println!("elapsed: {w}");
        println!("answer: {answer}");
        assert!(answer.to_lowercase().contains("pong"), "got {answer}");
    }

    #[tokio::test]
    #[ignore]
    async fn chat_latency_measurement() {
        let client = OpenAiClient::from_env().expect("set OPENAI_API_KEY");
        let mut ws = Vec::new();
        for i in 0..26 {
            let t0 = Instant::now();
            client.chat(SYSTEM, USER).await.unwrap();
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
