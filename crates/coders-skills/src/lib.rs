mod bundled;
mod tool;

pub use bundled::install_bundled_skills;
pub use tool::SkillTool;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct Skill {
    pub name: String,
    pub description: String,
    pub body: String,
    pub path: PathBuf,
}

/// Parses a SKILL.md file: a YAML-ish frontmatter block (only `name:` and
/// `description:` scalars are recognized) followed by free-form instructions.
fn parse_skill_md(path: &Path, text: &str) -> Option<Skill> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text); // tolerate BOM
    let rest = text.strip_prefix("---")?;
    let (frontmatter, body) = rest.split_once("---")?;

    let mut name = None;
    let mut description = None;
    for line in frontmatter.lines() {
        let line = line.trim();
        if let Some(value) = line.strip_prefix("name:") {
            name = Some(value.trim().trim_matches('"').to_string());
        } else if let Some(value) = line.strip_prefix("description:") {
            description = Some(value.trim().trim_matches('"').to_string());
        }
    }

    Some(Skill {
        name: name?,
        description: description.unwrap_or_default(),
        body: body.trim().to_string(),
        path: path.to_path_buf(),
    })
}

fn scan_dir(dir: &Path, out: &mut BTreeMap<String, Skill>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let skill_md = entry.path().join("SKILL.md");
        if !skill_md.is_file() {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&skill_md) else { continue };
        if let Some(skill) = parse_skill_md(&skill_md, &text) {
            out.insert(skill.name.clone(), skill);
        }
    }
}

/// Loads skills from `dirs` in order; later directories override earlier
/// ones on name collision (so a project-local skill can shadow a user one).
pub fn load_skills(dirs: &[PathBuf]) -> Vec<Skill> {
    let mut merged = BTreeMap::new();
    for dir in dirs {
        scan_dir(dir, &mut merged);
    }
    merged.into_values().collect()
}

/// Default search path: `~/.coders/skills` (user-level) then `./.coders/skills`
/// (project-level, overrides on name collision).
pub fn default_skill_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(home) = dirs::home_dir() {
        dirs.push(home.join(".coders/skills"));
    }
    dirs.push(PathBuf::from(".coders/skills"));
    dirs
}

/// A short catalog to embed in the system prompt so the model knows what's
/// available without paying for every skill's full body up front.
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_frontmatter_and_body() {
        let text = "---\nname: foo\ndescription: does foo things\n---\n\nBody text here.\n";
        let skill = parse_skill_md(Path::new("SKILL.md"), text).unwrap();
        assert_eq!(skill.name, "foo");
        assert_eq!(skill.description, "does foo things");
        assert_eq!(skill.body, "Body text here.");
    }

    #[test]
    fn missing_frontmatter_returns_none() {
        assert!(parse_skill_md(Path::new("SKILL.md"), "no frontmatter here").is_none());
    }

    #[test]
    fn project_dir_overrides_user_dir_on_name_collision() {
        let tmp = std::env::temp_dir().join(format!("coders-skills-test-{}", std::process::id()));
        let user_dir = tmp.join("user/dupe");
        let project_dir = tmp.join("project/dupe");
        std::fs::create_dir_all(&user_dir).unwrap();
        std::fs::create_dir_all(&project_dir).unwrap();
        std::fs::write(user_dir.join("SKILL.md"), "---\nname: dupe\ndescription: user version\n---\nuser body").unwrap();
        std::fs::write(project_dir.join("SKILL.md"), "---\nname: dupe\ndescription: project version\n---\nproject body").unwrap();

        let skills = load_skills(&[tmp.join("user"), tmp.join("project")]);
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].description, "project version");

        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn catalog_lists_names_and_descriptions() {
        let skills = vec![Skill { name: "a".into(), description: "does a".into(), body: String::new(), path: PathBuf::new() }];
        let out = catalog(&skills);
        assert!(out.contains("a: does a"));
        assert_eq!(catalog(&[]), "");
    }
}

pub fn catalog(skills: &[Skill]) -> String {
    if skills.is_empty() {
        return String::new();
    }
    let mut out = String::from(
        "\n\nAvailable skills (call the `skill` tool with the skill's name to load its full instructions before doing matching work):\n",
    );
    for skill in skills {
        out.push_str(&format!("- {}: {}\n", skill.name, skill.description));
    }
    out
}
