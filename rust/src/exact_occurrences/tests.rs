use super::*;
use crate::api::search_engine::{SearchEngine, INDEX_SCHEMA_VERSION};
use std::sync::atomic::AtomicUsize;
use tantivy::query::TermQuery;
use tantivy::schema::{Schema, TextFieldIndexing, TextOptions};
use tantivy::tokenizer::{PreTokenizedString, Token};
use tantivy::{Index, IndexWriter, TantivyDocument};
use tempfile::TempDir;

fn engine() -> (SearchEngine, TempDir) {
    let dir = TempDir::new().unwrap();
    let mut engine = SearchEngine::new(dir.path().to_str().unwrap());
    engine.set_bulk_indexing(true).unwrap();
    (engine, dir)
}

fn add(engine: &mut SearchEngine, id: u64, book: &str, text: &str, extra: &[&str]) {
    engine
        .add_document(
            id,
            "test title",
            "test ref",
            book,
            text,
            id,
            false,
            book,
            None,
            None,
            Some(extra.iter().map(|value| (*value).to_string()).collect()),
        )
        .unwrap();
}

fn control() -> ExactOccurrenceControl {
    ExactOccurrenceControl::with_timeout(
        ExactOccurrenceLimits::default(),
        Duration::from_secs(30),
        ExactOccurrenceCancellation::default(),
    )
    .unwrap()
}

fn limited(limits: ExactOccurrenceLimits) -> ExactOccurrenceControl {
    ExactOccurrenceControl::with_timeout(
        limits,
        Duration::from_secs(30),
        ExactOccurrenceCancellation::default(),
    )
    .unwrap()
}

fn count(engine: &SearchEngine, query: &str, facets: &[&str]) -> ExactTokenOccurrenceCount {
    engine
        .count_exact_token_occurrences(
            query,
            &facets
                .iter()
                .map(|value| (*value).to_string())
                .collect::<Vec<_>>(),
            &control(),
        )
        .unwrap()
}

#[test]
fn repeated_token_counts_positions_while_original_api_stays_document_count() {
    let (mut engine, _dir) = engine();
    add(&mut engine, 1, "/book/1", "משה משה", &[]);
    engine.commit().unwrap();

    let result = count(&engine, "משה", &["/book/1"]);
    assert_eq!(result.occurrences, 2);
    assert_eq!(result.matching_live_documents, 1);
    assert_eq!(result.work.inspected_positions, 2);
    assert_eq!(result.proof.analyzed_token, "משה");
    assert_eq!(result.proof.index_schema_version, INDEX_SCHEMA_VERSION);
    assert_eq!(result.proof.live_index_documents, 1);
    assert_eq!(
        engine
            .count_exact("משה".into(), vec!["/book/1".into()], false, false)
            .unwrap(),
        1,
        "the pre-existing API deliberately retains matching-document semantics",
    );
}

#[test]
fn count_is_not_a_page_size_or_distinct_document_count() {
    let (mut engine, _dir) = engine();
    for id in 1..=125 {
        add(&mut engine, id, "/book/1", "משה משה משה", &[]);
    }
    engine.commit().unwrap();
    let result = count(&engine, "משה", &[]);
    assert_eq!(result.occurrences, 375);
    assert_eq!(result.matching_live_documents, 125);
}

#[test]
fn original_hebrew_marks_boundaries_and_case_normalization_are_preserved() {
    let (mut engine, _dir) = engine();
    add(
        &mut engine,
        1,
        "/book/1",
        "מֹשֶׁה, משה־משה: משה ומשה MOSes moses",
        &[],
    );
    engine.commit().unwrap();
    let result = count(&engine, "מֹשֶׁה", &[]);
    assert_eq!(result.occurrences, 4);
    assert_eq!(result.proof.analyzed_token, "משה");
    assert_eq!(count(&engine, "ומשה", &[]).occurrences, 1);
    assert_eq!(count(&engine, "MOSES", &[]).occurrences, 2);
}

#[test]
fn quote_free_synonym_positions_are_not_double_counted() {
    let (mut engine, _dir) = engine();
    add(&mut engine, 1, "/book/1", "רמב\"ם רמב״ם רמבם", &[]);
    engine.commit().unwrap();
    assert_eq!(count(&engine, "רמבם", &[]).occurrences, 3);
    assert_eq!(count(&engine, "רמב״ם", &[]).occurrences, 2);
}

