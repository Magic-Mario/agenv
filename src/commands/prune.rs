use std::collections::HashSet;

use anyhow::Result;

use crate::commands::lock_path;
use crate::lock::Lock;
use crate::store;

pub fn run() -> Result<()> {
    let lock = Lock::load(&lock_path())?.unwrap_or_default();
    let keep: HashSet<(String, String)> = lock
        .skills
        .iter()
        .map(|s| (s.source.clone(), s.commit.clone()))
        .collect();

    let (removed, freed) = store::prune(&keep)?;
    println!("pruned {removed} unreferenced entries ({freed} bytes freed)");
    Ok(())
}
