use reqwest::Client;
use serde::Serialize;

#[derive(Serialize)]
struct SlackPayload<'a> {
    text: &'a str,
}

pub async fn notify(webhook_url: Option<String>, text: String) {
    // 1. If webhook URL is not provided → skip
    let Some(url) = webhook_url else {
        return;
    };

    if url.trim().is_empty() {
        return;
    }

    // 2. Send Slack request
    let client = Client::new();
    if let Err(e) = client.post(url).json(&SlackPayload { text: &text }).send().await {
        tracing::warn!(
            target="moniof::slack",
            "slack notify failed: {}",
            e
        );
    }
}
