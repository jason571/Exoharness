#[path = "../src/result_artifacts.rs"]
mod result_artifacts;
use serde_json::json;
#[test]
fn declared_public_outputs_only_and_duplicate_encoding_is_not_repeated() {
    let video = json!({"filename":"final.mp4","content_type":"video/mp4","download_url":"https://media.example/video?signature=abc","sha256":"original"});
    let hidden = json!({"filename":"private.json","content_type":"application/json","download_url":"https://media.example/private","_meta":{"exoharness":{"artifact_visibility":"internal"}}});
    let value =
        json!({"isError":false,"structuredContent":{"artifacts":[video.clone(),video,hidden]}});
    let cards = result_artifacts::collect(&value);
    assert_eq!(cards.len(), 1);
    assert_eq!(cards[0]["kind"], "video");
    assert!(cards[0]["url"].as_str().unwrap().contains("signature=abc"));
    assert!(
        result_artifacts::collect(
            &json!({"isError":true,"structuredContent":value["structuredContent"]})
        )
        .is_empty()
    );
}
#[test]
fn unadvertised_text_invalid_links_credentials_and_missing_metadata_are_not_cards() {
    let invalid = [
        json!({"filename":"x","content_type":"video/mp4","download_url":"javascript:alert(1)"}),
        json!({"filename":"x","content_type":"video/mp4","download_url":"file:///etc/passwd"}),
        json!({"filename":"x","content_type":"video/mp4","download_url":"https://user:secret@example.com/x"}),
        json!({"download_url":"https://example.com/x"}),
    ];
    assert!(
        result_artifacts::collect(&json!({"structuredContent":{"artifacts":invalid}})).is_empty()
    );
    assert!(
        result_artifacts::collect(
            &json!({"content":[{"type":"text","text":"https://example.com/fake.mp4"}]})
        )
        .is_empty()
    );
}
