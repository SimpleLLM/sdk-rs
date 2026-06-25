use std::time::Duration;

use bytes::Bytes;
use futures_util::StreamExt;
use reqwest::{multipart, Client};
use serde::de::DeserializeOwned;

use crate::error::{Result, SimpleLlmError};
use crate::types::*;

const DEFAULT_BASE_URL: &str = "https://api.simplellm.eu";
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(120);

/// Configuration builder for [`SimpleLLM`].
#[derive(Debug, Default)]
pub struct ClientConfig {
    pub api_key: Option<String>,
    pub base_url: Option<String>,
    pub timeout: Option<Duration>,
}

/// Async client for the SimpleLLM API.
///
/// # Example
/// ```no_run
/// # #[tokio::main] async fn main() -> simplellm::Result<()> {
/// let client = simplellm::SimpleLLM::from_env()?;
/// # Ok(()) }
/// ```
#[derive(Debug, Clone)]
pub struct SimpleLLM {
    api_key: String,
    base_url: String,
    http: Client,
}

impl SimpleLLM {
    /// Create a client from an explicit API key.
    pub fn new(api_key: impl Into<String>) -> Result<Self> {
        Self::with_config(ClientConfig {
            api_key: Some(api_key.into()),
            ..Default::default()
        })
    }

    /// Create a client reading `SIMPLELLM_API_KEY` and optionally `SIMPLELLM_BASE_URL` from env.
    pub fn from_env() -> Result<Self> {
        Self::with_config(ClientConfig::default())
    }

    /// Create a client with explicit configuration.
    pub fn with_config(cfg: ClientConfig) -> Result<Self> {
        let api_key = cfg
            .api_key
            .filter(|k| !k.is_empty())
            .or_else(|| {
                std::env::var("SIMPLELLM_API_KEY")
                    .ok()
                    .filter(|k| !k.is_empty())
            })
            .ok_or(SimpleLlmError::MissingApiKey)?;

        let base_url = cfg
            .base_url
            .or_else(|| std::env::var("SIMPLELLM_BASE_URL").ok())
            .unwrap_or_else(|| DEFAULT_BASE_URL.to_string())
            .trim_end_matches('/')
            .to_string();

        let timeout = cfg.timeout.unwrap_or(DEFAULT_TIMEOUT);

        let http = Client::builder()
            .use_rustls_tls()
            .timeout(timeout)
            .build()
            .map_err(SimpleLlmError::Http)?;

        Ok(Self {
            api_key,
            base_url,
            http,
        })
    }

