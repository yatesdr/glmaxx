use std::{fs, hint::black_box, path::Path, time::Instant};

use glm_cache::{PageTableConfig, PageTableDelta, SequencePageTable};
use serde::Serialize;

const CONTEXT_TOKENS: [u64; 5] = [0, 16_384, 32_768, 65_536, 131_072];
const CONCURRENCIES: [u16; 4] = [1, 2, 4, 8];
const MTP_DEPTHS: [u8; 2] = [0, 3];
const MAXIMUM_PROFILE_ITERATIONS: u32 = 100_000;
const PAGES_PER_RANK: u32 = 8_192;

#[derive(Clone, Copy, Debug, Serialize)]
struct NanosecondDistribution {
    minimum: u64,
    p50: u64,
    p95: u64,
    p99: u64,
    maximum: u64,
    mean: u64,
}

impl NanosecondDistribution {
    fn from_samples(samples: &[u64]) -> Result<Self, &'static str> {
        if samples.is_empty() {
            return Err("page transaction profile has no timing samples");
        }
        let mut ordered = samples.to_vec();
        ordered.sort_unstable();
        let sum = ordered
            .iter()
            .try_fold(0_u128, |sum, &sample| sum.checked_add(u128::from(sample)))
            .ok_or("page transaction profile timing sum overflow")?;
        let mean = sum / u128::try_from(ordered.len()).map_err(|_| "sample count overflow")?;
        Ok(Self {
            minimum: ordered[0],
            p50: nearest_rank(&ordered, 50),
            p95: nearest_rank(&ordered, 95),
            p99: nearest_rank(&ordered, 99),
            maximum: *ordered
                .last()
                .ok_or("page transaction profile has no timing samples")?,
            mean: u64::try_from(mean).map_err(|_| "page transaction profile mean overflow")?,
        })
    }
}

fn nearest_rank(ordered: &[u64], percentile: usize) -> u64 {
    let rank = percentile
        .checked_mul(ordered.len())
        .and_then(|value| value.checked_add(99))
        .expect("bounded page transaction percentile")
        / 100;
    ordered[rank.saturating_sub(1).min(ordered.len() - 1)]
}

#[derive(Clone, Copy, Debug, Serialize)]
struct PageTransactionSample {
    reserve_clone_ns: u64,
    reserve_mutation_ns: u64,
    reservation_delta_ns: u64,
    commit_clone_ns: u64,
    commit_mutation_ns: u64,
    commit_delta_ns: u64,
    total_ns: u64,
}

#[derive(Debug, Serialize)]
struct PageTransactionCell {
    context_tokens_per_sequence: u64,
    concurrency: u16,
    mtp_depth: u8,
    committed_tokens_per_step: u8,
    active_positions: u64,
    target_pages_used: [u32; 4],
    draft_pages_used: [u32; 4],
    warmups: u32,
    iterations: u32,
    reserve_clone_ns: NanosecondDistribution,
    reserve_mutation_ns: NanosecondDistribution,
    reservation_delta_ns: NanosecondDistribution,
    commit_clone_ns: NanosecondDistribution,
    commit_mutation_ns: NanosecondDistribution,
    commit_delta_ns: NanosecondDistribution,
    total_ns: NanosecondDistribution,
    samples: Vec<PageTransactionSample>,
}

#[derive(Debug, Serialize)]
struct PageTransactionReport {
    schema: &'static str,
    source_commit: String,
    contexts: [u64; 5],
    concurrencies: [u16; 4],
    mtp_depths: [u8; 2],
    page_table_config: PageTableConfigReport,
    warmups_per_cell: u32,
    iterations_per_cell: u32,
    cells: Vec<PageTransactionCell>,
    claim: &'static str,
}

#[derive(Clone, Copy, Debug, Serialize)]
struct PageTableConfigReport {
    target_pages_per_rank: u32,
    draft_pages_per_rank: u32,
}

