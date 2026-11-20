//! Bounded occurrence counting over the original lexical index.
//!
//! Rust-to-Rust only: no Dart/C binding is registered by this module. A success
//! counts distinct token positions in live documents matching the original
//! exact query. It is exact for one held Searcher snapshot, not evidence that
//! an application database, selected edition, or installed corpus is complete.
//! The host must bind this proof to its own database/profile/index witness.
//!
//! Cancellation is cooperative around Tantivy calls and position processing.
//! A call already executing inside Tantivy cannot be preempted here; a late
//! return is checked before any successful result is constructed.

use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use tantivy::postings::Postings;
use tantivy::query::{EnableScoring, Query};
use tantivy::schema::{FieldType, IndexRecordOption, Term};
use tantivy::{DocSet, Searcher, SearcherGeneration, TERMINATED};

/// Hard ceilings bound caller-controlled work and per-document allocation.
/// Lower request limits remain authoritative; exceeding any limit returns an
/// error and never a partial numeric result.
pub const MAX_OCCURRENCE_CANDIDATES: u64 = 1_000_000;
pub const MAX_OCCURRENCE_POSITIONS: u64 = 10_000_000;
pub const MAX_OCCURRENCE_POSITIONS_PER_DOCUMENT: u32 = 262_144;
pub const MAX_OCCURRENCE_SEGMENTS: usize = 4_096;
pub const MAX_OCCURRENCE_TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExactOccurrenceLimits {
    /// All visited term postings, including deleted and out-of-scope docs.
    pub max_candidate_documents: u64,
    /// Raw positions inspected, before deduplicating stacked positions.
    pub max_positions: u64,
    pub max_positions_per_document: u32,
    pub max_segments: usize,
}

impl Default for ExactOccurrenceLimits {
    fn default() -> Self {
        Self {
            max_candidate_documents: 100_000,
            max_positions: 1_000_000,
            max_positions_per_document: 65_536,
            max_segments: 1_024,
        }
    }
}

/// One cancellation token per request. Clones share cancellation, and a
/// cancelled token cannot be reset to authorize a later invocation.
#[derive(Debug, Clone, Default)]
pub struct ExactOccurrenceCancellation(Arc<AtomicBool>);

impl ExactOccurrenceCancellation {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

#[derive(Debug, Clone)]
pub struct ExactOccurrenceControl {
    limits: ExactOccurrenceLimits,
    deadline: Instant,
    cancellation: ExactOccurrenceCancellation,
}

impl ExactOccurrenceControl {
    pub fn new(
        limits: ExactOccurrenceLimits,
        deadline: Instant,
        cancellation: ExactOccurrenceCancellation,
    ) -> Result<Self, ExactOccurrenceError> {
        if limits.max_candidate_documents > MAX_OCCURRENCE_CANDIDATES
            || limits.max_positions > MAX_OCCURRENCE_POSITIONS
            || limits.max_positions_per_document > MAX_OCCURRENCE_POSITIONS_PER_DOCUMENT
            || limits.max_segments > MAX_OCCURRENCE_SEGMENTS
        {
            return Err(ExactOccurrenceError::InvalidLimits);
        }
        if deadline
            .checked_duration_since(Instant::now())
            .is_some_and(|remaining| remaining > MAX_OCCURRENCE_TIMEOUT)
        {
            return Err(ExactOccurrenceError::InvalidLimits);
        }
        Ok(Self {
            limits,
            deadline,
            cancellation,
        })
    }

    pub fn with_timeout(
        limits: ExactOccurrenceLimits,
        timeout: Duration,
        cancellation: ExactOccurrenceCancellation,
    ) -> Result<Self, ExactOccurrenceError> {
        if timeout > MAX_OCCURRENCE_TIMEOUT {
            return Err(ExactOccurrenceError::InvalidLimits);
        }
        let deadline = Instant::now()
            .checked_add(timeout)
            .ok_or(ExactOccurrenceError::InvalidLimits)?;
        Self::new(limits, deadline, cancellation)
    }

