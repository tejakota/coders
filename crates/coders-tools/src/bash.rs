use crate::Tool;
use anyhow::{Context, Result};
use async_trait::async_trait;
use serde_json::{json, Value};
use tokio::process::Command;

pub struct BashTool;

#[async_trait]
impl Tool for BashTool {
    fn name(&self) -> &str {
        "bash"
    }

    fn description(&self) -> &str {
        "Run a shell command (sh on Linux/macOS, cmd on Windows) and return its combined stdout/stderr output."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": { "command": { "type": "string", "description": "Shell command to run" } },
            "required": ["command"],
        })
    }

    fn requires_confirmation(&self) -> bool {
        true
    }

    async fn execute(&self, input: Value) -> Result<String> {
        let command = input.get("command").and_then(Value::as_str).context("missing 'command' argument")?;

        // A stock Windows install has no `sh` on PATH (that only shows up
        // via WSL/Git Bash), so shell out through cmd.exe there instead.
        #[cfg(windows)]
        let mut cmd = {
            let mut c = Command::new("cmd");
            c.arg("/C").arg(command);
            c
        };
        #[cfg(not(windows))]
        let mut cmd = {
            let mut c = Command::new("sh");
            c.arg("-c").arg(command);
            c
        };

        let output = cmd.output().await.context("spawning shell")?;
        let mut result = String::from_utf8_lossy(&output.stdout).to_string();
        result.push_str(&String::from_utf8_lossy(&output.stderr));
        if !output.status.success() {
            result.push_str(&format!("\n[exit code: {}]", output.status.code().unwrap_or(-1)));
        }
        Ok(result)
    }
}
