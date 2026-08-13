use crate::Tool;
use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use serde_json::{json, Value};

/// Markers delimiting one search/replace block. Deliberately the same
/// conflict-marker shape LLMs already emit for diffs, so the model doesn't
/// have to learn a bespoke syntax to use this tool.
const SEARCH_START: &str = "<<<<<<< SEARCH";
const DIVIDER: &str = "=======";
const REPLACE_END: &str = ">>>>>>> REPLACE";

/// One parsed search/replace pair. An empty `search` means "append
/// `replace` to the end of the file".
#[derive(Debug, PartialEq)]
struct Block {
    search: String,
    replace: String,
}

/// Surgical, in-place file editing: the model sends the exact lines it wants
/// gone plus what should stand in their place, instead of rewriting the whole
/// file through `write_file` (which burns tokens and risks clobbering parts of
/// the file it never read).
pub struct EditFileTool;

#[async_trait]
impl Tool for EditFileTool {
    fn name(&self) -> &str {
        "edit_file"
    }

    fn description(&self) -> &str {
        "Edit parts of an existing file in place using one or more search/replace blocks. \
         Prefer this over write_file when changing a file that already exists. Format:\n\
         <<<<<<< SEARCH\n(exact lines currently in the file)\n=======\n(lines to put in their place)\n>>>>>>> REPLACE\n\
         The SEARCH text must match the file exactly and must match exactly one place — include \
         surrounding context lines to disambiguate. An empty SEARCH section appends the REPLACE \
         text to the end of the file. Leave the REPLACE section empty to delete the matched lines. \
         Multiple blocks are applied in order, top to bottom."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Path to the existing file to edit" },
                "diff": {
                    "type": "string",
                    "description": "One or more blocks of the form: <<<<<<< SEARCH\\n<old lines>\\n=======\\n<new lines>\\n>>>>>>> REPLACE",
                },
            },
            "required": ["path", "diff"],
        })
    }

    /// Same blast radius as `write_file` — it rewrites a file on disk — so it
    /// goes through the same confirmation gate.
    fn requires_confirmation(&self) -> bool {
        true
    }

    async fn execute(&self, input: Value) -> Result<String> {
        let path = input.get("path").and_then(Value::as_str).context("missing 'path' argument")?;
        // Models drift on the argument name for the payload; accept the
        // near-misses rather than failing a turn over a synonym.
        let diff = ["diff", "edits", "undo_diff"]
            .iter()
            .find_map(|key| input.get(*key).and_then(Value::as_str))
            .context("missing 'diff' argument (the search/replace blocks to apply)")?;

        let blocks = parse_blocks(diff)?;

        let original = match tokio::fs::read_to_string(path).await {
            Ok(content) => content,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                bail!("{path} does not exist — use write_file to create a new file")
            }
            Err(err) => return Err(err).with_context(|| format!("reading {path}")),
        };

        let updated = apply_blocks(&original, &blocks).with_context(|| format!("editing {path}"))?;
        if updated == original {
            return Ok(format!("no change: the {} block(s) left {path} byte-identical", blocks.len()));
        }

        tokio::fs::write(path, &updated).await.with_context(|| format!("writing {path}"))?;
        Ok(format!("applied {} edit(s) to {path} ({} lines now)", blocks.len(), updated.lines().count()))
    }
}

/// Pulls every search/replace block out of `diff`. Text outside blocks
/// (prose, markdown fences) is ignored rather than rejected, since models
/// routinely wrap their output in explanation.
fn parse_blocks(diff: &str) -> Result<Vec<Block>> {
    let mut blocks = Vec::new();
    let mut lines = diff.lines();

    while let Some(line) = lines.next() {
        if !line.trim_end().starts_with(SEARCH_START) {
            continue;
        }

        let mut search = Vec::new();
        let mut divided = false;
        for line in lines.by_ref() {
            if line.trim_end() == DIVIDER {
                divided = true;
                break;
            }
            search.push(line);
        }
        if !divided {
            bail!("unterminated block: expected a '{DIVIDER}' divider after '{SEARCH_START}'");
        }

        let mut replace = Vec::new();
        let mut closed = false;
        for line in lines.by_ref() {
            if line.trim_end().starts_with(REPLACE_END) {
                closed = true;
                break;
            }
            replace.push(line);
        }
        if !closed {
            bail!("unterminated block: expected '{REPLACE_END}' to close the block");
        }

        blocks.push(Block { search: search.join("\n"), replace: replace.join("\n") });
    }

    if blocks.is_empty() {
        bail!("no search/replace blocks found — expected '{SEARCH_START}' / '{DIVIDER}' / '{REPLACE_END}'");
    }
    Ok(blocks)
}

