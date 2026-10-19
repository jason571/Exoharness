use super::*;

#[test]
fn explicit_single_file_results_render_all_delivery_types_without_model_links() {
    for (mime, kind, filename) in [
        ("video/mp4", "video", "final.mp4"),
        ("audio/mpeg", "audio", "song.mp3"),
        ("image/png", "image", "poster.png"),
        (
            "application/vnd.openxmlformats-officedocument.presentationml.presentation",
            "file",
            "report.pptx",
        ),
        (
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
            "file",
            "report.docx",
        ),
        ("application/pdf", "file", "report.pdf"),
    ] {
        let value = json!({"structuredContent":{"status":"completed","result":{
            "filename":filename,"content_type":mime,"download_url":"https://media.example/file?expires=0&signature=signed","url_ttl_seconds":0
        }},"content":[{"type":"text","text":"Completed, no file link in model prose"}]});
        let cards = collect(&value);
        assert_eq!(cards.len(), 1);
        assert_eq!(cards[0]["kind"], kind);
        assert_eq!(cards[0]["filename"], filename);
        assert_eq!(cards[0]["url_ttl_seconds"], 0);
    }
}

#[test]
fn legacy_result_preserves_internal_visibility_and_never_scans_prompt_or_text() {
    let descriptor = json!({"filename":"final.mp4","content_type":"video/mp4","download_url":"https://media.example/final.mp4"});
    for pointer in [
        "/_meta/exoharness/artifact_visibility",
        "/structuredContent/_meta/exoharness/artifact_visibility",
        "/structuredContent/_meta/exoharness/background_task/artifact_visibility",
        "/structuredContent/result/artifact_visibility",
    ] {
        let mut value = json!({"structuredContent":{"result":descriptor}});
        let parts: Vec<_> = pointer.trim_start_matches('/').split('/').collect();
        let mut node = &mut value;
        for part in &parts[..parts.len() - 1] {
            if node.get(*part).is_none() {
                node[*part] = json!({});
            }
            node = &mut node[*part];
        }
        node[parts[parts.len() - 1]] = json!("internal");
        assert!(collect(&value).is_empty(), "{pointer}");
    }
    for value in [
        json!({"isError":true,"structuredContent":{"result":descriptor}}),
        json!({"structuredContent":{"prompt":{"result":descriptor}}}),
        json!({"content":[{"type":"text","text":descriptor.to_string()}]}),
        json!({"structuredContent":{"result":{"download_url":"https://media.example/final.mp4"}}}),
        json!({"structuredContent":{"result":{"filename":"final.mp4","content_type":"video/mp4","download_url":"file:///private/final.mp4"}}}),
    ] {
        assert!(collect(&value).is_empty());
    }
}

#[test]
fn single_file_and_standard_artifacts_deduplicate() {
    let descriptor = json!({"filename":"final.mp4","content_type":"video/mp4","download_url":"https://media.example/final.mp4"});
    assert_eq!(collect(&json!({"structuredContent":{"result":descriptor,"artifacts":[descriptor]},"content":[{"type":"resource_link","uri":"https://media.example/final.mp4","name":"final.mp4","mimeType":"video/mp4"}]})).len(),1);
}
#[test]
fn standard_mcp_resource_links_render_images_audio_and_video_without_inventing_urls() {
    for (mime, kind, name) in [
        ("image/png", "image", "cover.png"),
        ("audio/mpeg", "audio", "song.mp3"),
        ("video/mp4", "video", "final.mp4"),
    ] {
        let cards = collect(
            &json!({"content":[{"type":"resource_link","uri":"https://media.example/file","name":name,"mimeType":mime}]}),
        );
        assert_eq!(cards.len(), 1);
        assert_eq!(cards[0]["kind"], kind);
        assert_eq!(cards[0]["url"], "https://media.example/file");
    }
}
#[test]
fn duplicate_error_private_and_non_http_links_are_not_displayed() {
    let link = json!({"type":"resource_link","uri":"https://media.example/final.mp4","name":"final.mp4","mimeType":"video/mp4"});
    let output = json!({"structuredContent":{"artifacts":[{"url":link["uri"],"filename":link["name"],"content_type":link["mimeType"],"sha256":"verified"}]},"content":[link]});
    assert_eq!(collect(&output).len(), 1);
    assert!(collect(&json!({"isError":true,"content":[link]})).is_empty());
    assert!(collect(&json!({"content":[{"type":"resource_link","uri":"file:///tmp/x","name":"x.png","mimeType":"image/png"}]})).is_empty());
    let mut internal = link.clone();
    internal["_meta"] = json!({"exoharness":{"artifact_visibility":"internal"}});
    assert!(collect(&json!({"content":[internal]})).is_empty());
}

#[test]
fn internal_outputs_are_validated_for_storage_without_becoming_public_cards() {
    let descriptor = json!({"filename":"clip.mp4","content_type":"video/mp4","download_url":"https://media.example/clip.mp4"});
    for value in [
        json!({"structuredContent":{"result":descriptor,"_meta":{"exoharness":{"artifact_visibility":"internal"}}}}),
        json!({"_meta":{"exoharness":{"artifact_visibility":"internal"}},"structuredContent":{"artifacts":[descriptor]}}),
        json!({"structuredContent":{"result":descriptor},"_meta":{"exoharness":{"background_task":{"artifact_visibility":"internal"}}}}),
        json!({"content":[{"type":"resource_link","uri":"https://media.example/clip.mp4","name":"clip.mp4","mimeType":"video/mp4","_meta":{"exoharness":{"artifact_visibility":"internal"}}}]}),
    ] {
        assert!(collect(&value).is_empty());
        let saved = descriptors(&value);
        assert_eq!(saved.len(), 1);
        assert_eq!(saved[0]["artifact_visibility"], "internal");
        assert_eq!(saved[0]["filename"], "clip.mp4");
    }
    for value in [
        json!({"isError":true,"structuredContent":{"result":descriptor}}),
        json!({"_meta":{"exoharness":{"artifact_visibility":"internal"}},"content":[{"type":"text","text":descriptor.to_string()}]}),
        json!({"structuredContent":{"result":{"filename":"clip.mp4","content_type":"video/mp4","download_url":"file:///private/clip.mp4"}}}),
    ] { assert!(descriptors(&value).is_empty()); }
}
