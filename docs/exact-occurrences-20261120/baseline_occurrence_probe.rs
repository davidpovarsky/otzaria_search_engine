// Test-only acceptance probe against the unmodified pinned engine.
// The expected failure demonstrates why document totals cannot implement occurrences.
use search_engine::api::search_engine::SearchEngine;

#[test]
fn two_occurrences_in_one_document_are_not_one_occurrence() {
    let dir = tempfile::TempDir::new().unwrap();
    let mut engine = SearchEngine::new(dir.path().to_str().unwrap());
    engine.add_document(1, "title", "ref", "/book/1", "משה משה", 0, false, "/book/1", None, None, None).unwrap();
    engine.commit().unwrap();
    let old_document_count = engine.count_exact("משה".to_string(), vec!["/book/1".to_string()], false, false).unwrap();
    assert_eq!(old_document_count, 2, "occurrence acceptance requires 2; the original API counts matching documents");
}
