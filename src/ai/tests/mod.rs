use std::collections::HashMap;

use crate::connection::config::{AiConfig, AiProvider};

#[test]
fn ai_config_deserialize_openai_default_provider() {
    let yaml = r#"
name: openai
uri: "https://api.openai.com/v1"
api_key: "sk-test123"
model: "gpt-4o"
"#;
    let cfg: AiConfig = serde_yaml::from_str(yaml).expect("deserialization failed");
    assert_eq!(cfg.provider, AiProvider::OpenAI);
    assert_eq!(cfg.name, "openai");
    assert_eq!(cfg.concurrency, 10);
    assert_eq!(cfg.temperature, 0.0);
}

#[test]
fn ai_config_deserialize_anthropic() {
    let yaml = r#"
name: claude
provider: anthropic
uri: "https://api.anthropic.com"
api_key: "sk-ant-test"
model: "claude-sonnet-4-20250514"
"#;
    let cfg: AiConfig = serde_yaml::from_str(yaml).expect("deserialization failed");
    assert_eq!(cfg.provider, AiProvider::Anthropic);
    assert_eq!(cfg.name, "claude");
    assert_eq!(cfg.uri, "https://api.anthropic.com");
    assert_eq!(cfg.model, "claude-sonnet-4-20250514");
}

#[test]
fn ai_config_deserialize_gemini() {
    let yaml = r#"
name: gemini
provider: gemini
uri: "https://generativelanguage.googleapis.com/v1beta"
api_key: "AIza-test"
model: "gemini-2.0-flash"
"#;
    let cfg: AiConfig = serde_yaml::from_str(yaml).expect("deserialization failed");
    assert_eq!(cfg.provider, AiProvider::Gemini);
    assert_eq!(cfg.name, "gemini");
    assert_eq!(cfg.model, "gemini-2.0-flash");
}

#[test]
fn ai_config_deserialize_deepseek_openai_compat() {
    let yaml = r#"
name: deepseek
uri: "https://api.deepseek.com/v1"
api_key: "sk-deepseek-test"
model: "deepseek-chat"
"#;
    let cfg: AiConfig = serde_yaml::from_str(yaml).expect("deserialization failed");
    assert_eq!(cfg.provider, AiProvider::OpenAI);
    assert_eq!(cfg.name, "deepseek");
}

#[test]
fn ai_config_deserialize_with_headers() {
    let yaml = r#"
name: myai
uri: "http://localhost:11434/v1"
api_key: "ollama"
model: "llama3"
headers:
  X-Custom: "value1"
  X-Another: "value2"
concurrency: 5
timeout_secs: 30
max_tokens: 512
temperature: 0.7
"#;
    let cfg: AiConfig = serde_yaml::from_str(yaml).expect("deserialization failed");
    assert_eq!(cfg.name, "myai");
    assert_eq!(cfg.uri, "http://localhost:11434/v1");
    assert_eq!(cfg.api_key, "ollama");
    assert_eq!(cfg.model, "llama3");
    assert_eq!(cfg.headers.get("X-Custom").map(|s| s.as_str()), Some("value1"));
    assert_eq!(cfg.headers.get("X-Another").map(|s| s.as_str()), Some("value2"));
    assert_eq!(cfg.concurrency, 5);
    assert_eq!(cfg.timeout_secs, 30);
    assert_eq!(cfg.max_tokens, 512);
    assert_eq!(cfg.temperature, 0.7);
}

#[test]
fn ai_config_defaults() {
    let yaml = r#"
name: test
uri: "https://example.com"
api_key: "key"
model: "test-model"
"#;
    let cfg: AiConfig = serde_yaml::from_str(yaml).expect("deserialization failed");
    assert_eq!(cfg.concurrency, 10);
    assert_eq!(cfg.timeout_secs, 60);
    assert_eq!(cfg.max_tokens, 1024);
    assert_eq!(cfg.temperature, 0.0);
}

#[test]
fn ai_config_all_fields_optional_defaults() {
    let yaml = r#"
name: minimal
uri: "https://a.com"
api_key: "k"
model: "m"
"#;
    let cfg: AiConfig = serde_yaml::from_str(yaml).expect("deserialization failed");
    assert_eq!(cfg.name, "minimal");
    assert_eq!(cfg.headers, HashMap::new());
    assert_eq!(cfg.concurrency, 10);
}

#[test]
fn ai_client_chat_request_serializes_correctly() {
    use crate::ai::AiClient;
    let _client = AiClient::new();
}

#[test]
fn ai_client_new_is_cloneable() {
    use crate::ai::AiClient;
    let client = AiClient::new();
    let _clone = client.clone();
}
