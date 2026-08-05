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

    /// Whether running this tool needs explicit user approval first —
    /// e.g. shell execution or overwriting a file, where a wrong or
    /// adversarial model response could do real damage. Defaults to false;
    /// override for anything irreversible or destructive.
    fn requires_confirmation(&self) -> bool {
        false
    }
}

pub fn builtin_tools() -> Vec<Box<dyn Tool>> {
    vec![Box::new(ReadFileTool), Box::new(WriteFileTool), Box::new(BashTool), Box::new(GrepTool), Box::new(FindTool)]
}
