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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sandbox: Option<SandboxSpec>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone, Default)]
pub struct SkillSpec {
    pub source: String,
    #[serde(rename = "ref", default)]
    pub r#ref: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub homepage: Option<String>,
}

/// Declared shape of an isolated runtime (OpenSandbox). `agenv` only describes
/// it: nothing here creates, connects to, or manages a sandbox.
#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct SandboxSpec {
    pub image: String,
    #[serde(default = "default_timeout")]
    pub timeout: String,
    #[serde(default = "default_skills_root")]
    pub skills_root: String,
    #[serde(default = "default_workdir")]
    pub workdir: String,
    #[serde(default)]
    pub credential_proxy: bool,
    #[serde(default)]
    pub resources: BTreeMap<String, String>,
    /// Values are indirections like `"env:REAL_VAR"`, never values. Enforced by
    /// `validate_sandbox` rather than by deserialization: a parse error renders
    /// the offending source line, which would print the very value this rule
    /// exists to keep out of logs.
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    #[serde(default)]
    pub network: NetworkSpec,
}

impl Default for SandboxSpec {
    /// The serde field defaults, so `SandboxSpec::default()` is a usable value
    /// (`#[serde(default = "…")]` applies to deserialization only).
    fn default() -> Self {
        SandboxSpec {
            image: String::new(),
            timeout: default_timeout(),
            skills_root: default_skills_root(),
            workdir: default_workdir(),
            credential_proxy: false,
            resources: BTreeMap::new(),
            env: BTreeMap::new(),
            network: NetworkSpec::default(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct NetworkSpec {
    #[serde(default = "default_network_action")]
    pub default_action: String,
    #[serde(default)]
    pub allow: Vec<String>,
    #[serde(default)]
    pub deny: Vec<String>,
}

impl Default for NetworkSpec {
    fn default() -> Self {
        NetworkSpec {
            default_action: default_network_action(),
            allow: Vec::new(),
            deny: Vec::new(),
        }
    }
}

pub fn default_timeout() -> String {
    "30m".to_string()
}

pub fn default_skills_root() -> String {
    ".agents/skills".to_string()
}

pub fn default_workdir() -> String {
    "/workspace".to_string()
}

pub fn default_network_action() -> String {
    "deny".to_string()
}

impl Manifest {
    pub fn load(path: &Path) -> Result<Manifest> {
        let text =
            fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        let mut doc = toml_edit::DocumentMut::new();
        doc["name"] = toml_edit::value(self.name.clone());
        let mut harness = toml_edit::Array::new();
        for h in &self.harness {
            harness.push(h.clone());
        }
        doc["harness"] = toml_edit::value(harness);
        if !self.skills.is_empty() {
            let mut skills = toml_edit::Table::new();
            for (name, spec) in &self.skills {
                let mut t = toml_edit::InlineTable::new();
                // keep in sync with SkillSpec fields
                t.insert("source", spec.source.clone().into());
                t.insert("ref", spec.r#ref.clone().into());
                if let Some(p) = &spec.path {
                    t.insert("path", p.clone().into());
                }
                if let Some(v) = &spec.description {
                    t.insert("description", v.clone().into());
                }
                if let Some(v) = &spec.license {
                    t.insert("license", v.clone().into());
                }
                if let Some(v) = &spec.version {
                    t.insert("version", v.clone().into());
                }
                if let Some(v) = &spec.homepage {
                    t.insert("homepage", v.clone().into());
                }
                skills.insert(
                    name,
                    toml_edit::Item::Value(toml_edit::Value::InlineTable(t)),
                );
            }
            doc["skills"] = toml_edit::Item::Table(skills);
        }
        if let Some(sandbox) = &self.sandbox {
            doc["sandbox"] = toml_edit::Item::Table(sandbox_table(sandbox));
        }
        fs::write(path, doc.to_string()).with_context(|| format!("writing {}", path.display()))
    }
}

/// Build the `[sandbox]` table. Scalars are always written so a reader sees the
/// effective value; collections only when non-empty.
fn sandbox_table(sb: &SandboxSpec) -> toml_edit::Table {
    let mut t = toml_edit::Table::new();
    t.insert("image", toml_edit::value(sb.image.clone()));
    t.insert("timeout", toml_edit::value(sb.timeout.clone()));
    t.insert("skills_root", toml_edit::value(sb.skills_root.clone()));
    t.insert("workdir", toml_edit::value(sb.workdir.clone()));
    t.insert("credential_proxy", toml_edit::value(sb.credential_proxy));

    if !sb.resources.is_empty() {
        let mut resources = toml_edit::Table::new();
        for (k, v) in &sb.resources {
            resources.insert(k, toml_edit::value(v.clone()));
        }
        t.insert("resources", toml_edit::Item::Table(resources));
    }

    if !sb.env.is_empty() {
        let mut env = toml_edit::Table::new();
        for (k, v) in &sb.env {
            env.insert(k, toml_edit::value(v.clone()));
        }
        t.insert("env", toml_edit::Item::Table(env));
    }

    let mut network = toml_edit::Table::new();
    network.insert(
        "default_action",
        toml_edit::value(sb.network.default_action.clone()),
    );
    for (key, values) in [("allow", &sb.network.allow), ("deny", &sb.network.deny)] {
        if !values.is_empty() {
            let mut arr = toml_edit::Array::new();
            for v in values {
                arr.push(v.clone());
            }
            network.insert(key, toml_edit::value(arr));
        }
    }
    t.insert("network", toml_edit::Item::Table(network));

    t
}

pub fn validate_skill(name: &str, spec: &SkillSpec) -> Result<()> {
    if !is_safe_name(name) {
        anyhow::bail!(
            "invalid skill name '{name}': must be a single path component (no '/' or '\\', not '.' or '..')"
        );
    }
    validate_metadata(name, spec)?;
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

fn validate_metadata(name: &str, spec: &SkillSpec) -> Result<()> {
    if let Some(v) = &spec.description {
        if v.is_empty() {
            anyhow::bail!("skill '{}' has an empty description", name);
        }
    }
    if let Some(v) = &spec.license {
        if v.is_empty() {
            anyhow::bail!("skill '{}' has an empty license", name);
        }
    }
    if let Some(v) = &spec.version {
        if !is_semver(v) {
            anyhow::bail!("skill '{}' has invalid version '{v}'", name);
        }
    }
    if let Some(v) = &spec.homepage {
        if !is_https_url(v) {
            anyhow::bail!("skill '{}' homepage must start with https://", name);
        }
    }
    Ok(())
}

fn is_semver(s: &str) -> bool {
    let core = s.split(['-', '+']).next().unwrap_or("");
    let parts: Vec<&str> = core.split('.').collect();
    parts.len() == 3
        && parts
            .iter()
            .all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
}

fn is_https_url(s: &str) -> bool {
    s.starts_with("https://")
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

/// Validate `[sandbox]` when present. Runs alongside `validate_harness` so every
/// command rejects the same inputs before doing any work.
pub fn validate_sandbox(sandbox: &Option<SandboxSpec>) -> Result<()> {
    let Some(sb) = sandbox else {
        return Ok(());
    };

    if sb.image.trim().is_empty() {
        anyhow::bail!("sandbox.image must not be empty");
    }
    if !is_duration(&sb.timeout) {
        anyhow::bail!(
            "sandbox.timeout '{}' must be a duration (30m, 1h, 45s) or a positive number of seconds",
            sb.timeout
        );
    }
    for (key, value) in &sb.resources {
        if !is_positive_amount(value) {
            anyhow::bail!(
                "sandbox.resources.{key} '{value}' must be a positive amount (e.g. 1, 0.5, 500m, 2Gi)"
            );
        }
    }
    if !is_safe_path(&sb.skills_root) {
        anyhow::bail!(
            "sandbox.skills_root '{}' must be a relative path without '..'",
            sb.skills_root
        );
    }
    if !sb.workdir.starts_with('/') {
        anyhow::bail!("sandbox.workdir '{}' must be an absolute path", sb.workdir);
    }
    if !matches!(sb.network.default_action.as_str(), "allow" | "deny") {
        anyhow::bail!(
            "sandbox.network.default_action '{}' must be 'allow' or 'deny'",
            sb.network.default_action
        );
    }
    for target in sb.network.allow.iter().chain(&sb.network.deny) {
        if !is_network_target(target) {
            anyhow::bail!(
                "sandbox.network target '{target}' must be a domain, wildcard domain, IP, or CIDR"
            );
        }
    }
    // Deliberately a plain string type and a validation rule, not a typed
    // struct: rejecting here during deserialization would be caught by the TOML
    // parser, whose error prints the offending source line — the secret itself.
    for (key, value) in &sb.env {
        if !is_indirection(value) {
            anyhow::bail!(
                "sandbox.env.{key} must be an indirection like \"env:REAL_VAR\"; \
                 agenv never stores a secret value"
            );
        }
    }
    Ok(())
}

/// `env:NAME` — a scheme plus an environment-variable name. A raw token cannot
/// satisfy this by accident, which is what makes the rule testable.
fn is_indirection(s: &str) -> bool {
    let Some(name) = s.strip_prefix("env:") else {
        return false;
    };
    let mut chars = name.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// `45s`, `30m`, `1h`, or a bare number of seconds.
fn is_duration(s: &str) -> bool {
    let digits = match s.as_bytes().last() {
        Some(b's') | Some(b'm') | Some(b'h') => &s[..s.len() - 1],
        _ => s,
    };
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return false;
    }
    digits.parse::<u64>().map(|n| n > 0).unwrap_or(false)
}

/// `1`, `0.5`, `500m`, `2Gi` — a positive number with an optional unit.
fn is_positive_amount(s: &str) -> bool {
    let number = ["Ki", "Mi", "Gi", "Ti", "K", "M", "G", "T", "m"]
        .iter()
        .find_map(|unit| s.strip_suffix(unit))
        .unwrap_or(s);
    number
        .parse::<f64>()
        .map(|n| n > 0.0 && n.is_finite())
        .unwrap_or(false)
}

/// A domain, `*.domain`, an IP, or a CIDR.
fn is_network_target(s: &str) -> bool {
    let host = s.strip_prefix("*.").unwrap_or(s);
    if let Some((addr, prefix)) = host.split_once('/') {
        return addr.parse::<std::net::IpAddr>().is_ok()
            && prefix.parse::<u8>().map(|p| p <= 128).unwrap_or(false);
    }
    host.parse::<std::net::IpAddr>().is_ok() || is_domain_name(host)
}

fn is_domain_name(s: &str) -> bool {
    s.contains('.')
        && s.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        })
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
                path: Some("skills/rust".to_string()),
                description: Some("Reviews Rust".to_string()),
                license: Some("MIT".to_string()),
                version: Some("1.2.0".to_string()),
                homepage: Some("https://example.com".to_string()),
            },
        );
        let manifest = Manifest {
            name: "payments-backend".to_string(),
            harness: vec!["claude-code".to_string()],
            skills,
            sandbox: None,
        };
        let path = tmp_manifest_path("roundtrip");
        manifest.save(&path).unwrap();
        let loaded = Manifest::load(&path).unwrap();
        assert_eq!(manifest, loaded);

        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("[skills]\n"), "{text}");
        assert!(!text.contains("[skills.rust-reviewer]"), "{text}");
        assert!(text.contains("rust-reviewer = {"), "{text}");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn empty_skills_omits_section() {
        let manifest = Manifest {
            name: "x".to_string(),
            harness: vec!["claude-code".to_string()],
            skills: BTreeMap::new(),
            sandbox: None,
        };
        let path = tmp_manifest_path("empty");
        manifest.save(&path).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(!text.contains("[skills]"), "{text}");
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
            ..Default::default()
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
    fn metadata_validation() {
        let ok = spec("git@github.com:a/b", "main", None);
        assert!(validate_skill("ok", &ok).is_ok());

        let mut bad_version = ok.clone();
        bad_version.version = Some("not-a-version".to_string());
        assert!(validate_skill("ok", &bad_version).is_err());

        let mut ok_version = ok.clone();
        ok_version.version = Some("1.2.3-beta.1+build".to_string());
        assert!(validate_skill("ok", &ok_version).is_ok());

        let mut bad_homepage = ok.clone();
        bad_homepage.homepage = Some("ftp://x".to_string());
        assert!(validate_skill("ok", &bad_homepage).is_err());

        let mut empty_desc = ok.clone();
        empty_desc.description = Some("".to_string());
        assert!(validate_skill("ok", &empty_desc).is_err());

        assert!(is_semver("1.2.3"));
        assert!(is_semver("1.2.3-rc.1+build.5"));
        assert!(!is_semver("1.2"));
        assert!(!is_semver("v1.2.3"));
        assert!(!is_semver(""));
        assert!(is_https_url("https://example.com"));
        assert!(!is_https_url("ftp://example.com"));
    }

    #[test]
    fn local_source_metadata_is_validated() {
        let tmp = std::env::temp_dir().join(format!("agenv-meta-{}", std::process::id()));
        std::fs::create_dir_all(&tmp).unwrap();
        let source = tmp.to_string_lossy().into_owned();

        let mut spec = SkillSpec {
            source: source.clone(),
            r#ref: String::new(),
            path: None,
            ..Default::default()
        };
        spec.version = Some("not-a-version".to_string());
        let err = validate_skill("ok", &spec).unwrap_err().to_string();
        assert!(err.contains("version"), "{err}");

        spec.version = None;
        spec.homepage = Some("ftp://x".to_string());
        let err = validate_skill("ok", &spec).unwrap_err().to_string();
        assert!(err.contains("homepage"), "{err}");

        let _ = std::fs::remove_dir_all(&tmp);
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

    #[test]
    fn sandbox_round_trip() {
        let mut resources = BTreeMap::new();
        resources.insert("cpu".to_string(), "1".to_string());
        resources.insert("memory".to_string(), "2Gi".to_string());
        let mut env = BTreeMap::new();
        env.insert(
            "ANTHROPIC_API_KEY".to_string(),
            "env:ANTHROPIC_API_KEY".to_string(),
        );

        let manifest = Manifest {
            name: "life-simulator-mvp".to_string(),
            harness: vec!["opencode".to_string()],
            skills: BTreeMap::new(),
            sandbox: Some(SandboxSpec {
                image: "node:22-bookworm-slim".to_string(),
                timeout: "30m".to_string(),
                skills_root: ".agents/skills".to_string(),
                workdir: "/workspace".to_string(),
                credential_proxy: true,
                resources,
                env,
                network: NetworkSpec {
                    default_action: "deny".to_string(),
                    allow: vec!["api.anthropic.com".to_string()],
                    deny: vec!["*.example.com".to_string()],
                },
            }),
        };

        let path = tmp_manifest_path("sandbox-roundtrip");
        manifest.save(&path).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("[sandbox]"), "{text}");
        assert!(
            text.contains("ANTHROPIC_API_KEY = \"env:ANTHROPIC_API_KEY\""),
            "{text}"
        );
        assert!(validate_sandbox(&manifest.sandbox).is_ok());
        assert_eq!(manifest, Manifest::load(&path).unwrap());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn sandbox_absent_is_omitted() {
        let manifest = Manifest {
            name: "x".to_string(),
            harness: vec!["claude-code".to_string()],
            skills: BTreeMap::new(),
            sandbox: None,
        };
        let path = tmp_manifest_path("sandbox-absent");
        manifest.save(&path).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(!text.contains("sandbox"), "{text}");
        assert_eq!(manifest, Manifest::load(&path).unwrap());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn sandbox_defaults_are_applied() {
        let manifest: Manifest = toml::from_str(
            "name = \"x\"\nharness = [\"claude-code\"]\n[sandbox]\nimage = \"alpine\"\n",
        )
        .unwrap();
        let sb = manifest.sandbox.as_ref().expect("sandbox is present");
        assert_eq!(sb.timeout, "30m");
        assert_eq!(sb.skills_root, ".agents/skills");
        assert_eq!(sb.workdir, "/workspace");
        assert!(!sb.credential_proxy);
        assert_eq!(sb.network.default_action, "deny");
        assert!(validate_sandbox(&manifest.sandbox).is_ok());
    }

    #[test]
    fn sandbox_validation_rules() {
        let ok = SandboxSpec {
            image: "alpine".to_string(),
            ..Default::default()
        };
        assert!(validate_sandbox(&None).is_ok());
        assert!(validate_sandbox(&Some(ok.clone())).is_ok());

        for bad in [
            SandboxSpec {
                image: "  ".to_string(),
                ..ok.clone()
            },
            SandboxSpec {
                timeout: "soon".to_string(),
                ..ok.clone()
            },
            SandboxSpec {
                timeout: "0".to_string(),
                ..ok.clone()
            },
            SandboxSpec {
                skills_root: "../etc".to_string(),
                ..ok.clone()
            },
            SandboxSpec {
                workdir: "workspace".to_string(),
                ..ok.clone()
            },
            SandboxSpec {
                network: NetworkSpec {
                    default_action: "maybe".to_string(),
                    ..NetworkSpec::default()
                },
                ..ok.clone()
            },
            SandboxSpec {
                network: NetworkSpec {
                    allow: vec!["not a domain".to_string()],
                    ..NetworkSpec::default()
                },
                ..ok.clone()
            },
            SandboxSpec {
                resources: BTreeMap::from([("cpu".to_string(), "0".to_string())]),
                ..ok.clone()
            },
            SandboxSpec {
                env: BTreeMap::from([("MY_KEY".to_string(), String::new())]),
                ..ok.clone()
            },
            SandboxSpec {
                env: BTreeMap::from([("MY_KEY".to_string(), "oops".to_string())]),
                ..ok.clone()
            },
            SandboxSpec {
                env: BTreeMap::from([("MY_KEY".to_string(), "sk-ant-live-0123456789".to_string())]),
                ..ok.clone()
            },
        ] {
            assert!(
                validate_sandbox(&Some(bad.clone())).is_err(),
                "should be rejected: {bad:?}"
            );
        }

        // A literal must be rejected by validation — and by a message that does
        // not echo the value, because this is the case where the user has just
        // pasted a real secret into the manifest.
        let with_literal = SandboxSpec {
            env: BTreeMap::from([("MY_KEY".to_string(), "sk-ant-live-0123456789".to_string())]),
            ..ok.clone()
        };
        let err = validate_sandbox(&Some(with_literal))
            .unwrap_err()
            .to_string();
        assert!(err.contains("sandbox.env.MY_KEY"), "{err}");
        assert!(err.contains("env:REAL_VAR"), "{err}");
        assert!(
            !err.contains("sk-ant"),
            "the message leaks the value: {err}"
        );

        let accepted = SandboxSpec {
            timeout: "45s".to_string(),
            resources: BTreeMap::from([("memory".to_string(), "512Mi".to_string())]),
            env: BTreeMap::from([("MY_KEY".to_string(), "env:THE_REAL_VAR".to_string())]),
            network: NetworkSpec {
                default_action: "allow".to_string(),
                allow: vec![
                    "api.anthropic.com".to_string(),
                    "*.pypi.org".to_string(),
                    "10.0.0.0/8".to_string(),
                ],
                deny: vec!["1.2.3.4".to_string()],
            },
            ..ok.clone()
        };
        assert!(validate_sandbox(&Some(accepted)).is_ok());
    }
}
