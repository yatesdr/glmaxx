use std::{fs, path::Path, time::Duration};

use glm_cache::{
    FileTierStore, NamespaceInputs, PageTableConfig, PrefixIndex, PrefixNamespace, ResidencyConfig,
};
use glm_engine::{
    AttentionTransport, CollectiveSchedule, CommittedTokens, GraphEntry, GraphKey, GraphProfile,
    RankExecutionError, RankExecutor, StepInput, StepMode, StepOutput, StepPlan, StepSampling,
    Tp4WorkerPool,
};
use glm_scheduler::{RequestSpec, RouteCatalog, SamplingCollective, SchedulerConfig, TenantConfig};
use glm_serving::{
    AdmissionStatus, PrefixRestoreCoordinator, RequestEvent, ServingConfig, ServingCoordinator,
    ServingStepObservation,
};
use serde::Serialize;

const CONCURRENCIES: [u16; 4] = [1, 2, 4, 8];
const MTP_DEPTHS: [u8; 2] = [0, 3];
const MAXIMUM_PROFILE_ITERATIONS: u32 = 100_000;
const PROFILE_TOKEN_BASE: u32 = 10_000;

#[derive(Clone, Copy, Debug)]
struct ProfileRankExecutor;

impl RankExecutor for ProfileRankExecutor {
    fn execute(
        &mut self,
        _rank: u8,
        plan: &StepPlan,
        _schedule: &CollectiveSchedule,
    ) -> Result<StepOutput, RankExecutionError> {
        synthetic_output(plan)
    }

    fn execute_bound(
        &mut self,
        _rank: u8,
        plan: &StepPlan,
        _schedule: &CollectiveSchedule,
        _input: &StepInput,
    ) -> Result<StepOutput, RankExecutionError> {
        synthetic_output(plan)
    }
}

fn synthetic_output(plan: &StepPlan) -> Result<StepOutput, RankExecutionError> {
    if matches!(plan.mode, StepMode::Prefill | StepMode::CacheOnly) {
        return Ok(StepOutput::empty());
    }
    if plan.mode == StepMode::Mixed {
        return Err(RankExecutionError::Invariant);
    }
    let committed = if plan.mode == StepMode::Decode {
        CommittedTokens::target(PROFILE_TOKEN_BASE)
    } else {
        let accepted = (0..plan.mtp_depth)
            .map(|ordinal| PROFILE_TOKEN_BASE + u32::from(ordinal))
            .collect::<Vec<_>>();
        CommittedTokens::verify(
            &accepted,
            Some(PROFILE_TOKEN_BASE + u32::from(plan.mtp_depth)),
        )
    }
    .map_err(|_| RankExecutionError::Invariant)?;
    let sequences = vec![committed; usize::from(plan.active_sequences)];
    StepOutput::new(&sequences).map_err(|_| RankExecutionError::Invariant)
}

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
            return Err("host profile has no timing samples");
        }
        let mut ordered = samples.to_vec();
        ordered.sort_unstable();
        let sum = ordered
            .iter()
            .try_fold(0_u128, |sum, &sample| sum.checked_add(u128::from(sample)));
        let sum = sum.ok_or("host profile timing sum overflow")?;
        let mean = sum / u128::try_from(ordered.len()).map_err(|_| "sample count overflow")?;
        Ok(Self {
            minimum: ordered[0],
            p50: nearest_rank(&ordered, 50),
            p95: nearest_rank(&ordered, 95),
            p99: nearest_rank(&ordered, 99),
            maximum: *ordered.last().ok_or("host profile has no timing samples")?,
            mean: u64::try_from(mean).map_err(|_| "host profile mean overflow")?,
        })
    }
}

fn nearest_rank(ordered: &[u64], percentile: usize) -> u64 {
    let rank = percentile
        .checked_mul(ordered.len())
        .and_then(|value| value.checked_add(99))
        .expect("bounded host profile percentile")
        / 100;
    ordered[rank.saturating_sub(1).min(ordered.len() - 1)]
}

#[derive(Debug, Serialize)]
struct HostStepSample {
    worker_round_trip_ns: u64,
    coordinator_overhead_ns: u64,
    total_step_ns: u64,
}

impl TryFrom<ServingStepObservation> for HostStepSample {
    type Error = &'static str;

    fn try_from(observation: ServingStepObservation) -> Result<Self, Self::Error> {
        Ok(Self {
            worker_round_trip_ns: duration_ns(observation.worker_round_trip)?,
            coordinator_overhead_ns: duration_ns(observation.coordinator_overhead)?,
            total_step_ns: duration_ns(observation.total_step_time)?,
        })
    }
}

fn duration_ns(duration: Duration) -> Result<u64, &'static str> {
    u64::try_from(duration.as_nanos()).map_err(|_| "host profile duration overflow")
}

