use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use walkdir::WalkDir;

pub fn tree_checksum(root: &Path) -> Result<String> {
    let mut paths: Vec<String> = Vec::new();
    let mut files: Vec<Vec<u8>> = Vec::new();
    for entry in WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| e.file_name() != ".git")
        .filter_map(|e| e.ok())
    {
        if !entry.file_type().is_file() {
            continue;
        }
        let rel = entry.path().strip_prefix(root).unwrap();
        let rel_str = rel
            .components()
            .map(|c| c.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/");
        files.push(
            fs::read(entry.path())
                .with_context(|| format!("reading {}", entry.path().display()))?,
        );
        paths.push(rel_str);
    }

    let mut order: Vec<usize> = (0..paths.len()).collect();
    order.sort_by(|&a, &b| paths[a].cmp(&paths[b]));

    let mut hasher = Sha256::new();
    for i in order {
        let p = &paths[i];
        let bytes = &files[i];
        hasher.update((p.len() as u64).to_le_bytes());
        hasher.update(p.as_bytes());
        hasher.update((bytes.len() as u64).to_le_bytes());
        hasher.update(bytes);
    }
    Ok(format!("sha256:{:x}", hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(root: &Path) {
        fs::create_dir_all(root.join("sub")).unwrap();
        fs::write(root.join("a.txt"), "alpha").unwrap();
        fs::write(root.join("sub/b.txt"), "beta").unwrap();
        fs::create_dir_all(root.join(".git")).unwrap();
        fs::write(root.join(".git/config"), "not relevant").unwrap();
    }

    #[test]
    fn deterministic_and_ignores_git() {
        let dir = std::env::temp_dir().join(format!("agenv-checksum-{}", std::process::id()));
        let root = dir.join("tree");
        fixture(&root);

        let with_git = tree_checksum(&root).unwrap();

        fs::remove_dir_all(root.join(".git")).unwrap();
        let without_git = tree_checksum(&root).unwrap();

        assert_eq!(with_git, without_git, ".git must be excluded");

        let again = tree_checksum(&root).unwrap();
        assert_eq!(without_git, again, "checksum must be stable");

        let _ = fs::remove_dir_all(&dir);
    }
}
