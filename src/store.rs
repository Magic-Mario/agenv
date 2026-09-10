use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};
use walkdir::WalkDir;

pub fn store_root() -> PathBuf {
    if let Some(dir) = std::env::var_os("AGENV_STORE") {
        return PathBuf::from(dir);
    }
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .expect("no HOME or USERPROFILE set");
    home.join(".local")
        .join("share")
        .join("agenv")
        .join("store")
}

fn sha256_hex(s: &str) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(s.as_bytes()))
}

pub fn redact(url: &str) -> String {
    match url.find("://") {
        Some(i) => match url[i + 3..].find('@') {
            Some(at) => format!("{}://{}", &url[..i], &url[i + 3 + at + 1..]),
            None => url.to_string(),
        },
        None => url.to_string(),
    }
}

pub fn repo_dir(source: &str) -> PathBuf {
    store_root().join(sha256_hex(source)).join("repo.git")
}

pub fn commit_dir(source: &str, commit: &str) -> PathBuf {
    store_root().join(sha256_hex(source)).join(commit)
}

fn ensure_git() -> Result<()> {
    let ok = Command::new("git")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if ok {
        Ok(())
    } else {
        anyhow::bail!("git not found; install git and ensure it is on your PATH")
    }
}

fn ensure_clone(source: &str) -> Result<PathBuf> {
    ensure_git()?;
    let repo = repo_dir(source);
    if repo.join("HEAD").exists() {
        return Ok(repo);
    }
    fs::create_dir_all(repo.parent().unwrap()).with_context(|| "creating store dir")?;
    let status = Command::new("git")
        .args(["clone", "--bare", "--quiet", "--"])
        .arg(source)
        .arg(&repo)
        .status()
        .context("running git clone")?;
    if !status.success() {
        anyhow::bail!("git clone failed for {}", redact(source));
    }
    Ok(repo)
}

pub fn fetch(source: &str) -> Result<PathBuf> {
    let repo = ensure_clone(source)?;
    let status = Command::new("git")
        .arg("--git-dir")
        .arg(&repo)
        .args([
            "fetch",
            "--quiet",
            "origin",
            "+refs/heads/*:refs/remotes/origin/*",
            "+refs/tags/*:refs/tags/*",
        ])
        .status()
        .context("running git fetch")?;
    if !status.success() {
        anyhow::bail!("git fetch failed for {}", redact(source));
    }
    Ok(repo)
}

pub fn checkout_tree(source: &str, commit: &str) -> Result<PathBuf> {
    let dir = commit_dir(source, commit);
    if dir.exists() {
        return Ok(dir);
    }
    let repo = ensure_clone(source)?;
    fs::create_dir_all(dir.parent().unwrap()).with_context(|| "creating commit dir")?;
    let status = Command::new("git")
        .arg("-C")
        .arg(&repo)
        .args(["worktree", "add", "--detach"])
        .arg(&dir)
        .arg(commit)
        .status()
        .context("running git worktree add")?;
    if !status.success() {
        anyhow::bail!("failed to extract tree at {commit}");
    }
    Ok(dir)
}

/// Delete store entries not in `keep` — bare clones and commit worktrees for
/// `(source, commit)` pairs that no lock references. Returns
/// `(entries removed, bytes freed)`.
///
/// ponytail: the store is global (shared across projects), but this prunes
/// against the *current* project's lock only. A commit used by another project
/// is re-fetched on their next install (cache miss, not data loss). A true
/// global GC would have to scan every project's lock.
pub fn prune(keep: &HashSet<(String, String)>) -> Result<(u64, u64)> {
    let root = store_root();
    if !root.exists() {
        return Ok((0, 0));
    }

    let keep_commits: HashSet<(String, String)> = keep
        .iter()
        .map(|(source, commit)| (sha256_hex(source), commit.clone()))
        .collect();
    let keep_sources: HashSet<String> = keep_commits.iter().map(|(s, _)| s.clone()).collect();

    let mut removed = 0u64;
    let mut freed = 0u64;

    for entry in fs::read_dir(&root).context("reading store")? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let source_dir = entry.path();
        let hash = entry.file_name().to_string_lossy().into_owned();

        if !keep_sources.contains(&hash) {
            freed += dir_size(&source_dir);
            removed += 1;
            fs::remove_dir_all(&source_dir)
                .with_context(|| format!("removing {}", source_dir.display()))?;
            continue;
        }

        for child in fs::read_dir(&source_dir)? {
            let child = child?;
            let name = child.file_name().to_string_lossy().into_owned();
            if name == "repo.git" || !child.file_type()?.is_dir() {
                continue;
            }
            if !keep_commits.contains(&(hash.clone(), name)) {
                freed += dir_size(&child.path());
                removed += 1;
                fs::remove_dir_all(child.path())
                    .with_context(|| format!("removing {}", child.path().display()))?;
            }
        }
    }

    Ok((removed, freed))
}

fn dir_size(path: &Path) -> u64 {
    WalkDir::new(path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .map(|e| e.metadata().map(|m| m.len()).unwrap_or(0))
        .sum()
}
