use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct Manifest {
    pub name: String,
    pub harness: Vec<String>,
    #[serde(default)]
    pub skills: BTreeMap<String, SkillSpec>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct SkillSpec {
    pub source: String,
    #[serde(rename = "ref", default)]
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
    if let Some(path) = &spec.path {
        if !is_safe_path(path) {
            anyhow::bail!(
                "invalid path '{path}' for skill '{name}': must be a relative path without '..'"
            );
        }
    }
    if is_local_source(&spec.source) {
        if !is_safe_path(&spec.source) {
            anyhow::bail!(
                "invalid local source '{}' for skill '{name}': must be a relative path without '..'",
                spec.source
            );
        }
        return Ok(());
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
    Ok(())
}

/// Single source of truth: harness name -> base directory.
pub const HARNESSES: &[(&str, &str)] = &[("claude-code", ".claude"), ("opencode", ".opencode")];

/// Project-level directory that holds local (non-git) skills, git-tracked and
/// outside the harness config dirs.
pub const SKILLS_DIR: &str = "skills";

/// The relative manifest `source` for a local skill named `name`.
pub fn local_source(name: &str) -> String {
    format!("./{SKILLS_DIR}/{name}")
}

pub fn supported_harnesses() -> String {
    HARNESSES
        .iter()
        .map(|(n, _)| *n)
        .collect::<Vec<_>>()
        .join(", ")
}

pub fn validate_harness(harness: &[String]) -> Result<()> {
    if harness.is_empty() {
        anyhow::bail!("manifest must declare at least one harness");
    }
    for h in harness {
        if !HARNESSES.iter().any(|(name, _)| name == h) {
            anyhow::bail!(
                "unknown harness '{h}' (supported: {})",
                supported_harnesses()
            );
        }
    }
    Ok(())
}

fn is_safe_path(path: &str) -> bool {
    if path.is_empty() || path.starts_with('/') || path.starts_with('\\') || path.contains(':') {
        return false;
    }
    // ponytail: rejects `..` and a path with no real component (e.g. "."). A
    // Windows component that normalizes to ".." (trailing space/dot) is not
    // detected here — canonicalize+containment would be the full fix if needed.
    let components: Vec<&str> = path.split(['/', '\\']).collect();
    components.iter().all(|c| *c != "..") && components.iter().any(|c| !c.is_empty() && *c != ".")
}

pub(crate) fn is_safe_name(name: &str) -> bool {
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
    if !path.is_dir() {
        return false;
    }
    // A path is a git source only if it is its own repo (root / bare), not a
    // subdir of some other repo (e.g. the project's own `skills/`).
    // `git rev-parse --show-cdup` is empty exactly at the work-tree root; for a
    // bare repo it fails, so fall back to `--is-bare-repository`.
    let out = Command::new("git")
        .arg("-C")
        .arg(path)
        .args(["rev-parse", "--show-cdup"])
        .output();
    if let Ok(out) = out {
        if out.status.success() {
            return String::from_utf8_lossy(&out.stdout).trim().is_empty();
        }
    }
    Command::new("git")
        .arg("-C")
        .arg(path)
        .args(["rev-parse", "--is-bare-repository"])
        .output()
        .map(|o| o.status.success() && String::from_utf8_lossy(&o.stdout).trim() == "true")
        .unwrap_or(false)
}

/// A local source is an existing directory that is not itself a git repo.
pub fn is_local_source(source: &str) -> bool {
    !is_git_source(source) && Path::new(source).is_dir()
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
            harness: vec!["claude-code".to_string()],
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
harness = ["claude-code"]
[skills.foo]
source = "https://github.com/a/b"
ref = 1.2.0
"#;
        assert!(toml::from_str::<Manifest>(raw).is_err());
    }

    #[test]
    fn rejects_string_harness() {
        // `harness` is a list; a bare string must fail to parse (no coercion).
        let raw = r#"
name = "x"
harness = "claude-code"
[skills]
"#;
        assert!(toml::from_str::<Manifest>(raw).is_err());
    }

    #[test]
    fn harness_validation() {
        assert!(validate_harness(&[]).is_err());
        assert!(validate_harness(&["bogus".to_string()]).is_err());
        assert!(validate_harness(&["claude-code".to_string()]).is_ok());
        assert!(validate_harness(&["claude-code".to_string(), "opencode".to_string()]).is_ok());
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

    #[test]
    fn local_source_detection() {
        let tmp = std::env::temp_dir().join(format!("agenv-local-{}", std::process::id()));
        std::fs::create_dir_all(&tmp).unwrap();
        let s = tmp.to_string_lossy().into_owned();
        assert!(is_local_source(&s));
        assert!(!is_git_source(&s));
        assert!(!is_local_source("https://github.com/a/b"));
        assert!(!is_local_source("does-not-exist"));
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn subdir_of_repo_is_not_git_source() {
        use std::process::Command;
        let tmp = std::env::temp_dir().join(format!("agenv-repo-{}", std::process::id()));
        let sub = tmp.join("sub");
        std::fs::create_dir_all(&sub).unwrap();
        let inited = Command::new("git")
            .arg("-C")
            .arg(&tmp)
            .args(["init", "-q"])
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if inited {
            let s = tmp.to_string_lossy().into_owned();
            assert!(is_git_source(&s));
            assert!(!is_local_source(&s));
            let sub_s = sub.to_string_lossy().into_owned();
            assert!(!is_git_source(&sub_s), "subdir must not be a git source");
            assert!(is_local_source(&sub_s));
        }
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn local_source_safe_path_rules() {
        assert!(is_safe_path("./skills/foo"));
        assert!(!is_safe_path("/abs/path"));
        assert!(!is_safe_path("../escape"));
        assert!(!is_safe_path(r"C:\abs"));
    }

    #[test]
    fn local_source_skips_git_checks() {
        let tmp = std::env::temp_dir().join(format!("agenv-localspec-{}", std::process::id()));
        std::fs::create_dir_all(&tmp).unwrap();
        let s = tmp.to_string_lossy().into_owned();
        // absolute path is rejected because local sources must be relative
        assert!(validate_skill("ok", &spec(&s, "", None)).is_err());
        // an empty ref on a git source is still rejected
        assert!(validate_skill("ok", &spec("git@github.com:a/b", "", None)).is_err());
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