pub fn write_page_transaction_profile(
    evidence_directory: &Path,
    source_commit: &str,
    warmups: u32,
    iterations: u32,
) -> Result<(), Box<dyn std::error::Error>> {
    validate_inputs(evidence_directory, source_commit, warmups, iterations)?;
    let mut cells =
        Vec::with_capacity(CONTEXT_TOKENS.len() * CONCURRENCIES.len() * MTP_DEPTHS.len());
    for mtp_depth in MTP_DEPTHS {
        for context_tokens in CONTEXT_TOKENS {
            for concurrency in CONCURRENCIES {
                cells.push(run_cell(
                    context_tokens,
                    concurrency,
                    mtp_depth,
                    warmups,
                    iterations,
                )?);
            }
        }
    }
    let report = PageTransactionReport {
        schema: "glmaxx.synthetic-page-transaction-profile.v1",
        source_commit: source_commit.to_owned(),
        contexts: CONTEXT_TOKENS,
        concurrencies: CONCURRENCIES,
        mtp_depths: MTP_DEPTHS,
        page_table_config: PageTableConfigReport {
            target_pages_per_rank: PAGES_PER_RANK,
            draft_pages_per_rank: PAGES_PER_RANK,
        },
        warmups_per_cell: warmups,
        iterations_per_cell: iterations,
        cells,
        claim: "Synthetic CPU SequencePageTable reserve/commit transaction overhead only; no CUDA, model, checkpoint, physical KV capacity, quality, latency, or serving-throughput claim",
    };
    let mut json = serde_json::to_vec_pretty(&report)?;
    json.push(b'\n');
    let output = evidence_directory.join("page-transaction-profile.json");
    fs::write(&output, &json)?;
    println!("wrote {} bytes to {}", json.len(), output.display());
    Ok(())
}

fn validate_inputs(
    evidence_directory: &Path,
    source_commit: &str,
    warmups: u32,
    iterations: u32,
) -> Result<(), Box<dyn std::error::Error>> {
    if !evidence_directory.is_dir() || fs::read_dir(evidence_directory)?.next().is_some() {
        return Err(
            "page-transaction-profile requires an existing empty evidence directory".into(),
        );
    }
    if source_commit.len() != 40
        || !source_commit
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err("page-transaction-profile requires a 40-digit lowercase Git commit".into());
    }
    if warmups == 0 || iterations == 0 || iterations > MAXIMUM_PROFILE_ITERATIONS {
        return Err("page-transaction-profile iteration counts are out of range".into());
    }
    Ok(())
}

