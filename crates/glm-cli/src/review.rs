use std::{
    collections::BTreeSet,
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
const HISTORICAL_HANDOFFS: [&str; 2] = [
    "docs/fable-phase-a-engine-handoff.md",
    "docs/fable-review-handoff.md",
];

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
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ReviewProof {
    pub schema: &'static str,
    pub repository_head: String,
    pub handoff_path: String,
    pub handoff_sha256: String,
    pub candidate_commit: String,
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
    requested_acceptance_token: Option<String>,
    bare_acceptance_lines: Vec<String>,
    inputs: Vec<ParsedInput>,
}

pub fn verify_review_handoff(
    repository: &Path,
    handoff: &Path,
    review: Option<&Path>,
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
        .map(|path| {
            verify_review_artifact(&root, path, parsed.requested_acceptance_token.as_deref())
        })
        .transpose()?;

    Ok(ReviewProof {
        schema: "glmaxx.review-provenance-proof.v1",
        repository_head: head,
        handoff_path: handoff_relative,
        handoff_sha256: sha256_hex(&handoff_bytes),
        candidate_commit: parsed.candidate_commit,
        requested_acceptance_token: parsed.requested_acceptance_token,
        handoff_bare_acceptance_lines: parsed.bare_acceptance_lines,
        inputs: input_proofs,
        review,
        verdict: "PASS",
    })
}

pub fn verify_all_review_handoffs(repository: &Path) -> Result<ReviewSuiteProof, ReviewProofError> {
    let root = repository_root(repository)?;
    let head = git_text(&root, &["rev-parse", "HEAD"])?;
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

    let mut verified_handoffs = Vec::new();
    let mut skipped_historical_handoffs = Vec::new();
    for handoff in candidates {
        let bytes = fs::read(&handoff)?;
        let relative = relative_string(&root, &handoff)?;
        let text =
            std::str::from_utf8(&bytes).map_err(|_| ReviewProofError::Utf8(relative.clone()))?;
        if has_candidate_label(text) {
            verified_handoffs.push(verify_review_handoff(&root, &handoff, None)?);
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
    Ok(ReviewSuiteProof {
        schema: "glmaxx.review-provenance-suite.v1",
        repository_head: head,
        verified_handoffs,
        skipped_historical_handoffs,
        verdict: "PASS",
    })
}

fn verify_review_artifact(
    root: &Path,
    path: &Path,
    requested_token: Option<&str>,
) -> Result<ReviewArtifactProof, ReviewProofError> {
    let path = repository_file(root, path)?;
    let relative = relative_string(root, &path)?;
    let bytes = fs::read(&path)?;
    let text = std::str::from_utf8(&bytes).map_err(|_| ReviewProofError::Utf8(relative.clone()))?;
    let (token_state, exact_token_lines) = classify_review_token(&relative, text, requested_token)?;
    Ok(ReviewArtifactProof {
        path: relative,
        sha256: sha256_hex(&bytes),
        token_state,
        exact_token_lines,
    })
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

    Ok(ParsedHandoff {
        candidate_commit,
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

    const HASH: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    fn modern_handoff() -> String {
        format!(
            "# Review\n\nReview candidate commit:\n\
             `0123456789abcdef0123456789abcdef01234567`\n\n\
             Requested acceptance token, only if every blocker and major is resolved:\n\
             `example-v1-accepted`\n\n\
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
}
