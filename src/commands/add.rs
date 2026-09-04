use anyhow::Result;

use crate::commands::manifest_path;
use crate::manifest::{validate_skill, Manifest, SkillSpec};

pub fn run(name: &str, source: &str, reference: &str) -> Result<()> {
    let path = manifest_path();
    if !path.exists() {
        anyhow::bail!("no agenv.toml found; run `agenv init` first");
    }

    let spec = SkillSpec {
        source: source.to_string(),
        r#ref: reference.to_string(),
    };
    validate_skill(name, &spec)?;

    let mut manifest = Manifest::load(&path)?;
    manifest.skills.insert(name.to_string(), spec);
    manifest.save(&path)?;

    println!("added {name} ({} @ {})", redact(source), reference);
    Ok(())
}

fn redact(url: &str) -> String {
    match url.find("://") {
        Some(i) => match url[i + 3..].find('@') {
            Some(at) => format!("{}://{}", &url[..i], &url[i + 3 + at + 1..]),
            None => url.to_string(),
        },
        None => url.to_string(),
    }
}
