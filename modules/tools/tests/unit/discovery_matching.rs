use super::score;
use crate::discovery::terms;

#[test]
fn cjk_requests_keep_embedded_identifiers_and_legacy_terms() {
    for (query, expected) in [
        ("为exoharness生成技术文档ppt", "ppt"),
        ("生成可编辑的PPTX，字大句短", "pptx"),
        ("创建HTML网页", "html"),
        ("读取README.md并生成文档", "readme.md"),
        ("使用render_plan生成内容", "render_plan"),
    ] {
        let found = terms(query);
        assert!(found.contains(&expected.into()), "{query}: {found:?}");
        assert!(found.contains(&query.to_lowercase()));
        assert!(!found.contains(&String::new()));
    }
    assert!(terms("生成技术文档ppt").contains(&"文档".into()));
    assert!(!terms("生成3页内容").contains(&"3".into()));
    assert_eq!(terms("render_plan"), vec!["render_plan"]);
}

#[test]
fn mixed_language_requests_match_english_registered_metadata() {
    let query = "为exoharness生成技术文档ppt";
    assert!(score(query, "pptx", "Create editable presentations") > 0);
    assert_eq!(score(query, "unrelated", "Audio transcription"), 0);
    assert!(score("创建HTML网页", "html-authoring", "Create websites") > 0);
    assert!(score("读取README.md", "readme.md", "Read project introduction") > 0);
}
