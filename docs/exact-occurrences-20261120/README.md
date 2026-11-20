# Exact-token occurrence primitive — 2026-11-20

The additive Rust primitive counts distinct indexed token positions for one token produced by the original exact analyzer. Existing document-count APIs retain their semantics. The original SearchEngine owns query construction, facets, index and tokenizer; no text scanning or alternate engine is introduced.

- Base: `265a14ea54f959a3e266d6e8acfa3a3b98101d68`.
- Focused original-engine tests: **19 passed, 0 failed**.
- Full default Rust library invocation: **360 passed, 4 failed**, exit101. The 19 new tests are included in 360. Four unchanged ambient diagnostics attempted the existing hardcoded user index and failed its schema check. This is not a full-suite PASS.
- The original API counterexample really ran: one document containing two occurrences returned document count1, and the deliberate occurrence assertion expecting2 failed.
- A compiled attempt failed on imports; the next behavioral attempt passed18/19, with a deletion fixture that did not guarantee a surviving tombstone. The fixture now calls original optimize before deletion; the strict deleted-position/snapshot assertions pass. All attempt logs are retained.
- Source-only patch applies cleanly to the pristine base index; `source.patch.gz` and `patch-verification.json` contain its exact bytes/hash.
- No FFI, Dart generated API, app/Core capability, pin, schema, Cargo dependency or artifact change. Apple SDK/XCFramework/Native remain **NOT_RUN**.

Read [CONTRACT.md](CONTRACT.md) for exact semantics, cooperative cancellation/deadline limits, source/scope proof and future host requirements. [verification-primitive.json](verification-primitive.json) binds the commands/results to LF-normalized source hashes.

## Legacy diagnostic boundary

The four actual failures are retained in `full-default-regression.log`. The constructor failed at `SearchEngine::new`'s `Index::open_or_create` schema check, before its writer creation or metadata helper. The post-run observation records that the hardcoded path existed,180 directory entries had creation/modification dates no later than June2026, and no engine metadata sidecar was present. There was no pre-run snapshot, so this evidence does not assert byte-for-byte unchanged state. No repair/reindex/replacement was attempted.

A separate approved test-hygiene follow-up will make these manual diagnostics explicit opt-in with an owned disposable fixture, retaining their query sets/assertions and reporting excluded diagnostics as ignored/NOT_RUN. Do not repeat the original ambient full-suite invocation.

## Ownership

Only 44 lines are added to original upstream files (`api/search_engine.rs` and `lib.rs`); the primitive and its tests are separate new modules. The upstream MIT license stays intact. The commit is local only: no push to `Otzaria/otzaria_search_engine`. Parent review/publication uses an explicit owner-controlled fork/pin with strict pristine checkout and artifact gates.

## Completed separate test-hygiene follow-up

The safe ordinary library suite executed 360 tests: 360 passed, 0 failed, and
4 optional diagnostics were explicitly ignored. The repeated focused suite
passed all 19 tests (0 failures). The separate filesystem guard probe passed
8 checks without linking the engine. These are not Native or Apple SDK results.

See TEST_HYGIENE.md and verification-hygiene.json. The earlier actual 360 PASS /
4 FAIL run remains unchanged in its original logs and verification record.
PRE_WRITER_FAILURE_TRACE.md contains exact original engine/Tantivy source excerpts.
No optional real-index diagnostic was rerun after the hygiene change.