#[derive(Debug, Serialize)]
struct HostProfileCell {
    concurrency: u16,
    configured_mtp_depth: u8,
    mode: &'static str,
    warmup_steps: u32,
    measured_steps: u32,
    useful_committed_tokens: u64,
    physical_steps: u32,
    synthetic_useful_tokens_per_second: f64,
    worker_round_trip_ns: NanosecondDistribution,
    coordinator_overhead_ns: NanosecondDistribution,
    total_step_ns: NanosecondDistribution,
    samples: Vec<HostStepSample>,
}

#[derive(Debug, Serialize)]
struct HostProfileReport {
    schema: &'static str,
    source_commit: String,
    tp_ranks: u8,
    worker_posture: &'static str,
    warmup_steps_per_cell: u32,
    measured_steps_per_cell: u32,
    cells: Vec<HostProfileCell>,
    claim: &'static str,
}

pub fn write_serving_host_profile(
    evidence_directory: &Path,
    source_commit: &str,
    warmup_steps: u32,
    measured_steps: u32,
) -> Result<(), Box<dyn std::error::Error>> {
    validate_inputs(
        evidence_directory,
        source_commit,
        warmup_steps,
        measured_steps,
    )?;
    let mut cells = Vec::with_capacity(CONCURRENCIES.len() * MTP_DEPTHS.len());
    for mtp_depth in MTP_DEPTHS {
        for concurrency in CONCURRENCIES {
            cells.push(run_cell(
                evidence_directory,
                concurrency,
                mtp_depth,
                warmup_steps,
                measured_steps,
            )?);
        }
    }
    let report = HostProfileReport {
        schema: "glmaxx.synthetic-serving-host-profile.v1",
        source_commit: source_commit.to_owned(),
        tp_ranks: 4,
        worker_posture: "custom-unverified-deterministic-cpu",
        warmup_steps_per_cell: warmup_steps,
        measured_steps_per_cell: measured_steps,
        cells,
        claim: "Rust scheduler/page-table/four-rank-worker host overhead only; no model, CUDA, checkpoint, quality, capacity, latency, or serving-throughput claim",
    };
    let mut json = serde_json::to_vec_pretty(&report)?;
    json.push(b'\n');
    let output = evidence_directory.join("serving-host-profile.json");
    fs::write(&output, &json)?;
    println!("wrote {} bytes to {}", json.len(), output.display());
    Ok(())
}

fn validate_inputs(
    evidence_directory: &Path,
    source_commit: &str,
    warmup_steps: u32,
    measured_steps: u32,
) -> Result<(), Box<dyn std::error::Error>> {
    if !evidence_directory.is_dir() || fs::read_dir(evidence_directory)?.next().is_some() {
        return Err("serving-host-profile requires an existing empty evidence directory".into());
    }
    if source_commit.len() != 40
        || !source_commit
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err("serving-host-profile requires a 40-digit lowercase Git commit".into());
    }
    if warmup_steps == 0 || measured_steps == 0 || measured_steps > MAXIMUM_PROFILE_ITERATIONS {
        return Err("serving-host-profile step counts are out of range".into());
    }
    let maximum_positions = u64::from(warmup_steps)
        .checked_add(u64::from(measured_steps))
        .and_then(|steps| steps.checked_mul(4))
        .and_then(|tokens| tokens.checked_add(1))
        .ok_or("serving-host-profile position arithmetic overflow")?;
    if maximum_positions > glm_cache::MODEL_POSITIONS {
        return Err("serving-host-profile would exceed the model position limit".into());
    }
    Ok(())
}

