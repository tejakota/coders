mod bash;
mod fs;
mod search;

pub use bash::BashTool;
pub use fs::{ReadFileTool, WriteFileTool};
pub use search::{FindTool, GrepTool};

use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;

/// A callable the model can invoke. External plugin tools (subprocess-backed,
/// described by a manifest) will implement this same trait later.
#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn input_schema(&self) -> Value;
    async fn execute(&self, input: Value) -> Result<String>;
}

pub fn builtin_tools() -> Vec<Box<dyn Tool>> {
    vec![Box::new(ReadFileTool), Box::new(WriteFileTool), Box::new(BashTool), Box::new(GrepTool), Box::new(FindTool)]
}
