use anyhow::Result;

use crate::checksum;
use crate::commands::{lock_path, manifest_path, materialize_all};
use crate::lock::{Lock, LockedSkill};
use crate::manifest::{validate_skill, Manifest};
use crate::{resolver, store};

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
        let frozen = locked.filter(|l| l.source == spec.source && l.r#ref == spec.r#ref);

        let (entry, tree) = match frozen {
            Some(l) => {
                let tree = store::checkout_tree(&l.source, &l.commit)?;
                let actual = checksum::tree_checksum(&tree)?;
                if actual != l.checksum {
                    anyhow::bail!(
                        "checksum mismatch for '{name}' at commit {} (expected {}, got {actual}); \
                         stored content is corrupted",
                        l.commit,
                        l.checksum
                    );
                }
                (l.clone(), tree)
            }
            None => {
                let r = resolver::resolve_tree(&spec.source, &spec.r#ref)?;
                (
                    LockedSkill {
                        name: name.clone(),
                        source: spec.source.clone(),
                        r#ref: spec.r#ref.clone(),
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
