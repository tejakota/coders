use anyhow::{Context, Result};
use std::path::PathBuf;

/// Where project instructions can live, in priority order: a workspace copy
/// (`./.coders/STORY.md`) overrides the per-user one (`~/.coders/STORY.md`).
fn story_candidates() -> Result<Vec<PathBuf>> {
    let home = shellexpand::full("~/.coders/STORY.md")?;
    Ok(vec![PathBuf::from("./.coders/STORY.md"), PathBuf::from(home.as_ref())])
}

/// Returns the content of the first STORY.md that exists, or `None` when the
/// project has no instructions to load.
pub fn load_story_instructions() -> Result<Option<String>> {
    load_first(&story_candidates()?)
}

/// The lookup itself, over an explicit candidate list. Split out so tests can
/// point it at a temp directory: the previous tests exercised this by writing
/// to the real `./.coders/` and `~/.coders/STORY.md`, which raced each other
/// under cargo's parallel runner and deleted the user's actual config.
fn load_first(candidates: &[PathBuf]) -> Result<Option<String>> {
    for path in candidates {
        if path.exists() {
            let content = std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
            return Ok(Some(content.trim().to_string()));
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Cleans up its backing directory on drop so repeated runs don't collide
    /// or leak, and nothing outside the temp directory is ever touched.
    struct TempDir(PathBuf);
    impl Drop for TempDir {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.0).ok();
        }
    }
    impl TempDir {
        fn new() -> Self {
            static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let dir = std::env::temp_dir().join(format!("coders-story-test-{}-{n}", std::process::id()));
            std::fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }

        /// Writes a STORY.md under `name/` and returns its path.
        fn story(&self, name: &str, content: &str) -> PathBuf {
            let path = self.0.join(name).join("STORY.md");
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, content).unwrap();
            path
        }

        fn missing(&self, name: &str) -> PathBuf {
            self.0.join(name).join("STORY.md")
        }
    }

    #[test]
    fn workspace_copy_wins_over_the_home_copy() {
        let dir = TempDir::new();
        let candidates = vec![dir.story("workspace", "You are building a web application"), dir.story("home", "You are testing from home")];
        let loaded = load_first(&candidates).unwrap().unwrap();
        assert!(loaded.contains("building a web application"), "{loaded}");
    }

    #[test]
    fn falls_back_to_the_home_copy() {
        let dir = TempDir::new();
        let candidates = vec![dir.missing("workspace"), dir.story("home", "You are testing from home")];
        let loaded = load_first(&candidates).unwrap().unwrap();
        assert!(loaded.contains("testing from home"), "{loaded}");
    }

    #[test]
    fn no_story_anywhere_is_not_an_error() {
        let dir = TempDir::new();
        let candidates = vec![dir.missing("workspace"), dir.missing("home")];
        assert!(load_first(&candidates).unwrap().is_none());
    }

    #[test]
    fn surrounding_whitespace_is_trimmed() {
        let dir = TempDir::new();
        let candidates = vec![dir.story("workspace", "\n\n  # Story\nbuild it well\n\n")];
        assert_eq!(load_first(&candidates).unwrap().unwrap(), "# Story\nbuild it well");
    }

    /// The real lookup must stay usable — it just must not be the thing under
    /// test for the loading logic itself.
    #[test]
    fn real_candidates_are_workspace_then_home() {
        let candidates = story_candidates().unwrap();
        assert_eq!(candidates[0], PathBuf::from("./.coders/STORY.md"));
        assert!(candidates[1].ends_with(".coders/STORY.md"), "{:?}", candidates[1]);
    }
}
