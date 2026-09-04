use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct Manifest {
    pub name: String,
    pub harness: String,
    #[serde(default)]
    pub skills: BTreeMap<String, SkillSpec>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct SkillSpec {
    pub source: String,
    #[serde(rename = "ref")]
    pub r#ref: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

impl Manifest {
    pub fn load(path: &Path) -> Result<Manifest> {
        let text =
            fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        let text = toml::to_string_pretty(self).context("serializing manifest")?;
        fs::write(path, text).with_context(|| format!("writing {}", path.display()))
    }
}

pub fn validate_skill(name: &str, spec: &SkillSpec) -> Result<()> {
    if !is_safe_name(name) {
        anyhow::bail!(
            "invalid skill name '{name}': must be a single path component (no '/' or '\\', not '.' or '..')"
        );
    }
    if !is_git_source(&spec.source) {
        anyhow::bail!(
            "source '{}' is not a git repository",
            crate::store::redact(&spec.source)
        );
    }
    if spec.r#ref.trim().is_empty() {
        anyhow::bail!("skill '{}' has an empty ref", name);
    }
    if let Some(path) = &spec.path {
        if !is_safe_path(path) {
            anyhow::bail!(
                "invalid path '{path}' for skill '{name}': must be a relative path without '..'"
            );
        }
    }
    Ok(())
}

fn is_safe_path(path: &str) -> bool {
    !path.is_empty()
        && !path.starts_with('/')
        && !path.starts_with('\\')
        && !path.contains(':')
        && path.split(['/', '\\']).all(|c| c != "..")
}

fn is_safe_name(name: &str) -> bool {
    !name.is_empty()
        && name != "."
        && name != ".."
        && !name.contains('/')
        && !name.contains('\\')
        && !name.contains(':')
}

pub fn is_git_source(source: &str) -> bool {
    if source.starts_with('-') {
        return false;
    }
    if source.starts_with("git@") || source.contains("://") || source.ends_with(".git") {
        return true;
    }
    let path = Path::new(source);
    if path.exists() {
        return Command::new("git")
            .arg("-C")
            .arg(path)
            .args(["rev-parse", "--git-dir"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn tmp_manifest_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("agenv-test-{}-{}.toml", name, std::process::id()))
    }

    #[test]
    fn round_trip() {
        let mut skills = BTreeMap::new();
        skills.insert(
            "rust-reviewer".to_string(),
            SkillSpec {
                source: "https://github.com/acme/skills".to_string(),
                r#ref: "v1.2.0".to_string(),
                path: None,
            },
        );
        let manifest = Manifest {
            name: "payments-backend".to_string(),
            harness: "claude-code".to_string(),
            skills,
        };
        let path = tmp_manifest_path("roundtrip");
        manifest.save(&path).unwrap();
        let loaded = Manifest::load(&path).unwrap();
        assert_eq!(manifest, loaded);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn rejects_unquoted_float_ref() {
        // TOML parses `ref = 1.2.0` as a float; deserializing into String must fail.
        let raw = r#"
name = "x"
harness = "claude-code"
[skills.foo]
source = "https://github.com/a/b"
ref = 1.2.0
"#;
        assert!(toml::from_str::<Manifest>(raw).is_err());
    }

    fn spec(source: &str, reference: &str, path: Option<&str>) -> SkillSpec {
        SkillSpec {
            source: source.to_string(),
            r#ref: reference.to_string(),
            path: path.map(String::from),
        }
    }

    #[test]
    fn validation_rules() {
        assert!(validate_skill("", &spec("git@github.com:a/b", "main", None)).is_err());
        assert!(validate_skill("ok", &spec("not-a-git-source", "main", None)).is_err());
        assert!(validate_skill("ok", &spec("git@github.com:a/b", "", None)).is_err());
        assert!(validate_skill("ok", &spec("git@github.com:a/b", "main", None)).is_ok());
        assert!(
            validate_skill("ok", &spec("git@github.com:a/b", "main", Some("../evil"))).is_err()
        );
        assert!(validate_skill("ok", &spec("git@github.com:a/b", "main", Some("/abs"))).is_err());
        assert!(
            validate_skill("ok", &spec("git@github.com:a/b", "main", Some("skills/x"))).is_ok()
        );
        for bad in ["../evil", "a/b", "a\\b", "C:evil", ".", ".."] {
            assert!(
                validate_skill(bad, &spec("git@github.com:a/b", "main", None)).is_err(),
                "name {bad:?} should be rejected"
            );
        }
    }
}
