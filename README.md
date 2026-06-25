# simplellm-rs

[![Crates.io](https://img.shields.io/crates/v/simplellm.svg)](https://crates.io/crates/simplellm)
[![CI](https://github.com/SimpleLLM/sdk-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/SimpleLLM/sdk-rs/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

Async Rust SDK for the [SimpleLLM](https://simplellm.eu) API — EU-hosted, OpenAI-compatible LLM inference.

## Install

```toml
[dependencies]
simplellm = "0.1"
```

Or with cargo-add:

```bash
cargo add simplellm
```

## Quick Start

### Non-streaming chat

```rust
use simplellm::{SimpleLLM, types::{ChatCompletionRequest, ChatMessage, Role}};

#[tokio::main]
async fn main() -> simplellm::Result<()> {
    let client = SimpleLLM::from_env()?;

    let resp = client.chat_completion(ChatCompletionRequest {
        model: "DeepSeek-Chat-V3.1".to_string(),
        messages: vec![ChatMessage {
            role: Role::User,
            content: Some("What is the capital of France?".to_string()),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        }],
        ..Default::default()
    }).await?;

    println!("{}", resp.choices[0].message.content.as_deref().unwrap_or(""));
    Ok(())
}
```

### Streaming chat

```rust
use futures_util::StreamExt;
use simplellm::{SimpleLLM, types::{ChatCompletionRequest, ChatMessage, Role}};

#[tokio::main]
async fn main() -> simplellm::Result<()> {
    let client = SimpleLLM::from_env()?;

    let mut stream = client.chat_completion_stream(ChatCompletionRequest {
        model: "DeepSeek-Chat-V3.1".to_string(),
        messages: vec![ChatMessage {
            role: Role::User,
            content: Some("Count to 5.".to_string()),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        }],
        ..Default::default()
    }).await?;

    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        if let Some(content) = chunk.choices[0].delta.content.as_deref() {
            print!("{}", content);
        }
    }
    println!();
    Ok(())
}
```

### Audio transcription

```rust
use simplellm::{SimpleLLM, types::TranscriptionRequest};

#[tokio::main]
async fn main() -> simplellm::Result<()> {
    let client = SimpleLLM::from_env()?;

    let audio = std::fs::read("recording.mp3")?;
    let result = client.transcribe(audio, "recording.mp3", TranscriptionRequest {
        model: Some("whisper-large-v3".to_string()),
        language: Some("en".to_string()),
        ..Default::default()
    }).await?;

    println!("{}", result.text);
    Ok(())
}
```

### Text-to-speech

```rust
use simplellm::{SimpleLLM, types::SpeechRequest};

#[tokio::main]
async fn main() -> simplellm::Result<()> {
    let client = SimpleLLM::from_env()?;

    let audio_bytes = client.speech(SpeechRequest {
        input: "Hello from SimpleLLM!".to_string(),
        model: Some("tts-1".to_string()),
        voice: Some("alloy".to_string()),
        ..Default::default()
    }).await?;

    std::fs::write("output.mp3", &audio_bytes)?;
    Ok(())
}
```

### Image generation

```rust
use simplellm::{SimpleLLM, types::ImageGenerationRequest};

#[tokio::main]
async fn main() -> simplellm::Result<()> {
    let client = SimpleLLM::from_env()?;

    let resp = client.generate_image(ImageGenerationRequest {
        prompt: "A sunset over the Alps".to_string(),
        model: Some("sdxl".to_string()),
        n: Some(1),
        size: Some("1024x1024".to_string()),
        ..Default::default()
    }).await?;

    if let Some(url) = &resp.data[0].url {
        println!("Image URL: {}", url);
    }
    Ok(())
}
```

### Account balance & usage

```rust
use simplellm::SimpleLLM;

#[tokio::main]
async fn main() -> simplellm::Result<()> {
    let client = SimpleLLM::from_env()?;

    let usage = client.usage().await?;
    if let Some(balance) = usage.balance {
        println!("Balance: {:.4} SC", balance);
    }
    if let Some(spent) = usage.total_spent {
        println!("Total spent: {:.4} SC", spent);
    }
    Ok(())
}
```

### API keys

```rust
use simplellm::SimpleLLM;

#[tokio::main]
async fn main() -> simplellm::Result<()> {
    let client = SimpleLLM::from_env()?;

    // List all keys
    let keys = client.keys().await?;
    for key in &keys.api_keys {
        println!("{}: {} (balance: {} SC)", key.id, key.name, key.balance_sc);
    }

    // Current key info
    let current = client.key_current().await?;
    println!("Current key: {}", current.prefix);

    // Usage for current key
    let key_usage = client.key_current_usage().await?;
    println!("Requests: {}, Tokens in: {}, out: {}",
        key_usage.total_requests,
        key_usage.total_tokens_in,
        key_usage.total_tokens_out,
    );

    Ok(())
}
```

## Configuration

| Method | Description |
|---|---|
| `SimpleLLM::new(api_key)` | Explicit API key |
| `SimpleLLM::from_env()` | Reads `SIMPLELLM_API_KEY` (and optionally `SIMPLELLM_BASE_URL`) from environment |
| `SimpleLLM::with_config(cfg)` | Full control via `ClientConfig` |

### Environment variables

| Variable | Default | Description |
|---|---|---|
| `SIMPLELLM_API_KEY` | — | Your API key (required if not passed directly) |
| `SIMPLELLM_BASE_URL` | `https://api.simplellm.eu` | Override the API base URL |

### ClientConfig

```rust
use std::time::Duration;
use simplellm::{SimpleLLM, ClientConfig};

let client = SimpleLLM::with_config(ClientConfig {
    api_key: Some("sk-simplellm-...".to_string()),
    base_url: Some("https://api.simplellm.eu".to_string()),
    timeout: Some(Duration::from_secs(60)),
})?;
```

## SDKs

| Language | Package | Repo |
|---|---|---|
| Node.js / TypeScript | `@simplellm/sdk` | [sdk-js](https://github.com/SimpleLLM/sdk-js) |
| Rust | `simplellm` | [sdk-rs](https://github.com/SimpleLLM/sdk-rs) |
| Go | `github.com/SimpleLLM/sdk-go` | [sdk-go](https://github.com/SimpleLLM/sdk-go) |
| C++ | `simplellm` | [sdk-cpp](https://github.com/SimpleLLM/sdk-cpp) |

## License

MIT — see [LICENSE](LICENSE).
