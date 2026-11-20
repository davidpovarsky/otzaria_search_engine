# Exact single-token occurrence primitive

## Scope and status

This is a Rust-only additive primitive on the canonical Otzaria engine at base commit
265a14ea54f959a3e266d6e8acfa3a3b98101d68. It does not change existing search, document-count,
facet-count, grouping, tokenizer, index schema, or generated Dart/C APIs. The application
has not been connected to it. Maktabah's search.countOccurrences capability must remain
unsupported until the reviewed engine build, host binding, source witness, and acceptance
gates actually pass.

The engine method is count_exact_token_occurrences(query, facets, control). It requires
exactly one token after the original hebrew_query analyzer and executes the original
mark-free build_exact_query including its facet semantics. No phrase, fuzzy, morphological,
regex, vocalization-specific or multi-token counting mode is exposed. Input is normalized
exactly as an existing exact query; "exact" means that original indexed-token semantics,
not byte-for-byte equality of raw strings.

## Result contract

A successful ExactTokenOccurrenceCount contains:

| Field | Meaning |
| --- | --- |
| occurrences: u64 | Distinct token positions in live matching documents. Repeated words in one document each count; duplicate stacked postings at one position count once. |
| matching_live_documents: u64 | A separate diagnostic count of matching documents. It is never substituted for occurrences. |
| proof.analyzed_token | The token emitted by the original query analyzer. |
| proof.requested_facets | The sorted, deduplicated filter strings used by the original facet query. These are not a claim of catalog/work/edition resolution. |
| proof.searcher_generation | The real Tantivy SearcherGeneration: generation ID plus segment UUID/delete-opstamp mapping. |
| proof.live_index_documents | The live-document count of that same held Searcher. This is an index metric, not corpus-coverage certification. |
| proof.index_schema_version | The original engine's schema constant. |
| proof.engine_package_version | The original crate package version; the host must separately bind the reviewed build commit/artifact. |
| work | Candidate documents visited (including deleted/out-of-scope term postings), raw positions inspected, and segments visited. |

Success has no lower-bound/partial alternative. Cancellation, deadline expiry, any budget
exhaustion, invalid/missing positions, malformed input, numeric overflow or engine error
returns a typed error without an exact numeric result.

The implementation holds one Searcher throughout. It walks real term postings, matches
the original query scorer, explicitly applies the segment alive bitset (scorers alone
do not remove deleted documents), checks raw frequency before allocating positions, and
counts distinct sorted positions. It never reads stored text or snippets, constructs an
alternate tokenizer/search engine, sums result rows, or pages through UI hits.

## Bounds, cancellation and deadlines

Limits must be explicit or use the documented finite defaults. Hard ceilings are
1,000,000 candidate documents, 10,000,000 raw positions, 262,144 positions in one
document, 4,096 index segments, and 60 seconds. Defaults are lower: 100,000 candidate
documents, 1,000,000 positions, 65,536 per document, 1,024 segments.

The query is bounded to 1,000 Unicode scalar characters; at most 200 filter strings,
each nonempty and at most 512 characters, are accepted. Zero configured work budgets
are valid and still cannot turn an incomplete search into exact zero.

ExactOccurrenceCancellation is an Arc-backed atomic one-way token. Clones share the
same cancellation; no reset can authorize a later invocation. ExactOccurrenceControl
holds a monotonic Instant deadline and validated limits. Checks run before query work,
around segment/query/postings operations, during long position processing, after a
real matched document, and before constructing success. Integer additions use checked
arithmetic.

Cancellation is cooperative. An individual synchronous operation already executing
inside Tantivy cannot be preempted by this wrapper; it is checked on return. The API
does not promise a hard real-time cutoff. The host must run it off the main actor,
forward cancellation, and revalidate the invocation deadline and source witness after
awaiting it. A total elapsed timeout must never be relabelled exact success.

## Host responsibilities before a future FFI integration

The primitive certifies only its held index snapshot. A host still has to prove:

