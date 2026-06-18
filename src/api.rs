use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

#[derive(Debug, Clone)]
pub struct Config {
    pub api_key: String,
    pub base_url: String,
    pub model: String,
}

impl Config {
    pub fn load() -> Result<Config, String> {
        Ok(Config {
            api_key: "sk-2929a6bad669429db16e982da112f5ac".to_string(),
            base_url: "https://api.deepseek.com".to_string(),
            model: "deepseek-v4-flash".into(),
        })
    }
}

#[derive(Serialize)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<Message>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream_options: Option<StreamOptions>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_format: Option<ResponseFormat>,
}

#[derive(Serialize)]
pub struct StreamOptions {
    pub include_usage: bool,
}

/// Controls the response format. `ResponseFormat::json()` forces JSON output.
#[derive(Serialize)]
pub struct ResponseFormat {
    #[serde(rename = "type")]
    pub type_: String,
}

impl ResponseFormat {
    pub fn json() -> Self {
        Self { type_: "json_object".into() }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: String,
}

#[derive(Deserialize)]
pub struct ChatResponse {
    pub choices: Vec<Choice>,
    #[serde(default)]
    pub usage: Option<Usage>,
}

#[derive(Deserialize)]
pub struct Choice {
    pub message: Message,
}

#[derive(Deserialize)]
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
    /// DeepSeek-style: "prompt_cache_hit_tokens"
    #[serde(default, alias = "prompt_cache_hit_tokens")]
    pub prompt_cache_hit_tokens: Option<u32>,
    /// OpenAI-style: nested object
    #[serde(default)]
    pub prompt_tokens_details: Option<PromptTokensDetails>,
}

impl Usage {
    /// Returns (hit_tokens, total_prompt_tokens) — both 0 if no cache info.
    pub fn cache_hit(&self) -> (u32, u32) {
        // DeepSeek style
        if let Some(hit) = self.prompt_cache_hit_tokens {
            return (hit, self.prompt_tokens);
        }
        // OpenAI style
        if let Some(ref details) = self.prompt_tokens_details {
            return (details.cached_tokens, self.prompt_tokens);
        }
        (0, 0)
    }
}

#[derive(Deserialize)]
pub struct PromptTokensDetails {
    #[serde(default, alias = "cached_tokens")]
    pub cached_tokens: u32,
}

/// Result of a chat completion call — content + optional usage stats.
pub struct CompletionResult {
    pub content: String,
    pub usage: Option<Usage>,
}

// ---------------------------------------------------------------------------
// SSE streaming types
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct StreamDelta {
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    reasoning_content: Option<String>,
}

#[derive(Deserialize)]
struct StreamChoice {
    delta: StreamDelta,
    #[serde(default)]
    #[allow(dead_code)]
    finish_reason: Option<String>,
}

#[derive(Deserialize)]
struct StreamChunk {
    choices: Vec<StreamChoice>,
    #[serde(default)]
    usage: Option<Usage>,
}

/// One event from the streaming SSE response.
pub enum StreamEvent {
    Reasoning(String),
    Content(String),
    Done(Option<Usage>),
    Error(String),
}

#[derive(Deserialize)]
pub struct ErrorBody {
    pub error: ErrorDetail,
}

#[derive(Deserialize)]
pub struct ErrorDetail {
    pub message: String,
    #[allow(dead_code)]
    #[serde(rename = "type")]
    pub kind: Option<String>,
}

// ---------------------------------------------------------------------------
// Client
// ---------------------------------------------------------------------------

pub async fn chat_completion(
    api_key: &str,
    base_url: &str,
    model: &str,
    messages: &[Message],
    reasoning: Option<bool>,
    response_format: Option<ResponseFormat>,
) -> Result<CompletionResult, String> {
    let url = format!("{}/chat/completions", base_url.trim_end_matches('/'));

    let body = ChatRequest {
        model: model.to_string(),
        messages: messages.to_vec(),
        temperature: Some(0.7),
        stream: false,
        stream_options: None,
        reasoning,
        response_format,
    };

    let client = reqwest::Client::new();
    let resp = client
        .post(&url)
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("HTTP error: {e}"))?;

    let status = resp.status();
    let text = resp.text().await.map_err(|e| format!("Read error: {e}"))?;

    if !status.is_success() {
        // Try to extract a nice error message from the JSON body.
        if let Ok(err) = serde_json::from_str::<ErrorBody>(&text) {
            return Err(format!(
                "API error ({}): {}",
                status.as_u16(),
                err.error.message
            ));
        }
        return Err(format!("API error ({}): {}", status.as_u16(), text));
    }

    let parsed: ChatResponse =
        serde_json::from_str(&text).map_err(|e| format!("JSON parse error: {e}\n---\n{text}"))?;

    let content = parsed
        .choices
        .into_iter()
        .next()
        .map(|c| c.message.content)
        .ok_or_else(|| "Empty response — no choices returned".to_string())?;

    Ok(CompletionResult {
        content,
        usage: parsed.usage,
    })
}

