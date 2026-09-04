pub mod add;
pub mod init;
pub mod install;
pub mod status;
pub mod sync;
pub mod update;

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use walkdir::WalkDir;

use crate::lock::{Lock, LockedSkill};
use crate::manifest::{validate_harness, validate_skill, Manifest, HARNESSES};
use crate::resolver;

pub fn manifest_path() -> PathBuf {
    PathBuf::from("agenv.toml")
}

pub fn lock_path() -> PathBuf {
    PathBuf::from("agenv.lock")
}

pub fn harness_base(harness: &str) -> Result<&'static str> {
    HARNESSES
        .iter()
        .find(|(name, _)| *name == harness)
        .map(|(_, base)| *base)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "unknown harness '{harness}' (supported: {})",
                crate::manifest::supported_harnesses()
            )
        })
}

/// The `<base>/skills` directory for a harness, refusing to operate through a
/// symlinked `base` or `skills` component (a shared/malicious repo could point
/// them elsewhere and turn materialize/prune into writes/deletes outside the
/// project).
pub fn checked_skills_dir(harness: &str) -> Result<PathBuf> {
    let base = PathBuf::from(harness_base(harness)?);
    let skills = base.join("skills");
    for dir in [&base, &skills] {
        if dir.exists() {
            let meta =
                fs::symlink_metadata(dir).with_context(|| format!("checking {}", dir.display()))?;
            if meta.file_type().is_symlink() {
                anyhow::bail!("refusing to operate through symlink {}", dir.display());
            }
        }
    }
    Ok(skills)
}

pub fn skill_dir(harness: &str, name: &str) -> Result<PathBuf> {
    Ok(PathBuf::from(harness_base(harness)?)
        .join("skills")
        .join(name))
}

pub fn materialize(tree: &Path, harness: &str, name: &str) -> Result<()> {
    let skills_dir = checked_skills_dir(harness)?;
    let dest = skills_dir.join(name);
    let parent = &skills_dir;
    fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;

    let tmp = parent.join(format!(".{}.tmp-{}", name, std::process::id()));
    if tmp.exists() {
        fs::remove_dir_all(&tmp).with_context(|| format!("removing {}", tmp.display()))?;
    }
    fs::create_dir_all(&tmp).with_context(|| format!("creating {}", tmp.display()))?;
    copy_tree(tree, &tmp)?;

    if dest.exists() {
        fs::remove_dir_all(&dest).with_context(|| format!("removing {}", dest.display()))?;
    }
    fs::rename(&tmp, &dest).with_context(|| format!("moving {} into place", dest.display()))?;
    Ok(())
}

pub fn materialize_all(
    items: &[(String, PathBuf)],
    harnesses: &[String],
    verb: &str,
) -> Result<()> {
    for (name, tree) in items {
        for harness in harnesses {
            materialize(tree, harness, name)?;
        }
        println!("{verb} {name}");
    }
    Ok(())
}

/// Resolve the manifest into lock entries + store trees, and return the updated
/// lock. Does not save the lock or materialize; shared by `install` and `sync`.
pub fn resolve_manifest(manifest: &Manifest) -> Result<(Lock, Vec<(String, PathBuf)>)> {
    validate_harness(&manifest.harness)?;

    let mut lock = Lock::load(&lock_path())?.unwrap_or(Lock {
        version: 1,
        skills: Vec::new(),
    });

    let mut resolved: Vec<(String, PathBuf)> = Vec::new();

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

    Ok((lock, resolved))
}

fn copy_tree(src: &Path, dest: &Path) -> Result<()> {
    for entry in WalkDir::new(src)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| e.file_name() != ".git")
        .filter_map(|e| e.ok())
    {
        let rel = entry.path().strip_prefix(src).unwrap();
        let target = dest.join(rel);
        if entry.file_type().is_dir() {
            fs::create_dir_all(&target)?;
        } else if entry.file_type().is_file() {
            fs::copy(entry.path(), &target)
                .with_context(|| format!("copying {}", entry.path().display()))?;
        }
    }
    Ok(())
}
