//! Turns tool activity into markdown-flavored terminal output — a headline
//! per call, bulleted arguments, and fenced blocks for anything multi-line —
//! instead of one long `name({"json":"blob"})` line that's unreadable the
//! moment an argument carries a code snippet or a diff.
//!
//! Every function here is pure and returns the lines to print, so the layout
//! is unit-testable without a terminal; `repl.rs` only does the printing.
//! Styling is applied via `console`, which drops color automatically when
//! stdout isn't a terminal.

use console::style;
use serde_json::Value;

/// The go-ahead prosign ("K") — shown next to a tool call, and the same
/// signal the operator keys back to grant a confirmation.
pub const GO_AHEAD: &str = "-.-";
/// The negative ("N") — the explicit decline, symmetric with `GO_AHEAD`.
pub const HOLD: &str = "-.";
/// Ham shorthand for "this is / from" — marks a skill invocation, since
/// loading a skill rides the same `Tool` trait as everything else.
const SKILL_MARKER: &str = "de";

/// Argument bullets and results hang under the tool name; block content sits
/// one step further in, the way a fenced block nests under a list item.
const FIELD_INDENT: &str = "      ";
const BLOCK_INDENT: &str = "        ";

/// Single-line values are for scanning, not reading — anything longer gets
/// elided rather than wrapping across the whole terminal.
const MAX_INLINE_CHARS: usize = 96;
const MAX_BLOCK_LINES: usize = 14;
const MAX_BLOCK_LINE_CHARS: usize = 120;

/// Human-readable headline for a tool call, so the line reads as a task
/// ("editing file") rather than as an API name.
fn tool_label(name: &str) -> &'static str {
    match name {
        "read_file" => "📖 reading file",
        "write_file" => "📝 writing file",
        "edit_file" => "✂️ editing file",
        "bash" => "⚡ running command",
        "grep" => "🔍 searching contents",
        "find" => "📁 finding files",
        "skill" => "🎯 loading skill",
        _ => "🔧 running tool",
    }
}

/// Truncates on a character boundary — slicing by byte index panics the
/// moment a path or code snippet contains non-ASCII text.
fn truncate_chars(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    text.chars().take(max.saturating_sub(1)).chain(std::iter::once('…')).collect()
}

/// Renders text as an indented block with a gutter bar, capped in both
/// directions so one huge argument or result can't flood the screen.
fn block_lines(text: &str, is_error: bool) -> Vec<String> {
    let lines: Vec<&str> = text.lines().collect();
    let shown = lines.len().min(MAX_BLOCK_LINES);
    let mut out: Vec<String> = lines[..shown]
        .iter()
        .map(|line| {
            let body = truncate_chars(line, MAX_BLOCK_LINE_CHARS);
            let body = if is_error { style(body).red() } else { style(body).dim() };
            format!("{BLOCK_INDENT}{} {body}", style("│").dim())
        })
        .collect();
    if lines.len() > shown {
        out.push(format!("{BLOCK_INDENT}{}", style(format!("… {} more lines", lines.len() - shown)).dim()));
    }
    out
}

/// Whether a value deserves its own block instead of sitting inline next to
/// its key — multi-line strings (diffs, file content) and non-empty
/// collections.
fn needs_block(value: &Value) -> bool {
    match value {
        Value::String(s) => s.contains('\n'),
        Value::Array(a) => !a.is_empty(),
        Value::Object(o) => !o.is_empty(),
        _ => false,
    }
}

/// Ranks the argument naming *what* the call acts on above its modifiers, so
/// the first bullet is the one worth reading. Everything unranked keeps the
/// alphabetical order the JSON map already provides.
fn field_rank(key: &str) -> usize {
    ["command", "pattern", "name", "path"].iter().position(|known| *known == key).unwrap_or(usize::MAX)
}

fn inline_value(value: &Value) -> String {
    match value {
        Value::String(s) => truncate_chars(s, MAX_INLINE_CHARS),
        other => truncate_chars(&other.to_string(), MAX_INLINE_CHARS),
    }
}

fn block_text(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        other => serde_json::to_string_pretty(other).unwrap_or_else(|_| other.to_string()),
    }
}

