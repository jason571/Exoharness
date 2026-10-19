use super::*;

fn package(items: &[(&str, &str, &str)]) -> BTreeMap<String, Vec<u8>> {
    let mut files = BTreeMap::new();
    let resources: Vec<_> = items
        .iter()
        .enumerate()
        .map(|(n, (term, text, _))| {
            let path = format!("references/{n}.md");
            files.insert(path.clone(), text.as_bytes().to_vec());
            json!({"id":format!("r{n}"),"path":path,"terms":[term],
            "sha256":format!("{:x}", Sha256::digest(text.as_bytes()))})
        })
        .collect();
    files.insert(
        INDEX.into(),
        json!({"version":1,"resources":resources})
            .to_string()
            .into_bytes(),
    );
    files
}

#[test]
fn selects_relevant_complete_resources_and_no_fallback() {
    let files = package(&[
        ("视频", "video workflow", ""),
        ("ppt", "slide workflow", ""),
    ]);
    let result = select(&files, "请生成一个视频").unwrap();
    assert_eq!(result["resources"].as_array().unwrap().len(), 1);
    assert_eq!(result["resources"][0]["content"], "video workflow");
    assert_eq!(
        select(&files, "PPT report").unwrap()["resources"][0]["content"],
        "slide workflow"
    );
    assert!(
        select(&files, "appts").unwrap()["resources"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert!(
        select(&files, "music").unwrap()["resources"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}

#[test]
fn selection_is_bounded_stable_and_never_truncates_a_resource() {
    let files = package(&[
        ("视频", "first", ""),
        ("视频", "second", ""),
        ("视频", "third", ""),
    ]);
    let result = select(&files, "视频").unwrap();
    assert_eq!(result["resources"].as_array().unwrap().len(), 2);
    assert_eq!(result, select(&files, "视频").unwrap());
    let large = "字".repeat(2000);
    let files = package(&[("视频", &large, ""), ("视频", &large, "")]);
    let result = select(&files, "视频").unwrap();
    assert_eq!(result["resources"].as_array().unwrap().len(), 1);
    assert_eq!(result["resources"][0]["content"], large);
}

#[test]
fn invalid_inventory_is_rejected_atomically_even_for_unmatched_resources() {
    let original = package(&[("视频", "first", ""), ("ppt", "second", "")]);
    for mutation in ["digest", "escape", "duplicate", "utf8", "term", "field"] {
        let mut files = original.clone();
        let mut index: Value = serde_json::from_slice(&files[INDEX]).unwrap();
        match mutation {
            "digest" => {
                files.insert("references/1.md".into(), b"changed".to_vec());
            }
            "escape" => index["resources"][1]["path"] = json!("references/../SKILL.md"),
            "duplicate" => index["resources"][1]["id"] = index["resources"][0]["id"].clone(),
            "utf8" => {
                files.insert("references/1.md".into(), vec![255]);
            }
            "term" => index["resources"][1]["terms"] = json!([""]),
            _ => index["unrecognized"] = json!(true),
        }
        files.insert(INDEX.into(), index.to_string().into_bytes());
        assert!(
            matches!(select(&files, "视频"), Err(ToolError::CorrectableInput(_))),
            "{mutation}"
        );
    }
}

#[test]
fn empty_and_excessive_queries_are_rejected() {
    let files = package(&[("视频", "video workflow", "")]);
    assert!(select(&files, " ").is_err());
    assert!(select(&files, &"字".repeat(2001)).is_err());
}
