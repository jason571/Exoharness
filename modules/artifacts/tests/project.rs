mod support;
use std::{collections::BTreeMap, io::Read};

#[test]
fn general_project_zip_preserves_sources_and_directory_structure() {
    let (_root, _store, artifacts) = support::fixture();
    let sources = BTreeMap::from([
        (
            "src/main.py".into(),
            support::file("print('你好')\n".as_bytes()),
        ),
        (".gitignore".into(), support::file(b"__pycache__/\n")),
    ]);
    let card = artifacts
        .publish_project("owner", "session", "run", None, sources)
        .unwrap();
    assert_eq!(card["filename"], "project.zip");
    assert!(card["preview_url"].is_null());
    let (expires, signature) = support::access(card["url"].as_str().unwrap());
    let (_, file) = artifacts
        .open_signed(
            card["id"].as_str().unwrap(),
            "project.zip",
            expires,
            &signature,
        )
        .unwrap();
    let mut zip = zip::ZipArchive::new(file).unwrap();
    assert_eq!(zip.len(), 2);
    let mut text = String::new();
    zip.by_name("src/main.py")
        .unwrap()
        .read_to_string(&mut text)
        .unwrap();
    assert_eq!(text, "print('你好')\n");
    assert!(zip.by_name(".gitignore").is_ok());
}
