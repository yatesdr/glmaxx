use std::{
    collections::{BTreeMap, BTreeSet},
    fmt, fs,
    path::{Component, Path, PathBuf},
    process::Command,
};

use serde::Serialize;
use sha2::{Digest, Sha256};

const CANDIDATE_LABELS: [&str; 3] = [
    "Review candidate commit:",
    "Candidate base commit:",
    "Base commit:",
];
const REQUESTED_TOKEN_LABEL: &str =
    "Requested acceptance token, only if every blocker and major is resolved:";
const REQUIRED_RESULT_LABEL: &str = "Required result path:";
const HISTORICAL_HANDOFFS: [&str; 2] = [
    "docs/fable-phase-a-engine-handoff.md",
    "docs/fable-review-handoff.md",
];
const REVIEW_QUEUE_PATH: &str = "docs/fable-review-queue-all-20260730.md";

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ReviewInputProof {
    pub path: String,
    pub expected_sha256: String,
    pub actual_sha256: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ReviewTokenState {
    NotConfigured,
    Withheld,
    Accepted,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ReviewArtifactProof {
    pub path: String,
    pub sha256: String,
    pub token_state: ReviewTokenState,
    pub exact_token_lines: usize,
    pub candidate_commit_attested: bool,
    pub attested_input_hashes: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ReviewProof {
    pub schema: &'static str,
    pub repository_head: String,
    pub handoff_path: String,
    pub handoff_sha256: String,
    pub candidate_commit: String,
    pub required_result_path: Option<String>,
    pub requested_acceptance_token: Option<String>,
    pub handoff_bare_acceptance_lines: Vec<String>,
    pub inputs: Vec<ReviewInputProof>,
    pub review: Option<ReviewArtifactProof>,
    pub verdict: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ReviewSuiteProof {
    pub schema: &'static str,
    pub repository_head: String,
    pub verified_handoffs: Vec<ReviewProof>,
    pub skipped_historical_handoffs: Vec<String>,
    pub configured_review_results: usize,
    pub present_review_results: usize,
    pub accepted_review_results: usize,
    pub withheld_review_results: usize,
    pub verdict: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum StagedReviewLintState {
    Ready,
    Rejected,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct StagedReviewLintEntry {
    pub handoff_path: String,
    pub required_result_path: String,
    pub staged_review_path: String,
    pub state: StagedReviewLintState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proof: Option<ReviewProof>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct StagedReviewLintSuite {
    pub schema: &'static str,
    pub repository_head: String,
    pub staging_directory: String,
    pub configured_handoffs: usize,
    pub present_staged_reviews: usize,
    pub ready_staged_reviews: usize,
    pub rejected_staged_reviews: usize,
    pub absent_staged_reviews: usize,
    pub unmatched_staging_files: Vec<String>,
    pub entries: Vec<StagedReviewLintEntry>,
    pub verdict: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ParsedInput {
    path: String,
    expected_sha256: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ParsedHandoff {
    candidate_commit: String,
    required_result_path: Option<String>,
    requested_acceptance_token: Option<String>,
    bare_acceptance_lines: Vec<String>,
    inputs: Vec<ParsedInput>,
}

pub fn verify_review_handoff(
    repository: &Path,
    handoff: &Path,
    review: Option<&Path>,
) -> Result<ReviewProof, ReviewProofError> {
    verify_review_handoff_with_policy(
        repository,
        handoff,
        review,
        true,
        "glmaxx.review-provenance-proof.v2",
        "PASS",
    )
}

/// Verifies that an operator-owned staged review contains everything needed
/// for acceptance after an exact byte-for-byte copy to the handoff's required
/// result path.
///
/// This deliberately does not relax `review-proof` or `review-proof-all`.
/// The returned schema and verdict state that the staged artifact is not a
/// recorded acceptance, and repository-wide acceptance counts remain driven
/// only by the exact required result path.
pub fn verify_staged_review_acceptance(
    repository: &Path,
    handoff: &Path,
    review: &Path,
) -> Result<ReviewProof, ReviewProofError> {
    let proof = verify_review_handoff_with_policy(
        repository,
        handoff,
        Some(review),
        false,
        "glmaxx.review-staged-acceptance-proof.v1",
        "STAGED_CONTENT_PASS_NOT_RECORDED",
    )?;
    if proof.required_result_path.is_none() {
        return Err(ReviewProofError::Format(
            "staged acceptance lint requires a handoff result path".to_owned(),
        ));
    }
    let requested_token = proof.requested_acceptance_token.as_deref().ok_or_else(|| {
        ReviewProofError::Format(
            "staged acceptance lint requires a requested acceptance token".to_owned(),
        )
    })?;
    let artifact = proof.review.as_ref().ok_or_else(|| {
        ReviewProofError::Format("staged acceptance lint requires a review artifact".to_owned())
    })?;
    require_staged_acceptance(artifact, requested_token)?;
    Ok(proof)
}

/// Validates every staged review whose basename uniquely matches a configured
/// handoff result. The suite is diagnostic only: it neither copies nor accepts
/// review artifacts, and it preserves every single-review acceptance rule.
pub fn verify_all_staged_review_acceptances(
    repository: &Path,
    staging_directory: &Path,
) -> Result<StagedReviewLintSuite, ReviewProofError> {
    let root = repository_root(repository)?;
    let head = git_text(&root, &["rev-parse", "HEAD"])?;
    let staging = repository_file(&root, staging_directory)?;
    if !staging.is_dir() {
        return Err(ReviewProofError::Format(format!(
            "staging path is not a directory: {}",
            staging.display()
        )));
    }
    let staging_relative = relative_string(&root, &staging)?;

    let mut bindings = BTreeMap::<String, (PathBuf, String, String)>::new();
    let mut configured_handoffs = 0usize;
    for handoff in current_handoff_paths(&root)? {
        let handoff_relative = relative_string(&root, &handoff)?;
        let bytes = fs::read(&handoff)?;
        let text = std::str::from_utf8(&bytes)
            .map_err(|_| ReviewProofError::Utf8(handoff_relative.clone()))?;
        if !has_candidate_label(text) {
            if HISTORICAL_HANDOFFS.contains(&handoff_relative.as_str()) {
                continue;
            }
            return Err(ReviewProofError::Format(format!(
                "nonhistorical handoff has no candidate commit label: {handoff_relative}"
            )));
        }
        let parsed = parse_handoff(text)?;
        let Some(required_result_path) = parsed.required_result_path else {
            continue;
        };
        configured_handoffs = configured_handoffs
            .checked_add(1)
            .ok_or_else(|| ReviewProofError::Format("handoff count overflow".to_owned()))?;
        let basename = Path::new(&required_result_path)
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| {
                ReviewProofError::Format(format!(
                    "required result has no UTF-8 basename: {required_result_path}"
                ))
            })?
            .to_owned();
        if let Some((_, prior_handoff, prior_result)) = bindings.get(&basename) {
            return Err(ReviewProofError::Format(format!(
                "staged review basename {basename} is ambiguous between \
                 {prior_handoff} ({prior_result}) and {handoff_relative} \
                 ({required_result_path})"
            )));
        }
        bindings.insert(basename, (handoff, handoff_relative, required_result_path));
    }

    let mut entries = Vec::new();
    let mut matched_basenames = BTreeSet::new();
    for (basename, (handoff, handoff_relative, required_result_path)) in &bindings {
        let staged = staging.join(basename);
        if !staged.try_exists()? {
            continue;
        }
        matched_basenames.insert(basename.clone());
        let staged_relative = relative_string(&root, &staged)?;
        let result = if staged.symlink_metadata()?.file_type().is_file() {
            verify_staged_review_acceptance(&root, handoff, &staged)
        } else {
            Err(ReviewProofError::Format(format!(
                "staged review is not a regular file: {staged_relative}"
            )))
        };
        match result {
            Ok(proof) => entries.push(StagedReviewLintEntry {
                handoff_path: handoff_relative.clone(),
                required_result_path: required_result_path.clone(),
                staged_review_path: staged_relative,
                state: StagedReviewLintState::Ready,
                proof: Some(proof),
                error: None,
            }),
            Err(error) => entries.push(StagedReviewLintEntry {
                handoff_path: handoff_relative.clone(),
                required_result_path: required_result_path.clone(),
                staged_review_path: staged_relative,
                state: StagedReviewLintState::Rejected,
                proof: None,
                error: Some(error.to_string()),
            }),
        }
    }

    let mut unmatched_staging_files = Vec::new();
    for entry in fs::read_dir(&staging)? {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }
        let basename = entry
            .file_name()
            .to_str()
            .ok_or_else(|| ReviewProofError::Utf8("staging directory entry".to_owned()))?
            .to_owned();
        if !matched_basenames.contains(&basename) {
            unmatched_staging_files.push(relative_string(&root, &entry.path())?);
        }
    }
    unmatched_staging_files.sort();

    entries.sort_by(|left, right| left.handoff_path.cmp(&right.handoff_path));
    let present_staged_reviews = entries.len();
    let ready_staged_reviews = entries
        .iter()
        .filter(|entry| entry.state == StagedReviewLintState::Ready)
        .count();
    let rejected_staged_reviews = present_staged_reviews
        .checked_sub(ready_staged_reviews)
        .expect("ready staged reviews are a subset of present reviews");
    let absent_staged_reviews = configured_handoffs
        .checked_sub(present_staged_reviews)
        .expect("present staged reviews are a subset of configured handoffs");
    Ok(StagedReviewLintSuite {
        schema: "glmaxx.review-staged-acceptance-suite.v1",
        repository_head: head,
        staging_directory: staging_relative,
        configured_handoffs,
        present_staged_reviews,
        ready_staged_reviews,
        rejected_staged_reviews,
        absent_staged_reviews,
        unmatched_staging_files,
        entries,
        verdict: if rejected_staged_reviews == 0 {
            "STAGED_CONTENT_PASS_NOT_RECORDED"
        } else {
            "STAGED_CONTENT_REJECTED"
        },
    })
}

fn require_staged_acceptance(
    artifact: &ReviewArtifactProof,
    requested_token: &str,
) -> Result<(), ReviewProofError> {
    if artifact.token_state == ReviewTokenState::Accepted {
        Ok(())
    } else {
        Err(ReviewProofError::AcceptanceTokenMissing {
            path: artifact.path.clone(),
            token: requested_token.to_owned(),
        })
    }
}

fn verify_review_handoff_with_policy(
    repository: &Path,
    handoff: &Path,
    review: Option<&Path>,
    enforce_required_result_path: bool,
    schema: &'static str,
    verdict: &'static str,
) -> Result<ReviewProof, ReviewProofError> {
    let root = repository_root(repository)?;
    let head = git_text(&root, &["rev-parse", "HEAD"])?;
    let handoff_path = repository_file(&root, handoff)?;
    let handoff_relative = relative_string(&root, &handoff_path)?;
    let handoff_bytes = fs::read(&handoff_path)?;
    let handoff_text = std::str::from_utf8(&handoff_bytes)
        .map_err(|_| ReviewProofError::Utf8(handoff_relative.clone()))?;
    let parsed = parse_handoff(handoff_text)?;
    verify_commit(&root, &parsed.candidate_commit)?;

    let mut input_proofs = Vec::with_capacity(parsed.inputs.len());
    for input in &parsed.inputs {
        validate_relative_path(&input.path)?;
        let object = format!("{}:{}", parsed.candidate_commit, input.path);
        let bytes = git_bytes(&root, &["cat-file", "blob", &object])?;
        let actual_sha256 = sha256_hex(&bytes);
        if actual_sha256 != input.expected_sha256 {
            return Err(ReviewProofError::HashMismatch {
                path: input.path.clone(),
                expected: input.expected_sha256.clone(),
                actual: actual_sha256,
            });
        }
        input_proofs.push(ReviewInputProof {
            path: input.path.clone(),
            expected_sha256: input.expected_sha256.clone(),
            actual_sha256,
        });
    }

    let review = review
        .map(|path| verify_review_artifact(&root, path, &parsed, enforce_required_result_path))
        .transpose()?;
    if let Some(artifact) = review.as_ref() {
        ensure_distinct_review_path(&handoff_relative, &artifact.path)?;
    }

    Ok(ReviewProof {
        schema,
        repository_head: head,
        handoff_path: handoff_relative,
        handoff_sha256: sha256_hex(&handoff_bytes),
        candidate_commit: parsed.candidate_commit,
        required_result_path: parsed.required_result_path,
        requested_acceptance_token: parsed.requested_acceptance_token,
        handoff_bare_acceptance_lines: parsed.bare_acceptance_lines,
        inputs: input_proofs,
        review,
        verdict,
    })
}

pub fn verify_all_review_handoffs(repository: &Path) -> Result<ReviewSuiteProof, ReviewProofError> {
    let root = repository_root(repository)?;
    let head = git_text(&root, &["rev-parse", "HEAD"])?;
    let candidates = current_handoff_paths(&root)?;

    let mut verified_handoffs = Vec::new();
    let mut skipped_historical_handoffs = Vec::new();
    let mut configured_review_results = 0usize;
    for handoff in candidates {
        let bytes = fs::read(&handoff)?;
        let relative = relative_string(&root, &handoff)?;
        let text =
            std::str::from_utf8(&bytes).map_err(|_| ReviewProofError::Utf8(relative.clone()))?;
        if has_candidate_label(text) {
            let parsed = parse_handoff(text)?;
            let review_path = if let Some(path) = parsed.required_result_path.as_deref() {
                configured_review_results += 1;
                let path = root.join(path);
                path.try_exists()?.then_some(path)
            } else {
                None
            };
            verified_handoffs.push(verify_review_handoff(
                &root,
                &handoff,
                review_path.as_deref(),
            )?);
        } else if HISTORICAL_HANDOFFS.contains(&relative.as_str()) {
            skipped_historical_handoffs.push(relative);
        } else {
            return Err(ReviewProofError::Format(format!(
                "nonhistorical handoff has no candidate commit label: {relative}"
            )));
        }
    }
    if verified_handoffs.is_empty() {
        return Err(ReviewProofError::Format(
            "no review handoff with a candidate commit was found".to_owned(),
        ));
    }
    verify_review_queue(&root, &verified_handoffs)?;
    let present_review_results = verified_handoffs
        .iter()
        .filter(|proof| proof.review.is_some())
        .count();
    let accepted_review_results = verified_handoffs
        .iter()
        .filter(|proof| {
            proof
                .review
                .as_ref()
                .is_some_and(|review| review.token_state == ReviewTokenState::Accepted)
        })
        .count();
    let withheld_review_results = verified_handoffs
        .iter()
        .filter(|proof| {
            proof
                .review
                .as_ref()
                .is_some_and(|review| review.token_state == ReviewTokenState::Withheld)
        })
        .count();
    Ok(ReviewSuiteProof {
        schema: "glmaxx.review-provenance-suite.v2",
        repository_head: head,
        verified_handoffs,
        skipped_historical_handoffs,
        configured_review_results,
        present_review_results,
        accepted_review_results,
        withheld_review_results,
        verdict: "PASS",
    })
}

fn verify_review_queue(
    root: &Path,
    verified_handoffs: &[ReviewProof],
) -> Result<(), ReviewProofError> {
    let path = root.join(REVIEW_QUEUE_PATH);
    let bytes = fs::read(&path)?;
    let text = std::str::from_utf8(&bytes)
        .map_err(|_| ReviewProofError::Utf8(REVIEW_QUEUE_PATH.to_owned()))?;
    verify_review_queue_text(text, verified_handoffs)
}

fn verify_review_queue_text(
    text: &str,
    verified_handoffs: &[ReviewProof],
) -> Result<(), ReviewProofError> {
    let mut proofs_by_path = BTreeMap::new();
    for proof in verified_handoffs {
        if proofs_by_path
            .insert(proof.handoff_path.as_str(), proof)
            .is_some()
        {
            return Err(ReviewProofError::Format(format!(
                "duplicate verified handoff path: {}",
                proof.handoff_path
            )));
        }
    }

    let mut seen_paths = BTreeSet::new();
    let mut expected_ordinal = 1usize;
    for line in text.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with('|') || !trimmed.ends_with('|') {
            continue;
        }
        let cells: Vec<_> = trimmed
            .trim_matches('|')
            .split('|')
            .map(str::trim)
            .collect();
        let Some(ordinal) = cells.first().and_then(|cell| cell.parse::<usize>().ok()) else {
            continue;
        };
        if cells.len() != 5 {
            return Err(ReviewProofError::Format(format!(
                "review queue row {ordinal} must contain exactly five columns"
            )));
        }
        if ordinal != expected_ordinal {
            return Err(ReviewProofError::Format(format!(
                "review queue ordinal mismatch: expected {expected_ordinal}, observed {ordinal}"
            )));
        }
        expected_ordinal = expected_ordinal
            .checked_add(1)
            .ok_or_else(|| ReviewProofError::Format("review queue ordinal overflow".to_owned()))?;

        let handoff_path = exact_code_cell(cells[1]).ok_or_else(|| {
            ReviewProofError::Format(format!(
                "review queue row {ordinal} handoff path must be an exact inline-code cell"
            ))
        })?;
        validate_relative_path(&handoff_path)?;
        if !seen_paths.insert(handoff_path.clone()) {
            return Err(ReviewProofError::Format(format!(
                "duplicate review queue handoff path: {handoff_path}"
            )));
        }
        let proof = proofs_by_path.get(handoff_path.as_str()).ok_or_else(|| {
            ReviewProofError::Format(format!(
                "review queue row {ordinal} references an unknown handoff: {handoff_path}"
            ))
        })?;

        let candidate = exact_code_cell(cells[2]).ok_or_else(|| {
            ReviewProofError::Format(format!(
                "review queue row {ordinal} candidate must be an exact inline-code cell"
            ))
        })?;
        if !is_lower_hex(&candidate, 40) {
            return Err(ReviewProofError::Format(format!(
                "review queue row {ordinal} candidate must be 40 lowercase hexadecimal characters"
            )));
        }
        if candidate != proof.candidate_commit {
            return Err(ReviewProofError::Format(format!(
                "review queue candidate mismatch for {handoff_path}: expected {}, observed {candidate}",
                proof.candidate_commit
            )));
        }

        if let Some(required_result_path) = exact_code_cell(cells[3])
            && proof.required_result_path.as_deref() != Some(required_result_path.as_str())
        {
            return Err(ReviewProofError::Format(format!(
                "review queue result-path mismatch for {handoff_path}: expected {:?}, observed {required_result_path}",
                proof.required_result_path
            )));
        }
        if let Some(requested_token) = exact_code_cell(cells[4])
            && proof.requested_acceptance_token.as_deref() != Some(requested_token.as_str())
        {
            return Err(ReviewProofError::Format(format!(
                "review queue token mismatch for {handoff_path}: expected {:?}, observed {requested_token}",
                proof.requested_acceptance_token
            )));
        }
    }

    if seen_paths.is_empty() {
        return Err(ReviewProofError::Format(
            "review queue contains no numbered rows".to_owned(),
        ));
    }
    let missing: Vec<_> = proofs_by_path
        .keys()
        .filter(|path| !seen_paths.contains(**path))
        .copied()
        .collect();
    if !missing.is_empty() {
        return Err(ReviewProofError::Format(format!(
            "review queue omits verified handoffs: {}",
            missing.join(", ")
        )));
    }
    Ok(())
}

fn current_handoff_paths(root: &Path) -> Result<Vec<PathBuf>, ReviewProofError> {
    let docs = root.join("docs");
    let mut candidates = Vec::new();
    for entry in fs::read_dir(&docs)? {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }
        let name = entry.file_name();
        let name = name
            .to_str()
            .ok_or_else(|| ReviewProofError::Utf8("docs directory entry".to_owned()))?;
        if name.starts_with("fable-") && name.ends_with("-handoff.md") {
            candidates.push(entry.path());
        }
    }
    candidates.sort();
    Ok(candidates)
}

fn verify_review_artifact(
    root: &Path,
    path: &Path,
    handoff: &ParsedHandoff,
    enforce_required_result_path: bool,
) -> Result<ReviewArtifactProof, ReviewProofError> {
    let path = repository_file(root, path)?;
    let relative = relative_string(root, &path)?;
    let bytes = fs::read(&path)?;
    let text = std::str::from_utf8(&bytes).map_err(|_| ReviewProofError::Utf8(relative.clone()))?;
    build_review_artifact_proof(
        relative,
        &bytes,
        text,
        handoff,
        enforce_required_result_path,
    )
}

fn build_review_artifact_proof(
    relative: String,
    bytes: &[u8],
    text: &str,
    handoff: &ParsedHandoff,
    enforce_required_result_path: bool,
) -> Result<ReviewArtifactProof, ReviewProofError> {
    let (token_state, exact_token_lines) = classify_review_token(
        &relative,
        text,
        handoff.requested_acceptance_token.as_deref(),
    )?;
    let candidate_commit_attested = contains_exact_hex_word(text, &handoff.candidate_commit);
    let attested_input_hashes = handoff
        .inputs
        .iter()
        .filter(|input| contains_exact_hex_word(text, &input.expected_sha256))
        .count();
    if token_state == ReviewTokenState::Accepted {
        let mismatched_result_path = enforce_required_result_path
            .then_some(handoff.required_result_path.as_deref())
            .flatten()
            .filter(|expected| relative.as_str() != *expected);
        if let Some(expected) = mismatched_result_path {
            return Err(ReviewProofError::ReviewPathMismatch {
                expected: expected.to_owned(),
                actual: relative,
            });
        }
        if !candidate_commit_attested {
            return Err(ReviewProofError::MissingReviewAttestation {
                path: relative,
                item: format!("candidate commit {}", handoff.candidate_commit),
            });
        }
        if attested_input_hashes != handoff.inputs.len() {
            let missing = handoff
                .inputs
                .iter()
                .find(|input| !contains_exact_hex_word(text, &input.expected_sha256))
                .expect("count mismatch requires a missing input");
            return Err(ReviewProofError::MissingReviewAttestation {
                path: relative,
                item: format!("{}={}", missing.path, missing.expected_sha256),
            });
        }
    }
    Ok(ReviewArtifactProof {
        path: relative,
        sha256: sha256_hex(bytes),
        token_state,
        exact_token_lines,
        candidate_commit_attested,
        attested_input_hashes,
    })
}

fn ensure_distinct_review_path(handoff: &str, review: &str) -> Result<(), ReviewProofError> {
    if handoff == review {
        Err(ReviewProofError::ReviewIsHandoff(handoff.to_owned()))
    } else {
        Ok(())
    }
}

fn classify_review_token(
    path: &str,
    text: &str,
    requested_token: Option<&str>,
) -> Result<(ReviewTokenState, usize), ReviewProofError> {
    let bare = bare_acceptance_lines(text);
    match requested_token {
        None => {
            if !bare.is_empty() {
                return Err(ReviewProofError::UnexpectedToken {
                    path: path.to_owned(),
                    token: bare[0].clone(),
                });
            }
            Ok((ReviewTokenState::NotConfigured, 0))
        }
        Some(token) => {
            if let Some(other) = bare.iter().find(|line| line.as_str() != token) {
                return Err(ReviewProofError::UnexpectedToken {
                    path: path.to_owned(),
                    token: other.clone(),
                });
            }
            let count = bare.iter().filter(|line| line.as_str() == token).count();
            if count > 1 {
                return Err(ReviewProofError::DuplicateToken {
                    path: path.to_owned(),
                    token: token.to_owned(),
                    count,
                });
            }
            Ok((
                if count == 1 {
                    ReviewTokenState::Accepted
                } else {
                    ReviewTokenState::Withheld
                },
                count,
            ))
        }
    }
}

fn parse_handoff(text: &str) -> Result<ParsedHandoff, ReviewProofError> {
    let lines: Vec<_> = text.lines().collect();
    let candidate_indices: Vec<_> = lines
        .iter()
        .enumerate()
        .filter(|(_, line)| CANDIDATE_LABELS.contains(&line.trim()))
        .map(|(index, _)| index)
        .collect();
    if candidate_indices.len() != 1 {
        return Err(ReviewProofError::Format(format!(
            "expected exactly one candidate commit label, found {}",
            candidate_indices.len()
        )));
    }
    let candidate_commit = next_code_value(&lines, candidate_indices[0] + 1)
        .ok_or_else(|| ReviewProofError::Format("candidate commit value is missing".to_owned()))?;
    if !is_lower_hex(&candidate_commit, 40) {
        return Err(ReviewProofError::Format(
            "candidate commit must be 40 lowercase hexadecimal characters".to_owned(),
        ));
    }

    let table_header = lines
        .iter()
        .enumerate()
        .skip(candidate_indices[0] + 1)
        .find(|(_, line)| {
            let trimmed = line.trim();
            trimmed.starts_with('|') && trimmed.ends_with('|') && trimmed.contains("SHA-256")
        })
        .map(|(index, _)| index)
        .ok_or_else(|| ReviewProofError::Format("provenance table is missing".to_owned()))?;

    let mut inputs = Vec::new();
    let mut seen_paths = BTreeSet::new();
    for line in lines.iter().skip(table_header + 1) {
        let trimmed = line.trim();
        if !trimmed.starts_with('|') {
            if !inputs.is_empty() {
                break;
            }
            continue;
        }
        let cells: Vec<_> = trimmed
            .trim_matches('|')
            .split('|')
            .map(str::trim)
            .collect();
        if cells.len() < 2
            || cells[0]
                .chars()
                .all(|character| character == '-' || character == ':')
        {
            continue;
        }
        let path = first_code_value(cells[0]).ok_or_else(|| {
            ReviewProofError::Format(format!(
                "provenance input path must use inline code: {}",
                cells[0]
            ))
        })?;
        validate_relative_path(&path)?;
        let expected_sha256 = first_code_value(cells[1]).ok_or_else(|| {
            ReviewProofError::Format(format!(
                "provenance SHA-256 must use inline code for {path}"
            ))
        })?;
        if !is_lower_hex(&expected_sha256, 64) {
            return Err(ReviewProofError::Format(format!(
                "invalid SHA-256 for {path}"
            )));
        }
        if !seen_paths.insert(path.clone()) {
            return Err(ReviewProofError::Format(format!(
                "duplicate provenance input {path}"
            )));
        }
        inputs.push(ParsedInput {
            path,
            expected_sha256,
        });
    }
    if inputs.is_empty() {
        return Err(ReviewProofError::Format(
            "provenance table contains no inputs".to_owned(),
        ));
    }

    let requested_indices: Vec<_> = lines
        .iter()
        .enumerate()
        .filter(|(_, line)| line.trim() == REQUESTED_TOKEN_LABEL)
        .map(|(index, _)| index)
        .collect();
    if requested_indices.len() > 1 {
        return Err(ReviewProofError::Format(
            "multiple requested acceptance token labels".to_owned(),
        ));
    }
    let bare_acceptance_lines = bare_acceptance_lines(text);
    let requested_acceptance_token = if let Some(&index) = requested_indices.first() {
        let token = next_code_value(&lines, index + 1).ok_or_else(|| {
            ReviewProofError::Format("requested acceptance token value is missing".to_owned())
        })?;
        validate_acceptance_token(&token)?;
        if bare_acceptance_lines.iter().any(|line| line == &token) {
            return Err(ReviewProofError::Format(
                "modern handoff must not contain its requested token as a bare line".to_owned(),
            ));
        }
        Some(token)
    } else {
        match bare_acceptance_lines.as_slice() {
            [] => None,
            [token] => Some(token.clone()),
            _ => {
                return Err(ReviewProofError::Format(
                    "legacy handoff contains multiple bare acceptance-token lines".to_owned(),
                ));
            }
        }
    };

    let result_indices: Vec<_> = lines
        .iter()
        .enumerate()
        .filter(|(_, line)| line.trim_start().starts_with(REQUIRED_RESULT_LABEL))
        .map(|(index, _)| index)
        .collect();
    if result_indices.len() > 1 {
        return Err(ReviewProofError::Format(
            "multiple required result path labels".to_owned(),
        ));
    }
    let required_result_path = result_indices
        .first()
        .map(|&index| {
            first_code_value(lines[index])
                .or_else(|| next_code_value(&lines, index + 1))
                .ok_or_else(|| {
                    ReviewProofError::Format("required result path is missing".to_owned())
                })
        })
        .transpose()?;
    if let Some(path) = required_result_path.as_deref() {
        validate_relative_path(path)?;
    }

    Ok(ParsedHandoff {
        candidate_commit,
        required_result_path,
        requested_acceptance_token,
        bare_acceptance_lines,
        inputs,
    })
}

fn has_candidate_label(text: &str) -> bool {
    text.lines()
        .any(|line| CANDIDATE_LABELS.contains(&line.trim()))
}

fn next_code_value(lines: &[&str], start: usize) -> Option<String> {
    lines
        .iter()
        .skip(start)
        .find(|line| !line.trim().is_empty())
        .and_then(|line| first_code_value(line))
}

fn first_code_value(text: &str) -> Option<String> {
    let start = text.find('`')?;
    let remainder = &text[start + 1..];
    let end = remainder.find('`')?;
    let value = &remainder[..end];
    (!value.is_empty()).then(|| value.to_owned())
}

fn exact_code_cell(text: &str) -> Option<String> {
    let value = text.trim().strip_prefix('`')?.strip_suffix('`')?;
    (!value.is_empty() && !value.contains('`')).then(|| value.to_owned())
}

fn bare_acceptance_lines(text: &str) -> Vec<String> {
    text.lines()
        .map(str::trim)
        .filter(|line| is_acceptance_token(line))
        .map(str::to_owned)
        .collect()
}

fn validate_acceptance_token(token: &str) -> Result<(), ReviewProofError> {
    if !is_acceptance_token(token) {
        return Err(ReviewProofError::Format(format!(
            "invalid acceptance token {token}"
        )));
    }
    Ok(())
}

fn is_acceptance_token(value: &str) -> bool {
    value.ends_with("-accepted")
        && value.len() > "-accepted".len()
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'-')
        })
}

fn is_lower_hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn contains_exact_hex_word(text: &str, expected: &str) -> bool {
    text.split(|character: char| !character.is_ascii_hexdigit())
        .any(|word| word == expected)
}

fn validate_relative_path(path: &str) -> Result<(), ReviewProofError> {
    if path.is_empty()
        || path.contains(':')
        || Path::new(path).components().any(|component| {
            !matches!(component, Component::Normal(_))
                || component.as_os_str().to_string_lossy() == ".git"
        })
    {
        return Err(ReviewProofError::Format(format!(
            "provenance path is not a clean repository-relative path: {path}"
        )));
    }
    Ok(())
}

fn repository_root(path: &Path) -> Result<PathBuf, ReviewProofError> {
    let output = Command::new("git")
        .arg("-C")
        .arg(path)
        .args(["rev-parse", "--show-toplevel"])
        .output()?;
    if !output.status.success() {
        return Err(ReviewProofError::Git(
            String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        ));
    }
    let root = std::str::from_utf8(&output.stdout)
        .map_err(|_| ReviewProofError::Utf8("git repository root".to_owned()))?
        .trim();
    if root.is_empty() {
        return Err(ReviewProofError::Git(
            "git returned an empty repository root".to_owned(),
        ));
    }
    Ok(PathBuf::from(root))
}

fn repository_file(root: &Path, path: &Path) -> Result<PathBuf, ReviewProofError> {
    let joined = if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    };
    let canonical = joined.canonicalize()?;
    canonical.strip_prefix(root).map_err(|_| {
        ReviewProofError::Format(format!(
            "artifact is outside the repository: {}",
            canonical.display()
        ))
    })?;
    Ok(canonical)
}

fn relative_string(root: &Path, path: &Path) -> Result<String, ReviewProofError> {
    path.strip_prefix(root)
        .map_err(|_| ReviewProofError::Format("path is outside the repository".to_owned()))?
        .to_str()
        .map(str::to_owned)
        .ok_or_else(|| ReviewProofError::Utf8(path.display().to_string()))
}

fn verify_commit(root: &Path, commit: &str) -> Result<(), ReviewProofError> {
    let object = format!("{commit}^{{commit}}");
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["cat-file", "-e", &object])
        .output()?;
    if output.status.success() {
        Ok(())
    } else {
        Err(ReviewProofError::Git(format!(
            "candidate commit does not resolve: {commit}"
        )))
    }
}

