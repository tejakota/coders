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

# Either paste your key directly...
# api_key = "sk-..."
# ...or (recommended) name an env var that holds it:
api_key_env = "ANTHROPIC_API_KEY"

# base_url = "http://localhost:11434/v1/chat/completions"  # e.g. Ollama
# extra_ca_cert = "C:\\path\\to\\corp-ca.pem"              # see "Corporate VPN / proxy TLS" below
# system_prompt = "You are my coding assistant."
```

`api_key_env` is the *name* of an environment variable, not the key itself
— set that variable in your shell:
```sh
export ANTHROPIC_API_KEY=sk-...
```

Then run:
```sh
coders
```

If `api_key_env` is set in config.toml but the variable isn't actually set
in that terminal (a common trap: Windows `setx` only takes effect in a
*new* terminal, not the one you ran it in), `coders` now fails immediately
with a clear message instead of silently sending an unauthenticated
request and surfacing a confusing 401 from the server.

### Corporate VPN / proxy TLS ("invalid peer certificate")

`coders` uses rustls with the OS's native certificate store (Windows
Certificate Store, macOS Keychain, or the system CA bundle on Linux), so it
already trusts any root CA installed system-wide — including ones a
corporate VPN client or MITM-inspecting proxy installs automatically.

If you hit `invalid peer certificate` or `client error (connect)` anyway
(e.g. your IT team hands you a standalone `.pem`/`.crt` instead of
installing it into the OS store), point `extra_ca_cert` in
`~/.coders/config.toml` at that file — it gets trusted in addition to the
native store, no restart or reinstall needed beyond editing the config.

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

Two skills — `code-analysis` and `code-generation` (Python, JS, TS, HTML,
Dart, Rust) — are bundled into the binary itself and auto-installed into
`~/.coders/skills/` on first run. They're never overwritten once present,
so editing them locally sticks.

## Workspace layout

- `crates/coders-provider` — `ProviderClient` trait + Anthropic and
  OpenAI-compatible backends; TLS via rustls + OS native cert store, with
  optional `extra_ca_cert` support
- `crates/coders-tools` — `Tool` trait + built-ins (`read_file`,
  `write_file`, `bash`, `grep`, `find`)
- `crates/coders-skills` — SKILL.md loader, the `skill` tool, and the two
  bundled skills under `assets/skills/`
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
