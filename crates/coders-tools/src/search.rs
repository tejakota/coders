use crate::Tool;
use anyhow::{Context, Result};
use async_trait::async_trait;
use globset::{Glob, GlobSetBuilder};
use regex::Regex;
use serde_json::{json, Value};
use walkdir::WalkDir;

/// Directories skipped by default in grep/find so results stay signal, not
/// noise from VCS internals, build output, and dependency trees.
const SKIP_DIRS: &[&str] = &[
    ".git", "target", "node_modules", ".venv", "venv", "__pycache__", ".coders", "dist", "build", ".next", ".dart_tool",
];

fn is_skipped_dir(entry: &walkdir::DirEntry) -> bool {
    entry.file_type().is_dir() && entry.file_name().to_str().map(|n| SKIP_DIRS.contains(&n)).unwrap_or(false)
}

pub struct GrepTool;

#[async_trait]
impl Tool for GrepTool {
    fn name(&self) -> &str {
        "grep"
    }

    fn description(&self) -> &str {
        "Search file contents for a regex pattern, recursively under a directory. Returns matching lines as 'path:line: text'."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "pattern": { "type": "string", "description": "Regex pattern to search for" },
                "path": { "type": "string", "description": "Directory to search under (default: current directory)" },
                "file_glob": { "type": "string", "description": "Only search files matching this glob, e.g. '*.rs'" },
            },
            "required": ["pattern"],
        })
    }

    async fn execute(&self, input: Value) -> Result<String> {
        let pattern = input.get("pattern").and_then(Value::as_str).context("missing 'pattern' argument")?.to_string();
        let path = input.get("path").and_then(Value::as_str).unwrap_or(".").to_string();
        let file_glob = input.get("file_glob").and_then(Value::as_str).map(str::to_string);

        tokio::task::spawn_blocking(move || grep_blocking(&pattern, &path, file_glob.as_deref())).await?
    }
}

const MAX_MATCHES: usize = 200;

fn grep_blocking(pattern: &str, path: &str, file_glob: Option<&str>) -> Result<String> {
    let regex = Regex::new(pattern).with_context(|| format!("invalid regex: {pattern}"))?;
    let glob = file_glob.map(|g| Glob::new(g).with_context(|| format!("invalid glob: {g}"))).transpose()?.map(|g| g.compile_matcher());

    let mut matches = Vec::new();
    let mut truncated = false;

    for entry in WalkDir::new(path).into_iter().filter_entry(|e| !is_skipped_dir(e)) {
        let entry = entry.context("walking directory")?;
        if !entry.file_type().is_file() {
            continue;
        }
        if let Some(glob) = &glob {
            if !glob.is_match(entry.file_name()) {
                continue;
            }
        }
        let Ok(content) = std::fs::read_to_string(entry.path()) else { continue }; // skip binary/non-utf8 files
        for (i, line) in content.lines().enumerate() {
            if regex.is_match(line) {
                if matches.len() >= MAX_MATCHES {
                    truncated = true;
                    break;
                }
                matches.push(format!("{}:{}: {}", entry.path().display(), i + 1, line.trim()));
            }
        }
        if truncated {
            break;
        }
    }

    if matches.is_empty() {
        return Ok("no matches".to_string());
    }
    let mut out = matches.join("\n");
    if truncated {
        out.push_str(&format!("\n... truncated at {MAX_MATCHES} matches"));
    }
    Ok(out)
}

pub struct FindTool;

#[async_trait]
impl Tool for FindTool {
    fn name(&self) -> &str {
        "find"
    }

    fn description(&self) -> &str {
        "Find files by name glob (e.g. '*.rs', '**/test_*.py'), recursively under a directory. Returns matching file paths."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "pattern": { "type": "string", "description": "Glob pattern to match file paths against, e.g. '*.rs' or '**/*.test.ts'" },
                "path": { "type": "string", "description": "Directory to search under (default: current directory)" },
            },
            "required": ["pattern"],
        })
    }

    async fn execute(&self, input: Value) -> Result<String> {
        let pattern = input.get("pattern").and_then(Value::as_str).context("missing 'pattern' argument")?.to_string();
        let path = input.get("path").and_then(Value::as_str).unwrap_or(".").to_string();

        tokio::task::spawn_blocking(move || find_blocking(&pattern, &path)).await?
    }
}