fn git_text(root: &Path, arguments: &[&str]) -> Result<String, ReviewProofError> {
    let bytes = git_bytes(root, arguments)?;
    let text = std::str::from_utf8(&bytes)
        .map_err(|_| ReviewProofError::Utf8(format!("git {}", arguments.join(" "))))?;
    Ok(text.trim().to_owned())
}

fn git_bytes(root: &Path, arguments: &[&str]) -> Result<Vec<u8>, ReviewProofError> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(arguments)
        .output()?;
    if output.status.success() {
        Ok(output.stdout)
    } else {
        Err(ReviewProofError::Git(
            String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        ))
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[derive(Debug)]
pub enum ReviewProofError {
    Io(std::io::Error),
    Utf8(String),
    Git(String),
    Format(String),
    HashMismatch {
        path: String,
        expected: String,
        actual: String,
    },
    UnexpectedToken {
        path: String,
        token: String,
    },
    DuplicateToken {
        path: String,
        token: String,
        count: usize,
    },
    ReviewPathMismatch {
        expected: String,
        actual: String,
    },
    MissingReviewAttestation {
        path: String,
        item: String,
    },
    AcceptanceTokenMissing {
        path: String,
        token: String,
    },
    ReviewIsHandoff(String),
}

impl fmt::Display for ReviewProofError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "{error}"),
            Self::Utf8(path) => write!(formatter, "artifact is not valid UTF-8: {path}"),
            Self::Git(error) => write!(formatter, "git verification failed: {error}"),
            Self::Format(error) => write!(formatter, "review handoff is malformed: {error}"),
            Self::HashMismatch {
                path,
                expected,
                actual,
            } => write!(
                formatter,
                "candidate hash mismatch for {path}: expected {expected}, observed {actual}"
            ),
            Self::UnexpectedToken { path, token } => {
                write!(
                    formatter,
                    "unexpected bare acceptance token in {path}: {token}"
                )
            }
            Self::DuplicateToken { path, token, count } => write!(
                formatter,
                "acceptance token appears {count} times in {path}: {token}"
            ),
            Self::ReviewPathMismatch { expected, actual } => write!(
                formatter,
                "review artifact path mismatch: expected {expected}, observed {actual}"
            ),
            Self::MissingReviewAttestation { path, item } => {
                write!(formatter, "accepted review {path} does not attest {item}")
            }
            Self::AcceptanceTokenMissing { path, token } => write!(
                formatter,
                "staged review {path} does not contain the requested acceptance token \
                 exactly once on a bare line: {token}"
            ),
            Self::ReviewIsHandoff(path) => {
                write!(formatter, "review artifact is the handoff itself: {path}")
            }
        }
    }
}

