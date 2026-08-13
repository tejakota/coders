use crate::types::{ChatRequest, ChatResponse, ContentBlock, Message, Role, StopReason};
use crate::{ProviderClient, ProviderConfig};
use anyhow::{Context, Result};
use async_trait::async_trait;
use reqwest::Client;
use serde_json::{json, Value};

const DEFAULT_BASE_URL: &str = "https://api.openai.com/v1/chat/completions";

/// Any backend speaking the OpenAI chat-completions schema: OpenAI itself,
/// Ollama, vLLM, OpenRouter, etc. Pick the API base via config.base_url.
pub struct OpenAiCompatProvider {
    client: Client,
    config: ProviderConfig,
}

impl OpenAiCompatProvider {
    pub fn new(config: ProviderConfig) -> Result<Self> {
        let client = crate::client::build_client(&config)?;
        Ok(Self { client, config })
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

/// OpenAI splits a "turn" into distinct messages: assistant text+tool_calls,
/// then one role:"tool" message per tool result. Flatten our block-based
/// Message into that shape.
fn messages_to_json(messages: &[Message]) -> Vec<Value> {
    let mut out = Vec::new();
    for message in messages {
        let text: String = message
            .content
            .iter()
            .filter_map(|b| match b {
                ContentBlock::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect();

        let tool_calls: Vec<Value> = message
            .content
            .iter()
            .filter_map(|b| match b {
                ContentBlock::ToolUse { id, name, input } => Some(json!({
                    "id": id,
                    "type": "function",
                    "function": { "name": name, "arguments": input.to_string() },
                })),
                _ => None,
            })
            .collect();

        if !text.is_empty() || !tool_calls.is_empty() {
            let mut entry = json!({ "role": role_str(&message.role), "content": text });
            if !tool_calls.is_empty() {
                entry["tool_calls"] = json!(tool_calls);
            }
            out.push(entry);
        }

        for block in &message.content {
            if let ContentBlock::ToolResult { tool_use_id, content, .. } = block {
                out.push(json!({
                    "role": "tool",
                    "tool_call_id": tool_use_id,
                    "content": content,
                }));
            }
        }
    }
    out
}

/// Builds an OpenAI chat-completions request body. `max_tokens_field` lets
/// the `custom` provider rename the token-limit key for gateways that
/// require e.g. `max_completion_tokens` instead of `max_tokens`.
pub(crate) fn build_body(request: &ChatRequest, max_tokens_field: &str) -> Value {
    let mut messages = Vec::new();
    if let Some(system) = &request.system {
        messages.push(json!({ "role": "system", "content": system }));
    }
    messages.extend(messages_to_json(&request.messages));

    let tools: Vec<Value> = request
        .tools
        .iter()
        .map(|t| {
            json!({
                "type": "function",
                "function": { "name": t.name, "description": t.description, "parameters": t.input_schema },
            })
        })
        .collect();

    let mut body = json!({
        "model": request.model,
        "messages": messages,
    });
    body[max_tokens_field] = json!(request.max_tokens);
    if !tools.is_empty() {
        body["tools"] = json!(tools);
    }
    body
}

pub(crate) fn parse_response(payload: &Value) -> (Vec<ContentBlock>, StopReason) {
    let choice = &payload["choices"][0];
    let message = &choice["message"];
    let mut content = Vec::new();

    if let Some(text) = message.get("content").and_then(Value::as_str) {
        if !text.is_empty() {
            content.push(ContentBlock::Text { text: text.to_string() });
        }
    }

    if let Some(tool_calls) = message.get("tool_calls").and_then(Value::as_array) {
        for call in tool_calls {
            let id = call["id"].as_str().unwrap_or_default().to_string();
            let name = call["function"]["name"].as_str().unwrap_or_default().to_string();
            let args_str = call["function"]["arguments"].as_str().unwrap_or("{}");
            let input: Value = serde_json::from_str(args_str).unwrap_or(json!({}));
            content.push(ContentBlock::ToolUse { id, name, input });
        }
    }

    let stop_reason = match choice.get("finish_reason").and_then(Value::as_str) {
        Some("tool_calls") => StopReason::ToolUse,
        Some("stop") => StopReason::EndTurn,
        Some(other) => StopReason::Other(other.to_string()),
        None => StopReason::EndTurn,
    };

    (content, stop_reason)
}

#[async_trait]
impl ProviderClient for OpenAiCompatProvider {
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse> {
        let body = build_body(&request, "max_tokens");

        let mut req = self.client.post(self.url()).header("content-type", "application/json").json(&body);
        if !self.config.api_key.is_empty() {
            req = req.bearer_auth(&self.config.api_key);
        }

        let resp = req.send().await.context("sending request to OpenAI-compatible endpoint")?;
        let status = resp.status();
        let payload: Value = resp.json().await.context("parsing response")?;
        if !status.is_success() {
            anyhow::bail!("API error ({status}): {payload}");
        }

        let (content, stop_reason) = parse_response(&payload);
        Ok(ChatResponse { content, stop_reason })
    }
}
