# Explicit disposable-fixture diagnostics

This is a separate test-hygiene change after primitive commit
fffd346555d4c4a0e487b30619aa79253b2372c3. It does not change the new occurrence
algorithm, its 19 tests, the generated FFI, Cargo dependencies, or public engine
API signatures.

## What changed

Four existing tests are explicitly ignored by default:

- api::benchmark::tests::test_run_benchmark_on_real_index
- api::benchmark::tests::test_comprehensive_benchmark
- api::diagnostic_test::tests::test_diagnostic
- api::focused_benchmark::tests::test_focused_benchmark

The query sets, original benchmark assertions, and diagnostic behavior are
preserved. The two public diagnostic wrappers retain their existing behavior.
Small private helpers accept a path so their tests never invoke the ambient
public wrappers. The test-only fixture helper lives outside the FFI-scanned API
module.

An explicit opt-in test requires all of these conditions before constructing
SearchEngine:

1. OTZARIA_DIAGNOSTIC_FIXTURE_ROOT is set to an absolute, existing directory.
2. Its canonical parent is the canonical platform temporary directory.
3. Its name starts with otzaria-diagnostic-fixture- and has a unique suffix of at
   least 12 ASCII letters, digits, or hyphens.
4. The root contains the exact ownership marker documented below.
5. Its pre-existing index child canonicalizes within that same root and retains
   the index name. A link escaping the root is rejected.
6. The index directory already contains meta.json. This guard creates no index,
   repairs no schema, and supplies no fallback path.

The caller who owns the disposable fixture must create the marker
.otzaria-disposable-diagnostic-fixture with exactly this UTF-8 content including
its final LF newline:

    I own this disposable fixture and authorize diagnostic writes.

This declaration is an explicit ownership/permission assertion by the fixture
creator. It is not a security sandbox or an ACL ownership attestation. Use only
a disposable corpus that the caller owns and may mutate. Never point this test
workflow at a user's live index, or copy a marker onto a live index.

## Opt-in commands

Supply an already built, schema-compatible disposable fixture using the
environment variable above. Run each selected test with --exact and --ignored,
for example:

    cargo test --locked --manifest-path rust/Cargo.toml --lib api::diagnostic_test::tests::test_diagnostic -- --exact --ignored --test-threads=1

The other three exact test names are listed above. These four optional
benchmarks were NOT_RUN after the hygiene change. Ignored tests are not passes.
The original bodies also print some query errors rather than asserting on them;
this historical behavior was preserved and is not a new completeness claim.

## Guard verification

diagnostic_fixture_guard_probe.rs compiles the actual test-only helper in an
isolated executable, with no SearchEngine or Tantivy linked. It verifies seven
rejections (missing path, relative path, temporary root itself, missing marker,
incorrect marker, missing index, missing metadata) and one accepted explicitly
owned fixture path. Its own temporary directory contains only a marker and a
placeholder metadata file; the placeholder is never opened as an index. The
probe removes only the temporary directory it created.

The 8 checks passed. See diagnostic-fixture-guard.json for command, result and
source hashes. They are separate guard checks, not extra engine or Native tests.

## Historical failure evidence remains

full-default-regression.log and verification-primitive.json remain unchanged:
360 passed and 4 failed in the original default suite. All four actual failures
occurred while opening C:\אוצריא\index with an incompatible schema. See
SAFE_TEST_INVENTORY.md and PRE_WRITER_FAILURE_TRACE.md for the exact call path.
The previous result is not relabeled as a full pass.

The only user-index filesystem observation was after that original run.
legacy-index-post-run-observation.json records that limitation. There is no
before snapshot, byte-equality claim, repair, reindex, replacement, or second
user-index opening in this follow-up.

The safe ordinary suite and the repeated focused suite are recorded separately
in safe-library-regression.json and focused-after-hygiene.json. Their ignored
counts must be reported separately from executed tests.

## Remaining integration gates

This is Rust source plus host validation. Apple SDK builds, an XCFramework,
FFI integration, host source identity binding, Core capability exposure, and
Native acceptance are not completed by these tests. The parent integration
agent owns review and any publication to an explicitly owned fork; no engine
upstream push or application pin change was performed.