impl std::error::Error for ReviewProofError {}

impl From<std::io::Error> for ReviewProofError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    const HASH: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    static TEMP_REPOSITORY_ID: AtomicU64 = AtomicU64::new(0);

    struct TempRepository {
        path: PathBuf,
    }

    impl TempRepository {
        fn new(label: &str) -> Self {
            let id = TEMP_REPOSITORY_ID.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir()
                .join(format!("glmaxx-review-{label}-{}-{id}", std::process::id()));
            if path.try_exists().unwrap() {
                fs::remove_dir_all(&path).unwrap();
            }
            fs::create_dir_all(path.join("docs/reviews")).unwrap();
            fs::create_dir_all(path.join("src")).unwrap();
            let status = Command::new("git")
                .arg("-C")
                .arg(&path)
                .args(["init", "--quiet"])
                .status()
                .unwrap();
            assert!(status.success());
            fs::write(path.join("src/lib.rs"), b"candidate\n").unwrap();
            let status = Command::new("git")
                .arg("-C")
                .arg(&path)
                .args(["add", "src/lib.rs"])
                .status()
                .unwrap();
            assert!(status.success());
            let status = Command::new("git")
                .arg("-C")
                .arg(&path)
                .args([
                    "-c",
                    "user.name=glmaxx-test",
                    "-c",
                    "user.email=glmaxx-test@example.invalid",
                    "commit",
                    "--quiet",
                    "-m",
                    "candidate",
                ])
                .status()
                .unwrap();
            assert!(status.success());
            Self { path }
        }

        fn head(&self) -> String {
            git_text(&self.path, &["rev-parse", "HEAD"]).unwrap()
        }

        fn write_handoff(&self, name: &str, required_result: Option<&str>) {
            let result = required_result.map_or_else(String::new, |path| {
                format!("\nRequired result path:\n`{path}`\n")
            });
            let text = format!(
                "# Review\n\nReview candidate commit:\n`{}`\n\n\
                 Requested acceptance token, only if every blocker and major is resolved:\n\
                 `example-{name}-accepted`\n{result}\n\
                 | Input at candidate commit | SHA-256 |\n\
                 |---|---|\n\
                 | `src/lib.rs` | `{}` |\n",
                self.head(),
                sha256_hex(b"candidate\n")
            );
            fs::write(
                self.path.join(format!("docs/fable-{name}-handoff.md")),
                text,
            )
            .unwrap();
        }
    }

    impl Drop for TempRepository {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn modern_handoff() -> String {
        format!(
            "# Review\n\nReview candidate commit:\n\
             `0123456789abcdef0123456789abcdef01234567`\n\n\
             Requested acceptance token, only if every blocker and major is resolved:\n\
             `example-v1-accepted`\n\n\
             Required result path:\n\
             `example-review.md` at the repository root.\n\n\
             | Input at candidate commit | SHA-256 |\n\
             |---|---|\n\
             | `src/lib.rs` | `{HASH}` |\n"
        )
    }

    #[test]
    fn modern_handoff_parses_without_materializing_a_bare_token() {
        let parsed = parse_handoff(&modern_handoff()).unwrap();
        assert_eq!(
            parsed.candidate_commit,
            "0123456789abcdef0123456789abcdef01234567"
        );
        assert_eq!(
            parsed.requested_acceptance_token.as_deref(),
            Some("example-v1-accepted")
        );
        assert_eq!(
            parsed.required_result_path.as_deref(),
            Some("example-review.md")
        );
        let same_line = modern_handoff().replace(
            "Required result path:\n`example-review.md` at the repository root.",
            "Required result path: `example-review.md` at the repository root.",
        );
        assert_eq!(
            parse_handoff(&same_line)
                .unwrap()
                .required_result_path
                .as_deref(),
            Some("example-review.md")
        );
        assert!(parsed.bare_acceptance_lines.is_empty());
        assert_eq!(
            parsed.inputs,
            [ParsedInput {
                path: "src/lib.rs".to_owned(),
                expected_sha256: HASH.to_owned(),
            }]
        );
    }

    #[test]
    fn legacy_handoff_derives_one_bare_requested_token() {
        let text = format!(
            "# Review\n\nCandidate base commit:\n\
             `0123456789abcdef0123456789abcdef01234567`\n\n\
             | Input | SHA-256 |\n|---|---|\n\
             | prior `review.md` | `{HASH}` |\n\n\
             example-v1-accepted\n"
        );
        let parsed = parse_handoff(&text).unwrap();
        assert_eq!(
            parsed.requested_acceptance_token.as_deref(),
            Some("example-v1-accepted")
        );
        assert_eq!(parsed.inputs[0].path, "review.md");
    }

    #[test]
    fn duplicate_paths_and_unsafe_paths_fail_closed() {
        let duplicate = modern_handoff().replace(
            &format!("| `src/lib.rs` | `{HASH}` |"),
            &format!("| `src/lib.rs` | `{HASH}` |\n| `src/lib.rs` | `{HASH}` |"),
        );
        assert!(matches!(
            parse_handoff(&duplicate),
            Err(ReviewProofError::Format(_))
        ));
        let unsafe_path = modern_handoff().replace("src/lib.rs", "../src/lib.rs");
        assert!(matches!(
            parse_handoff(&unsafe_path),
            Err(ReviewProofError::Format(_))
        ));
    }

    #[test]
    fn hash_and_token_shapes_are_strict() {
        let bad_hash = modern_handoff().replace(HASH, "ABC");
        assert!(matches!(
            parse_handoff(&bad_hash),
            Err(ReviewProofError::Format(_))
        ));
        let bare = format!("{}\nexample-v1-accepted\n", modern_handoff());
        assert!(matches!(
            parse_handoff(&bare),
            Err(ReviewProofError::Format(_))
        ));
        assert!(!is_acceptance_token("`example-v1-accepted`"));
        assert!(!is_acceptance_token("Example-v1-accepted"));
    }

    #[test]
    fn review_token_classification_distinguishes_withheld_and_accepted() {
        assert_eq!(
            classify_review_token("review.md", "no gate token\n", Some("example-v1-accepted"))
                .unwrap(),
            (ReviewTokenState::Withheld, 0)
        );
        assert_eq!(
            classify_review_token(
                "review.md",
                "findings\nexample-v1-accepted\n",
                Some("example-v1-accepted")
            )
            .unwrap(),
            (ReviewTokenState::Accepted, 1)
        );
        assert!(matches!(
            classify_review_token(
                "review.md",
                "wrong-v1-accepted\n",
                Some("example-v1-accepted")
            ),
            Err(ReviewProofError::UnexpectedToken { .. })
        ));
        assert!(matches!(
            classify_review_token(
                "review.md",
                "example-v1-accepted\nexample-v1-accepted\n",
                Some("example-v1-accepted")
            ),
            Err(ReviewProofError::DuplicateToken { count: 2, .. })
        ));
    }

    #[test]
    fn accepted_review_requires_candidate_and_every_input_hash() {
        let handoff = parse_handoff(&modern_handoff()).unwrap();
        let missing_candidate = format!("{HASH}\nexample-v1-accepted\n");
        let error =
            verify_review_artifact_text_for_test("example-review.md", &missing_candidate, &handoff)
                .unwrap_err();
        assert!(matches!(
            error,
            ReviewProofError::MissingReviewAttestation { .. }
        ));

        let missing_hash = format!("{}\nexample-v1-accepted\n", handoff.candidate_commit);
        let error =
            verify_review_artifact_text_for_test("example-review.md", &missing_hash, &handoff)
                .unwrap_err();
        assert!(matches!(
            error,
            ReviewProofError::MissingReviewAttestation { .. }
        ));

        let accepted = format!(
            "{}\n{}\nexample-v1-accepted\n",
            handoff.candidate_commit, HASH
        );
        let proof =
            verify_review_artifact_text_for_test("example-review.md", &accepted, &handoff).unwrap();
        assert_eq!(proof.token_state, ReviewTokenState::Accepted);
        assert!(proof.candidate_commit_attested);
        assert_eq!(proof.attested_input_hashes, 1);
        assert!(matches!(
            verify_review_artifact_text_for_test("wrong-review.md", &accepted, &handoff),
            Err(ReviewProofError::ReviewPathMismatch { .. })
        ));
        let staged = build_review_artifact_proof(
            "docs/reviews/example-review.md".to_owned(),
            accepted.as_bytes(),
            &accepted,
            &handoff,
            false,
        )
        .unwrap();
        assert_eq!(staged.token_state, ReviewTokenState::Accepted);
        assert!(matches!(
            ensure_distinct_review_path("handoff.md", "handoff.md"),
            Err(ReviewProofError::ReviewIsHandoff(_))
        ));
    }

    #[test]
    fn staged_acceptance_requires_one_bare_requested_token() {
        let handoff = parse_handoff(&modern_handoff()).unwrap();
        let wrapped = format!(
            "{}\n{}\nToken: `example-v1-accepted`\n",
            handoff.candidate_commit, HASH
        );
        let withheld = build_review_artifact_proof(
            "docs/reviews/example-review.md".to_owned(),
            wrapped.as_bytes(),
            &wrapped,
            &handoff,
            false,
        )
        .unwrap();
        assert_eq!(withheld.token_state, ReviewTokenState::Withheld);
        assert!(matches!(
            require_staged_acceptance(&withheld, "example-v1-accepted"),
            Err(ReviewProofError::AcceptanceTokenMissing { .. })
        ));

        let accepted = format!(
            "{}\n{}\nexample-v1-accepted\n",
            handoff.candidate_commit, HASH
        );
        let ready = build_review_artifact_proof(
            "docs/reviews/example-review.md".to_owned(),
            accepted.as_bytes(),
            &accepted,
            &handoff,
            false,
        )
        .unwrap();
        assert!(require_staged_acceptance(&ready, "example-v1-accepted").is_ok());
    }

    #[test]
    fn staged_acceptance_suite_reports_ready_rejected_absent_and_unmatched() {
        let repository = TempRepository::new("staged-suite");
        repository.write_handoff("ready", Some("ready-review.md"));
        repository.write_handoff("rejected", Some("rejected-review.md"));
        repository.write_handoff("absent", Some("absent-review.md"));
        repository.write_handoff("legacy-status", None);

        let candidate = repository.head();
        let input_hash = sha256_hex(b"candidate\n");
        fs::write(
            repository.path.join("docs/reviews/ready-review.md"),
            format!("{candidate}\n{input_hash}\nexample-ready-accepted\n"),
        )
        .unwrap();
        fs::write(
            repository.path.join("docs/reviews/rejected-review.md"),
            format!("{candidate}\nexample-rejected-accepted\n"),
        )
        .unwrap();
        fs::write(
            repository.path.join("docs/reviews/README.md"),
            "operator-owned inbox\n",
        )
        .unwrap();

        let proof =
            verify_all_staged_review_acceptances(&repository.path, Path::new("docs/reviews"))
                .unwrap();
        assert_eq!(proof.configured_handoffs, 3);
        assert_eq!(proof.present_staged_reviews, 2);
        assert_eq!(proof.ready_staged_reviews, 1);
        assert_eq!(proof.rejected_staged_reviews, 1);
        assert_eq!(proof.absent_staged_reviews, 1);
        assert_eq!(proof.unmatched_staging_files, ["docs/reviews/README.md"]);
        assert_eq!(proof.verdict, "STAGED_CONTENT_REJECTED");
        let rejected = proof
            .entries
            .iter()
            .find(|entry| entry.state == StagedReviewLintState::Rejected)
            .unwrap();
        assert!(
            rejected
                .error
                .as_deref()
                .is_some_and(|error| error.contains("does not attest"))
        );
        let ready = proof
            .entries
            .iter()
            .find(|entry| entry.state == StagedReviewLintState::Ready)
            .unwrap();
        assert!(ready.proof.is_some());
    }

    #[test]
    fn staged_acceptance_suite_rejects_ambiguous_result_basenames() {
        let repository = TempRepository::new("staged-ambiguous");
        repository.write_handoff("first", Some("first/result.md"));
        repository.write_handoff("second", Some("second/result.md"));

        let error =
            verify_all_staged_review_acceptances(&repository.path, Path::new("docs/reviews"))
                .unwrap_err();
        assert!(matches!(error, ReviewProofError::Format(_)));
        assert!(error.to_string().contains("ambiguous"));
    }

    #[test]
    fn review_queue_binds_every_handoff_field_and_row() {
        let first = queue_proof(
            "docs/fable-first-handoff.md",
            "0123456789abcdef0123456789abcdef01234567",
            Some("fable-first.md"),
            Some("first-v1-accepted"),
        );
        let second = queue_proof(
            "docs/fable-second-handoff.md",
            "89abcdef0123456789abcdef0123456789abcdef",
            None,
            None,
        );
        let queue = "| # | Handoff | Candidate | Required result | Requested token |\n\
                     |---:|---|---|---|---|\n\
                     | 1 | `docs/fable-first-handoff.md` | `0123456789abcdef0123456789abcdef01234567` | `fable-first.md` | `first-v1-accepted` |\n\
                     | 2 | `docs/fable-second-handoff.md` | `89abcdef0123456789abcdef0123456789abcdef` | — | design verdict only |\n";
        verify_review_queue_text(queue, &[first.clone(), second.clone()]).unwrap();

        let bad_candidate = queue.replace(
            "0123456789abcdef0123456789abcdef01234567",
            "1123456789abcdef0123456789abcdef01234567",
        );
        assert!(
            verify_review_queue_text(&bad_candidate, &[first.clone(), second.clone()])
                .unwrap_err()
                .to_string()
                .contains("candidate mismatch")
        );
        let bad_result = queue.replace("`fable-first.md`", "`fable-wrong.md`");
        assert!(
            verify_review_queue_text(&bad_result, &[first.clone(), second.clone()])
                .unwrap_err()
                .to_string()
                .contains("result-path mismatch")
        );
        let bad_token = queue.replace("`first-v1-accepted`", "`wrong-v1-accepted`");
        assert!(
            verify_review_queue_text(&bad_token, &[first.clone(), second.clone()])
                .unwrap_err()
                .to_string()
                .contains("token mismatch")
        );
        let bad_ordinal = queue.replace("| 2 |", "| 3 |");
        assert!(
            verify_review_queue_text(&bad_ordinal, &[first.clone(), second.clone()])
                .unwrap_err()
                .to_string()
                .contains("ordinal mismatch")
        );
        let missing_row = queue.lines().take(3).collect::<Vec<_>>().join("\n");
        assert!(
            verify_review_queue_text(&missing_row, &[first, second])
                .unwrap_err()
                .to_string()
                .contains("omits verified handoffs")
        );
    }

    fn queue_proof(
        handoff_path: &str,
        candidate_commit: &str,
        required_result_path: Option<&str>,
        requested_acceptance_token: Option<&str>,
    ) -> ReviewProof {
        ReviewProof {
            schema: "test",
            repository_head: candidate_commit.to_owned(),
            handoff_path: handoff_path.to_owned(),
            handoff_sha256: HASH.to_owned(),
            candidate_commit: candidate_commit.to_owned(),
            required_result_path: required_result_path.map(str::to_owned),
            requested_acceptance_token: requested_acceptance_token.map(str::to_owned),
            handoff_bare_acceptance_lines: Vec::new(),
            inputs: Vec::new(),
            review: None,
            verdict: "PASS",
        }
    }

    fn verify_review_artifact_text_for_test(
        relative: &str,
        text: &str,
        handoff: &ParsedHandoff,
    ) -> Result<ReviewArtifactProof, ReviewProofError> {
        build_review_artifact_proof(relative.to_owned(), text.as_bytes(), text, handoff, true)
    }
}