const MAX_RESULTS: usize = 500;

fn find_blocking(pattern: &str, path: &str) -> Result<String> {
    let mut builder = GlobSetBuilder::new();
    builder.add(Glob::new(pattern).with_context(|| format!("invalid glob: {pattern}"))?);
    // Also match bare filenames against the pattern (so "*.rs" matches without needing "**/*.rs").
    builder.add(Glob::new(&format!("**/{pattern}"))?);
    let glob_set = builder.build().context("building glob matcher")?;

    let mut results = Vec::new();
    let mut truncated = false;

    for entry in WalkDir::new(path).into_iter().filter_entry(|e| !is_skipped_dir(e)) {
        let entry = entry.context("walking directory")?;
        if !entry.file_type().is_file() {
            continue;
        }
        let rel = entry.path().strip_prefix(path).unwrap_or(entry.path());
        if glob_set.is_match(rel) || glob_set.is_match(entry.file_name()) {
            if results.len() >= MAX_RESULTS {
                truncated = true;
                break;
            }
            results.push(entry.path().display().to_string());
        }
    }

    if results.is_empty() {
        return Ok("no matches".to_string());
    }
    let mut out = results.join("\n");
    if truncated {
        out.push_str(&format!("\n... truncated at {MAX_RESULTS} results"));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    /// Cleans up its backing directory on drop so repeated test runs don't collide or leak.
    struct TempDir(std::path::PathBuf);
    impl Drop for TempDir {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.0).ok();
        }
    }
    impl TempDir {
        fn path(&self) -> &Path {
            &self.0
        }
    }

    fn setup(files: &[(&str, &str)]) -> TempDir {
        static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("coders-tools-search-test-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for (rel, content) in files {
            let p = dir.join(rel);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, content).unwrap();
        }
        TempDir(dir)
    }

    #[test]
    fn grep_finds_matching_lines() {
        let dir = setup(&[("a.rs", "fn foo() {}\nfn bar() {}\n"), ("b.py", "def foo(): pass\n")]);
        let out = grep_blocking("fn foo", dir.path().to_str().unwrap(), None).unwrap();
        assert!(out.contains("a.rs:1"));
        assert!(!out.contains("b.py"));
    }

    #[test]
    fn grep_respects_file_glob() {
        let dir = setup(&[("a.rs", "TODO fix\n"), ("b.py", "TODO fix\n")]);
        let out = grep_blocking("TODO", dir.path().to_str().unwrap(), Some("*.py")).unwrap();
        assert!(out.contains("b.py"));
        assert!(!out.contains("a.rs"));
    }

    #[test]
    fn grep_skips_skip_dirs() {
        let dir = setup(&[("target/gen.rs", "fn foo() {}\n"), ("src/foo.rs", "fn foo() {}\n")]);
        let out = grep_blocking("fn foo", dir.path().to_str().unwrap(), None).unwrap();
        assert!(out.contains("src/foo.rs") || out.contains("src\\foo.rs"));
        assert!(!out.contains("target"));
    }

    #[test]
    fn find_matches_by_extension() {
        let dir = setup(&[("a.rs", ""), ("b.py", ""), ("nested/c.rs", "")]);
        let out = find_blocking("*.rs", dir.path().to_str().unwrap()).unwrap();
        assert!(out.contains("a.rs"));
        assert!(out.contains("c.rs"));
        assert!(!out.contains("b.py"));
    }

    #[test]
    fn find_no_matches_reports_clearly() {
        let dir = setup(&[("a.rs", "")]);
        let out = find_blocking("*.nonexistent", dir.path().to_str().unwrap()).unwrap();
        assert_eq!(out, "no matches");
    }
}
