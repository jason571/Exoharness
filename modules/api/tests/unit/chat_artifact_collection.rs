use super::*;
use serde_json::json;

#[test]
fn repeated_result_retrieval_retains_one_card_and_preserves_other_files() {
    let file = json!({"id":"image-1","url":"https://example.test/image","filename":"image.png"});
    let mut files = vec![file.clone()];
    append(
        &mut files,
        vec![
            file.clone(),
            json!({"id":"image-2","url":"https://example.test/other","filename":"other.png"}),
        ],
    );
    assert_eq!(files.len(), 2);
    assert_eq!(files[0], file);
}

#[test]
fn stable_identity_survives_url_changes_and_missing_ids_use_url_and_filename() {
    let mut files =
        vec![json!({"id":"same","url":"https://example.test/old","filename":"image.png"})];
    append(
        &mut files,
        vec![json!({"id":"same","url":"https://example.test/new","filename":"image.png"})],
    );
    assert_eq!(files.len(), 1);
    let file = json!({"url":"https://example.test/no-id","filename":"file.png"});
    append(&mut files, vec![file.clone(), file]);
    assert_eq!(files.len(), 2);
    append(
        &mut files,
        vec![
            json!({"label":"no identity"}),
            json!({"label":"another file"}),
        ],
    );
    assert_eq!(files.len(), 4);
}

#[test]
fn same_content_retrieved_under_new_owned_ids_has_one_card() {
    let mut files = vec![
        serde_json::json!({"id":"saved-1","url":"https://example.test/1","sha256":"digest","filename":"logo.png","content_type":"image/png"}),
    ];
    append(
        &mut files,
        vec![
            serde_json::json!({"id":"saved-2","url":"https://example.test/2","sha256":"digest","filename":"logo.png","content_type":"image/png"}),
        ],
    );
    assert_eq!(files.len(), 1);
}
