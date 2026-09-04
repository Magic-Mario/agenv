use std::fs;
use std::path::PathBuf;
use std::process::Command;

use anyhow::{Context, Result};

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
        anyhow::bail!("git clone failed for {source}");
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
        anyhow::bail!("git fetch failed for {source}");
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
