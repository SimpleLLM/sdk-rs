//! # simplellm
//!
//! Async Rust client for the [SimpleLLM](https://simplellm.eu) API.
//!
//! ```no_run
//! use simplellm::{SimpleLLM, types::ChatCompletionRequest, types::ChatMessage, types::Role};
//!
//! #[tokio::main]
//! async fn main() -> simplellm::Result<()> {
//!     let client = SimpleLLM::from_env()?;
//!     let resp = client.chat_completion(ChatCompletionRequest {
//!         model: "DeepSeek-Chat-V3.1".to_string(),
//!         messages: vec![ChatMessage {
//!             role: Role::User,
//!             content: Some("Hello!".to_string()),
//!             name: None,
//!             tool_calls: None,
//!             tool_call_id: None,
//!         }],
//!         ..Default::default()
//!     }).await?;
//!     println!("{}", resp.choices[0].message.content.as_deref().unwrap_or(""));
//!     Ok(())
//! }
//! ```

pub mod client;
pub mod error;
pub mod types;

pub use client::{ClientConfig, SimpleLLM};
pub use error::{Result, SimpleLlmError};
