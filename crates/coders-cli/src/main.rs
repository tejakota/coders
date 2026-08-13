mod config;
mod render;
mod repl;
mod story_loader;

use anyhow::Result;
use coders_core::Agent;
use coders_provider::{build_provider, ProviderConfig};
use config::Config;
use console::style;
use repl::{ReplGate, ReplUi};
use std::io::Write;

const DEFAULT_SYSTEM_PROMPT: &str = "You are coders, a terminal-based coding assistant. \
You have access to tools for reading, searching, writing, and editing files, and for running shell commands. \
Use them when needed to complete your user's request. \
To change a file that already exists, read it first and then use edit_file with search/replace blocks — \
reserve write_file for creating new files or full rewrites.";

#[tokio::main]
async fn main() -> Result<()> {
    // Prints a canned transcript through the real renderer — a way to see
    // exactly what tool activity will look like without spending a model turn.
    if std::env::args().nth(1).as_deref() == Some("render-demo") {
        for line in render::demo_lines() {
            println!("{line}");
        }
        return Ok(());
    }

    if std::env::args().nth(1).as_deref() == Some("init") {
        let path = Config::ensure_scaffold()?;
        println!("coders home ready at {}", path.parent().unwrap().display());
        println!("Edit {} to set your provider, model, and api key, then run `coders`.", path.display());
        return Ok(());
    }

    let config = Config::load()?;
    let custom = config.custom.as_ref().map(|c| c.to_provider_options()).transpose()?;
    let provider = build_provider(
        &config.provider,
        ProviderConfig {
            api_key: config.require_api_key()?,
            base_url: config.base_url.clone(),
            extra_ca_cert: config.extra_ca_cert.clone(),
            custom,
        },
    )?;

    let mut tools = coders_tools::builtin_tools();
    let skill_dirs = coders_skills::default_skill_dirs();
    let skills = coders_skills::load_skills(&skill_dirs);
    let skill_catalog = coders_skills::catalog(&skills);
    if !skills.is_empty() {
        tools.push(Box::new(coders_skills::SkillTool::new(skills.clone())));
    }

    // Load STORY.md instructions
    let story_instructions = match story_loader::load_story_instructions() {
        Ok(Some(instructions)) => instructions,
        Ok(None) => String::new(),
        Err(e) => {
            eprintln!("Warning: Failed to load STORY.md: {e}");
            String::new()
        }
    };

    let base_system = config.system_prompt.clone().unwrap_or_else(|| DEFAULT_SYSTEM_PROMPT.to_string());
    let mut system = format!("{base_system}{skill_catalog}");

    if !story_instructions.is_empty() {
        system.push_str("\n\nSTORY.md INSTRUCTIONS:\n");
        system.push_str(&story_instructions);
        system.push_str("\n");
    }

    let mut agent = Agent::new(provider, tools, Box::new(ReplGate), config.model.clone(), Some(system));

    println!("coders — {} / {}", config.provider, config.model);
    
    // Print STORY.md status if found
    if !story_instructions.is_empty() {
        println!("STORY.md loaded - following project-specific instructions");
    }
    
    // Self-diagnosing: shows exactly where it looked and what it found, so
    // a skill that silently failed to parse (or was dropped in the wrong
    // directory) is visible immediately instead of just "not working".
    let searched = skill_dirs.iter().map(|d| d.display().to_string()).collect::<Vec<_>>().join(", ");
    if skills.is_empty() {
        println!("No skills found (searched: {searched})");
    } else {
        let names = skills.iter().map(|s| s.name.as_str()).collect::<Vec<_>>().join(", ");
        println!("Loaded {} skill(s): {names} (searched: {searched})", skills.len());
    }
    println!("Type your request, or /exit to quit.\n");

    let stdin = std::io::stdin();
    let mut ui = ReplUi::new();
    loop {
        print!("{} ", style(">").green().bold());
        std::io::stdout().flush().ok();

        let mut line = String::new();
        if stdin.read_line(&mut line)? == 0 {
            break; // EOF
        }
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if line == "/exit" || line == "/quit" {
            break;
        }

        match agent.send(line, |event| ui.on_event(event)).await {
            Ok(reply) => {
                ui.finish();
                println!("\n{reply}\n");
            }
            Err(err) => {
                ui.finish();
                eprintln!("\n{} {err:#}\n", style("error:").red().bold());
            }
        }
    }

    Ok(())
}