fn run_cell(
    evidence_directory: &Path,
    concurrency: u16,
    mtp_depth: u8,
    warmup_steps: u32,
    measured_steps: u32,
) -> Result<HostProfileCell, Box<dyn std::error::Error>> {
    let store_root = evidence_directory.join(format!("kv-c{concurrency}-mtp{mtp_depth}"));
    drop(FileTierStore::open(&store_root)?);
    let namespace = PrefixNamespace::new(NamespaceInputs {
        model_revision_sha256: [1; 32],
        tokenizer_sha256: [2; 32],
        chat_template_sha256: [3; 32],
        weight_policy_hash: [4; 32],
        target_kv_abi_sha256: [5; 32],
        draft_kv_abi_sha256: [6; 32],
        rope_parameters_sha256: [7; 32],
    })?;
    let prefix = PrefixRestoreCoordinator::new(
        PrefixIndex::new(namespace),
        &store_root,
        ResidencyConfig {
            hbm_bytes: 1,
            dram_bytes: 1,
        },
        2,
    )?;
    let executors =
        std::array::from_fn(|_| Box::new(ProfileRankExecutor) as Box<dyn RankExecutor + Send>);
    let mut serving = ServingCoordinator::new(
        ServingConfig {
            epoch: 1,
            event_capacity: 4_096,
            maximum_retained_prompt_bytes: 1 << 20,
            page_table: PageTableConfig {
                target_pages_per_rank: 4_096,
                draft_pages_per_rank: 4_096,
            },
        },
        SchedulerConfig {
            maximum_batch_sequences: 8,
            maximum_prefill_tokens: 64,
            maximum_decode_burst: 64,
        },
        profile()?,
        vec![TenantConfig {
            tenant: 1,
            weight: 1,
            maximum_active_requests: 8,
        }],
        routes(),
        Tp4WorkerPool::spawn(2, executors)?,
    )?;
    serving.attach_prefix_cache(prefix)?;

    let maximum_new_tokens = warmup_steps
        .checked_add(measured_steps)
        .and_then(|steps| steps.checked_mul(u32::from(mtp_depth) + 1))
        .and_then(|tokens| tokens.checked_add(64))
        .ok_or("serving-host-profile output token overflow")?;
    for row in 0..concurrency {
        let request_id = u64::from(mtp_depth) * 1_000 + u64::from(row) + 1;
        let spec = RequestSpec {
            id: request_id,
            tenant: 1,
            prompt_tokens: 1,
            maximum_new_tokens,
            mtp_depth,
            sampling: SamplingCollective::Greedy,
        };
        let status = serving.begin_admit_tokens_with_sampling_options(
            spec,
            StepSampling::greedy(request_id),
            &[42],
            true,
        )?;
        if !matches!(
            status,
            AdmissionStatus::Admitted {
                cached_prompt_tokens: 0
            }
        ) {
            return Err("empty prefix profile admission was unexpectedly asynchronous".into());
        }
    }
    let _ = serving.drain_events();

    let expected_mode = if mtp_depth == 0 {
        StepMode::Decode
    } else {
        StepMode::Verify
    };
    let required_steps = warmup_steps
        .checked_add(measured_steps)
        .ok_or("serving-host-profile step count overflow")?;
    let expected_tokens_per_step = u64::from(concurrency) * (u64::from(mtp_depth) + 1);
    let mut selected_steps = 0_u32;
    let mut useful_committed_tokens = 0_u64;
    let mut samples: Vec<HostStepSample> = Vec::with_capacity(usize::try_from(measured_steps)?);
    while selected_steps < required_steps {
        let observation = serving
            .tick_observed()?
            .ok_or("serving-host-profile scheduler became idle")?;
        let events = serving.drain_events();
        if observation.mode == StepMode::Prefill {
            continue;
        }
        if observation.mode != expected_mode
            || observation.real_sequences != concurrency
            || observation.bucket_sequences != concurrency
            || observation.mtp_depth != mtp_depth
        {
            return Err("serving-host-profile selected an unexpected graph route".into());
        }
        let token_events = u64::try_from(
            events
                .iter()
                .filter(|event| matches!(event, RequestEvent::Token { .. }))
                .count(),
        )?;
        if token_events != expected_tokens_per_step {
            return Err("serving-host-profile useful-token accounting drifted".into());
        }
        if selected_steps >= warmup_steps {
            useful_committed_tokens = useful_committed_tokens
                .checked_add(token_events)
                .ok_or("serving-host-profile useful-token overflow")?;
            samples.push(observation.try_into()?);
        }
        selected_steps += 1;
    }
    if samples.len() != usize::try_from(measured_steps)? {
        return Err("serving-host-profile sample count drifted".into());
    }

    let worker_samples = samples
        .iter()
        .map(|sample| sample.worker_round_trip_ns)
        .collect::<Vec<_>>();
    let coordinator_samples = samples
        .iter()
        .map(|sample| sample.coordinator_overhead_ns)
        .collect::<Vec<_>>();
    let total_samples = samples
        .iter()
        .map(|sample| sample.total_step_ns)
        .collect::<Vec<_>>();
    let total_ns = total_samples
        .iter()
        .try_fold(0_u128, |sum, &sample| sum.checked_add(u128::from(sample)));
    let total_ns = total_ns.ok_or("serving-host-profile total duration overflow")?;
    let synthetic_useful_tokens_per_second = if total_ns == 0 {
        return Err("serving-host-profile measured a zero duration".into());
    } else {
        useful_committed_tokens as f64 * 1_000_000_000.0 / total_ns as f64
    };
    Ok(HostProfileCell {
        concurrency,
        configured_mtp_depth: mtp_depth,
        mode: if mtp_depth == 0 { "decode" } else { "verify" },
        warmup_steps,
        measured_steps,
        useful_committed_tokens,
        physical_steps: measured_steps,
        synthetic_useful_tokens_per_second,
        worker_round_trip_ns: NanosecondDistribution::from_samples(&worker_samples)?,
        coordinator_overhead_ns: NanosecondDistribution::from_samples(&coordinator_samples)?,
        total_step_ns: NanosecondDistribution::from_samples(&total_samples)?,
        samples,
    })
}