1. The chosen backend is Otzaria, the request is the supported exact single-token
   subset, and the original catalog resolves every requested work/edition.
2. Actual book IDs/facets correspond to those catalog records; a Hebrew label such
   as "Torah" is not itself an authoritative filter.
3. The opened index is compatible and complete for the intended source/scope. A
   nonempty index alone is insufficient. Preserve the existing managed artifact
   manifest/document-count validation and local database identity checks; require
   stronger coverage proof where those checks do not establish completeness.
4. The same database generation, profile, selected backend, search publication
   revision, index meta/identity fingerprint, invocation deadline and authority
   hold before and after the count. Existing A-to-B-to-A invalidation protection
   must remain effective.
5. Any follow-up openSearchResults uses that same typed query/scope/source rather
   than a new textual reconstruction. If the source changes, do not associate
   an old scalar with new results.
6. Deleted docs, unavailable index positions, unsupported modes/providers, and
   exhausted budgets remain explicit failures, never matching-hit fallbacks.

No Swift/Core/UIRouting, iOS bridge, C header, source pin, artifact identity or CI
checkout/build guard is modified in this tranche.

## Original source audit

At the pinned base:

- rust/src/api/search_engine.rs: count_exact_with_status calls run_count, whose
  collector is Tantivy Count. BookCountCollector adds one per matching document.
  SearchPageResult.total_count and merged_count are document/group metrics.
- The same source schema stores WithFreqsAndPositions on text and textVocalized.
  gap_phrase.rs already reads full positions to verify phrase feasibility; it
  exposes a boolean predicate, not a public occurrence count.
- The original Hebrew index tokenizer emits quote-free synonyms at the same
  position; its separate query analyzer avoids emitting that duplicate query
  token. Distinct positions are therefore the appropriate counting unit.
- Maktabah's iOS bridge exposes search pages/book counts/facet counts, but no
  complete-position or occurrence-count/cancellation API.
- Maktabah's Zayit adapter exposes line hits and a bounded snippet. The snippet
  builder selects bounded windows/anchors and cannot certify all occurrences.
- Legacy SQLite search exposes paged rows and bounded highlights; the fallback
  total can be a lower-bound estimate, not an exact occurrence count.
- Sefaria remote/embedded search contracts return hit totals and snippets, without
  complete match positions or an atomic provider occurrence snapshot. Their
  current Core gates remain unsupported for occurrence counting.

## Ownership and publication

The original repository remote is https://github.com/Otzaria/otzaria_search_engine.git.
No commit is pushed there. The MIT license and attribution remain intact.

The Maktabah build currently demands a pristine checkout at the exact UPSTREAM_COMMIT;
there is no reviewed engine-patch application path. Do not weaken those guards or
silently build a dirty upstream tree. The parent will review the local additive commit
and full source patch, then arrange an explicit owner-controlled fork/pin and genuine
Apple artifact validation if appropriate. A read-only GitHub lookup did not resolve
davidpovarsky/otzaria_search_engine in the current authenticated context.

## Validation evidence

baseline_occurrence_probe.rs is a test-only counterexample against the unmodified
pinned production source: a real index document containing two instances of "משה"
has original document count 1. The deliberate acceptance assertion expecting an
occurrence count of 2 fails. This is not a bug in the documented document-count API.

The final verification record and raw logs identify the exact source hashes, commands,
exit codes and executed tests. Failed compilation attempts are retained as failed;
they are not counted as behavioral or Native passes. The new focused tests use the
real original engine. Five low-level tests also construct a real Tantivy index for
stacked duplicate positions, missing-position schema, held-snapshot behavior and
deterministic cancellation/deadline after reading real positions. Their private
observer cannot replace documents, query, counters or positions; production supplies
a no-op observer.

Apple SDK, XCFramework, Simulator/device Native, live Sefaria, and application/Core
exposure are outside this Rust tranche and remain NOT_RUN here.
