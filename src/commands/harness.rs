use std::path::Path;

use anyhow::Result;

use crate::commands::{add_gitignore_entry, harness_base, manifest_path};
use crate::manifest::{supported_harnesses, Manifest, HARNESSES};

pub fn run_add(name: &str) -> Result<()> {
    if !HARNESSES.iter().any(|(n, _)| *n == name) {
        anyhow::bail!(
            "unknown harness '{name}' (supported: {})",
            supported_harnesses()
        );
    }

    let path = manifest_path();
    if !path.exists() {
        anyhow::bail!("agenv.toml not found — run `agenv init` first");
    }
    let mut manifest = Manifest::load(&path)?;

    if manifest.harness.iter().any(|h| h == name) {
        println!("harness '{name}' already active");
    } else {
        manifest.harness.push(name.to_string());
        manifest.save(&path)?;
        println!("added harness '{name}'");
    }

    add_gitignore_entry(
        Path::new(".gitignore"),
        &format!("{}/skills/", harness_base(name)?),
    )?;
    Ok(())
}
