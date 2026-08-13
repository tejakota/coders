---
name: code-generation
description: Use when asked to write new code or extend existing code - Python, JavaScript, TypeScript, HTML, Dart, Rust, or another language. Covers matching project conventions and per-language idiom.
---

Before writing code into an existing project, look at how it already does
things: use `find`/`grep`/`read_file` to check naming conventions, existing
error-handling patterns, import style, and whichever of the project's own
libraries already solve part of the problem. Match what's there rather than
introducing a second way to do the same thing. Create new files with
`write_file` and change existing ones with `edit_file` — its SEARCH text has
to match the file exactly, so read the file first and copy the lines you're
replacing. Re-read what you wrote before calling it done.

## General approach

1. Check for an existing pattern to extend before inventing a new one —
   `grep` for similar function/component names first.
2. Match the file's existing style (quote style, brace placement, import
   grouping) even where it differs from your default preference.
3. Only add the abstraction the task needs. No speculative config options,
   no unused parameters, no framework for a single call site.
4. Handle the errors that can actually occur at this boundary; don't wrap
   every call in defensive checks for impossible states.

## Language-specific idiom

**Python** — type hints on public function signatures; f-strings over
`.format()`/`%`; `with` for anything acquiring a resource; list/dict
comprehensions over manual accumulation loops when they stay readable;
`dataclass` or `pydantic` model over a bare dict for structured data that's
passed around.

**JavaScript / TypeScript** — prefer `const`; explicit return types on
exported functions in TS; `async`/`await` over raw `.then()` chains;
narrow types at the boundary (parse untrusted input, don't just cast);
avoid `any` — use `unknown` and narrow, or define the real type.

**HTML** — semantic elements (`<button>`, `<nav>`, `<main>`) over generic
`<div>`/`<span>` with click handlers bolted on; every interactive control
keyboard-operable; forms have associated `<label>`s.

**Dart / Flutter** — `const` constructors wherever the widget subtree is
static; extract widgets instead of deeply nested builder closures; prefer
`copyWith`-based immutable state over mutating fields in place; dispose of
every controller/subscription you create.

**Rust** — return `Result<T, E>` from anything fallible, don't panic in
library code; take `&str`/`&[T]` in function signatures over owned
`String`/`Vec<T>` unless ownership is actually needed; prefer iterator
chains over manual index loops when equally clear; `impl Trait` in argument
position for simple generic bounds.

## Before finishing

Re-read the file you wrote (or the diff, if editing). Check it compiles/
parses in your head: matching braces, imports for everything used, no
leftover placeholder text. If the project has a test or lint command
available via `bash`, run it.
