use coders_core::AgentEvent;
use console::style;
use indicatif::{ProgressBar, ProgressStyle};
use serde_json::Value;
use std::time::Duration;

const MAX_RESULT_PREVIEW: usize = 300;

/// Cute one-word status messages, telegraphed in Morse while the model or a
/// tool is running — a small delight in place of a boring |/-\ spinner.
const CUTE_WORDS: &[&str] = &[
    "thinking", "cooking", "vibing", "pondering", "scheming", "percolating", "noodling", "brewing", "conjuring", "tinkering", "musing",
    "plotting",
];

fn morse_char(c: char) -> &'static str {
    match c {
        'a' => ".-",
        'b' => "-...",
        'c' => "-.-.",
        'd' => "-..",
        'e' => ".",
        'f' => "..-.",
        'g' => "--.",
        'h' => "....",
        'i' => "..",
        'j' => ".---",
        'k' => "-.-",
        'l' => ".-..",
        'm' => "--",
        'n' => "-.",
        'o' => "---",
        'p' => ".--.",
        'q' => "--.-",
        'r' => ".-.",
        's' => "...",
        't' => "-",
        'u' => "..-",
        'v' => "...-",
        'w' => ".--",
        'x' => "-..-",
        'y' => "-.--",
        'z' => "--..",
        _ => "",
    }
}

fn to_morse(word: &str) -> String {
    word.chars().map(morse_char).collect::<Vec<_>>().join(" ")
}

fn pick_word(seed: usize) -> &'static str {
    CUTE_WORDS[seed % CUTE_WORDS.len()]
}

/// Reveals each word's Morse code dot-by-dot, holds it fully spelled out
/// briefly, then moves to the next cute word — forever, until the caller
/// aborts the task (when the turn it was covering for finishes). Pure
/// Morse, on purpose — no English label alongside it to decode it for you.
async fn animate_morse(pb: ProgressBar) {
    let mut word_index = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.subsec_nanos() as usize).unwrap_or(0);
    loop {
        let word = pick_word(word_index);
        word_index = word_index.wrapping_add(1);
        let morse = to_morse(word);
        let morse_chars: Vec<char> = morse.chars().collect();

        for i in 1..=morse_chars.len() {
            let revealed: String = morse_chars[..i].iter().collect();
            pb.set_message(revealed);
            pb.tick();
            tokio::time::sleep(Duration::from_millis(55)).await;
        }
        pb.set_message(morse);
        pb.tick();
        tokio::time::sleep(Duration::from_millis(650)).await;
    }
}

fn spinner() -> (ProgressBar, tokio::task::JoinHandle<()>) {
    let pb = ProgressBar::new_spinner();
    pb.set_style(ProgressStyle::with_template("{msg}").unwrap());
    let handle = tokio::spawn(animate_morse(pb.clone()));
    (pb, handle)
}

fn compact_args(input: &Value) -> String {
    match input {
        Value::Object(map) if map.is_empty() => String::new(),
        other => serde_json::to_string(other).unwrap_or_default(),
    }
}

fn truncate(text: &str, max_chars: usize) -> String {
    let text = text.trim();
    let char_count = text.chars().count();
    if char_count <= max_chars {
        return text.to_string();
    }
    let head: String = text.chars().take(max_chars).collect();
    format!("{head}... ({} more chars)", char_count - max_chars)
}

/// Turns `AgentEvent`s into live terminal output: a Morse-code spinner while
/// the model or a tool is running, a line per tool call, and a truncated
/// preview of each result — so the REPL shows what's happening instead of
/// going silent until the final answer. Color/spinner rendering auto-disables
/// when stdout isn't a terminal (piped output, `NO_COLOR`, etc), handled by
/// the `console`/`indicatif` crates themselves.
pub struct ReplUi {
    spinner: Option<(ProgressBar, tokio::task::JoinHandle<()>)>,
}

impl ReplUi {
    pub fn new() -> Self {
        Self { spinner: None }
    }

    pub fn on_event(&mut self, event: AgentEvent) {
        match event {
            AgentEvent::Thinking => {
                self.clear_spinner();
                self.spinner = Some(spinner());
            }
            AgentEvent::ToolCall { name, input } => {
                self.clear_spinner();
                // The `skill` tool is how skill invocations flow through the
                // same Tool trait as everything else — mark it distinctly.
                let marker = if name == "skill" { "de" } else { "-.-" };
                let args = compact_args(&input);
                println!("{} {}", style(marker).cyan().bold(), style(format!("{name}({args})")).cyan());
                self.spinner = Some(spinner());
            }
            AgentEvent::ToolResult { output, is_error, .. } => {
                self.clear_spinner();
                if is_error {
                    println!("  {} {}", style("error:").red().bold(), style(truncate(&output, MAX_RESULT_PREVIEW)).red());
                } else {
                    println!("  {}", style(truncate(&output, MAX_RESULT_PREVIEW)).dim());
                }
            }
        }
    }

    /// Clears any dangling spinner once a turn is fully done (the final
    /// text reply doesn't fire its own event, so this is the backstop).
    pub fn finish(&mut self) {
        self.clear_spinner();
    }

    fn clear_spinner(&mut self) {
        if let Some((pb, handle)) = self.spinner.take() {
            handle.abort();
            pb.finish_and_clear();
        }
    }
}
