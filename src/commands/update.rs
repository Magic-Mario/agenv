use anyhow::Result;

use crate::commands::{lock_path, manifest_path, materialize_all};
use crate::lock::{Lock, LockedSkill};
use crate::manifest::{validate_harness, validate_skill, Manifest};
use crate::resolver;

pub fn run(name: Option<&str>) -> Result<()> {
    let manifest = Manifest::load(&manifest_path())?;
    validate_harness(&manifest.harness)?;
    let mut lock = Lock::load(&lock_path())?.unwrap_or(Lock {
        version: 1,
        skills: Vec::new(),
    });

    let targets: Vec<String> = match name {
        Some(n) => {
            if !manifest.skills.contains_key(n) {
                anyhow::bail!("skill '{n}' is not in the manifest");
            }
            vec![n.to_string()]
        }
        None => manifest.skills.keys().cloned().collect(),
    };

    let mut updated: Vec<(String, std::path::PathBuf)> = Vec::new();
    for n in &targets {
        let spec = &manifest.skills[n];
        validate_skill(n, spec)?;
        let r = resolver::resolve_tree(&spec.source, &spec.r#ref, spec.path.as_deref())?;
        let entry = LockedSkill {
            name: n.clone(),
            source: spec.source.clone(),
            r#ref: spec.r#ref.clone(),
            path: spec.path.clone(),
            commit: r.commit,
            checksum: r.checksum,
        };
        lock.upsert(entry);
        updated.push((n.clone(), r.tree));
    }

    lock.skills
        .retain(|s| manifest.skills.contains_key(&s.name));
    lock.save(&lock_path())?;

    materialize_all(&updated, &manifest.harness, "updated")?;
    Ok(())
}
