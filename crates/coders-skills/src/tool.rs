use crate::Skill;
use anyhow::{Context, Result};
use async_trait::async_trait;
use coders_tools::Tool;
use serde_json::{json, Value};

/// Exposes loaded skills to the model as a single tool: call it with a
/// skill's name to receive that skill's full instruction body, which the
/// model then follows for the rest of the task.
pub struct SkillTool {
    skills: Vec<Skill>,
}

impl SkillTool {
    pub fn new(skills: Vec<Skill>) -> Self {
        Self { skills }
    }
}

#[async_trait]
impl Tool for SkillTool {
    fn name(&self) -> &str {
        "skill"
    }

    fn description(&self) -> &str {
        "Load the full instructions for a named skill. Call this before performing a task that matches one of the available skills listed in the system prompt."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": { "name": { "type": "string", "description": "Name of the skill to load" } },
            "required": ["name"],
        })
    }

    async fn execute(&self, input: Value) -> Result<String> {
        let name = input.get("name").and_then(Value::as_str).context("missing 'name' argument")?;
        let skill = self.skills.iter().find(|s| s.name == name).with_context(|| {
            let available: Vec<&str> = self.skills.iter().map(|s| s.name.as_str()).collect();
            format!("no skill named '{name}'. Available: {}", available.join(", "))
        })?;
        Ok(skill.body.clone())
    }
}
