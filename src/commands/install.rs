use anyhow::Result;

use crate::commands::{lock_path, manifest_path, materialize_all};
use crate::lock::{Lock, LockedSkill};
use crate::manifest::{validate_skill, Manifest};
use crate::resolver;

pub fn run() -> Result<()> {
    let manifest = Manifest::load(&manifest_path())?;
    let mut lock = Lock::load(&lock_path())?.unwrap_or(Lock {
        version: 1,
        skills: Vec::new(),
    });

    let mut resolved: Vec<(String, std::path::PathBuf)> = Vec::new();

    for (name, spec) in &manifest.skills {
        validate_skill(name, spec)?;
        let locked = lock.get(name);
        let frozen = locked
            .filter(|l| l.source == spec.source && l.r#ref == spec.r#ref && l.path == spec.path);

        let (entry, tree) = match frozen {
            Some(l) => {
                let r = resolver::checkout(&l.source, &l.commit, l.path.as_deref())?;
                if r.checksum != l.checksum {
                    anyhow::bail!(
                        "checksum mismatch for '{name}' at commit {} (expected {}, got {}); \
                         stored content is corrupted",
                        l.commit,
                        l.checksum,
                        r.checksum
                    );
                }
                (l.clone(), r.tree)
            }
            None => {
                let r = resolver::resolve_tree(&spec.source, &spec.r#ref, spec.path.as_deref())?;
                (
                    LockedSkill {
                        name: name.clone(),
                        source: spec.source.clone(),
                        r#ref: spec.r#ref.clone(),
                        path: spec.path.clone(),
                        commit: r.commit,
                        checksum: r.checksum,
                    },
                    r.tree,
                )
            }
        };

        lock.upsert(entry);
        resolved.push((name.clone(), tree));
    }

    // Drop skills that were removed from the manifest.
    lock.skills
        .retain(|s| manifest.skills.contains_key(&s.name));
    lock.save(&lock_path())?;

    materialize_all(&resolved, "installed")?;
    println!("installed {} skills", resolved.len());
    Ok(())
}
