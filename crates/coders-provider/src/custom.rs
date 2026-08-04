use crate::{anthropic, openai_compat, ChatRequest, ChatResponse, ProviderClient, ProviderConfig};
use anyhow::{Context, Result};
use async_trait::async_trait;
use reqwest::Client;
use serde_json::{Map, Value};

/// Which base request/response JSON shape a custom endpoint speaks.
/// Everything else (auth header, field names, extra static fields) is
/// layered on top via `CustomProviderOptions`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestStyle {
    OpenAi,
    Anthropic,
}

#[derive(Debug, Clone)]
pub struct CustomProviderOptions {
    pub style: RequestStyle,
    /// Only used for `RequestStyle::OpenAi`. Some gateways require
    /// `max_completion_tokens` instead of `max_tokens`.
    pub max_tokens_field: String,
    /// Header the API key is sent in, e.g. "Authorization" or "api-key".
    pub auth_header: String,
    /// Prefix placed before the key in `auth_header`, e.g. "Bearer " — pass
    /// "" for headers that want the raw key (like "api-key" or "x-api-key").
    pub auth_scheme: String,
    /// Extra static headers sent on every request (e.g. an API version pin).
    pub extra_headers: Vec<(String, String)>,
    /// Extra top-level fields merged into the request body, overriding any
    /// same-named field the base style would otherwise set.
    pub extra_body: Map<String, Value>,
}

impl Default for CustomProviderOptions {
    fn default() -> Self {
        Self {
            style: RequestStyle::OpenAi,
            max_tokens_field: "max_tokens".to_string(),
            auth_header: "Authorization".to_string(),
            auth_scheme: "Bearer ".to_string(),
            extra_headers: Vec::new(),
            extra_body: Map::new(),
        }
    }
}

/// A company/gateway-specific endpoint that mostly speaks OpenAI's or
/// Anthropic's request shape but differs in auth header, field names, or
/// needs extra static fields — configured entirely from config.toml instead
/// of requiring a new Rust provider per quirk.
pub struct CustomProvider {
    client: Client,
    config: ProviderConfig,
    options: CustomProviderOptions,
}

impl CustomProvider {
    pub fn new(config: ProviderConfig, options: CustomProviderOptions) -> Result<Self> {
        let client = crate::client::build_client(&config)?;
        Ok(Self { client, config, options })
    }
}

#[async_trait]
impl ProviderClient for CustomProvider {
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse> {
        let url = self.config.base_url.clone().context("provider 'custom' requires base_url to be set in config.toml")?;

        let mut body = match self.options.style {
            RequestStyle::OpenAi => openai_compat::build_body(&request, &self.options.max_tokens_field),
            RequestStyle::Anthropic => anthropic::build_body(&request),
        };
        if let Value::Object(map) = &mut body {
            for (key, value) in &self.options.extra_body {
                map.insert(key.clone(), value.clone());
            }
        }

        let mut req = self.client.post(&url).header("content-type", "application/json");
        if !self.config.api_key.is_empty() {
            req = req.header(self.options.auth_header.as_str(), format!("{}{}", self.options.auth_scheme, self.config.api_key));
        }
        for (key, value) in &self.options.extra_headers {
            req = req.header(key.as_str(), value.as_str());
        }

        let resp = req.json(&body).send().await.context("sending request to custom endpoint")?;
        let status = resp.status();
        let payload: Value = resp.json().await.context("parsing response from custom endpoint")?;
        if !status.is_success() {
            anyhow::bail!("custom endpoint error ({status}): {payload}");
        }

        let (content, stop_reason) = match self.options.style {
            RequestStyle::OpenAi => openai_compat::parse_response(&payload),
            RequestStyle::Anthropic => anthropic::parse_response(&payload),
        };
        Ok(ChatResponse { content, stop_reason })
    }
}
