use base64::{Engine, engine::general_purpose::STANDARD};
use exoharness_skills::{ImportRequest, decode};
use serde_json::{Value, json};
use std::io::{Cursor, Write};
use zip::{ZipWriter, write::SimpleFileOptions};

const TEXT: &str = "---\nname: fixture-skill\ndescription: Read fixture instructions.\n---\n# Fixture\nNo execution on import.\n";
fn request(files: &[(&str, &[u8])]) -> ImportRequest {
    serde_json::from_value(json!({"package_kind":"skill","files":files.iter().map(|(path, content)|json!({"path":path,"content_base64":STANDARD.encode(content)})).collect::<Vec<_>>()})).unwrap()
}
fn archive(entries: &[(&str, &[u8])]) -> ImportRequest {
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    for (name, content) in entries {
        zip.start_file(
            *name,
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated),
        )
        .unwrap();
        zip.write_all(content).unwrap();
    }
    let bytes = zip.finish().unwrap().into_inner();
    serde_json::from_value(json!({"archive_base64":STANDARD.encode(bytes)})).unwrap()
}
#[test]
fn directory_zip_and_markdown_have_identical_canonical_content() {
    let flat = decode(&request(&[
        ("SKILL.md", TEXT.as_bytes()),
        ("references/notes.md", b"opaque"),
    ]))
    .unwrap();
    let wrapped = decode(&request(&[
        ("fixture/SKILL.md", TEXT.as_bytes()),
        ("fixture/references/notes.md", b"opaque"),
    ]))
    .unwrap();
    let zipped = decode(&archive(&[
        ("fixture/SKILL.md", TEXT.as_bytes()),
        ("fixture/references/notes.md", b"opaque"),
    ]))
    .unwrap();
    assert_eq!(flat.source_digest, wrapped.source_digest);
    assert_eq!(flat.source_digest, zipped.source_digest);
    assert_eq!(flat.files.len(), 2);
    let single = decode(&request(&[("my-skill.md", TEXT.as_bytes())])).unwrap();
    assert_eq!(single.files[0].path.to_str(), Some("SKILL.md"));
}
#[test]
fn order_is_irrelevant_but_any_byte_change_is_a_new_digest() {
    let first = decode(&request(&[("SKILL.md", TEXT.as_bytes()), ("a.txt", b"a")])).unwrap();
    let second = decode(&request(&[("a.txt", b"a"), ("SKILL.md", TEXT.as_bytes())])).unwrap();
    let changed = decode(&request(&[("a.txt", b"b"), ("SKILL.md", TEXT.as_bytes())])).unwrap();
    assert_eq!(first.source_digest, second.source_digest);
    assert_ne!(first.source_digest, changed.source_digest);
}
#[test]
fn multiline_optional_yaml_and_bom_are_preserved_not_executed() {
    let text = "\u{feff}---\nname: fixture-skill\ndescription: >-\n  First line\n  second line\nallowed-tools: [Read, Bash]\nmetadata:\n  nested: true\noptional:\n---\nKeep the original text.\n";
    let parsed = decode(&request(&[("SKILL.md", text.as_bytes())])).unwrap();
    assert_eq!(parsed.manifest.description, "First line second line");
    assert!(
        matches!(parsed.manifest.spec, exoharness_contracts::capability::CapabilitySpec::Skill(ref s) if !s.executable_scripts)
    );
}
#[test]
fn unsafe_paths_duplicates_collisions_and_missing_roots_are_rejected() {
    for path in [
        "../escape",
        "/tmp/escape",
        "C:/escape",
        "foo\\bar",
        "a/../escape",
        "a/./escape",
        "a//escape",
    ] {
        assert!(
            decode(&request(&[("SKILL.md", TEXT.as_bytes()), (path, b"x")])).is_err(),
            "{path}"
        );
    }
    for entries in [
        vec![("SKILL.md", TEXT.as_bytes()), ("SKILL.md", TEXT.as_bytes())],
        vec![("SKILL.md", TEXT.as_bytes()), ("skill.md", TEXT.as_bytes())],
        vec![
            ("one/SKILL.md", TEXT.as_bytes()),
            ("two/a.txt", b"x".as_slice()),
        ],
    ] {
        assert!(decode(&request(&entries)).is_err());
    }
    for path in ["../escape", "a/../escape", "/escape"] {
        assert!(decode(&archive(&[("SKILL.md", TEXT.as_bytes()), (path, b"x")])).is_err());
    }
}
#[test]
fn zip_symlinks_are_rejected_without_following_the_target() {
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    zip.start_file("SKILL.md", SimpleFileOptions::default())
        .unwrap();
    zip.write_all(TEXT.as_bytes()).unwrap();
    zip.add_symlink("link", "/etc/passwd", SimpleFileOptions::default())
        .unwrap();
    let value: ImportRequest = serde_json::from_value(
        json!({"archive_base64":STANDARD.encode(zip.finish().unwrap().into_inner())}),
    )
    .unwrap();
    assert!(decode(&value).is_err());
}
#[test]
fn metadata_and_transport_input_fail_with_actionable_diagnostics() {
    for text in [
        "not frontmatter",
        "---\nname: x\n---\n",
        "---\nname: x\nname: y\ndescription: z\n---",
        "---\nname: x\ndescription: \n---",
        "---\nname: x\ndescription: [invalid]\n---",
    ] {
        assert!(decode(&request(&[("SKILL.md", text.as_bytes())])).is_err());
    }
    assert!(decode(&request(&[("SKILL.md", &[0xff])])).is_err());
    let mut value: Value = json!({"files":[{"path":"SKILL.md","content_base64":"invalid$"}]});
    assert!(decode(&serde_json::from_value(value.clone()).unwrap()).is_err());
    value["archive_base64"] = json!("abcd");
    assert!(decode(&serde_json::from_value(value).unwrap()).is_err());
    assert!(decode(&serde_json::from_value(json!({})).unwrap()).is_err());
}
#[test]
fn upload_limits_are_bounded_and_large_valid_support_files_are_allowed() {
    assert!(
        decode(&request(&[
            ("SKILL.md", TEXT.as_bytes()),
            ("ref/data.bin", &vec![42; 70_000])
        ]))
        .is_ok()
    );
    assert!(
        decode(&request(&[
            ("SKILL.md", TEXT.as_bytes()),
            ("oversize.bin", &vec![42; 4 * 1024 * 1024 + 1])
        ]))
        .is_err()
    );
    let too_many = (0..1025)
        .map(|i| json!({"path":format!("{i}.txt"),"content_base64":""}))
        .collect::<Vec<_>>();
    let value: ImportRequest = serde_json::from_value(json!({"files":too_many})).unwrap();
    assert!(decode(&value).is_err());
}
