mod config;
mod repl;

use anyhow::Result;
use coders_core::Agent;
use coders_provider::{build_provider, ProviderConfig};
use config::Config;
use console::style;
use repl::ReplUi;
use std::io::Write;

const DEFAULT_SYSTEM_PROMPT: &str = "You are coders, a terminal-based coding assistant. \
You have access to tools for reading files, writing files, and running shell commands. \
Use them when needed to complete the user's request.";

#[tokio::main]
async fn main() -> Result<()> {
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
    let skills = coders_skills::load_skills(&coders_skills::default_skill_dirs());
    let skill_catalog = coders_skills::catalog(&skills);
    if !skills.is_empty() {
        tools.push(Box::new(coders_skills::SkillTool::new(skills)));
    }

    let base_system = config.system_prompt.clone().unwrap_or_else(|| DEFAULT_SYSTEM_PROMPT.to_string());
    let system = Some(format!("{base_system}{skill_catalog}"));

    let mut agent = Agent::new(provider, tools, config.model.clone(), system);

    println!("coders — {} / {}", config.provider, config.model);
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