    pub(crate) fn check(&self) -> Result<(), ExactOccurrenceError> {
        if self.cancellation.is_cancelled() {
            return Err(ExactOccurrenceError::Cancelled);
        }
        if Instant::now() >= self.deadline {
            return Err(ExactOccurrenceError::DeadlineExceeded);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExactOccurrenceBudget {
    CandidateDocuments,
    Positions,
    PositionsPerDocument,
    Segments,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExactOccurrenceError {
    /// Zero or multiple tokens after the original exact-query analyzer.
    SingleAnalyzedTokenRequired {
        token_count: usize,
    },
    QueryTooLong,
    InvalidFacets,
    InvalidLimits,
    PositionsUnavailable,
    InvalidPositions,
    BudgetExceeded(ExactOccurrenceBudget),
    CountOverflow,
    Cancelled,
    DeadlineExceeded,
    Engine(String),
}

impl fmt::Display for ExactOccurrenceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SingleAnalyzedTokenRequired { token_count } => {
                write!(
                    f,
                    "exact occurrence counting requires one analyzed token, got {token_count}"
                )
            }
            Self::QueryTooLong => f.write_str("occurrence query exceeds the supported bound"),
            Self::InvalidFacets => {
                f.write_str("occurrence facet filters exceed the supported bound")
            }
            Self::InvalidLimits => f.write_str("invalid occurrence count limits"),
            Self::PositionsUnavailable => {
                f.write_str("complete indexed token positions unavailable")
            }
            Self::InvalidPositions => f.write_str("indexed token positions failed validation"),
            Self::BudgetExceeded(budget) => {
                write!(f, "occurrence count budget exceeded: {budget:?}")
            }
            Self::CountOverflow => f.write_str("occurrence count overflow"),
            Self::Cancelled => f.write_str("occurrence count cancelled"),
            Self::DeadlineExceeded => f.write_str("occurrence count deadline exceeded"),
            Self::Engine(message) => write!(f, "occurrence engine error: {message}"),
        }
    }
}

impl std::error::Error for ExactOccurrenceError {}

impl From<tantivy::TantivyError> for ExactOccurrenceError {
    fn from(error: tantivy::TantivyError) -> Self {
        Self::Engine(error.to_string())
    }
}

impl From<std::io::Error> for ExactOccurrenceError {
    fn from(error: std::io::Error) -> Self {
        Self::Engine(error.to_string())
    }
}

/// Engine-source proof. Generation includes the real segment UUID/delete
/// opstamp map, not just a process-local generation number. The engine schema
/// and package version do not substitute for the host's pinned build identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExactOccurrenceProof {
    pub analyzed_token: String,
    /// Sorted/deduplicated request strings passed to the original facet query.
    /// These are not a claim that any catalog resolves a named work/edition.
    pub requested_facets: Vec<String>,
    pub searcher_generation: SearcherGeneration,
    pub live_index_documents: u64,
    pub index_schema_version: u32,
    pub engine_package_version: String,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ExactOccurrenceWork {
    pub visited_candidate_documents: u64,
    pub inspected_positions: u64,
    pub visited_segments: usize,
}

/// Constructed only after exhausting the query in one snapshot with every
/// budget, cancellation, deadline, and position-integrity check satisfied.
/// No lower-bound or partial success variant exists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExactTokenOccurrenceCount {
    pub occurrences: u64,
    pub matching_live_documents: u64,
    pub proof: ExactOccurrenceProof,
    pub work: ExactOccurrenceWork,
}

pub(crate) fn validate_request(
    query: &str,
    facets: &[String],
    control: &ExactOccurrenceControl,
) -> Result<(), ExactOccurrenceError> {
    control.check()?;
    if query.chars().take(1_001).count() > 1_000 {
        return Err(ExactOccurrenceError::QueryTooLong);
    }
    if facets.len() > 200
        || facets
            .iter()
            .any(|facet| facet.is_empty() || facet.chars().take(513).count() > 512)
    {
        return Err(ExactOccurrenceError::InvalidFacets);
    }
    Ok(())
}

/// The owning SearchEngine supplies its original analyzer term, exact query,
/// schema version and one Searcher. No stored text or snippets are read.
pub(crate) fn count_on_snapshot(
    searcher: &Searcher,
    term: Term,
    analyzed_token: String,
    requested_facets: Vec<String>,
    query: &dyn Query,
    index_schema_version: u32,
    control: &ExactOccurrenceControl,
) -> Result<ExactTokenOccurrenceCount, ExactOccurrenceError> {
    count_on_snapshot_observed(
        searcher,
        term,
        analyzed_token,
        requested_facets,
        query,
        index_schema_version,
        control,
        |_| {},
    )
}

/// The private observer supports deterministic tests that cancel after real
/// postings have been consumed. Production always supplies the no-op closure.
/// It cannot replace the query, corpus, counters, positions or control check.
#[allow(clippy::too_many_arguments)]
fn count_on_snapshot_observed(
    searcher: &Searcher,
    term: Term,
    analyzed_token: String,
    requested_facets: Vec<String>,
    query: &dyn Query,
    index_schema_version: u32,
    control: &ExactOccurrenceControl,
    mut observe: impl FnMut(ExactOccurrenceWork),
) -> Result<ExactTokenOccurrenceCount, ExactOccurrenceError> {
    control.check()?;
    let has_positions = match searcher.schema().get_field_entry(term.field()).field_type() {
        FieldType::Str(options) => options.get_indexing_options().is_some_and(|indexing| {
            indexing.index_option() == IndexRecordOption::WithFreqsAndPositions
        }),
        _ => false,
    };
    if !has_positions {
        return Err(ExactOccurrenceError::PositionsUnavailable);
    }
    if searcher.segment_readers().len() > control.limits.max_segments {
        return Err(ExactOccurrenceError::BudgetExceeded(
            ExactOccurrenceBudget::Segments,
        ));
    }
    let weight = query.weight(EnableScoring::disabled_from_searcher(searcher))?;
    control.check()?;
    let mut work = ExactOccurrenceWork::default();
    let mut occurrences = 0u64;
    let mut matching_live_documents = 0u64;
    let mut positions = Vec::new();

    for reader in searcher.segment_readers() {
        control.check()?;
        work.visited_segments += 1;
        let inverted = reader.inverted_index(term.field())?;
        let Some(mut postings) =
            inverted.read_postings(&term, IndexRecordOption::WithFreqsAndPositions)?
        else {
            control.check()?;
            continue;
        };
        let mut scorer = weight.scorer(reader, 1.0)?;
        control.check()?;

        while postings.doc() != TERMINATED {
            control.check()?;
            work.visited_candidate_documents = work
                .visited_candidate_documents
                .checked_add(1)
                .ok_or(ExactOccurrenceError::CountOverflow)?;
            if work.visited_candidate_documents > control.limits.max_candidate_documents {
                return Err(ExactOccurrenceError::BudgetExceeded(
                    ExactOccurrenceBudget::CandidateDocuments,
                ));
            }
            let doc = postings.doc();
            // Scorers do not remove tombstones. Match Tantivy's original
            // collector rule explicitly before looking at any positions.
            if reader
                .alive_bitset()
                .is_none_or(|alive| alive.is_alive(doc))
            {
                // DocSet::seek requires target >= current doc; an original
                // facet scorer can already be ahead of this term posting.
                if scorer.doc() < doc {
                    scorer.seek(doc);
                    control.check()?;
                }
                if scorer.doc() == TERMINATED {
                    break;
                }
                if scorer.doc() == doc {
                    let raw_frequency = postings.term_freq();
                    if raw_frequency == 0 {
                        return Err(ExactOccurrenceError::InvalidPositions);
                    }
                    if raw_frequency > control.limits.max_positions_per_document {
                        return Err(ExactOccurrenceError::BudgetExceeded(
                            ExactOccurrenceBudget::PositionsPerDocument,
                        ));
                    }
                    let next_positions = work
                        .inspected_positions
                        .checked_add(u64::from(raw_frequency))
                        .ok_or(ExactOccurrenceError::CountOverflow)?;
                    if next_positions > control.limits.max_positions {
                        return Err(ExactOccurrenceError::BudgetExceeded(
                            ExactOccurrenceBudget::Positions,
                        ));
                    }
                    // Check frequency before allocation. Counting its value
                    // directly would double count duplicate stacked positions.
                    postings.positions(&mut positions);
                    control.check()?;
                    if positions.len() != raw_frequency as usize {
                        return Err(ExactOccurrenceError::PositionsUnavailable);
                    }
                    let mut distinct = 0u64;
                    let mut previous = None;
                    for (index, &position) in positions.iter().enumerate() {
                        if index % 256 == 0 {
                            control.check()?;
                        }
                        if previous.is_some_and(|last| position < last) {
                            return Err(ExactOccurrenceError::InvalidPositions);
                        }
                        if previous != Some(position) {
                            distinct = distinct
                                .checked_add(1)
                                .ok_or(ExactOccurrenceError::CountOverflow)?;
                        }
                        previous = Some(position);
                    }
                    occurrences = occurrences
                        .checked_add(distinct)
                        .ok_or(ExactOccurrenceError::CountOverflow)?;
                    matching_live_documents = matching_live_documents
                        .checked_add(1)
                        .ok_or(ExactOccurrenceError::CountOverflow)?;
                    work.inspected_positions = next_positions;
                    observe(work);
                    control.check()?;
                }
            }
            postings.advance();
            control.check()?;
        }
    }

    control.check()?;
    let proof = ExactOccurrenceProof {
        analyzed_token,
        requested_facets,
        searcher_generation: searcher.generation().clone(),
        live_index_documents: searcher.num_docs(),
        index_schema_version,
        engine_package_version: env!("CARGO_PKG_VERSION").to_string(),
    };
    control.check()?;
    Ok(ExactTokenOccurrenceCount {
        occurrences,
        matching_live_documents,
        proof,
        work,
    })
}

#[cfg(test)]
mod tests;
