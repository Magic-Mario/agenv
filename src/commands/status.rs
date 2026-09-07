use anyhow::Result;

use crate::checksum;
use crate::commands::{lock_path, manifest_path, skill_dir};
use crate::lock::{Lock, LockedSkill};
use crate::manifest::{is_local_source, validate_harness, Manifest};

pub fn run() -> Result<()> {
    let manifest = Manifest::load(&manifest_path())?;
    validate_harness(&manifest.harness)?;
    let lock = Lock::load(&lock_path())?;

    let mut drift = false;

    let locked: &[LockedSkill] = lock.as_ref().map(|l| l.skills.as_slice()).unwrap_or(&[]);

    let mut lock_names: Vec<&str> = locked.iter().map(|s| s.name.as_str()).collect();
    lock_names.sort_unstable();

    for (name, spec) in &manifest.skills {
        if is_local_source(&spec.source) {
            for harness in &manifest.harness {
                let dir = skill_dir(harness, name)?;
                if !dir.exists() {
                    println!("missing: {name} ({harness} not materialized)");
                    drift = true;
                }
            }
            continue;
        }
        match locked.iter().find(|s| s.name == *name) {
            None => {
                println!("added:   {name}");
                drift = true;
            }
            Some(l) if l.source != spec.source || l.r#ref != spec.r#ref || l.path != spec.path => {
                println!(
                    "ref-changed: {name} ({}@{} -> {}@{})",
                    l.source, l.r#ref, spec.source, spec.r#ref
                );
                drift = true;
            }
            Some(l) => {
                for harness in &manifest.harness {
                    let dir = skill_dir(harness, name)?;
                    if !dir.exists() {
                        println!("missing: {name} ({harness} not materialized)");
                        drift = true;
                    } else if checksum::tree_checksum(&dir)? != l.checksum {
                        println!("checksum-mismatch: {name} ({harness})");
                        drift = true;
                    }
                }
            }
        }
    }

    for name in lock_names {
        if !manifest.skills.contains_key(name) {
            println!("removed: {name}");
            drift = true;
        }
    }

    if drift {
        anyhow::bail!("drift detected");
    }
    println!("clean");
    Ok(())
}