    // ── Internal ──

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }

    fn auth_header(&self) -> String {
        format!("Bearer {}", self.api_key)
    }

    async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        let res = self
            .http
            .get(self.url(path))
            .header("Authorization", self.auth_header())
            .send()
            .await?;

        self.handle_response(res).await
    }

    async fn post_json<B: serde::Serialize, T: DeserializeOwned>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T> {
        let res = self
            .http
            .post(self.url(path))
            .header("Authorization", self.auth_header())
            .json(body)
            .send()
            .await?;

        self.handle_response(res).await
    }

    async fn handle_response<T: DeserializeOwned>(&self, res: reqwest::Response) -> Result<T> {
        let status = res.status().as_u16();
        if res.status().is_success() {
            Ok(res.json::<T>().await?)
        } else {
            let body = res.text().await.unwrap_or_default();
            let (message, code) = parse_error_body(&body);
            Err(SimpleLlmError::Api {
                message,
                status,
                code,
            })
        }
    }

    // ── Chat Completions ──

    /// Send a non-streaming chat completion request.
    pub async fn chat_completion(&self, req: ChatCompletionRequest) -> Result<ChatCompletion> {
        let req = ChatCompletionRequest {
            stream: Some(false),
            ..req
        };
        self.post_json("/v1/chat/completions", &req).await
    }

    /// Send a streaming chat completion request. Returns a Stream of chunks.
    pub async fn chat_completion_stream(
        &self,
        req: ChatCompletionRequest,
    ) -> Result<impl futures_util::Stream<Item = Result<ChatCompletionChunk>> + Send> {
        let req = ChatCompletionRequest {
            stream: Some(true),
            ..req
        };

        let res = self
            .http
            .post(self.url("/v1/chat/completions"))
            .header("Authorization", self.auth_header())
            .json(&req)
            .send()
            .await?;

        let status = res.status().as_u16();
        if !res.status().is_success() {
            let body = res.text().await.unwrap_or_default();
            let (message, code) = parse_error_body(&body);
            return Err(SimpleLlmError::Api {
                message,
                status,
                code,
            });
        }

        let byte_stream = res.bytes_stream();

        Ok(futures_util::stream::unfold(
            (byte_stream, String::new()),
            |(mut stream, mut buf)| async move {
                loop {
                    // first drain the buffer
                    if let Some(pos) = buf.find('\n') {
                        let line = buf[..pos].to_string();
                        buf = buf[pos + 1..].to_string();
                        let trimmed = line.trim().to_string();
                        if trimmed.is_empty() {
                            continue;
                        }
                        if let Some(data) = trimmed.strip_prefix("data: ") {
                            if data == "[DONE]" {
                                return None;
                            }
                            let chunk = serde_json::from_str::<ChatCompletionChunk>(data)
                                .map_err(|e| SimpleLlmError::SseParse(e.to_string()));
                            return Some((chunk, (stream, buf)));
                        }
                        continue;
                    }
                    // need more data
                    match stream.next().await {
                        Some(Ok(bytes)) => {
                            buf.push_str(&String::from_utf8_lossy(&bytes));
                        }
                        Some(Err(e)) => {
                            return Some((Err(SimpleLlmError::Http(e)), (stream, buf)));
                        }
                        None => {
                            if buf.is_empty() {
                                return None;
                            }
                            // process remaining buffer
                            let line = std::mem::take(&mut buf);
                            let trimmed = line.trim().to_string();
                            if let Some(data) = trimmed.strip_prefix("data: ") {
                                if data != "[DONE]" {
                                    let chunk = serde_json::from_str::<ChatCompletionChunk>(data)
                                        .map_err(|e| SimpleLlmError::SseParse(e.to_string()));
                                    return Some((chunk, (stream, buf)));
                                }
                            }
                            return None;
                        }
                    }
                }
            },
        ))
    }

    // ── Models ──

    /// List available models.
    pub async fn models(&self) -> Result<ModelList> {
        self.get("/v1/models").await
    }

    // ── Audio ──

    /// Transcribe audio. `file_bytes` is the raw audio file content; `filename` sets the multipart filename.
    pub async fn transcribe(
        &self,
        file_bytes: Vec<u8>,
        filename: impl Into<String>,
        req: TranscriptionRequest,
    ) -> Result<Transcription> {
        let file_part = multipart::Part::bytes(file_bytes)
            .file_name(filename.into())
            .mime_str("audio/mpeg")
            .map_err(SimpleLlmError::Http)?;

        let mut form = multipart::Form::new().part("file", file_part);
        if let Some(m) = &req.model {
            form = form.text("model", m.clone());
        }
        if let Some(l) = &req.language {
            form = form.text("language", l.clone());
        }
        if let Some(p) = &req.prompt {
            form = form.text("prompt", p.clone());
        }
        if let Some(rf) = &req.response_format {
            form = form.text("response_format", rf.clone());
        }
        if let Some(t) = req.temperature {
            form = form.text("temperature", t.to_string());
        }

        let res = self
            .http
            .post(self.url("/v1/audio/transcriptions"))
            .header("Authorization", self.auth_header())
            .multipart(form)
            .send()
            .await?;

        self.handle_response(res).await
    }

    /// Generate speech audio. Returns raw bytes (mp3 or the requested format).
    pub async fn speech(&self, req: SpeechRequest) -> Result<Bytes> {
        let res = self
            .http
            .post(self.url("/v1/audio/speech"))
            .header("Authorization", self.auth_header())
            .json(&req)
            .send()
            .await?;

        let status = res.status().as_u16();
        if !res.status().is_success() {
            let body = res.text().await.unwrap_or_default();
            let (message, code) = parse_error_body(&body);
            return Err(SimpleLlmError::Api {
                message,
                status,
                code,
            });
        }

        Ok(res.bytes().await?)
    }

    // ── Images ──

    /// Generate images.
    pub async fn generate_image(&self, req: ImageGenerationRequest) -> Result<ImageResponse> {
        self.post_json("/v1/images/generations", &req).await
    }

    // ── Account Usage ──

    /// Get account balance and usage statistics.
    pub async fn usage(&self) -> Result<AccountUsage> {
        self.get("/v1/usage").await
    }

    // ── API Keys ──

    /// List all API keys for the account.
    pub async fn keys(&self) -> Result<ApiKeyList> {
        self.get("/v1/keys").await
    }

    /// Get info about the current API key.
    pub async fn key_current(&self) -> Result<ApiKeyInfo> {
        self.get("/v1/keys/current").await
    }

    /// Get usage for the current API key.
    pub async fn key_current_usage(&self) -> Result<ApiKeyUsage> {
        self.get("/v1/keys/current/usage").await
    }

    /// Get daily usage for the current API key.
    pub async fn key_current_daily_usage(&self) -> Result<ApiKeyDailyUsageList> {
        self.get("/v1/keys/current/usage/daily").await
    }

    /// Get usage for a specific API key by ID.
    pub async fn key_usage(&self, key_id: &str) -> Result<ApiKeyUsage> {
        self.get(&format!("/v1/keys/{}/usage", urlencoding_encode(key_id)))
            .await
    }

    /// Get daily usage for a specific API key by ID.
    pub async fn key_daily_usage(&self, key_id: &str) -> Result<ApiKeyDailyUsageList> {
        self.get(&format!(
            "/v1/keys/{}/usage/daily",
            urlencoding_encode(key_id)
        ))
        .await
    }
}