/// Applies blocks in order, each against the result of the previous one, so
/// later blocks see the file as it will actually be on disk.
fn apply_blocks(content: &str, blocks: &[Block]) -> Result<String> {
    let total = blocks.len();
    blocks.iter().enumerate().try_fold(content.to_string(), |current, (i, block)| {
        apply_block(&current, &block.search, &block.replace).with_context(|| format!("block {} of {total}", i + 1))
    })
}

fn apply_block(content: &str, search: &str, replace: &str) -> Result<String> {
    if search.is_empty() {
        let mut out = content.to_string();
        if !out.is_empty() && !out.ends_with('\n') {
            out.push('\n');
        }
        out.push_str(replace);
        return Ok(out);
    }

    match content.matches(search).count() {
        1 => Ok(content.replacen(search, replace, 1)),
        0 => apply_relaxed(content, search, replace),
        n => bail!("SEARCH text matches {n} places — include more surrounding context so it matches exactly one"),
    }
}

/// Exact matching fails on differences the model can't see in the text it was
/// given — trailing spaces, CRLF line endings. Retry line-by-line ignoring
/// those, still refusing anything ambiguous. Lines keep their terminators
/// here so untouched parts of the file come back byte-identical.
fn apply_relaxed(content: &str, search: &str, replace: &str) -> Result<String> {
    let haystack: Vec<&str> = content.split_inclusive('\n').collect();
    let needle: Vec<&str> = search.lines().collect();
    if needle.is_empty() || needle.len() > haystack.len() {
        bail!("SEARCH text not found — read the file again and copy the exact lines to replace");
    }

    let matches_at = |start: usize| haystack[start..start + needle.len()].iter().zip(&needle).all(|(a, b)| a.trim_end() == b.trim_end());
    let hits: Vec<usize> = (0..=haystack.len() - needle.len()).filter(|start| matches_at(*start)).collect();

    let &[start] = hits.as_slice() else {
        match hits.len() {
            0 => bail!("SEARCH text not found — read the file again and copy the exact lines to replace"),
            n => bail!("SEARCH text matches {n} places — include more surrounding context so it matches exactly one"),
        }
    };

    let end = start + needle.len();
    let eol = if content.contains("\r\n") { "\r\n" } else { "\n" };
    // Only the very last line of a file may lack a terminator; if that's the
    // line being replaced, the new text must not grow one.
    let terminate_last = end < haystack.len() || haystack[end - 1].ends_with('\n');

    let mut out = String::with_capacity(content.len() + replace.len());
    out.push_str(&haystack[..start].concat());
    let mut new_lines = replace.lines().peekable();
    while let Some(line) = new_lines.next() {
        out.push_str(line);
        if new_lines.peek().is_some() || terminate_last {
            out.push_str(eol);
        }
    }
    out.push_str(&haystack[end..].concat());
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block(search: &str, replace: &str) -> Block {
        Block { search: search.to_string(), replace: replace.to_string() }
    }

    fn diff(search: &str, replace: &str) -> String {
        format!("{SEARCH_START}\n{search}\n{DIVIDER}\n{replace}\n{REPLACE_END}")
    }

    #[test]
    fn parses_a_single_block() {
        let parsed = parse_blocks(&diff("old line", "new line")).unwrap();
        assert_eq!(parsed, vec![block("old line", "new line")]);
    }

    #[test]
    fn parses_multiple_blocks_and_ignores_surrounding_prose() {
        let input = format!("here you go:\n{}\n\nand also:\n{}\n", diff("a", "b"), diff("c", "d"));
        let parsed = parse_blocks(&input).unwrap();
        assert_eq!(parsed, vec![block("a", "b"), block("c", "d")]);
    }

    #[test]
    fn parse_rejects_unterminated_blocks() {
        let err = parse_blocks(&format!("{SEARCH_START}\nold\n{DIVIDER}\nnew\n")).unwrap_err().to_string();
        assert!(err.contains("unterminated"), "{err}");

        let err = parse_blocks(&format!("{SEARCH_START}\nold\n")).unwrap_err().to_string();
        assert!(err.contains("unterminated"), "{err}");
    }

    #[test]
    fn parse_rejects_input_with_no_blocks() {
        let err = parse_blocks("just some prose").unwrap_err().to_string();
        assert!(err.contains("no search/replace blocks"), "{err}");
    }

    #[test]
    fn applies_an_exact_match_in_place() {
        let out = apply_blocks("fn a() {}\nfn b() {}\n", &[block("fn b() {}", "fn c() {}")]).unwrap();
        assert_eq!(out, "fn a() {}\nfn c() {}\n");
    }

    #[test]
    fn applies_blocks_in_sequence() {
        let out = apply_blocks("one\ntwo\n", &[block("one", "1"), block("two", "2")]).unwrap();
        assert_eq!(out, "1\n2\n");
    }

    #[test]
    fn empty_replace_deletes_the_matched_lines() {
        let out = apply_blocks("keep\ndrop me\nkeep\n", &[block("drop me\n", "")]).unwrap();
        assert_eq!(out, "keep\nkeep\n");
    }

    #[test]
    fn empty_search_appends_to_the_end() {
        let out = apply_blocks("existing\n", &[block("", "added\n")]).unwrap();
        assert_eq!(out, "existing\nadded\n");
    }

    #[test]
    fn empty_search_adds_a_separator_when_the_file_lacks_a_final_newline() {
        let out = apply_blocks("existing", &[block("", "added")]).unwrap();
        assert_eq!(out, "existing\nadded");
    }

    /// `{err:#}` renders the whole context chain — plain `to_string()` would
    /// only show the outermost "block N of M" wrapper.
    #[test]
    fn ambiguous_search_is_refused() {
        let err = apply_blocks("x = 1\nx = 1\n", &[block("x = 1", "x = 2")]).unwrap_err();
        assert!(format!("{err:#}").contains("matches 2 places"), "{err:#}");
    }

    #[test]
    fn missing_search_reports_which_block_failed() {
        let err = apply_blocks("a\n", &[block("a", "b"), block("nope", "x")]).unwrap_err();
        let chain = format!("{err:#}");
        assert!(chain.contains("block 2 of 2"), "{chain}");
        assert!(chain.contains("not found"), "{chain}");
    }

    #[test]
    fn tolerates_trailing_whitespace_and_keeps_crlf_endings() {
        let content = "fn main() {   \r\n    println!(\"hi\");\r\n}\r\n";
        let out = apply_blocks(content, &[block("fn main() {\n    println!(\"hi\");", "fn main() {\n    println!(\"bye\");")]).unwrap();
        assert_eq!(out, "fn main() {\r\n    println!(\"bye\");\r\n}\r\n");
    }

    #[test]
    fn relaxed_match_preserves_a_missing_final_newline() {
        let out = apply_blocks("a  \nb", &[block("a\nb", "z\ny")]).unwrap();
        assert_eq!(out, "z\ny");
    }

    #[test]
    fn relaxed_match_keeps_an_existing_final_newline() {
        let out = apply_blocks("a  \nb\n", &[block("a\nb", "z\ny")]).unwrap();
        assert_eq!(out, "z\ny\n");
    }

    #[test]
    fn relaxed_match_refuses_ambiguity_too() {
        let err = apply_blocks("x  \ny\nx\t\ny\n", &[block("x\ny", "one")]).unwrap_err();
        assert!(format!("{err:#}").contains("matches 2 places"), "{err:#}");
    }

    /// Removes its file on drop so repeated runs don't collide or leak.
    struct TempFile(std::path::PathBuf);
    impl Drop for TempFile {
        fn drop(&mut self) {
            std::fs::remove_file(&self.0).ok();
        }
    }
    impl TempFile {
        fn new(content: &str) -> Self {
            static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!("coders-tools-edit-test-{}-{n}.txt", std::process::id()));
            std::fs::write(&path, content).unwrap();
            Self(path)
        }
        fn as_str(&self) -> &str {
            self.0.to_str().unwrap()
        }
        fn read(&self) -> String {
            std::fs::read_to_string(&self.0).unwrap()
        }
    }

    #[tokio::test]
    async fn execute_rewrites_the_file_on_disk() {
        let file = TempFile::new("let x = 1;\nlet y = 2;\n");
        let out = EditFileTool
            .execute(json!({ "path": file.as_str(), "diff": diff("let y = 2;", "let y = 3;") }))
            .await
            .unwrap();
        assert!(out.contains("applied 1 edit(s)"), "{out}");
        assert_eq!(file.read(), "let x = 1;\nlet y = 3;\n");
    }

    #[tokio::test]
    async fn execute_leaves_the_file_untouched_when_a_block_fails() {
        let file = TempFile::new("original\n");
        let err = EditFileTool
            .execute(json!({ "path": file.as_str(), "diff": diff("not here", "x") }))
            .await
            .unwrap_err();
        assert!(format!("{err:#}").contains("not found"), "{err:#}");
        assert_eq!(file.read(), "original\n");
    }

    #[tokio::test]
    async fn execute_reports_a_missing_file_clearly() {
        let err = EditFileTool
            .execute(json!({ "path": "/definitely/not/a/real/path.rs", "diff": diff("a", "b") }))
            .await
            .unwrap_err()
            .to_string();
        assert!(err.contains("does not exist"), "{err}");
    }

    #[tokio::test]
    async fn execute_accepts_the_edits_argument_alias() {
        let file = TempFile::new("alpha\n");
        EditFileTool.execute(json!({ "path": file.as_str(), "edits": diff("alpha", "beta") })).await.unwrap();
        assert_eq!(file.read(), "beta\n");
    }
}
