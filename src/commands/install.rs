use std::path::Path;

use anyhow::{Context, Result};

use crate::commands::add::Meta;
use crate::commands::{lock_path, manifest_path, materialize_all, resolve_manifest};
use crate::manifest::Manifest;

pub fn run(requirements: Option<&Path>) -> Result<()> {
    if let Some(file) = requirements {
        add_from_requirements(file)?;
    }

    let manifest = Manifest::load(&manifest_path())?;

    let (mut lock, resolved) = resolve_manifest(&manifest)?;
    lock.save(&lock_path())?;

    materialize_all(&resolved, &manifest.harness, "installed")?;
    println!("installed {} skills", resolved.len());
    Ok(())
}

fn add_from_requirements(file: &Path) -> Result<()> {
    let text =
        std::fs::read_to_string(file).with_context(|| format!("reading {}", file.display()))?;
    for line in text.lines() {
        let source = line.trim();
        if source.is_empty() || source.starts_with('#') {
            continue;
        }
        crate::commands::add::run(
            None,
            source,
            None,
            None,
            Meta {
                description: None,
                license: None,
                version: None,
                homepage: None,
            },
        )?;
    }
    Ok(())
}
