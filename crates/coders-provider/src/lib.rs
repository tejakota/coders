mod anthropic;
mod openai_compat;
pub mod types;

pub use anthropic::AnthropicProvider;
pub use openai_compat::OpenAiCompatProvider;
pub use types::*;

use anyhow::Result;
use async_trait::async_trait;

/// Implemented once per LLM backend. New providers (Gemini, Bedrock, ...)
/// plug in here without touching the agent loop or tool layer.
#[async_trait]
pub trait ProviderClient: Send + Sync {
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse>;
}

#[derive(Debug, Clone)]
pub struct ProviderConfig {
    pub api_key: String,
    pub base_url: Option<String>,
}

pub fn build_provider(kind: &str, config: ProviderConfig) -> Result<Box<dyn ProviderClient>> {
    match kind {
        "anthropic" => Ok(Box::new(AnthropicProvider::new(config))),
        "openai" | "openai-compat" | "ollama" => Ok(Box::new(OpenAiCompatProvider::new(config))),
        other => anyhow::bail!("unknown provider kind: {other}"),
    }
}
