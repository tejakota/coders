use crate::types::{ChatRequest, ChatResponse, ContentBlock, Message, Role, StopReason};
use crate::{ProviderClient, ProviderConfig};
use anyhow::{Context, Result};
use async_trait::async_trait;
use reqwest::Client;
use serde_json::{json, Value};

const DEFAULT_BASE_URL: &str = "https://api.anthropic.com/v1/messages";
const ANTHROPIC_VERSION: &str = "2023-06-01";

pub struct AnthropicProvider {
    client: Client,
    config: ProviderConfig,
}

impl AnthropicProvider {
    pub fn new(config: ProviderConfig) -> Self {
        Self { client: Client::new(), config }
    }

    fn url(&self) -> String {
        self.config.base_url.clone().unwrap_or_else(|| DEFAULT_BASE_URL.to_string())
    }
}

fn role_str(role: &Role) -> &'static str {
    match role {
        Role::User => "user",
        Role::Assistant => "assistant",
    }
}

fn block_to_json(block: &ContentBlock) -> Value {
    match block {
        ContentBlock::Text { text } => json!({ "type": "text", "text": text }),
        ContentBlock::ToolUse { id, name, input } => {
            json!({ "type": "tool_use", "id": id, "name": name, "input": input })
        }
        ContentBlock::ToolResult { tool_use_id, content, is_error } => json!({
            "type": "tool_result",
            "tool_use_id": tool_use_id,
            "content": content,
            "is_error": is_error,
        }),
    }
}

fn message_to_json(message: &Message) -> Value {
    json!({
        "role": role_str(&message.role),
        "content": message.content.iter().map(block_to_json).collect::<Vec<_>>(),
    })
}

fn parse_content_block(value: &Value) -> Option<ContentBlock> {
    match value.get("type").and_then(Value::as_str)? {
        "text" => Some(ContentBlock::Text { text: value.get("text")?.as_str()?.to_string() }),
        "tool_use" => Some(ContentBlock::ToolUse {
            id: value.get("id")?.as_str()?.to_string(),
            name: value.get("name")?.as_str()?.to_string(),
            input: value.get("input")?.clone(),
        }),
        _ => None,
    }
}

#[async_trait]
impl ProviderClient for AnthropicProvider {
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse> {
        let tools: Vec<Value> = request
            .tools
            .iter()
            .map(|t| json!({ "name": t.name, "description": t.description, "input_schema": t.input_schema }))
            .collect();

        let mut body = json!({
            "model": request.model,
            "max_tokens": request.max_tokens,
            "messages": request.messages.iter().map(message_to_json).collect::<Vec<_>>(),
        });
        if let Some(system) = &request.system {
            body["system"] = json!(system);
        }
        if !tools.is_empty() {
            body["tools"] = json!(tools);
        }

        let resp = self
            .client
            .post(self.url())
            .header("x-api-key", &self.config.api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await
            .context("sending request to Anthropic")?;

        let status = resp.status();
        let payload: Value = resp.json().await.context("parsing Anthropic response")?;
        if !status.is_success() {
            anyhow::bail!("Anthropic API error ({status}): {payload}");
        }

        let content = payload
            .get("content")
            .and_then(Value::as_array)
            .map(|arr| arr.iter().filter_map(parse_content_block).collect())
            .unwrap_or_default();

        let stop_reason = match payload.get("stop_reason").and_then(Value::as_str) {
            Some("tool_use") => StopReason::ToolUse,
            Some("end_turn") => StopReason::EndTurn,
            Some(other) => StopReason::Other(other.to_string()),
            None => StopReason::EndTurn,
        };

        Ok(ChatResponse { content, stop_reason })
    }
}
