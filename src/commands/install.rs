use anyhow::Result;

use crate::commands::{lock_path, manifest_path, materialize_all, resolve_manifest};
use crate::manifest::Manifest;

pub fn run() -> Result<()> {
    let manifest = Manifest::load(&manifest_path())?;

    let (mut lock, resolved) = resolve_manifest(&manifest)?;
    lock.save(&lock_path())?;

    materialize_all(&resolved, &manifest.harness, "installed")?;
    println!("installed {} skills", resolved.len());
    Ok(())
}
