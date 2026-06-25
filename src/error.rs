use thiserror::Error;

#[derive(Debug, Error)]
pub enum SimpleLlmError {
    #[error("API error {status}: {message}")]
    Api {
        message: String,
        status: u16,
        code: Option<String>,
    },
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Missing API key: pass api_key or set SIMPLELLM_API_KEY env var")]
    MissingApiKey,
    #[error("SSE parse error: {0}")]
    SseParse(String),
}

pub type Result<T> = std::result::Result<T, SimpleLlmError>;
