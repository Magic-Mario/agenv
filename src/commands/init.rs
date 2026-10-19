use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

use crate::commands::{add_gitignore_entry, manifest_path};
use crate::manifest::{Manifest, SkillSpec, AGENTS_DIR};

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
        skills: BTreeMap::<String, SkillSpec>::new(),
    };
    manifest.save(&path)?;

    add_gitignore_entry(Path::new(".gitignore"), &format!("{AGENTS_DIR}/skills/"))?;
    let skills = Path::new(AGENTS_DIR).join("skills");
    fs::create_dir_all(&skills).with_context(|| format!("creating {}", skills.display()))?;

    println!("created agenv.toml ({} skills)", 0);
    Ok(())
}
