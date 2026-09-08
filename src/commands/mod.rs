pub mod add;
pub mod harness;
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
use crate::manifest::{
    is_local_source, validate_harness, validate_skill, Manifest, HARNESSES, SKILLS_DIR,
};
use crate::resolver;

pub fn manifest_path() -> PathBuf {
    PathBuf::from("agenv.toml")
}

/// Append `entry` to `.gitignore` if not already present (idempotent).
pub fn add_gitignore_entry(gitignore: &Path, entry: &str) -> Result<()> {
    if !gitignore.exists() {
        fs::write(gitignore, format!("{entry}\n")).context("writing .gitignore")?;
        return Ok(());
    }
    let content = fs::read_to_string(gitignore).context("reading .gitignore")?;
    if content.lines().any(|l| l.trim() == entry) {
        return Ok(());
    }
    let mut out = content;
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out.push_str(&format!("{entry}\n"));
    fs::write(gitignore, out).context("writing .gitignore")?;
    Ok(())
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

/// Refuse to operate through a symlinked path component (a shared/malicious
/// repo could point `base`, `skills`, or a local source elsewhere and turn
/// materialize/prune/vendor into writes/deletes/reads outside the project).
fn ensure_not_symlink(path: &Path) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }
    let meta =
        fs::symlink_metadata(path).with_context(|| format!("checking {}", path.display()))?;
    if meta.file_type().is_symlink() {
        anyhow::bail!("refusing to operate through symlink {}", path.display());
    }
    Ok(())
}

/// The `<base>/skills` directory for a harness, refusing to operate through a
/// symlinked `base` or `skills` component.
pub fn checked_skills_dir(harness: &str) -> Result<PathBuf> {
    let base = PathBuf::from(harness_base(harness)?);
    let skills = base.join("skills");
    ensure_not_symlink(&base)?;
    ensure_not_symlink(&skills)?;
    Ok(skills)
}

pub fn skill_dir(harness: &str, name: &str) -> Result<PathBuf> {
    Ok(PathBuf::from(harness_base(harness)?)
        .join("skills")
        .join(name))
}

/// The project's local skills source dir (`skills/`), refusing to operate
/// through a symlinked `skills/` component.
pub fn checked_local_skills_dir() -> Result<PathBuf> {
    let dir = PathBuf::from(SKILLS_DIR);
    ensure_not_symlink(&dir)?;
    Ok(dir)
}

/// Resolve a local skill source to a path, refusing any symlinked path component
/// (so a committed `skills -> /etc` link can't make materialize read outside).
pub fn checked_local_source(source: &str) -> Result<PathBuf> {
    let path = PathBuf::from(source);
    let mut cur = PathBuf::new();
    for comp in path.components() {
        cur.push(comp.as_os_str());
        ensure_not_symlink(&cur)?;
    }
    Ok(path)
}

/// Copy a skill directory into `skills/<name>/` (vendor it into the project).
pub fn vendor_skill(from: &Path, name: &str) -> Result<()> {
    let base = checked_local_skills_dir()?;
    let dest = base.join(name);
    ensure_not_symlink(&dest)?;
    if dest.exists() {
        fs::remove_dir_all(&dest).with_context(|| format!("removing {}", dest.display()))?;
    }
    fs::create_dir_all(&base).with_context(|| format!("creating {}", base.display()))?;
    copy_tree(from, &dest)
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

    let mut lock = Lock::load(&lock_path())?.unwrap_or_default();

    let mut resolved: Vec<(String, PathBuf)> = Vec::new();

    for (name, spec) in &manifest.skills {
        validate_skill(name, spec)?;
        if is_local_source(&spec.source) {
            lock.remove(name);
            resolved.push((name.clone(), checked_local_source(&spec.source)?));
            continue;
        }
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
