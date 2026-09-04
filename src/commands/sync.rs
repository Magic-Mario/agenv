use std::fs;

use anyhow::{Context, Result};

use crate::commands::{
    checked_skills_dir, lock_path, manifest_path, materialize_all, resolve_manifest,
};
use crate::manifest::Manifest;

pub fn run() -> Result<()> {
    let manifest = Manifest::load(&manifest_path())?;

    let (mut lock, resolved) = resolve_manifest(&manifest)?;
    lock.save(&lock_path())?;

    materialize_all(&resolved, &manifest.harness, "synced")?;
    prune_stale(&manifest)?;

    println!("synced {} skills", resolved.len());
    Ok(())
}

fn prune_stale(manifest: &Manifest) -> Result<()> {
    for harness in &manifest.harness {
        let skills_dir = checked_skills_dir(harness)?;
        if !skills_dir.exists() {
            continue;
        }
        for entry in fs::read_dir(&skills_dir)
            .with_context(|| format!("reading {}", skills_dir.display()))?
        {
            let entry = entry.with_context(|| format!("reading {}", skills_dir.display()))?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().into_owned();
            // Deliberate: prune removes every directory under <base>/skills/ not
            // in the manifest, not just dirs agenv created. See spec R4.
            if !manifest.skills.contains_key(&name) {
                fs::remove_dir_all(entry.path())
                    .with_context(|| format!("removing {}", entry.path().display()))?;
                println!("removed {name} ({harness})");
            }
        }
    }
    Ok(())
}
