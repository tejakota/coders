# coders

A terminal coding assistant, like Claude Code, that works with any LLM that
exposes an API — Anthropic, OpenAI, or any OpenAI-compatible endpoint
(Ollama, vLLM, OpenRouter, etc).

## Install

**Linux:**
```sh
./install.sh
```

**Windows (PowerShell):**
```powershell
.\install.ps1
```

**macOS:** no prebuilt binary yet (cross-compiling to macOS needs a Mac or
Apple SDK access, which this dev environment doesn't have). `install.sh`
falls back to `cargo build --release` automatically — install Rust from
https://rustup.rs first, then run `./install.sh` as normal.

Windows and Linux binaries are cross-compiled from this repo and committed
under `dist/`; the Windows one hasn't been execution-tested on an actual
Windows machine (only build-verified as a valid PE32+ binary), so report
back if `coders.exe` misbehaves.

Either script copies the `coders` binary onto your PATH (`~/.local/bin` by
default, override with `CODERS_INSTALL_DIR`) and scaffolds `~/.coders/`.

## Configure

Edit `~/.coders/config.toml`:

```toml
provider = "anthropic"        # "anthropic" | "openai" | "ollama"
model = "claude-sonnet-5"
api_key_env = "ANTHROPIC_API_KEY"   # name of the env var holding your key

# base_url = "http://localhost:11434/v1/chat/completions"  # e.g. Ollama
# system_prompt = "You are my coding assistant."
```

Set the API key in your shell:
```sh
export ANTHROPIC_API_KEY=sk-...
```

Then run:
```sh
coders
```

## Skills

Drop a `SKILL.md` file in `~/.coders/skills/<name>/` (user-level) or
`.coders/skills/<name>/` in a project directory (project-level, overrides
user skills of the same name):

```markdown
---
name: commit-message
description: Use when writing a git commit message, to follow Conventional Commits style.
---

Write commit messages as `<type>(<scope>): <summary>` ...
```

Skill names and descriptions are listed in the system prompt; the model
calls the built-in `skill` tool with a skill's name to load its full body
into context before acting on it.

## Workspace layout

- `crates/coders-provider` — `ProviderClient` trait + Anthropic and
  OpenAI-compatible backends
- `crates/coders-tools` — `Tool` trait + built-ins (`read_file`,
  `write_file`, `bash`)
- `crates/coders-skills` — SKILL.md loader + the `skill` tool
- `crates/coders-core` — `Agent`: the send → tool-call → tool-result loop
- `crates/coders-cli` — the `coders` binary, config, REPL

## Building prebuilt binaries yourself

```sh
# Linux (native)
cargo build --release -p coders-cli
cp target/release/coders dist/linux-x86_64/coders

# Windows (cross-compile from Linux, needs mingw-w64)
rustup target add x86_64-pc-windows-gnu
cargo build --release -p coders-cli --target x86_64-pc-windows-gnu
cp target/x86_64-pc-windows-gnu/release/coders.exe dist/windows-x86_64/coders.exe
```

Not yet done: external subprocess plugins (a plugin manifest + shell-out,
matching the tool/skill traits already in place), streaming responses, and
a real macOS prebuilt binary.
