use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

use crate::commands::{add_gitignore_entry, harness_base, manifest_path};
use crate::manifest::{validate_harness, Manifest, SkillSpec};

pub fn run(harnesses: &[String]) -> Result<()> {
    let path = manifest_path();
    if path.exists() {
        anyhow::bail!("agenv.toml already exists");
    }

    let name = std::env::current_dir()
        .ok()
        .and_then(|d| d.file_name().map(|n| n.to_string_lossy().into_owned()))
        .unwrap_or_else(|| "project".to_string());

    let harness = if harnesses.is_empty() {
        vec!["claude-code".to_string()]
    } else {
        harnesses.to_vec()
    };
    validate_harness(&harness)?;

    let manifest = Manifest {
        name,
        harness,
        skills: BTreeMap::<String, SkillSpec>::new(),
    };
    manifest.save(&path)?;

    let gitignore = Path::new(".gitignore");
    for harness in &manifest.harness {
        add_gitignore_entry(gitignore, &format!("{}/skills/", harness_base(harness)?))?;
        let skills = Path::new(harness_base(harness)?).join("skills");
        fs::create_dir_all(&skills).with_context(|| format!("creating {}", skills.display()))?;
    }

    println!("created agenv.toml ({} skills)", 0);
    Ok(())
}
