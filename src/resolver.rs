use std::path::PathBuf;
use std::process::Command;

use anyhow::Result;

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
    anyhow::bail!("ref '{reference}' not found on {source}");
}

pub fn resolve_tree(source: &str, reference: &str) -> Result<Resolved> {
    let commit = resolve(source, reference)?;
    let tree = store::checkout_tree(source, &commit)?;
    let checksum = checksum::tree_checksum(&tree)?;
    Ok(Resolved {
        commit,
        checksum,
        tree,
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
