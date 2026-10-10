use exoharness_coding::search::matches;
#[test]
fn literal_unicode_matches_are_bounded_and_explicitly_truncated() {
    let source = format!("unrelated\n关键词{}\n关键词 second\n", "字".repeat(500));
    let results: Vec<_> = matches("src/main.ts", &source, "关键词", 1).collect();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].line, 2);
    assert_eq!(results[0].text.chars().count(), 300);
    assert!(results[0].truncated);
    assert_eq!(results[0].path, "src/main.ts");
    assert_eq!(matches("a", "[.*]\n", ".*", 4).count(), 1);
    assert_eq!(matches("a", "text", "", 4).count(), 0);
}