fn profile() -> Result<GraphProfile, glm_engine::GraphProfileError> {
    let mut entries = Vec::with_capacity(CONCURRENCIES.len() * 3);
    let mut graph_id = 1_u32;
    for concurrency in CONCURRENCIES {
        entries.push(graph_entry(
            graph_id,
            StepMode::Prefill,
            concurrency,
            u32::from(concurrency),
            0,
        ));
        graph_id += 1;
        entries.push(graph_entry(
            graph_id,
            StepMode::Decode,
            concurrency,
            u32::from(concurrency),
            0,
        ));
        graph_id += 1;
        entries.push(graph_entry(
            graph_id,
            StepMode::Verify,
            concurrency,
            u32::from(concurrency) * 4,
            3,
        ));
        graph_id += 1;
    }
    GraphProfile::new(entries)
}

fn graph_entry(
    graph_id: u32,
    mode: StepMode,
    sequence_bucket: u16,
    rows: u32,
    mtp_depth: u8,
) -> GraphEntry {
    GraphEntry {
        graph_id,
        key: GraphKey {
            mode,
            sequence_bucket,
            verifier_row_bucket: if mode == StepMode::Prefill { 0 } else { rows },
            mtp_depth,
            attention_transport: if mode == StepMode::Prefill {
                AttentionTransport::PrefillQuery
            } else {
                AttentionTransport::DecodeQueryLse
            },
        },
        maximum_active_sequences: sequence_bucket,
        maximum_prompt_tokens: if mode == StepMode::Prefill { rows } else { 0 },
        maximum_query_rows: rows,
        compatible_tp_routes: vec![1],
        compatible_dcp_routes: vec![3, 4, 5],
        compatible_sampling_routes: if mode == StepMode::Prefill {
            vec![]
        } else {
            vec![6]
        },
        maximum_scratch_bytes: 1,
        argument_bytes: 1,
        graph_object_bytes: 1,
        resident_module_bytes: 1,
        admission_slo_class: 1,
    }
}

fn routes() -> RouteCatalog {
    RouteCatalog {
        tp_route_id: 1,
        dcp_ckv_route_id: 2,
        dcp_query_route_id: 3,
        dcp_candidate_route_id: 4,
        dcp_partial_route_id: 5,
        greedy_route_id: 6,
        top_k_route_id: 7,
        mass_route_id: 8,
        packed_ckv_bytes_per_row: 32,
        query_bytes_per_row: 32,
        candidate_bytes_per_row: 32,
        partial_state_bytes_per_row: 32,
        tp_reduce_bytes_per_row: 32,
        greedy_bytes_per_row: 8,
        top_k_bytes_per_row: 64,
        mass_bytes_per_row: 16,
    }
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    #[test]
    fn nearest_rank_and_input_validation_are_exact() {
        let samples = (1..=100).collect::<Vec<_>>();
        let distribution = NanosecondDistribution::from_samples(&samples).unwrap();
        assert_eq!(distribution.minimum, 1);
        assert_eq!(distribution.p50, 50);
        assert_eq!(distribution.p95, 95);
        assert_eq!(distribution.p99, 99);
        assert_eq!(distribution.maximum, 100);
        assert_eq!(distribution.mean, 50);
        assert_eq!(nearest_rank(&[7], 99), 7);
    }

    #[test]
    fn synthetic_executor_emits_exact_mtp_membership() {
        let profile = profile().unwrap();
        let decode = profile
            .entries
            .iter()
            .find(|entry| entry.key.mode == StepMode::Decode && entry.key.sequence_bucket == 1);
        let verify = profile
            .entries
            .iter()
            .find(|entry| entry.key.mode == StepMode::Verify && entry.key.sequence_bucket == 1);
        assert!(decode.is_some());
        assert!(verify.is_some());
        assert_eq!(profile.entries.len(), 12);
    }

    #[test]
    fn eight_cell_profile_runs_and_writes_bounded_evidence() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = std::env::temp_dir().join(format!(
            "glmaxx-host-profile-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir(&directory).unwrap();
        write_serving_host_profile(&directory, "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", 1, 1)
            .unwrap();
        let bytes = fs::read(directory.join("serving-host-profile.json")).unwrap();
        let report: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(report["schema"], "glmaxx.synthetic-serving-host-profile.v1");
        assert_eq!(report["cells"].as_array().unwrap().len(), 8);
        assert!(report["cells"].as_array().unwrap().iter().all(|cell| {
            cell["samples"]
                .as_array()
                .is_some_and(|samples| samples.len() == 1)
        }));
        fs::remove_dir_all(directory).unwrap();
    }
}
