use anyhow::{Context, Result};
use coders_provider::{ChatRequest, ContentBlock, Message, ProviderClient, StopReason, ToolDef};
use coders_tools::Tool;

const MAX_TOOL_ROUNDS: usize = 25;

/// Owns conversation history and drives the read-plan-act loop: send the
/// transcript to the model, execute any tool calls it asks for, feed the
/// results back, and repeat until the model produces a plain text turn.
pub struct Agent {
    provider: Box<dyn ProviderClient>,
    tools: Vec<Box<dyn Tool>>,
    model: String,
    system: Option<String>,
    max_tokens: u32,
    history: Vec<Message>,
}

impl Agent {
    pub fn new(provider: Box<dyn ProviderClient>, tools: Vec<Box<dyn Tool>>, model: String, system: Option<String>) -> Self {
        Self { provider, tools, model, system, max_tokens: 4096, history: Vec::new() }
    }

    fn tool_defs(&self) -> Vec<ToolDef> {
        self.tools
            .iter()
            .map(|t| ToolDef { name: t.name().to_string(), description: t.description().to_string(), input_schema: t.input_schema() })
            .collect()
    }

    fn find_tool(&self, name: &str) -> Option<&dyn Tool> {
        self.tools.iter().find(|t| t.name() == name).map(|t| t.as_ref())
    }

    /// Sends one user turn through the loop, executing any requested tools
    /// along the way, and returns the model's final text reply.
    pub async fn send(&mut self, user_input: &str) -> Result<String> {
        self.history.push(Message::user_text(user_input));

        for _ in 0..MAX_TOOL_ROUNDS {
            let request = ChatRequest {
                model: self.model.clone(),
                system: self.system.clone(),
                messages: self.history.clone(),
                tools: self.tool_defs(),
                max_tokens: self.max_tokens,
            };

            let response = self.provider.chat(request).await.context("provider chat call failed")?;
            self.history.push(Message { role: coders_provider::Role::Assistant, content: response.content.clone() });

            if response.stop_reason != StopReason::ToolUse {
                return Ok(response.text());
            }

            let tool_uses: Vec<(String, String, serde_json::Value)> =
                response.tool_uses().into_iter().map(|(id, name, input)| (id.to_string(), name.to_string(), input.clone())).collect();

            let mut result_blocks = Vec::new();
            for (id, name, input) in tool_uses {
                let outcome = match self.find_tool(&name) {
                    Some(tool) => tool.execute(input).await,
                    None => Err(anyhow::anyhow!("unknown tool: {name}")),
                };
                let (content, is_error) = match outcome {
                    Ok(text) => (text, false),
                    Err(err) => (err.to_string(), true),
                };
                result_blocks.push(ContentBlock::ToolResult { tool_use_id: id, content, is_error });
            }
            self.history.push(Message { role: coders_provider::Role::User, content: result_blocks });
        }

        anyhow::bail!("exceeded max tool-call rounds ({MAX_TOOL_ROUNDS}) without a final answer")
    }
}
