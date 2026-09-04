pub mod add;
pub mod init;
pub mod install;
pub mod status;
pub mod update;

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use walkdir::WalkDir;

pub fn manifest_path() -> PathBuf {
    PathBuf::from("agenv.toml")
}

pub fn lock_path() -> PathBuf {
    PathBuf::from("agenv.lock")
}

pub fn skill_dir(name: &str) -> PathBuf {
    PathBuf::from(".claude").join("skills").join(name)
}

pub fn materialize(tree: &Path, name: &str) -> Result<()> {
    let dest = skill_dir(name);
    let parent = dest.parent().unwrap();
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

pub fn materialize_all(items: &[(String, PathBuf)], verb: &str) -> Result<()> {
    for (name, tree) in items {
        materialize(tree, name)?;
        println!("{verb} {name}");
    }
    Ok(())
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
