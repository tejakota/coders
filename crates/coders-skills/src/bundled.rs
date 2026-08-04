use anyhow::{Context, Result};
use std::path::Path;

struct BundledSkill {
    name: &'static str,
    content: &'static str,
}

const BUNDLED: &[BundledSkill] = &[
    BundledSkill { name: "code-analysis", content: include_str!("../assets/skills/code-analysis/SKILL.md") },
    BundledSkill { name: "code-generation", content: include_str!("../assets/skills/code-generation/SKILL.md") },
];

/// Writes each bundled skill into `skills_dir/<name>/SKILL.md` if that file
/// doesn't already exist. Never overwrites, so a user's own edits (or a
/// same-named custom skill) always win over the bundled default.
pub fn install_bundled_skills(skills_dir: &Path) -> Result<()> {
    for skill in BUNDLED {
        let dir = skills_dir.join(skill.name);
        let path = dir.join("SKILL.md");
        if path.exists() {
            continue;
        }
        std::fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
        std::fs::write(&path, skill.content).with_context(|| format!("writing {}", path.display()))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn installs_bundled_skills_without_overwriting_existing() {
        let dir = std::env::temp_dir().join(format!("coders-skills-bundled-test-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("code-analysis")).unwrap();
        std::fs::write(dir.join("code-analysis/SKILL.md"), "---\nname: code-analysis\ndescription: custom\n---\nmine").unwrap();

        install_bundled_skills(&dir).unwrap();

        let analysis = std::fs::read_to_string(dir.join("code-analysis/SKILL.md")).unwrap();
        assert!(analysis.contains("mine"), "existing skill must not be overwritten");

        let generation = std::fs::read_to_string(dir.join("code-generation/SKILL.md")).unwrap();
        assert!(generation.contains("name: code-generation"));

        std::fs::remove_dir_all(&dir).ok();
    }
}
