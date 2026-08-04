use crate::Tool;
use anyhow::{Context, Result};
use async_trait::async_trait;
use serde_json::{json, Value};

pub struct ReadFileTool;

#[async_trait]
impl Tool for ReadFileTool {
    fn name(&self) -> &str {
        "read_file"
    }

    fn description(&self) -> &str {
        "Read the contents of a file at the given path."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": { "path": { "type": "string", "description": "Path to the file" } },
            "required": ["path"],
        })
    }

    async fn execute(&self, input: Value) -> Result<String> {
        let path = input.get("path").and_then(Value::as_str).context("missing 'path' argument")?;
        let content = tokio::fs::read_to_string(path).await.with_context(|| format!("reading {path}"))?;
        Ok(content)
    }
}

pub struct WriteFileTool;

#[async_trait]
impl Tool for WriteFileTool {
    fn name(&self) -> &str {
        "write_file"
    }

    fn description(&self) -> &str {
        "Write (overwrite) a file at the given path with the given content."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Path to the file" },
                "content": { "type": "string", "description": "New file content" },
            },
            "required": ["path", "content"],
        })
    }

    async fn execute(&self, input: Value) -> Result<String> {
        let path = input.get("path").and_then(Value::as_str).context("missing 'path' argument")?;
        let content = input.get("content").and_then(Value::as_str).context("missing 'content' argument")?;
        if let Some(parent) = std::path::Path::new(path).parent() {
            if !parent.as_os_str().is_empty() {
                tokio::fs::create_dir_all(parent).await.ok();
            }
        }
        tokio::fs::write(path, content).await.with_context(|| format!("writing {path}"))?;
        Ok(format!("wrote {} bytes to {path}", content.len()))
    }
}