#[test]
fn original_facet_or_within_dimension_and_and_between_dimensions() {
    let (mut engine, _dir) = engine();
    add(&mut engine, 1, "/book/1", "משה משה", &["/author/first"]);
    add(&mut engine, 2, "/book/2", "משה", &["/author/second"]);
    add(&mut engine, 3, "/book/3", "משה משה משה", &["/author/first"]);
    engine.commit().unwrap();

    assert_eq!(count(&engine, "משה", &[]).occurrences, 6);
    assert_eq!(
        count(&engine, "משה", &["/book/1", "/book/2"]).occurrences,
        3
    );
    let result = count(
        &engine,
        "משה",
        &["/book/2", "/author/first", "/book/1", "/book/1"],
    );
    assert_eq!(result.occurrences, 2);
    assert_eq!(result.matching_live_documents, 1);
    assert_eq!(
        result.proof.requested_facets,
        vec!["/author/first", "/book/1", "/book/2"]
    );
    let original_docs = engine
        .count_exact(
            "משה".into(),
            result.proof.requested_facets.clone(),
            false,
            false,
        )
        .unwrap();
    assert_eq!(u64::from(original_docs), result.matching_live_documents);
    assert_eq!(count(&engine, "משה", &["/book/99"]).occurrences, 0);
}

#[test]
fn deleted_documents_are_excluded_and_delete_snapshot_is_retained() {
    let (mut engine, _dir) = engine();
    add(&mut engine, 1, "/book/1", "משה משה", &[]);
    add(&mut engine, 2, "/book/1", "משה", &[]);
    engine.commit().unwrap();
    // The original writer may distribute these two docs to separate segments.
    // Merge before deletion so the proof exercises a surviving segment with
    // a real tombstone, not deletion of an entire one-document segment.
    engine.optimize().unwrap();
    let before = count(&engine, "משה", &[]);
    assert_eq!(before.proof.searcher_generation.segments().len(), 1);
    assert_eq!(before.occurrences, 3);

    engine.delete_document_by_id(1).unwrap();
    engine.commit().unwrap();
    let after = count(&engine, "משה", &[]);
    assert_eq!(after.occurrences, 1);
    assert_eq!(after.matching_live_documents, 1);
    assert_eq!(after.proof.live_index_documents, 1);
    assert_ne!(
        before.proof.searcher_generation,
        after.proof.searcher_generation
    );
    assert!(after
        .proof
        .searcher_generation
        .segments()
        .values()
        .any(Option::is_some));
}

#[test]
fn counts_all_original_segments_and_ignores_uncommitted_documents() {
    let (mut engine, _dir) = engine();
    add(&mut engine, 1, "/book/1", "משה משה", &[]);
    engine.commit().unwrap();
    add(&mut engine, 2, "/book/1", "משה", &[]);
    engine.commit().unwrap();
    add(&mut engine, 3, "/book/1", "משה משה משה", &[]);

    let result = count(&engine, "משה", &[]);
    assert_eq!(result.occurrences, 3);
    assert_eq!(result.matching_live_documents, 2);
    assert_eq!(result.proof.live_index_documents, 2);
    assert!(result.proof.searcher_generation.segments().len() >= 2);
}

#[test]
fn complete_zero_is_distinct_from_unsupported_empty_query() {
    let (mut engine, _dir) = engine();
    add(&mut engine, 1, "/book/1", "משה משה", &[]);
    engine.commit().unwrap();
    let zero = count(&engine, "אהרן", &[]);
    assert_eq!(zero.occurrences, 0);
    assert_eq!(zero.matching_live_documents, 0);
    assert_eq!(zero.proof.live_index_documents, 1);
    for query in ["", "?!", "משה אהרן", "משה־אהרן"] {
        assert!(
            matches!(
                engine.count_exact_token_occurrences(query, &[], &control()),
                Err(ExactOccurrenceError::SingleAnalyzedTokenRequired { .. }),
            ),
            "{query:?} is not one analyzed token"
        );
    }
}

#[test]
fn oversized_queries_and_facets_fail_without_an_exact_value() {
    let (engine, _dir) = engine();
    assert_eq!(
        engine.count_exact_token_occurrences(&"מ".repeat(1_001), &[], &control()),
        Err(ExactOccurrenceError::QueryTooLong),
    );
    assert_eq!(
        engine.count_exact_token_occurrences("משה", &vec!["/book/1".into(); 201], &control()),
        Err(ExactOccurrenceError::InvalidFacets),
    );
    assert_eq!(
        engine.count_exact_token_occurrences("משה", &[String::new()], &control()),
        Err(ExactOccurrenceError::InvalidFacets),
    );
}

#[test]
fn candidate_document_budget_returns_error_instead_of_partial_count() {
    let (mut engine, _dir) = engine();
    add(&mut engine, 1, "/book/1", "משה משה", &[]);
    add(&mut engine, 2, "/book/1", "משה", &[]);
    engine.commit().unwrap();
    let limits = ExactOccurrenceLimits {
        max_candidate_documents: 1,
        ..Default::default()
    };
    assert_eq!(
        engine.count_exact_token_occurrences("משה", &[], &limited(limits)),
        Err(ExactOccurrenceError::BudgetExceeded(
            ExactOccurrenceBudget::CandidateDocuments
        )),
    );
}

