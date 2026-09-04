use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

use crate::commands::manifest_path;
use crate::manifest::{Manifest, SkillSpec};

pub fn run() -> Result<()> {
    let path = manifest_path();
    if path.exists() {
        anyhow::bail!("agenv.toml already exists");
    }

    let name = std::env::current_dir()
        .ok()
        .and_then(|d| d.file_name().map(|n| n.to_string_lossy().into_owned()))
        .unwrap_or_else(|| "project".to_string());

    let manifest = Manifest {
        name,
        harness: "claude-code".to_string(),
        skills: BTreeMap::<String, SkillSpec>::new(),
    };
    manifest.save(&path)?;

    let gitignore = Path::new(".gitignore");
    let entry = ".claude/skills/";
    if !gitignore.exists() {
        fs::write(gitignore, format!("{entry}\n")).context("writing .gitignore")?;
    } else {
        let content = fs::read_to_string(gitignore).context("reading .gitignore")?;
        if !content.lines().any(|l| l.trim() == entry) {
            let mut out = content;
            if !out.ends_with('\n') {
                out.push('\n');
            }
            out.push_str(&format!("{entry}\n"));
            fs::write(gitignore, out).context("writing .gitignore")?;
        }
    }

    println!("created agenv.toml ({} skills)", 0);
    Ok(())
}
