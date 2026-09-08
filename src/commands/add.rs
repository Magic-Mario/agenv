use std::io::Write;

use anyhow::Result;

use crate::commands::manifest_path;
use crate::manifest::{is_git_source, is_local_source, validate_skill, Manifest, SkillSpec};
use crate::resolver;
use crate::store::redact;

struct Inferred {
    source: String,
    r#ref: Option<String>,
    path: Option<String>,
    name: Option<String>,
}

pub struct Meta {
    pub description: Option<String>,
    pub license: Option<String>,
    pub version: Option<String>,
    pub homepage: Option<String>,
}

pub fn run(
    name: Option<&str>,
    source: &str,
    reference: Option<&str>,
    path: Option<&str>,
    meta: Meta,
) -> Result<()> {
    let manifest_path = manifest_path();
    if !manifest_path.exists() {
        anyhow::bail!("no agenv.toml found; run `agenv init` first");
    }

    let inf = parse_source(source);

    let local = is_local_source(&inf.source);
    if !local && !is_git_source(&inf.source) {
        anyhow::bail!(
            "source '{}' is not a git repository or existing directory",
            redact(&inf.source)
        );
    }

    let path = if local {
        None
    } else {
        path.map(String::from).or(inf.path.clone())
    };

    let name = match name {
        Some(n) => n.to_string(),
        None => match path
            .as_deref()
            .and_then(last_component)
            .or_else(|| inf.name.clone())
        {
            Some(n) => n,
            None => prompt_name()?,
        },
    };

    let r#ref = if local {
        reference.map(String::from).unwrap_or_default()
    } else {
        match reference {
            Some(r) => r.to_string(),
            None => match inf.r#ref.clone() {
                Some(r) => r,
                None => resolver::default_branch(&inf.source)?,
            },
        }
    };

    let spec = SkillSpec {
        source: inf.source.clone(),
        r#ref: r#ref.clone(),
        path: path.clone(),
        description: meta.description,
        license: meta.license,
        version: meta.version,
        homepage: meta.homepage,
    };
    validate_skill(&name, &spec)?;

    let mut manifest = Manifest::load(&manifest_path)?;
    manifest.skills.insert(name.clone(), spec);
    manifest.save(&manifest_path)?;

    match path {
        Some(p) => println!("added {name} ({} @ {ref}, path {p})", redact(&inf.source)),
        None => println!("added {name} ({} @ {ref})", redact(&inf.source)),
    }
    Ok(())
}

fn parse_source(input: &str) -> Inferred {
    let clean = input.split(['?', '#']).next().unwrap_or(input);

    for marker in ["/blob/", "/tree/"] {
        let Some(idx) = clean.find(marker) else {
            continue;
        };
        let repo = &clean[..idx];
        let rest = &clean[idx + marker.len()..];
        let (r, subpath) = match rest.split_once('/') {
            Some((r, p)) => (r, p),
            None => (rest, ""),
        };
        let subpath = if marker == "/blob/" {
            subpath.strip_suffix("/SKILL.md").unwrap_or(subpath)
        } else {
            subpath
        }
        .trim_end_matches('/');
        let name = last_component(subpath).or_else(|| repo_basename(repo));
        return Inferred {
            source: repo.to_string(),
            r#ref: (!r.is_empty()).then(|| r.to_string()),
            path: (!subpath.is_empty()).then(|| subpath.to_string()),
            name,
        };
    }

    Inferred {
        source: clean.to_string(),
        r#ref: None,
        path: None,
        name: repo_basename(clean),
    }
}

fn last_component(path: &str) -> Option<String> {
    path.rsplit('/')
        .next()
        .filter(|s| !s.is_empty())
        .map(String::from)
}

fn repo_basename(url: &str) -> Option<String> {
    url.trim_end_matches('/')
        .trim_end_matches(".git")
        .rsplit(['/', '\\', ':'])
        .next()
        .filter(|s| !s.is_empty())
        .map(String::from)
}

fn prompt_name() -> Result<String> {
    eprint!("couldn't infer the skill name from the source; enter a name: ");
    std::io::stderr().flush().ok();
    let mut line = String::new();
    std::io::stdin().read_line(&mut line)?;
    let name = line.trim().to_string();
    if name.is_empty() {
        anyhow::bail!("a skill name is required");
    }
    Ok(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blob_url_infers_name_ref_path() {
        let inf = parse_source(
            "https://github.com/mattpocock/skills/blob/main/skills/engineering/grill-with-docs/SKILL.md",
        );
        assert_eq!(inf.source, "https://github.com/mattpocock/skills");
        assert_eq!(inf.r#ref.as_deref(), Some("main"));
        assert_eq!(
            inf.path.as_deref(),
            Some("skills/engineering/grill-with-docs")
        );
        assert_eq!(inf.name.as_deref(), Some("grill-with-docs"));
    }

    #[test]
    fn blob_url_strips_query() {
        let inf = parse_source(
            "https://github.com/mattpocock/skills/blob/main/skills/engineering/grill-with-docs/SKILL.md?plain=1",
        );
        assert_eq!(
            inf.path.as_deref(),
            Some("skills/engineering/grill-with-docs")
        );
        assert_eq!(inf.name.as_deref(), Some("grill-with-docs"));
    }

    #[test]
    fn tree_url_infers_name_path() {
        let inf = parse_source(
            "https://github.com/mattpocock/skills/tree/main/skills/engineering/grill-with-docs",
        );
        assert_eq!(inf.r#ref.as_deref(), Some("main"));
        assert_eq!(
            inf.path.as_deref(),
            Some("skills/engineering/grill-with-docs")
        );
        assert_eq!(inf.name.as_deref(), Some("grill-with-docs"));
    }

    #[test]
    fn plain_repo_infers_name_only() {
        let inf = parse_source("https://github.com/acme/skills");
        assert_eq!(inf.source, "https://github.com/acme/skills");
        assert_eq!(inf.r#ref, None);
        assert_eq!(inf.path, None);
        assert_eq!(inf.name.as_deref(), Some("skills"));
    }

    #[test]
    fn scp_repo_strips_git_suffix() {
        let inf = parse_source("git@github.com:acme/skills.git");
        assert_eq!(inf.name.as_deref(), Some("skills"));
        assert_eq!(inf.path, None);
    }

    #[test]
    fn windows_path_infers_repo_name() {
        let inf = parse_source(r"C:\Users\me\repos\grill-skill");
        assert_eq!(inf.name.as_deref(), Some("grill-skill"));
        assert_eq!(inf.path, None);
    }
}
