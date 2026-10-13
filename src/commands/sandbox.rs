use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use serde::Serialize;

use crate::commands::{ensure_not_symlink, manifest_path};
use crate::manifest::{duration_seconds, validate_sandbox, Manifest, SandboxSpec};

/// `image` is the schema's `ImageSpec`, not a bare string: `{ "uri": "…" }`.
#[derive(Serialize)]
struct ImageSpec {
    uri: String,
}

/// The OpenSandbox `CreateSandboxRequest` wire shape (camelCase), carrying what
/// `[sandbox]` declares. Field order is this declaration order, so the document
/// is byte-stable without a `preserve_order` feature.
#[derive(Serialize)]
struct CreateSandboxRequest {
    image: ImageSpec,
    timeout: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    env: Option<BTreeMap<String, String>>,
    #[serde(rename = "resourceLimits", skip_serializing_if = "Option::is_none")]
    resource_limits: Option<BTreeMap<String, String>>,
    #[serde(rename = "networkPolicy")]
    network_policy: NetworkPolicy,
    #[serde(rename = "credentialProxy", skip_serializing_if = "Option::is_none")]
    credential_proxy: Option<CredentialProxy>,
}

#[derive(Serialize)]
struct NetworkPolicy {
    #[serde(rename = "defaultAction")]
    default_action: String,
    egress: Vec<NetworkRule>,
}

#[derive(Serialize)]
struct NetworkRule {
    action: &'static str,
    target: String,
}

#[derive(Serialize)]
struct CredentialProxy {
    enabled: bool,
}

impl CreateSandboxRequest {
    fn from_spec(spec: &SandboxSpec) -> Result<Self> {
        // `validate_sandbox` already ran, so this only fails if the two disagree.
        let timeout = duration_seconds(&spec.timeout).ok_or_else(|| {
            anyhow::anyhow!("sandbox.timeout '{}' is not a duration", spec.timeout)
        })?;

        // Deny rules come first: the wire evaluates egress rules in order and the
        // first match wins, so a deny has to be able to shadow a broad allow.
        let mut egress = Vec::new();
        for target in &spec.network.deny {
            egress.push(NetworkRule {
                action: "deny",
                target: target.clone(),
            });
        }
        for target in &spec.network.allow {
            egress.push(NetworkRule {
                action: "allow",
                target: target.clone(),
            });
        }

        Ok(CreateSandboxRequest {
            image: ImageSpec {
                uri: spec.image.clone(),
            },
            timeout,
            // An indirection is emitted as the reference it declares — the value
            // it points at is resolved by whoever consumes this document.
            env: (!spec.env.is_empty()).then(|| spec.env.clone()),
            resource_limits: (!spec.resources.is_empty()).then(|| spec.resources.clone()),
            network_policy: NetworkPolicy {
                default_action: spec.network.default_action.clone(),
                egress,
            },
            credential_proxy: spec
                .credential_proxy
                .then_some(CredentialProxy { enabled: true }),
        })
    }
}

/// The document as it is written — stdout and `--out` produce the same bytes.
fn render_request(spec: &SandboxSpec) -> Result<String> {
    let mut json = serde_json::to_string_pretty(&CreateSandboxRequest::from_spec(spec)?)
        .context("serializing the request")?;
    json.push('\n');
    Ok(json)
}

pub fn run(out: Option<&Path>) -> Result<()> {
    let manifest = Manifest::load(&manifest_path())?;
    // This path reads the manifest and nothing else, so it does not go through
    // `resolve_manifest`; validating here is what makes R8 hold.
    validate_sandbox(&manifest.sandbox)?;
    let spec = manifest
        .sandbox
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("agenv.toml has no [sandbox] section to render"))?;

    let json = render_request(spec)?;

    match out {
        Some(path) => {
            // The path is operator-typed, but the file at it can come from the
            // repository (a committed symlink plus a script with a fixed `--out`),
            // and every other write in this crate refuses the same pattern.
            ensure_not_symlink(path)?;
            fs::write(path, json).with_context(|| format!("writing {}", path.display()))
        }
        None => {
            print!("{json}");
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::NetworkSpec;

    fn spec(image: &str) -> SandboxSpec {
        SandboxSpec {
            image: image.to_string(),
            ..Default::default()
        }
    }

    fn render(spec: &SandboxSpec) -> String {
        render_request(spec).unwrap()
    }

    #[test]
    fn a_minimal_sandbox_renders_exactly_this() {
        // Pins the whole document: field order, the seconds conversion, the
        // fail-closed policy, and what is left out.
        assert_eq!(
            render(&spec("alpine")),
            r#"{
  "image": {
    "uri": "alpine"
  },
  "timeout": 1800,
  "networkPolicy": {
    "defaultAction": "deny",
    "egress": []
  }
}
"#
        );
    }

    #[test]
    fn a_full_sandbox_renders_every_declared_field() {
        let mut full = spec("node:22-bookworm-slim");
        full.timeout = "90s".to_string();
        full.credential_proxy = true;
        full.resources = BTreeMap::from([
            ("cpu".to_string(), "1".to_string()),
            ("memory".to_string(), "2Gi".to_string()),
        ]);
        full.env = BTreeMap::from([(
            "ANTHROPIC_API_KEY".to_string(),
            "env:ANTHROPIC_API_KEY".to_string(),
        )]);
        full.network = NetworkSpec {
            default_action: "deny".to_string(),
            allow: vec!["api.anthropic.com".to_string()],
            deny: vec!["*.example.com".to_string()],
        };

        assert_eq!(
            render(&full),
            r#"{
  "image": {
    "uri": "node:22-bookworm-slim"
  },
  "timeout": 90,
  "env": {
    "ANTHROPIC_API_KEY": "env:ANTHROPIC_API_KEY"
  },
  "resourceLimits": {
    "cpu": "1",
    "memory": "2Gi"
  },
  "networkPolicy": {
    "defaultAction": "deny",
    "egress": [
      {
        "action": "deny",
        "target": "*.example.com"
      },
      {
        "action": "allow",
        "target": "api.anthropic.com"
      }
    ]
  },
  "credentialProxy": {
    "enabled": true
  }
}
"#
        );
    }

    #[test]
    fn a_deny_rule_precedes_an_allow_rule() {
        // The wire evaluates egress rules in order and the first match wins, so a
        // deny has to be able to shadow a broad allow. (The always-present policy
        // itself is pinned by the minimal-document test.)
        let mut both = spec("alpine");
        both.network = NetworkSpec {
            default_action: "allow".to_string(),
            allow: vec!["a.com".to_string()],
            deny: vec!["b.com".to_string()],
        };
        let rendered = render(&both);
        // Search the rules, not the bare words: `defaultAction` can be either.
        let deny_at = rendered.find("\"action\": \"deny\"").unwrap();
        let allow_at = rendered.find("\"action\": \"allow\"").unwrap();
        assert!(deny_at < allow_at, "deny must be emitted first: {rendered}");
    }
}
