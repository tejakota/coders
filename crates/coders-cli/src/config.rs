use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::PathBuf;

const DEFAULT_CONFIG_TEMPLATE: &str = r#"# coders config — edit this file, then re-run `coders`.

# "anthropic", "openai", or "ollama" (any OpenAI-compatible endpoint)
provider = "anthropic"
model = "claude-sonnet-5"

# Either paste your key directly:
# api_key = "sk-..."
# ...or (recommended, keeps the key out of this file) name an environment
# variable that holds it:
api_key_env = "ANTHROPIC_API_KEY"

# override the default endpoint, e.g. for Ollama:
# base_url = "http://localhost:11434/v1/chat/completions"

# path to an extra CA cert (PEM/CRT) to trust, e.g. for a corporate VPN
# or proxy that TLS-inspects with its own root certificate:
# extra_ca_cert = "C:\\path\\to\\corp-ca.pem"

# system_prompt = "You are my coding assistant."
"#;

fn default_provider() -> String {
    "anthropic".to_string()
}

fn default_model() -> String {
    "claude-sonnet-5".to_string()
}

#[derive(Debug, Deserialize)]
pub struct Config {
    #[serde(default = "default_provider")]
    pub provider: String,
    #[serde(default = "default_model")]
    pub model: String,
    /// The API key itself, pasted directly. Takes priority over
    /// `api_key_env` if both are set.
    pub api_key: Option<String>,
    /// Name of the env var holding the API key (e.g. "ANTHROPIC_API_KEY").
    /// Ignored if `api_key` is set. Omit both for local endpoints that
    /// don't need auth.
    pub api_key_env: Option<String>,
    /// Override the default endpoint, e.g. for Ollama: "http://localhost:11434/v1/chat/completions".
    pub base_url: Option<String>,
    /// Path to an extra CA cert (PEM/CRT) to trust, e.g. for a corporate
    /// VPN/proxy that TLS-inspects with its own root certificate.
    pub extra_ca_cert: Option<String>,
    pub system_prompt: Option<String>,
}

/// `~/.coders` — created on install/first run, holds config.toml and skills/.
pub fn coders_home() -> Result<PathBuf> {
    dirs::home_dir().map(|h| h.join(".coders")).context("could not determine home directory")
}

fn config_path() -> Result<PathBuf> {
    Ok(coders_home()?.join("config.toml"))
}

impl Config {
    /// Creates `~/.coders/{config.toml,skills/}` if they don't already exist.
    /// Safe to call on every startup — never overwrites an existing config.
    pub fn ensure_scaffold() -> Result<PathBuf> {
        let home = coders_home()?;
        let skills_dir = home.join("skills");
        std::fs::create_dir_all(&skills_dir).context("creating ~/.coders/skills")?;
        coders_skills::install_bundled_skills(&skills_dir).context("installing bundled skills")?;

        let path = config_path()?;
        if !path.exists() {
            std::fs::write(&path, DEFAULT_CONFIG_TEMPLATE).context("writing default config.toml")?;
        }
        Ok(path)
    }

    pub fn load() -> Result<Self> {
        let path = Self::ensure_scaffold()?;
        let text = std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
        let config: Config = toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))?;
        Ok(config)
    }

    /// Resolves the API key: `api_key` directly if set, else the value of
    /// the env var named by `api_key_env`. Errors loudly if `api_key_env`
    /// names a variable that isn't actually set, rather than silently
    /// sending an unauthenticated request and surfacing a confusing remote
    /// "bearer token" error instead. Leaving both unset (e.g. a local
    /// Ollama with no auth) is fine and resolves to no key.
    pub fn require_api_key(&self) -> Result<String> {
        if let Some(key) = &self.api_key {
            if !key.trim().is_empty() {
                return Ok(key.clone());
            }
        }
        let Some(env_name) = &self.api_key_env else { return Ok(String::new()) };
        match std::env::var(env_name) {
            Ok(value) if !value.is_empty() => Ok(value),
            _ => anyhow::bail!(
                "api_key_env is set to '{env_name}' in ~/.coders/config.toml, but that \
                 environment variable is not set (or is empty) in this terminal session.\n\n\
                 Either paste the key directly as `api_key = \"...\"` in config.toml, or set \
                 the environment variable and run coders again in the SAME terminal:\n\
                 \x20 bash/zsh:   export {env_name}=sk-...\n\
                 \x20 PowerShell: $env:{env_name} = \"sk-...\"\n\
                 \x20 Windows cmd, persistent: setx {env_name} \"sk-...\"  (then open a NEW terminal — setx does not affect the current one)"
            ),
        }
    }
}