#[test]
fn position_budgets_apply_before_materializing_an_oversized_document() {
    let (mut engine, _dir) = engine();
    add(&mut engine, 1, "/book/1", "משה משה משה", &[]);
    engine.commit().unwrap();
    let total = ExactOccurrenceLimits {
        max_positions: 2,
        ..Default::default()
    };
    assert_eq!(
        engine.count_exact_token_occurrences("משה", &[], &limited(total)),
        Err(ExactOccurrenceError::BudgetExceeded(
            ExactOccurrenceBudget::Positions
        )),
    );
    let per_doc = ExactOccurrenceLimits {
        max_positions_per_document: 2,
        ..Default::default()
    };
    assert_eq!(
        engine.count_exact_token_occurrences("משה", &[], &limited(per_doc)),
        Err(ExactOccurrenceError::BudgetExceeded(
            ExactOccurrenceBudget::PositionsPerDocument
        )),
    );
}

#[test]
fn segment_budget_is_checked_before_collection() {
    let (mut engine, _dir) = engine();
    add(&mut engine, 1, "/book/1", "משה", &[]);
    engine.commit().unwrap();
    let limits = ExactOccurrenceLimits {
        max_segments: 0,
        ..Default::default()
    };
    assert_eq!(
        engine.count_exact_token_occurrences("משה", &[], &limited(limits)),
        Err(ExactOccurrenceError::BudgetExceeded(
            ExactOccurrenceBudget::Segments
        )),
    );
}

#[test]
fn cancelled_and_expired_invocations_cannot_publish_a_count() {
    let (mut engine, _dir) = engine();
    add(&mut engine, 1, "/book/1", "משה משה", &[]);
    engine.commit().unwrap();
    let token = ExactOccurrenceCancellation::default();
    let shared = token.clone();
    let cancelled =
        ExactOccurrenceControl::with_timeout(Default::default(), Duration::from_secs(30), token)
            .unwrap();
    shared.cancel();
    assert_eq!(
        engine.count_exact_token_occurrences("משה", &[], &cancelled),
        Err(ExactOccurrenceError::Cancelled),
    );
    let expired = ExactOccurrenceControl::with_timeout(
        Default::default(),
        Duration::ZERO,
        Default::default(),
    )
    .unwrap();
    assert_eq!(
        engine.count_exact_token_occurrences("משה", &[], &expired),
        Err(ExactOccurrenceError::DeadlineExceeded),
    );
}

#[test]
fn hostile_overflow_sized_budgets_are_rejected() {
    let limits = ExactOccurrenceLimits {
        max_positions: u64::MAX,
        ..Default::default()
    };
    assert!(matches!(
        ExactOccurrenceControl::with_timeout(limits, Duration::from_secs(1), Default::default()),
        Err(ExactOccurrenceError::InvalidLimits),
    ));
    assert!(matches!(
        ExactOccurrenceControl::with_timeout(Default::default(), Duration::MAX, Default::default()),
        Err(ExactOccurrenceError::InvalidLimits),
    ));
}

// The following fixtures use a real Tantivy index to exercise conditions the
// original source tokenizer normally prevents (duplicate stacked tokens and
// missing positions) and to deterministically observe in-flight cancellation.
// They are not mock providers or an alternate text search implementation.
fn positional_index(
    option: IndexRecordOption,
) -> (Index, IndexWriter<TantivyDocument>, tantivy::schema::Field) {
    let mut schema = Schema::builder();
    let text = schema.add_text_field(
        "text",
        TextOptions::default().set_indexing_options(
            TextFieldIndexing::default()
                .set_tokenizer("raw")
                .set_index_option(option),
        ),
    );
    let index = Index::create_in_ram(schema.build());
    let writer = index.writer::<TantivyDocument>(15_000_000).unwrap();
    (index, writer, text)
}

fn add_positions(
    writer: &mut IndexWriter<TantivyDocument>,
    text: tantivy::schema::Field,
    positions: &[usize],
) {
    let mut document = TantivyDocument::default();
    document.add_pre_tokenized_text(
        text,
        PreTokenizedString {
            text: "משה".into(),
            tokens: positions
                .iter()
                .map(|&position| Token {
                    offset_from: 0,
                    offset_to: "משה".len(),
                    position,
                    text: "משה".into(),
                    position_length: 1,
                })
                .collect(),
        },
    );
    writer.add_document(document).unwrap();
}

