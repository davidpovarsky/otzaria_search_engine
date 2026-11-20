# Default Rust test inventory and legacy diagnostic incident

The original default-library invocation on the primitive source executed364 tests:
360 passed and4 failed (exit101). The19 new tests are included in360. The four actual
failure outputs are retained in full-default-regression.log.

## The four ambient diagnostics

| Test | Opening code and purpose |
| --- | --- |
| api::benchmark::tests::test_run_benchmark_on_real_index | api/benchmark.rs525–533 sets the hardcoded path, then RegexBenchmarker::new32–35 calls SearchEngine::new; original smaller real-index regex query set. |
| api::benchmark::tests::test_comprehensive_benchmark | api/benchmark.rs597–605 uses the same path/constructor and the original comprehensive benchmark/assertions. |
| api::diagnostic_test::tests::test_diagnostic | api/diagnostic_test.rs96 calls run_diagnostic_test5–17, opening the same index to diagnose query behavior. |
| api::focused_benchmark::tests::test_focused_benchmark | api/focused_benchmark.rs129 calls run_focused_benchmark5–17 for the original focused regex phrase queries. |

Their path is C:\אוצריא\index. It existed. All four failed the schema check. These are
ambient diagnostics, not isolated unit fixtures; no original user-path rerun is allowed.

## Source trace before writes

At original base265a14, SearchEngine::new1727 opens MmapDirectory and1728 calls
Index::open_or_create. The schema-error arm1729–1734 panics. Writer construction is
later at1791; ensure_current_index_metadata is at1799. Neither is reached by these
recorded failures.

Pinned Tantivy0.26.1 index/index.rs218–231 checks Index::exists(meta.json), reads
Index::open, compares schema and returns SchemaError. It does not take the create
branch when meta.json exists. Index::open510–516 calls ManagedDirectory::wrap and
load_metas; wrap64–90 reads managed metadata. MmapDirectory::open236–270 checks and
canonicalizes the existing directory; atomic_read451–468 uses File::open/read_to_end.

This is a source trace, not a fabricated filesystem before/after comparison.

## Observation boundary

Read-only after-run observation recorded180 entries; the latest observed creation
and modification dates were June2026. The old meta.json/.managed.json existed and
otzaria_index_meta.json was absent. No pre-run snapshot/content hashes were captured.
There is no claim that every byte, access timestamp, or filesystem event was unchanged.
No repair, replacement, reindex, new user index or user-path cleanup was attempted.
The full filename inventory stays local under build; the committed summary is
legacy-index-post-run-observation.json. The full test process has exited.

## Isolated default-feature cases inspected

- exact_occurrences::tests:19 actual original-engine TempDir or real Tantivy RAM tests.
  None opens user data; private observation cannot replace provider/query/positions.
- api::search_engine::tests: inspected constructors/write/remove sites derive paths
  from make_engine/TempDir/NamedTempFile. Indexed /books/... identities are metadata.
- display_highlight, hebrew_query, hebrew_tokenizer, magic::blacklist and
  magic::normalize tests use constructed text/values.
- lexicons tests use NamedTempFile dictionaries.
- magic::dictionary tests use tempdir-derived SQLite files. In particular
  large_lemma_does_not_materialize_cross_product296–339 writes10,000 rows to its
  own tempdir/lexical.db, then opens it read-only through MagicDictionary; it passed.
- api::benchmark::tests::test_benchmark_creation has no active index constructor.
- semantic_corpus/production embedding feature suites were not part of this
  default-feature run; they require their own inventory before execution.

## Approved separate follow-up

Make the four diagnostics ignored by default with explicit opt-in to an owned
disposable fixture. Preserve public APIs, original query sets and assertions.
No default test may fall back to the user's index. Report ignored/NOT_RUN separately
from executed tests; no new corpus is needed just to turn optional diagnostics green.
Until the separate hygiene commit is present, do not repeat the original unfiltered
library command.

## Completed separate test-hygiene follow-up

The safe ordinary library suite executed 360 tests: 360 passed, 0 failed, and
4 optional diagnostics were explicitly ignored. The repeated focused suite
passed all 19 tests (0 failures). The separate filesystem guard probe passed
8 checks without linking the engine. These are not Native or Apple SDK results.

See TEST_HYGIENE.md and verification-hygiene.json. The earlier actual 360 PASS /
4 FAIL run remains unchanged in its original logs and verification record.
PRE_WRITER_FAILURE_TRACE.md contains exact original engine/Tantivy source excerpts.
No optional real-index diagnostic was rerun after the hygiene change.
