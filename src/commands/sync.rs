use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};

use crate::commands::{
    checked_local_skills_dir, checked_skills_dir, lock_path, manifest_path, materialize_all,
    resolve_manifest, vendor_skill,
};
use crate::lock::Lock;
use crate::manifest::{is_safe_name, local_source, Manifest, SkillSpec};

pub fn run(prune: bool) -> Result<()> {
    let mut manifest = Manifest::load(&manifest_path())?;

    if !prune {
        let adopted = adopt(&mut manifest)?;
        if adopted > 0 {
            manifest.save(&manifest_path())?;
        }
    }

    let (mut lock, resolved) = resolve_manifest(&manifest)?;
    lock.save(&lock_path())?;

    materialize_all(&resolved, &manifest.harness, "synced")?;

    if prune {
        prune_stale(&manifest)?;
    }

    println!("synced {} skills", resolved.len());
    Ok(())
}

/// Add skills found on disk (harness dirs + `skills/`) but missing from the
/// manifest. Source recovery order: lock entry (git) -> existing `skills/<name>`
/// -> vendor a copy from a harness dir. Never writes an absolute path.
fn adopt(manifest: &mut Manifest) -> Result<usize> {
    let lock = Lock::load(&lock_path())?.unwrap_or_default();

    let mut discovered: BTreeSet<String> = BTreeSet::new();

    for harness in &manifest.harness {
        let skills_dir = checked_skills_dir(harness)?;
        if !skills_dir.exists() {
            continue;
        }
        for entry in fs::read_dir(&skills_dir)
            .with_context(|| format!("reading {}", skills_dir.display()))?
        {
            let entry = entry.with_context(|| format!("reading {}", skills_dir.display()))?;
            if entry.file_type()?.is_dir() {
                discovered.insert(entry.file_name().to_string_lossy().into_owned());
            }
        }
    }

    let local_dir = checked_local_skills_dir()?;
    if local_dir.exists() {
        for entry in
            fs::read_dir(&local_dir).with_context(|| format!("reading {}", local_dir.display()))?
        {
            let entry = entry.with_context(|| format!("reading {}", local_dir.display()))?;
            if entry.file_type()?.is_dir() {
                discovered.insert(entry.file_name().to_string_lossy().into_owned());
            }
        }
    }

    let mut added = 0;
    for name in discovered {
        if !is_safe_name(&name) {
            eprintln!("skipping unsafe skill name '{name}'");
            continue;
        }
        if manifest.skills.contains_key(&name) {
            continue;
        }
        let spec = if let Some(l) = lock.get(&name) {
            SkillSpec {
                source: l.source.clone(),
                r#ref: l.r#ref.clone(),
                path: l.path.clone(),
                ..Default::default()
            }
        } else if local_dir.join(&name).is_dir() {
            SkillSpec {
                source: local_source(&name),
                r#ref: String::new(),
                path: None,
                ..Default::default()
            }
        } else {
            let src = harness_dir_with(&manifest.harness, &name)?
                .ok_or_else(|| anyhow::anyhow!("skill '{name}' not found on disk"))?;
            vendor_skill(&src, &name)?;
            println!("vendored {name}");
            SkillSpec {
                source: local_source(&name),
                r#ref: String::new(),
                path: None,
                ..Default::default()
            }
        };
        println!("adopted {name}");
        manifest.skills.insert(name.clone(), spec);
        added += 1;
    }
    Ok(added)
}

fn harness_dir_with(harnesses: &[String], name: &str) -> Result<Option<PathBuf>> {
    for harness in harnesses {
        let dir = checked_skills_dir(harness)?.join(name);
        if dir.is_dir() {
            return Ok(Some(dir));
        }
    }
    Ok(None)
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
