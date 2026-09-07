use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct Lock {
    pub version: u32,
    #[serde(default)]
    pub skills: Vec<LockedSkill>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct LockedSkill {
    pub name: String,
    pub source: String,
    #[serde(rename = "ref")]
    pub r#ref: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    pub commit: String,
    pub checksum: String,
}

impl Lock {
    pub fn load(path: &Path) -> Result<Option<Lock>> {
        if !path.exists() {
            return Ok(None);
        }
        let text =
            fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        let lock = toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))?;
        Ok(Some(lock))
    }

    pub fn save(&mut self, path: &Path) -> Result<()> {
        self.skills.sort_by(|a, b| a.name.cmp(&b.name));
        let text = toml::to_string_pretty(self).context("serializing lock")?;
        fs::write(path, text).with_context(|| format!("writing {}", path.display()))
    }

    pub fn get(&self, name: &str) -> Option<&LockedSkill> {
        self.skills.iter().find(|s| s.name == name)
    }

    pub fn upsert(&mut self, skill: LockedSkill) {
        match self.skills.iter_mut().find(|s| s.name == skill.name) {
            Some(existing) => *existing = skill,
            None => self.skills.push(skill),
        }
    }

    pub fn remove(&mut self, name: &str) {
        self.skills.retain(|s| s.name != name);
    }
}

impl Default for Lock {
    fn default() -> Self {
        Lock {
            version: 1,
            skills: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn tmp_lock_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("agenv-test-{}-{}.lock", name, std::process::id()))
    }

    fn locked(name: &str) -> LockedSkill {
        LockedSkill {
            name: name.to_string(),
            source: "https://github.com/acme/skills".to_string(),
            r#ref: "v1.2.0".to_string(),
            path: None,
            commit: "f3c2a1b9".to_string(),
            checksum: "sha256:abcd".to_string(),
        }
    }

    #[test]
    fn round_trip() {
        let mut lock = Lock {
            version: 1,
            skills: vec![locked("b"), locked("a")],
        };
        let path = tmp_lock_path("roundtrip");
        lock.save(&path).unwrap();
        let loaded = Lock::load(&path).unwrap().unwrap();
        assert_eq!(lock, loaded);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn saved_sorted_by_name() {
        let mut lock = Lock {
            version: 1,
            skills: vec![locked("zeta"), locked("alpha")],
        };
        let path = tmp_lock_path("sorted");
        lock.save(&path).unwrap();
        let text = fs::read_to_string(&path).unwrap();
        let alpha = text.find("alpha").unwrap();
        let zeta = text.find("zeta").unwrap();
        assert!(alpha < zeta);
        let _ = std::fs::remove_file(&path);
    }
}