fn raw_count(
    searcher: &Searcher,
    text: tantivy::schema::Field,
    control: &ExactOccurrenceControl,
) -> Result<ExactTokenOccurrenceCount, ExactOccurrenceError> {
    let term = Term::from_field_text(text, "משה");
    let query = TermQuery::new(term.clone(), IndexRecordOption::Basic);
    count_on_snapshot(
        searcher,
        term,
        "משה".into(),
        vec![],
        &query,
        INDEX_SCHEMA_VERSION,
        control,
    )
}

#[test]
fn duplicate_stacked_positions_in_a_real_index_count_once() {
    let (index, mut writer, text) = positional_index(IndexRecordOption::WithFreqsAndPositions);
    add_positions(&mut writer, text, &[0, 0, 1]);
    writer.commit().unwrap();
    let reader = index.reader().unwrap();
    let result = raw_count(&reader.searcher(), text, &control()).unwrap();
    assert_eq!(result.occurrences, 2);
    assert_eq!(result.matching_live_documents, 1);
    assert_eq!(result.work.inspected_positions, 3);
}

#[test]
fn missing_position_storage_is_unsupported_even_when_no_hit_exists() {
    let (index, _writer, text) = positional_index(IndexRecordOption::WithFreqs);
    let reader = index.reader().unwrap();
    assert_eq!(
        raw_count(&reader.searcher(), text, &control()),
        Err(ExactOccurrenceError::PositionsUnavailable),
    );
}

#[test]
fn one_held_searcher_snapshot_survives_a_new_commit_without_mixing_counts() {
    let (index, mut writer, text) = positional_index(IndexRecordOption::WithFreqsAndPositions);
    add_positions(&mut writer, text, &[0, 1]);
    writer.commit().unwrap();
    let reader = index.reader().unwrap();
    let held = reader.searcher();
    let before = raw_count(&held, text, &control()).unwrap();

    add_positions(&mut writer, text, &[0]);
    writer.commit().unwrap();
    reader.reload().unwrap();
    let old = raw_count(&held, text, &control()).unwrap();
    let current = raw_count(&reader.searcher(), text, &control()).unwrap();
    assert_eq!(before, old);
    assert_eq!(old.occurrences, 2);
    assert_eq!(current.occurrences, 3);
    assert_ne!(
        old.proof.searcher_generation,
        current.proof.searcher_generation
    );
}

#[test]
fn cancellation_after_real_positions_are_read_discards_the_accumulated_count() {
    let (index, mut writer, text) = positional_index(IndexRecordOption::WithFreqsAndPositions);
    add_positions(&mut writer, text, &[0, 1]);
    add_positions(&mut writer, text, &[0]);
    writer.commit().unwrap();
    let reader = index.reader().unwrap();
    let term = Term::from_field_text(text, "משה");
    let query = TermQuery::new(term.clone(), IndexRecordOption::Basic);
    let token = ExactOccurrenceCancellation::default();
    let control = ExactOccurrenceControl::with_timeout(
        Default::default(),
        Duration::from_secs(30),
        token.clone(),
    )
    .unwrap();
    let observations = AtomicUsize::new(0);
    let result = count_on_snapshot_observed(
        &reader.searcher(),
        term,
        "משה".into(),
        vec![],
        &query,
        INDEX_SCHEMA_VERSION,
        &control,
        |work| {
            assert!(work.inspected_positions > 0);
            observations.fetch_add(1, Ordering::Relaxed);
            token.cancel();
        },
    );
    assert_eq!(observations.load(Ordering::Relaxed), 1);
    assert_eq!(result, Err(ExactOccurrenceError::Cancelled));
}

#[test]
fn expiry_after_the_last_real_match_still_cannot_publish_exact_success() {
    let (index, mut writer, text) = positional_index(IndexRecordOption::WithFreqsAndPositions);
    add_positions(&mut writer, text, &[0, 1]);
    writer.commit().unwrap();
    let reader = index.reader().unwrap();
    let term = Term::from_field_text(text, "משה");
    let query = TermQuery::new(term.clone(), IndexRecordOption::Basic);
    let control = ExactOccurrenceControl::with_timeout(
        Default::default(),
        Duration::from_secs(2),
        Default::default(),
    )
    .unwrap();
    let observations = AtomicUsize::new(0);
    let result = count_on_snapshot_observed(
        &reader.searcher(),
        term,
        "משה".into(),
        vec![],
        &query,
        INDEX_SCHEMA_VERSION,
        &control,
        |_| {
            observations.fetch_add(1, Ordering::Relaxed);
            std::thread::sleep(
                control.deadline.saturating_duration_since(Instant::now())
                    + Duration::from_millis(1),
            );
        },
    );
    assert_eq!(observations.load(Ordering::Relaxed), 1);
    assert_eq!(result, Err(ExactOccurrenceError::DeadlineExceeded));
}
