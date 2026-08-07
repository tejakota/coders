# coders

A terminal coding assistant, like Claude Code, that works with any LLM that
exposes an API — Anthropic, OpenAI, any OpenAI-compatible endpoint (Ollama,
vLLM, OpenRouter, etc), or a fully custom company gateway.

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
provider = "anthropic"        # "anthropic" | "openai" | "ollama" | "custom"
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

### Custom / company gateways

`provider = "openai"` stays plain-vanilla OpenAI chat-completions — no
per-company hacks live there. For a gateway that's *close to* OpenAI's or
Anthropic's shape but not identical (a renamed field like
`max_completion_tokens` instead of `max_tokens`, a different auth header,
extra required fields), use `provider = "custom"` and configure the
differences instead of patching code:

```toml
provider = "custom"
model = "gpt-4o-company"
base_url = "https://gateway.example.com/v1/chat/completions"
api_key = "..."

[custom]
style = "openai"                        # "openai" | "anthropic" — base request/response shape
max_tokens_field = "max_completion_tokens"   # only used when style = "openai"
auth_header = "Authorization"           # e.g. "api-key" for some gateways
auth_scheme = "Bearer "                 # prefix before the key; "" for raw-key headers

[custom.extra_headers]
"api-version" = "2024-05-01"

[custom.extra_body]
temperature = 0.3
user = "my-app"
```

`extra_body` accepts arbitrary fields — anything in that table gets merged
into the JSON request body as-is, overriding same-named fields the base
`style` would otherwise set.

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

## REPL

Tool calls and their results print live as the agent works, instead of the
terminal going silent until the final answer:

```
> find the auth middleware
-.- grep({"pattern":"middleware","path":"src"})
  src/auth.rs:12: pub fn auth_middleware(req: Request) -> Response {

It's in src/auth.rs:12.
```

Markers are Morse/telegraph shorthand: `-.-` (prosign "K", "go ahead") marks
a regular tool call; `de` (ham radio for "this is / from") marks a skill
invocation specifically, since loading a skill goes through the same `skill`
tool under the hood.

### Confirmation for risky tools

`bash` and `write_file` can execute arbitrary shell commands or overwrite
arbitrary files — a model response acting on bad input, a prompt injection
from a file it read, or its own mistake could do real damage with no gate
at all. Both require an explicit confirmation before they run:

```
-.- bash({"command":"rm -rf build/"})
  use -.- to send, -. to hold the line
>
```

Key back `-.-` (the same go-ahead shown next to the call) to run it, or
`-.` (or anything else, including a blank line — declining is the default)
to hold the line. Any tool can opt into this via `Tool::requires_confirmation()`;
`read_file`/`grep`/`find`/`skill` don't, since they're read-only.

While waiting on the model or a tool, a spinner plays a cute word
telegraphed in Morse code, revealed dot-by-dot — pure Morse, no English
label — cycling until the turn resolves. Colors and the spinner both
auto-disable when stdout isn't a terminal (piped output, `NO_COLOR`, CI, etc).

## Workspace layout

- `crates/coders-provider` — `ProviderClient` trait + Anthropic,
  OpenAI-compatible, and `custom` backends; TLS via rustls + OS native cert
  store, with optional `extra_ca_cert` support
- `crates/coders-tools` — `Tool` trait + built-ins (`read_file`,
  `write_file`, `bash` — `cmd` on Windows, `sh` elsewhere — `grep`, `find`)
- `crates/coders-skills` — SKILL.md loader, the `skill` tool, and the two
  bundled skills under `assets/skills/`
- `crates/coders-core` — `Agent`: the send → tool-call → tool-result loop,
  emitting `AgentEvent`s a caller can render live
- `crates/coders-cli` — the `coders` binary, config, REPL (spinner + colored
  tool-call output)

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
