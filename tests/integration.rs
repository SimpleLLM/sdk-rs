use simplellm::types::*;
use simplellm::{ClientConfig, SimpleLLM, SimpleLlmError};
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn test_chat_completion() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .and(header("Authorization", "Bearer test-key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "chatcmpl-123",
            "object": "chat.completion",
            "created": 1700000000_i64,
            "model": "DeepSeek-Chat-V3.1",
            "choices": [{
                "index": 0,
                "message": {"role": "assistant", "content": "Hello!"},
                "finish_reason": "stop"
            }],
            "usage": {"prompt_tokens": 10, "completion_tokens": 5, "total_tokens": 15}
        })))
        .mount(&server)
        .await;

    let client = SimpleLLM::with_config(ClientConfig {
        api_key: Some("test-key".to_string()),
        base_url: Some(server.uri()),
        ..Default::default()
    })
    .unwrap();

    let resp = client
        .chat_completion(ChatCompletionRequest {
            model: "DeepSeek-Chat-V3.1".to_string(),
            messages: vec![ChatMessage {
                role: Role::User,
                content: Some("Hello!".to_string()),
                name: None,
                tool_calls: None,
                tool_call_id: None,
            }],
            ..Default::default()
        })
        .await
        .unwrap();

    assert_eq!(resp.id, "chatcmpl-123");
    assert_eq!(resp.choices[0].message.content.as_deref(), Some("Hello!"));
    assert!(resp.usage.is_some());
    let usage = resp.usage.unwrap();
    assert_eq!(usage.prompt_tokens, 10);
    assert_eq!(usage.completion_tokens, 5);
    assert_eq!(usage.total_tokens, 15);
}

#[tokio::test]
async fn test_models() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "object": "list",
            "data": [
                {
                    "id": "DeepSeek-Chat-V3.1",
                    "object": "model",
                    "created": 1700000000_i64,
                    "owned_by": "simplellm"
                }
            ]
        })))
        .mount(&server)
        .await;

    let client = SimpleLLM::with_config(ClientConfig {
        api_key: Some("test-key".to_string()),
        base_url: Some(server.uri()),
        ..Default::default()
    })
    .unwrap();

    let list = client.models().await.unwrap();
    assert_eq!(list.data.len(), 1);
    assert_eq!(list.data[0].id, "DeepSeek-Chat-V3.1");
    assert_eq!(list.data[0].owned_by, "simplellm");
}

#[tokio::test]
async fn test_api_error() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
            "error": {"message": "Unauthorized", "code": "auth_error"}
        })))
        .mount(&server)
        .await;

    let client = SimpleLLM::with_config(ClientConfig {
        api_key: Some("bad-key".to_string()),
        base_url: Some(server.uri()),
        ..Default::default()
    })
    .unwrap();

    let err = client.models().await.unwrap_err();
    match err {
        SimpleLlmError::Api {
            status,
            code,
            message,
        } => {
            assert_eq!(status, 401);
            assert_eq!(code.as_deref(), Some("auth_error"));
            assert_eq!(message, "Unauthorized");
        }
        other => panic!("Expected Api error, got: {:?}", other),
    }
}

#[tokio::test]
async fn test_missing_api_key() {
    // Ensure env var is not set for this test
    std::env::remove_var("SIMPLELLM_API_KEY");

    let result = SimpleLLM::with_config(ClientConfig {
        api_key: None,
        base_url: None,
        ..Default::default()
    });

    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), SimpleLlmError::MissingApiKey));
}