pub(crate) fn parse_error_body(body: &str) -> (String, Option<String>) {
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(body) {
        let msg = v
            .get("error")
            .and_then(|e| e.get("message"))
            .and_then(|m| m.as_str())
            .or_else(|| v.get("message").and_then(|m| m.as_str()))
            .unwrap_or("Unknown error")
            .to_string();
        let code = v
            .get("error")
            .and_then(|e| e.get("code"))
            .and_then(|c| c.as_str())
            .or_else(|| v.get("code").and_then(|c| c.as_str()))
            .map(|s| s.to_string());
        (msg, code)
    } else {
        (
            if body.is_empty() {
                "Unknown error".to_string()
            } else {
                body.to_string()
            },
            None,
        )
    }
}

fn urlencoding_encode(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' || c == '.' || c == '~' {
                c.to_string()
            } else {
                format!("%{:02X}", c as u32)
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_error_body_valid_json() {
        let body = r#"{"error":{"message":"Unauthorized","code":"auth_error"}}"#;
        let (msg, code) = parse_error_body(body);
        assert_eq!(msg, "Unauthorized");
        assert_eq!(code.as_deref(), Some("auth_error"));
    }

    #[test]
    fn test_parse_error_body_empty() {
        let (msg, code) = parse_error_body("");
        assert_eq!(msg, "Unknown error");
        assert!(code.is_none());
    }

    #[test]
    fn test_parse_error_body_plain_text() {
        let (msg, code) = parse_error_body("Internal Server Error");
        assert_eq!(msg, "Internal Server Error");
        assert!(code.is_none());
    }

    #[test]
    fn test_chat_request_serialization_omits_optional_fields() {
        let req = ChatCompletionRequest {
            model: "deepseek-v3".to_string(),
            messages: vec![ChatMessage {
                role: Role::User,
                content: Some("Hi".to_string()),
                name: None,
                tool_calls: None,
                tool_call_id: None,
            }],
            ..Default::default()
        };

        let json = serde_json::to_value(&req).unwrap();
        assert!(
            json.get("temperature").is_none(),
            "temperature should be absent"
        );
        assert!(json.get("top_p").is_none(), "top_p should be absent");
        assert!(
            json.get("max_tokens").is_none(),
            "max_tokens should be absent"
        );
        assert!(json.get("stream").is_none(), "stream should be absent");
        assert!(json.get("stop").is_none(), "stop should be absent");
        assert!(json.get("tools").is_none(), "tools should be absent");
        assert!(
            json.get("tool_choice").is_none(),
            "tool_choice should be absent"
        );
        assert_eq!(json["model"], "deepseek-v3");
    }

    #[test]
    fn test_urlencoding_encode() {
        assert_eq!(urlencoding_encode("abc-123_test.~"), "abc-123_test.~");
        assert_eq!(urlencoding_encode("key id"), "key%20id");
        assert_eq!(urlencoding_encode("key/id"), "key%2Fid");
    }
}
