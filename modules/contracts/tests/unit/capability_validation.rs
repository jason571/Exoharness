use super::*;
use crate::capability::{CapabilityScope, CapabilitySource, SecretReference, SkillSpec};

fn skill_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: MANIFEST_SCHEMA_VERSION,
        name: "docs-review".into(),
        version: "1.0.0".into(),
        description: "Review project documentation".into(),
        scope: CapabilityScope::Global,
        source: CapabilitySource::Local {
            path: "/tmp/docs-review".into(),
        },
        permissions: vec!["workspace.read".into()],
        secret_refs: vec![],
        spec: CapabilitySpec::Skill(SkillSpec {
            entrypoint: "SKILL.md".into(),
            content_root: ".".into(),
            executable_scripts: false,
        }),
    }
}

#[test]
fn validates_and_hashes_skill_manifest() {
    let manifest = validate_and_normalize_manifest(skill_manifest()).unwrap();
    assert_eq!(manifest.kind(), crate::capability::CapabilityKind::Skill);
    let digest = manifest_digest(&manifest).unwrap();
    assert_eq!(digest.len(), 64);
}

#[test]
fn rejects_skill_script_execution() {
    let mut manifest = skill_manifest();
    let CapabilitySpec::Skill(spec) = &mut manifest.spec else {
        unreachable!()
    };
    spec.executable_scripts = true;
    assert!(validate_and_normalize_manifest(manifest).is_err());
}

#[test]
fn rejects_path_escape() {
    let mut manifest = skill_manifest();
    let CapabilitySpec::Skill(spec) = &mut manifest.spec else {
        unreachable!()
    };
    spec.entrypoint = "../SKILL.md".into();
    assert!(validate_and_normalize_manifest(manifest).is_err());
}

#[test]
fn rejects_unknown_secret_binding() {
    let mut manifest = skill_manifest();
    manifest.secret_refs.push(SecretReference {
        name: "known".into(),
        provider: "environment".into(),
        key: "KNOWN_TOKEN".into(),
    });
    manifest.spec = CapabilitySpec::McpServer(crate::capability::McpServerSpec {
        namespace: "github".into(),
        transport: McpTransport::StreamableHttp {
            url: "https://mcp.example.test".into(),
            auth_secret: Some("missing".into()),
            headers: Default::default(),
        },
        protocol_version: None,
        limits: Default::default(),
    });
    assert!(validate_and_normalize_manifest(manifest).is_err());
}

#[test]
fn validates_secret_backed_http_headers_and_rejects_plaintext_credentials() {
    let mut manifest = skill_manifest();
    manifest.secret_refs.push(SecretReference {
        name: "video_authorization".into(),
        provider: "environment".into(),
        key: "VIDEO_AUTHORIZATION".into(),
    });
    manifest.spec = CapabilitySpec::McpServer(crate::capability::McpServerSpec {
        namespace: "video".into(),
        transport: McpTransport::StreamableHttp {
            url: "https://mcp.example.test".into(),
            auth_secret: None,
            headers: std::collections::BTreeMap::from([
                (
                    "Authorization".into(),
                    EnvironmentBinding::SecretRef {
                        reference: "video_authorization".into(),
                    },
                ),
                (
                    "X-Client-Name".into(),
                    EnvironmentBinding::Literal {
                        value: "exoharness".into(),
                    },
                ),
            ]),
        },
        protocol_version: None,
        limits: Default::default(),
    });
    assert!(validate_and_normalize_manifest(manifest.clone()).is_ok());

    let CapabilitySpec::McpServer(spec) = &mut manifest.spec else {
        unreachable!()
    };
    let McpTransport::StreamableHttp { headers, .. } = &mut spec.transport else {
        unreachable!()
    };
    headers.insert(
        "Authorization".into(),
        EnvironmentBinding::Literal {
            value: "plaintext-token".into(),
        },
    );
    assert!(validate_and_normalize_manifest(manifest).is_err());
}

#[test]
fn secure_urls_are_required_except_for_loopback_and_private_mcp_hosts() {
    assert!(validate_http_url("url", "https://example.test/mcp").is_ok());
    assert!(validate_http_url("url", "http://localhost:3000/mcp").is_ok());
    assert!(validate_http_url("url", "http://example.test/mcp").is_err());
    assert!(validate_mcp_http_url("url", "http://192.168.108.50:8765/mcp").is_ok());
    assert!(validate_mcp_http_url("url", "http://10.20.30.40/mcp").is_ok());
    assert!(validate_mcp_http_url("url", "http://example.test/mcp").is_err());
    assert!(validate_http_url("url", "http://127.0.0.1.evil.test/mcp").is_err());
}