fn run_cell(
    context_tokens: u64,
    concurrency: u16,
    mtp_depth: u8,
    warmups: u32,
    iterations: u32,
) -> Result<PageTransactionCell, Box<dyn std::error::Error>> {
    let config = PageTableConfig {
        target_pages_per_rank: PAGES_PER_RANK,
        draft_pages_per_rank: PAGES_PER_RANK,
    };
    let mut baseline = SequencePageTable::new(config)?;
    for request_id in 1..=u64::from(concurrency) {
        baseline.admit_with_prefix(request_id, mtp_depth != 0, &[])?;
        baseline.append_committed(request_id, context_tokens)?;
    }
    let stats = baseline.stats()?;
    let expected_positions = context_tokens
        .checked_mul(u64::from(concurrency))
        .ok_or("page transaction active-position overflow")?;
    if stats.active_positions != expected_positions
        || stats.active_sequences != u32::from(concurrency)
    {
        return Err("page transaction baseline stats drifted".into());
    }

    let committed_tokens = mtp_depth
        .checked_add(1)
        .ok_or("page transaction commit count overflow")?;
    let total_iterations = warmups
        .checked_add(iterations)
        .ok_or("page transaction iteration count overflow")?;
    let mut samples = Vec::with_capacity(usize::try_from(iterations)?);
    for iteration in 0..total_iterations {
        let total_start = Instant::now();

        let phase_start = Instant::now();
        let mut reserved = baseline.clone();
        let reserve_clone_ns = elapsed_ns(phase_start)?;

        let phase_start = Instant::now();
        for request_id in 1..=u64::from(concurrency) {
            reserved.begin_tentative(request_id, committed_tokens)?;
        }
        let reserve_mutation_ns = elapsed_ns(phase_start)?;

        let phase_start = Instant::now();
        let reservation_delta = PageTableDelta::between(&baseline, &reserved, 1, 2)?;
        if reservation_delta.updates().len() != usize::from(concurrency) {
            return Err("page transaction reservation delta lost a sequence".into());
        }
        black_box(reservation_delta.global_digest());
        let reservation_delta_ns = elapsed_ns(phase_start)?;

        let phase_start = Instant::now();
        let reserved_snapshot = reserved.clone();
        let commit_clone_ns = elapsed_ns(phase_start)?;

        let phase_start = Instant::now();
        for request_id in 1..=u64::from(concurrency) {
            reserved.commit_tentative(request_id, committed_tokens)?;
        }
        let commit_mutation_ns = elapsed_ns(phase_start)?;

        let phase_start = Instant::now();
        let commit_delta = PageTableDelta::between(&reserved_snapshot, &reserved, 2, 3)?;
        if commit_delta.updates().len() != usize::from(concurrency) {
            return Err("page transaction commit delta lost a sequence".into());
        }
        black_box(commit_delta.global_digest());
        let commit_delta_ns = elapsed_ns(phase_start)?;
        let total_ns = elapsed_ns(total_start)?;

        // Verification is a proof assertion here, not a coordinator phase.
        // Rank-mirror verification belongs to worker timing in the serving
        // path, so keep it outside every retained distribution.
        reservation_delta.verify()?;
        commit_delta.verify()?;

        if iteration >= warmups {
            samples.push(PageTransactionSample {
                reserve_clone_ns,
                reserve_mutation_ns,
                reservation_delta_ns,
                commit_clone_ns,
                commit_mutation_ns,
                commit_delta_ns,
                total_ns,
            });
        }
    }
    if samples.len() != usize::try_from(iterations)? {
        return Err("page transaction sample count drifted".into());
    }

    Ok(PageTransactionCell {
        context_tokens_per_sequence: context_tokens,
        concurrency,
        mtp_depth,
        committed_tokens_per_step: committed_tokens,
        active_positions: stats.active_positions,
        target_pages_used: stats.target_pages_used,
        draft_pages_used: stats.draft_pages_used,
        warmups,
        iterations,
        reserve_clone_ns: distribution(&samples, |sample| sample.reserve_clone_ns)?,
        reserve_mutation_ns: distribution(&samples, |sample| sample.reserve_mutation_ns)?,
        reservation_delta_ns: distribution(&samples, |sample| sample.reservation_delta_ns)?,
        commit_clone_ns: distribution(&samples, |sample| sample.commit_clone_ns)?,
        commit_mutation_ns: distribution(&samples, |sample| sample.commit_mutation_ns)?,
        commit_delta_ns: distribution(&samples, |sample| sample.commit_delta_ns)?,
        total_ns: distribution(&samples, |sample| sample.total_ns)?,
        samples,
    })
}

fn distribution(
    samples: &[PageTransactionSample],
    select: impl Fn(&PageTransactionSample) -> u64,
) -> Result<NanosecondDistribution, &'static str> {
    NanosecondDistribution::from_samples(&samples.iter().map(select).collect::<Vec<_>>())
}

fn elapsed_ns(start: Instant) -> Result<u64, &'static str> {
    u64::try_from(start.elapsed().as_nanos()).map_err(|_| "page transaction duration overflow")
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    #[test]
    fn nearest_rank_is_exact() {
        let samples = (1..=100).collect::<Vec<_>>();
        let distribution = NanosecondDistribution::from_samples(&samples).unwrap();
        assert_eq!(distribution.minimum, 1);
        assert_eq!(distribution.p50, 50);
        assert_eq!(distribution.p95, 95);
        assert_eq!(distribution.p99, 99);
        assert_eq!(distribution.maximum, 100);
        assert_eq!(distribution.mean, 50);
    }

    #[test]
    fn full_matrix_runs_and_retains_one_sample_per_cell() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = std::env::temp_dir().join(format!(
            "glmaxx-page-profile-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir(&directory).unwrap();
        write_page_transaction_profile(
            &directory,
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            1,
            1,
        )
        .unwrap();
        let bytes = fs::read(directory.join("page-transaction-profile.json")).unwrap();
        let report: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(
            report["schema"],
            "glmaxx.synthetic-page-transaction-profile.v1"
        );
        assert_eq!(report["cells"].as_array().unwrap().len(), 40);
        assert!(report["cells"].as_array().unwrap().iter().all(|cell| {
            cell["samples"]
                .as_array()
                .is_some_and(|samples| samples.len() == 1)
        }));
        fs::remove_dir_all(directory).unwrap();
    }
}
