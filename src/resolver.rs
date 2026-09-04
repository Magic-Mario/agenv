use std::path::PathBuf;
use std::process::Command;

use anyhow::{Context, Result};

use crate::{checksum, store};

pub struct Resolved {
    pub commit: String,
    pub checksum: String,
    pub tree: PathBuf,
}

pub fn resolve(source: &str, reference: &str) -> Result<String> {
    let repo = store::fetch(source)?;
    for spec in [reference.to_string(), format!("origin/{reference}")] {
        if let Some(commit) = rev_parse_commit(&repo, &spec) {
            return Ok(commit);
        }
    }
    anyhow::bail!("ref '{reference}' not found on {}", store::redact(source));
}

pub fn default_branch(source: &str) -> Result<String> {
    let out = Command::new("git")
        .args(["ls-remote", "--symref", "--"])
        .arg(source)
        .arg("HEAD")
        .output()
        .context("running git ls-remote")?;
    if !out.status.success() {
        anyhow::bail!(
            "could not determine default branch of {}",
            store::redact(source)
        );
    }
    let text = String::from_utf8(out.stdout).context("decoding ls-remote output")?;
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("ref: refs/heads/") {
            return Ok(rest.split_whitespace().next().unwrap_or("HEAD").to_string());
        }
    }
    anyhow::bail!(
        "could not determine default branch of {}",
        store::redact(source)
    )
}

pub fn resolve_tree(source: &str, reference: &str, path: Option<&str>) -> Result<Resolved> {
    let commit = resolve(source, reference)?;
    checkout(source, &commit, path)
}

pub fn checkout(source: &str, commit: &str, path: Option<&str>) -> Result<Resolved> {
    let tree = store::checkout_tree(source, commit)?;
    let root = match path {
        Some(p) => {
            let sub = tree.join(p);
            let canonical_tree = tree
                .canonicalize()
                .with_context(|| format!("resolving {}", tree.display()))?;
            let canonical_sub = sub.canonicalize().with_context(|| {
                format!(
                    "path '{p}' not found at commit {commit} of {}",
                    store::redact(source)
                )
            })?;
            if !canonical_sub.starts_with(&canonical_tree) {
                anyhow::bail!(
                    "path '{p}' escapes the repository tree of {}",
                    store::redact(source)
                );
            }
            canonical_sub
        }
        None => tree,
    };
    let checksum = checksum::tree_checksum(&root)?;
    Ok(Resolved {
        commit: commit.to_string(),
        checksum,
        tree: root,
    })
}

fn rev_parse_commit(repo: &std::path::Path, spec: &str) -> Option<String> {
    let out = Command::new("git")
        .arg("--git-dir")
        .arg(repo)
        .args(["rev-parse", "--verify", &format!("{spec}^{{commit}}")])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    String::from_utf8(out.stdout)
        .ok()
        .map(|s| s.trim().to_string())
}