/// Formats one tool call: a blank spacer, the headline, then its arguments.
/// Short arguments come first so the identifying ones (`path`, `pattern`)
/// stay together at the top, with bulky blocks below them.
pub fn tool_call_lines(name: &str, input: &Value) -> Vec<String> {
    let marker = if name == "skill" { SKILL_MARKER } else { GO_AHEAD };
    let mut out = vec![
        String::new(),
        format!("  {} {}  {}", style(marker).cyan().bold(), style(name).cyan().bold(), style(tool_label(name)).cyan()),
    ];

    let Some(fields) = input.as_object() else {
        if !input.is_null() {
            out.extend(block_lines(&block_text(input), false));
        }
        return out;
    };

    let (blocks, mut inline): (Vec<_>, Vec<_>) = fields.iter().partition(|(_, value)| needs_block(value));
    // Stable, so same-rank keys stay alphabetical.
    inline.sort_by_key(|(key, _)| field_rank(key.as_str()));
    for (key, value) in inline {
        out.push(format!("{FIELD_INDENT}{} {} {}", style("•").dim(), style(format!("{key}:")).dim().italic(), inline_value(value)));
    }
    for (key, value) in blocks {
        out.push(format!("{FIELD_INDENT}{} {}", style("•").dim(), style(format!("{key}:")).dim().italic()));
        out.extend(block_lines(&block_text(value), false));
    }
    out
}

/// Formats a tool result: a one-line summary, plus the output itself as a
/// block when it spans several lines.
pub fn tool_result_lines(output: &str, is_error: bool) -> Vec<String> {
    let text = output.trim();
    let marker = if is_error { style("✗").red().bold() } else { style("↳").green().bold() };

    if text.is_empty() {
        return vec![format!("{FIELD_INDENT}{marker} {}", style("(no output)").dim())];
    }

    let count = text.lines().count();
    if count == 1 {
        let body = truncate_chars(text, MAX_INLINE_CHARS);
        let body = if is_error { style(body).red() } else { style(body).dim() };
        return vec![format!("{FIELD_INDENT}{marker} {body}")];
    }

    let summary = if is_error { style(format!("failed ({count} lines)")).red() } else { style(format!("{count} lines")).dim() };
    let mut out = vec![format!("{FIELD_INDENT}{marker} {summary}")];
    out.extend(block_lines(text, is_error));
    out
}

/// The confirmation prompt for gated tools, kept in the same visual language
/// as the call it follows. The caller prints these, then reads a reply.
pub fn confirmation_lines() -> Vec<String> {
    vec![
        format!("{FIELD_INDENT}{} {}", style("⚠").yellow().bold(), style("confirmation required").yellow().bold()),
        format!(
            "{FIELD_INDENT}{} to send, {} to hold the line",
            style(GO_AHEAD).green().bold(),
            style(HOLD).yellow().bold()
        ),
    ]
}

