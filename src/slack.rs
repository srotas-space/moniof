use reqwest::Client;
use serde::Serialize;

#[derive(Serialize)]
struct SlackPayload<'a> { text: &'a str }

pub async fn notify(webhook_url: String, text: String) {
    let client = Client::new();
    if let Err(e) = client.post(webhook_url).json(&SlackPayload { text: &text }).send().await {
        tracing::warn!(target="moniof::slack", "slack notify failed: {}", e);
    }
}