// ---------------------------------------------------------------------------
// Streaming chat completion
// ---------------------------------------------------------------------------

/// Send a streaming chat completion request and return a channel receiver.
///
/// Each received event is one of:
/// - `StreamEvent::Reasoning(...)` — model's internal chain-of-thought (dim display)
/// - `StreamEvent::Content(...)` — visible answer tokens
/// - `StreamEvent::Done(usage)` — stream finished
/// - `StreamEvent::Error(msg)` — something went wrong
pub async fn chat_completion_stream(
    config: &Config,
    messages: impl IntoIterator<Item = &Message>,
    reasoning: Option<bool>,
    response_format: Option<ResponseFormat>,
) -> mpsc::Receiver<StreamEvent> {
    let (tx, rx) = mpsc::channel::<StreamEvent>(64);
    let messages = messages.into_iter().cloned().collect();
    let config = config.clone();

    tokio::spawn(async move {
        let url = format!("{}/chat/completions", config.base_url.trim_end_matches('/'));

        let body = ChatRequest {
            model: config.model,
            messages,
            temperature: Some(0.7),
            stream: true,
            stream_options: Some(StreamOptions {
                include_usage: true,
            }),
            reasoning,
            response_format,
        };

        let client = reqwest::Client::new();
        let response = match client
            .post(&url)
            .header("Content-Type", "application/json")
            .header("Authorization", format!("Bearer {}", config.api_key))
            .json(&body)
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => {
                let _ = tx
                    .send(StreamEvent::Error(format!("HTTP error: {e}")))
                    .await;
                return;
            }
        };

        // Check for HTTP-level errors before entering stream mode
        let status = response.status();
        if !status.is_success() {
            let text = match response.text().await {
                Ok(t) => t,
                Err(e) => {
                    let _ = tx
                        .send(StreamEvent::Error(format!("Read error: {e}")))
                        .await;
                    return;
                }
            };
            let msg = match serde_json::from_str::<ErrorBody>(&text) {
                Ok(err) => format!("API error ({}): {}", status.as_u16(), err.error.message),
                Err(_) => format!("API error ({}): {}", status.as_u16(), text),
            };
            let _ = tx.send(StreamEvent::Error(msg)).await;
            return;
        }

        let mut resp = response; // make mutable for .chunk()
        let mut buf = String::new();
        let mut usage: Option<Usage> = None;

        loop {
            match resp.chunk().await {
                Ok(Some(chunk)) => {
                    let chunk_str = String::from_utf8_lossy(&chunk);
                    buf.push_str(&chunk_str);

                    // Drain complete lines from the buffer
                    while let Some(pos) = buf.find('\n') {
                        let line = buf[..pos].trim().to_string();
                        buf = buf[pos + 1..].to_string();

                        let Some(data) = line.strip_prefix("data: ") else {
                            continue;
                        };
                        let data = data.trim();

                        if data == "[DONE]" {
                            let _ = tx.send(StreamEvent::Done(usage.take())).await;
                            return;
                        }

                        // Parse the SSE JSON chunk
                        if let Ok(chunk) = serde_json::from_str::<StreamChunk>(data) {
                            if let Some(choice) = chunk.choices.into_iter().next() {
                                if let Some(r) = choice.delta.reasoning_content {
                                    if !r.is_empty() {
                                        let _ = tx.send(StreamEvent::Reasoning(r)).await;
                                    }
                                }
                                if let Some(c) = choice.delta.content {
                                    if !c.is_empty() {
                                        let _ = tx.send(StreamEvent::Content(c)).await;
                                    }
                                }
                            }
                            // Last chunk sometimes carries usage
                            if let Some(u) = chunk.usage {
                                usage = Some(u);
                            }
                        }
                    }
                }
                Ok(None) => {
                    // Stream ended without [DONE] — still a clean finish
                    let _ = tx.send(StreamEvent::Done(usage.take())).await;
                    return;
                }
                Err(e) => {
                    let _ = tx
                        .send(StreamEvent::Error(format!("Stream error: {e}")))
                        .await;
                    return;
                }
            }
        }
    });

    rx
}