/// A representative transcript rendered through the real formatting path, so
/// the terminal output can be eyeballed (`coders render-demo`) without an API
/// key, a provider, or a live model turn.
pub fn demo_lines() -> Vec<String> {
    let diff = "<<<<<<< SEARCH\n            for (i, key) in keys.iter().enumerate() {\n=======\n            for key in keys.iter() {\n>>>>>>> REPLACE";
    let calls = [
        (
            serde_json::json!({ "pattern": "keys.iter", "path": "crates", "file_glob": "*.rs" }),
            "grep",
            "crates/coders-cli/src/repl.rs:104:             for (i, key) in keys.iter().enumerate() {",
            false,
        ),
        (
            serde_json::json!({ "path": "./crates/coders-cli/src/repl.rs", "diff": diff }),
            "edit_file",
            "applied 1 edit(s) to ./crates/coders-cli/src/repl.rs (319 lines now)",
            false,
        ),
        (
            serde_json::json!({ "command": "cargo test -p coders-cli" }),
            "bash",
            "running 11 tests\ntest render::tests::call_headline_names_the_task_and_the_tool ... ok\ntest render::tests::skill_calls_use_their_own_marker ... ok\n\ntest result: ok. 11 passed; 0 failed",
            false,
        ),
        (
            serde_json::json!({ "path": "src/missing.rs", "diff": diff }),
            "edit_file",
            "src/missing.rs does not exist — use write_file to create a new file",
            true,
        ),
    ];

    let mut out = Vec::new();
    for (input, name, result, is_error) in calls {
        out.extend(tool_call_lines(name, &input));
        out.extend(tool_result_lines(result, is_error));
    }
    out.extend(tool_call_lines("bash", &serde_json::json!({ "command": "rm -rf build/" })));
    out.extend(confirmation_lines());
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Colors are off in tests anyway (stdout isn't a terminal), but strip
    /// explicitly so assertions hold no matter where the suite runs.
    fn plain(lines: Vec<String>) -> Vec<String> {
        lines.iter().map(|l| console::strip_ansi_codes(l).to_string()).collect()
    }

    #[test]
    fn call_headline_names_the_task_and_the_tool() {
        let out = plain(tool_call_lines("read_file", &json!({ "path": "src/main.rs" })));
        assert_eq!(out[0], "");
        assert!(out[1].contains("-.-") && out[1].contains("read_file") && out[1].contains("reading file"), "{:?}", out[1]);
        assert!(out[2].contains("• path: src/main.rs"), "{:?}", out[2]);
    }

    #[test]
    fn the_identifying_argument_is_listed_before_its_modifiers() {
        let out = plain(tool_call_lines("grep", &json!({ "file_glob": "*.rs", "path": "src", "pattern": "todo" })));
        assert!(out[2].contains("pattern: todo"), "{out:?}");
        assert!(out[3].contains("path: src"), "{out:?}");
        assert!(out[4].contains("file_glob: *.rs"), "{out:?}");
    }

    #[test]
    fn skill_calls_use_their_own_marker() {
        let out = plain(tool_call_lines("skill", &json!({ "name": "code-analysis" })));
        assert!(out[1].starts_with("  de "), "{:?}", out[1]);
    }

    #[test]
    fn multiline_arguments_render_as_a_block_after_short_ones() {
        let out = plain(tool_call_lines("edit_file", &json!({ "path": "a.rs", "diff": "<<<<<<< SEARCH\nold\n=======\nnew\n>>>>>>> REPLACE" })));
        let path_at = out.iter().position(|l| l.contains("path:")).unwrap();
        let diff_at = out.iter().position(|l| l.contains("diff:")).unwrap();
        assert!(path_at < diff_at, "short args should come first: {out:?}");
        assert!(out[diff_at + 1].contains("│ <<<<<<< SEARCH"), "{:?}", out[diff_at + 1]);
        assert!(out.iter().any(|l| l.contains("│ >>>>>>> REPLACE")), "{out:?}");
    }

    #[test]
    fn long_values_are_elided_without_splitting_a_character() {
        let value = "é".repeat(400);
        let out = plain(tool_call_lines("bash", &json!({ "command": value })));
        let line = out.iter().find(|l| l.contains("command:")).unwrap();
        assert!(line.ends_with('…'), "{line:?}");
        assert!(line.chars().count() < 140, "{line:?}");
    }

    #[test]
    fn blocks_are_capped_with_a_remaining_count() {
        let long = (1..=40).map(|i| format!("line {i}")).collect::<Vec<_>>().join("\n");
        let out = plain(tool_call_lines("write_file", &json!({ "path": "a.txt", "content": long })));
        assert!(out.iter().any(|l| l.contains("line 14")), "{out:?}");
        assert!(!out.iter().any(|l| l.contains("line 15")), "{out:?}");
        assert!(out.last().unwrap().contains("… 26 more lines"), "{:?}", out.last());
    }

    #[test]
    fn empty_arguments_render_just_the_headline() {
        let out = plain(tool_call_lines("find", &json!({})));
        assert_eq!(out.len(), 2, "{out:?}");
    }

    #[test]
    fn single_line_results_stay_on_one_line() {
        let out = plain(tool_result_lines("wrote 12 bytes to a.txt", false));
        assert_eq!(out, vec!["      ↳ wrote 12 bytes to a.txt"]);
    }

    #[test]
    fn multiline_results_are_summarized_then_shown() {
        let out = plain(tool_result_lines("a.rs:1: fn a\na.rs:2: fn b", false));
        assert!(out[0].contains("↳ 2 lines"), "{:?}", out[0]);
        assert!(out[1].contains("│ a.rs:1: fn a"), "{:?}", out[1]);
    }

    #[test]
    fn errors_are_marked_distinctly() {
        let out = plain(tool_result_lines("SEARCH text not found", true));
        assert!(out[0].contains("✗"), "{:?}", out[0]);
    }

    #[test]
    fn empty_output_says_so_instead_of_printing_nothing() {
        let out = plain(tool_result_lines("   \n  ", false));
        assert!(out[0].contains("(no output)"), "{:?}", out[0]);
    }

    /// Guards `coders render-demo` against panicking on its own sample data.
    #[test]
    fn demo_transcript_renders() {
        let out = plain(demo_lines());
        assert!(out.iter().any(|l| l.contains("edit_file")), "{out:?}");
        assert!(out.iter().any(|l| l.contains("✗")), "{out:?}");
        assert!(out.last().unwrap().contains("to hold the line"), "{:?}", out.last());
    }
}
