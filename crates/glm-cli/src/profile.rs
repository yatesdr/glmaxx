use std::fmt;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const PROFILE_DECODE_ROWS: [u32; 8] = [1, 2, 4, 8, 16, 32, 64, 128];
pub const PROFILE_PREFILL_ROWS: [u32; 5] = [256, 512, 1024, 2048, 3072];
pub const PROFILE_EXL3_ROWS: [u32; 4] = [1, 2, 4, 8];

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProfileBackend {
    Nvfp4DirectFc1,
    Nvfp4GroupedFc1,
    Nvfp4DirectFc2,
    Nvfp4GroupedFc2,
    Exl3Gate,
    Exl3Up,
    Exl3Down,
}

impl ProfileBackend {
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Nvfp4DirectFc1 => "nvfp4-direct-fc1",
            Self::Nvfp4GroupedFc1 => "nvfp4-grouped-fc1",
            Self::Nvfp4DirectFc2 => "nvfp4-direct-fc2",
            Self::Nvfp4GroupedFc2 => "nvfp4-grouped-fc2",
            Self::Exl3Gate => "exl3-gate",
            Self::Exl3Up => "exl3-up",
            Self::Exl3Down => "exl3-down",
        }
    }

    #[must_use]
    pub const fn is_exl3(self) -> bool {
        matches!(self, Self::Exl3Gate | Self::Exl3Up | Self::Exl3Down)
    }

    #[must_use]
    #[cfg(feature = "cuda-ffi")]
    pub const fn is_grouped(self) -> bool {
        matches!(self, Self::Nvfp4GroupedFc1 | Self::Nvfp4GroupedFc2)
    }
}

impl FromStr for ProfileBackend {
    type Err = ProfileError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "nvfp4-direct-fc1" => Ok(Self::Nvfp4DirectFc1),
            "nvfp4-grouped-fc1" => Ok(Self::Nvfp4GroupedFc1),
            "nvfp4-direct-fc2" => Ok(Self::Nvfp4DirectFc2),
            "nvfp4-grouped-fc2" => Ok(Self::Nvfp4GroupedFc2),
            "exl3-gate" => Ok(Self::Exl3Gate),
            "exl3-up" => Ok(Self::Exl3Up),
            "exl3-down" => Ok(Self::Exl3Down),
            _ => Err(ProfileError::Backend),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProfileMode {
    Eager,
    Graph,
}

impl ProfileMode {
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Eager => "eager",
            Self::Graph => "graph",
        }
    }
}

impl FromStr for ProfileMode {
    type Err = ProfileError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "eager" => Ok(Self::Eager),
            "graph" => Ok(Self::Graph),
            _ => Err(ProfileError::Mode),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProfilePhase {
    Quantize,
    Core,
    Reduce,
    Inclusive,
    GraphInclusive,
    Projection,
}

impl ProfilePhase {
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Quantize => "quantize",
            Self::Core => "core",
            Self::Reduce => "reduce",
            Self::Inclusive => "inclusive",
            Self::GraphInclusive => "graph-inclusive",
            Self::Projection => "projection",
        }
    }
}

impl FromStr for ProfilePhase {
    type Err = ProfileError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "quantize" => Ok(Self::Quantize),
            "core" => Ok(Self::Core),
            "reduce" => Ok(Self::Reduce),
            "inclusive" => Ok(Self::Inclusive),
            "graph-inclusive" => Ok(Self::GraphInclusive),
            "projection" => Ok(Self::Projection),
            _ => Err(ProfileError::Phase),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProfileRouting {
    NotApplicable,
    EmptyExperts,
    OneHot,
    Uniform,
    Zipf,
    MaximallySkewed,
}

impl ProfileRouting {
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::NotApplicable => "not-applicable",
            Self::EmptyExperts => "empty-experts",
            Self::OneHot => "one-hot",
            Self::Uniform => "uniform",
            Self::Zipf => "zipf",
            Self::MaximallySkewed => "maximally-skewed",
        }
    }
}

impl FromStr for ProfileRouting {
    type Err = ProfileError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "not-applicable" => Ok(Self::NotApplicable),
            "empty-experts" => Ok(Self::EmptyExperts),
            "one-hot" => Ok(Self::OneHot),
            "uniform" => Ok(Self::Uniform),
            "zipf" => Ok(Self::Zipf),
            "maximally-skewed" => Ok(Self::MaximallySkewed),
            _ => Err(ProfileError::Routing),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ProfileCaseSpec {
    pub backend: ProfileBackend,
    pub mode: ProfileMode,
    pub phase: ProfilePhase,
    pub routing: ProfileRouting,
    pub rows: u32,
    pub warmup_iterations: u32,
    pub measured_iterations: u32,
}

impl ProfileCaseSpec {
    pub fn validate(self) -> Result<Self, ProfileError> {
        if self.warmup_iterations == 0
            || self.warmup_iterations > 100
            || self.measured_iterations == 0
            || self.measured_iterations > 10_000
        {
            return Err(ProfileError::Iterations);
        }
        let row_is_nvfp4 =
            PROFILE_DECODE_ROWS.contains(&self.rows) || PROFILE_PREFILL_ROWS.contains(&self.rows);
        if self.backend.is_exl3() {
            if !PROFILE_EXL3_ROWS.contains(&self.rows)
                || self.mode != ProfileMode::Eager
                || self.phase != ProfilePhase::Projection
                || self.routing != ProfileRouting::NotApplicable
            {
                return Err(ProfileError::Combination);
            }
            return Ok(self);
        }
        if !row_is_nvfp4 || self.routing == ProfileRouting::NotApplicable {
            return Err(ProfileError::Combination);
        }
        if self.mode == ProfileMode::Graph {
            if self.backend != ProfileBackend::Nvfp4DirectFc1
                || self.phase != ProfilePhase::GraphInclusive
            {
                return Err(ProfileError::Combination);
            }
            return Ok(self);
        }
        if self.phase == ProfilePhase::GraphInclusive || self.phase == ProfilePhase::Projection {
            return Err(ProfileError::Combination);
        }
        if matches!(
            self.backend,
            ProfileBackend::Nvfp4DirectFc1 | ProfileBackend::Nvfp4DirectFc2
        ) && self.routing != ProfileRouting::OneHot
        {
            return Err(ProfileError::Combination);
        }
        if matches!(
            self.backend,
            ProfileBackend::Nvfp4DirectFc1 | ProfileBackend::Nvfp4GroupedFc1
        ) && self.phase == ProfilePhase::Reduce
        {
            return Err(ProfileError::Combination);
        }
        Ok(self)
    }

    #[must_use]
    pub fn case_id(self) -> String {
        format!(
            "{}-{}-{}-{}-m{:04}",
            self.backend.id(),
            self.mode.id(),
            self.phase.id(),
            self.routing.id(),
            self.rows
        )
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[cfg(any(feature = "cuda-ffi", test))]
pub struct LatencyDistribution {
    pub unit: &'static str,
    pub sample_count: u32,
    pub samples: Vec<f64>,
    pub minimum: f64,
    pub p50: f64,
    pub p90: f64,
    pub p95: f64,
    pub p99: f64,
    pub maximum: f64,
    pub mean: f64,
    pub population_stddev: f64,
}

#[cfg(any(feature = "cuda-ffi", test))]
impl LatencyDistribution {
    pub fn from_microseconds(samples: Vec<f64>) -> Result<Self, ProfileError> {
        if samples.is_empty()
            || samples.len() > 10_000
            || samples
                .iter()
                .any(|value| !value.is_finite() || *value < 0.0)
        {
            return Err(ProfileError::Samples);
        }
        let sample_count = u32::try_from(samples.len()).map_err(|_| ProfileError::Samples)?;
        let mut ordered = samples.clone();
        ordered.sort_by(f64::total_cmp);
        let mean = samples.iter().sum::<f64>() / f64::from(sample_count);
        let variance = samples
            .iter()
            .map(|value| {
                let delta = value - mean;
                delta * delta
            })
            .sum::<f64>()
            / f64::from(sample_count);
        Ok(Self {
            unit: "microseconds",
            sample_count,
            minimum: ordered[0],
            p50: nearest_rank(&ordered, 50),
            p90: nearest_rank(&ordered, 90),
            p95: nearest_rank(&ordered, 95),
            p99: nearest_rank(&ordered, 99),
            maximum: *ordered.last().ok_or(ProfileError::Samples)?,
            mean,
            population_stddev: variance.sqrt(),
            samples,
        })
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[cfg(any(feature = "cuda-ffi", test))]
pub struct ByteLedger {
    pub input_bytes: u64,
    pub packed_value_bytes: u64,
    pub packed_scale_bytes: u64,
    pub metadata_bytes: u64,
    pub output_bytes: u64,
    pub temporary_bytes: u64,
    pub contract_read_bytes: u64,
    pub contract_write_bytes: u64,
}

#[cfg(any(feature = "cuda-ffi", test))]
impl ByteLedger {
    pub fn contract_traffic_bytes(self) -> Result<u64, ProfileError> {
        self.contract_read_bytes
            .checked_add(self.contract_write_bytes)
            .ok_or(ProfileError::Arithmetic)
    }

    #[cfg(feature = "cuda-ffi")]
    pub fn p50_gib_per_second(self, latency: &LatencyDistribution) -> Result<f64, ProfileError> {
        if latency.p50 == 0.0 {
            return Ok(0.0);
        }
        Ok(
            self.contract_traffic_bytes()? as f64 / latency.p50 * 1_000_000.0
                / (1024.0 * 1024.0 * 1024.0),
        )
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ProfilePlanCase {
    pub spec: ProfileCaseSpec,
    pub cuda_event_timing: bool,
    pub nsys_trace: bool,
    pub ncu_counter_replay: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ProfileBoundary {
    pub name: String,
    pub measurement: String,
    pub status: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ProfilePlan {
    pub schema: String,
    pub target: String,
    pub timer_runner: String,
    pub profiler_runner: String,
    pub cases: Vec<ProfilePlanCase>,
    pub non_kernel_boundaries: Vec<ProfileBoundary>,
}

pub const EVIDENCE_MANIFEST_NAME: &str = "evidence-manifest.json";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct EvidenceArtifact {
    pub relative_path: String,
    pub bytes: u64,
    pub sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct EvidenceManifest {
    pub schema: String,
    pub source_commit: String,
    pub artifacts: Vec<EvidenceArtifact>,
}

pub fn build_evidence_manifest(
    root: &Path,
    source_commit: &str,
) -> Result<EvidenceManifest, Box<dyn std::error::Error>> {
    if source_commit.len() != 40
        || !source_commit
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err("source commit must be exactly 40 lowercase hexadecimal characters".into());
    }
    let root = root.canonicalize()?;
    if !root.is_dir() {
        return Err("evidence root must be a directory".into());
    }
    let mut paths = Vec::new();
    collect_evidence_paths(&root, &root, &mut paths)?;
    paths.sort();
    let mut artifacts = Vec::with_capacity(paths.len());
    for path in paths {
        let relative = path.strip_prefix(&root)?;
        let relative = relative
            .to_str()
            .ok_or("evidence paths must be valid UTF-8")?
            .replace(std::path::MAIN_SEPARATOR, "/");
        if relative == EVIDENCE_MANIFEST_NAME {
            continue;
        }
        let metadata = path.symlink_metadata()?;
        artifacts.push(EvidenceArtifact {
            relative_path: relative,
            bytes: metadata.len(),
            sha256: file_sha256(&path)?,
        });
    }
    if artifacts.is_empty() {
        return Err("evidence root contains no nonempty regular artifacts".into());
    }
    Ok(EvidenceManifest {
        schema: "glmaxx.sm120-profiler-evidence-manifest.v1".to_owned(),
        source_commit: source_commit.to_owned(),
        artifacts,
    })
}

pub fn validate_evidence_manifest(
    root: &Path,
) -> Result<EvidenceManifest, Box<dyn std::error::Error>> {
    let manifest_path = root.join(EVIDENCE_MANIFEST_NAME);
    let metadata = manifest_path.symlink_metadata()?;
    if !metadata.file_type().is_file() || metadata.len() == 0 {
        return Err("evidence manifest must be a nonempty regular file".into());
    }
    let manifest: EvidenceManifest = serde_json::from_slice(&fs::read(&manifest_path)?)?;
    let rebuilt = build_evidence_manifest(root, &manifest.source_commit)?;
    if manifest != rebuilt {
        return Err("evidence manifest does not exactly match the immutable artifact set".into());
    }
    Ok(manifest)
}

fn collect_evidence_paths(
    root: &Path,
    directory: &Path,
    output: &mut Vec<PathBuf>,
) -> Result<(), Box<dyn std::error::Error>> {
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        let metadata = path.symlink_metadata()?;
        if metadata.file_type().is_symlink() {
            return Err(
                format!("evidence tree contains a symbolic link: {}", path.display()).into(),
            );
        }
        if metadata.file_type().is_dir() {
            collect_evidence_paths(root, &path, output)?;
        } else if metadata.file_type().is_file() {
            let canonical = path.canonicalize()?;
            if !canonical.starts_with(root) {
                return Err("evidence artifact escaped its root".into());
            }
            output.push(canonical);
        } else {
            return Err(format!("unsupported evidence file type: {}", path.display()).into());
        }
    }
    Ok(())
}

fn file_sha256(path: &Path) -> Result<String, Box<dyn std::error::Error>> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 1024 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

impl ProfilePlan {
    pub fn deterministic() -> Result<Self, ProfileError> {
        let mut cases = Vec::new();
        let nvfp4_rows = PROFILE_DECODE_ROWS.into_iter().chain(PROFILE_PREFILL_ROWS);
        for rows in nvfp4_rows.clone() {
            for phase in [
                ProfilePhase::Quantize,
                ProfilePhase::Core,
                ProfilePhase::Inclusive,
            ] {
                push_plan_case(
                    &mut cases,
                    ProfileBackend::Nvfp4DirectFc1,
                    ProfileMode::Eager,
                    phase,
                    ProfileRouting::OneHot,
                    rows,
                )?;
            }
            push_plan_case(
                &mut cases,
                ProfileBackend::Nvfp4DirectFc1,
                ProfileMode::Graph,
                ProfilePhase::GraphInclusive,
                ProfileRouting::OneHot,
                rows,
            )?;
        }
        for rows in nvfp4_rows.clone() {
            for routing in grouped_routing_cases() {
                for phase in [
                    ProfilePhase::Quantize,
                    ProfilePhase::Core,
                    ProfilePhase::Inclusive,
                ] {
                    push_plan_case(
                        &mut cases,
                        ProfileBackend::Nvfp4GroupedFc1,
                        ProfileMode::Eager,
                        phase,
                        routing,
                        rows,
                    )?;
                }
            }
        }
        for rows in nvfp4_rows.clone() {
            for phase in [
                ProfilePhase::Quantize,
                ProfilePhase::Core,
                ProfilePhase::Reduce,
                ProfilePhase::Inclusive,
            ] {
                push_plan_case(
                    &mut cases,
                    ProfileBackend::Nvfp4DirectFc2,
                    ProfileMode::Eager,
                    phase,
                    ProfileRouting::OneHot,
                    rows,
                )?;
            }
        }
        for rows in nvfp4_rows {
            for routing in grouped_routing_cases() {
                for phase in [
                    ProfilePhase::Quantize,
                    ProfilePhase::Core,
                    ProfilePhase::Reduce,
                    ProfilePhase::Inclusive,
                ] {
                    push_plan_case(
                        &mut cases,
                        ProfileBackend::Nvfp4GroupedFc2,
                        ProfileMode::Eager,
                        phase,
                        routing,
                        rows,
                    )?;
                }
            }
        }
        for backend in [
            ProfileBackend::Exl3Gate,
            ProfileBackend::Exl3Up,
            ProfileBackend::Exl3Down,
        ] {
            for rows in PROFILE_EXL3_ROWS {
                push_plan_case(
                    &mut cases,
                    backend,
                    ProfileMode::Eager,
                    ProfilePhase::Projection,
                    ProfileRouting::NotApplicable,
                    rows,
                )?;
            }
        }
        cases.sort_by_key(|case| case.spec.case_id());
        let mut identifiers = std::collections::BTreeSet::new();
        if cases.len() != 571
            || cases
                .iter()
                .any(|case| !identifiers.insert(case.spec.case_id()))
        {
            return Err(ProfileError::Plan);
        }
        Ok(Self {
            schema: "glmaxx.sm120-profiler-plan.v1".to_owned(),
            target: "GLM-5.2 TP4 on four PCIe SM120 GPUs".to_owned(),
            timer_runner: "glmaxx gpu-time-case".to_owned(),
            profiler_runner: "glmaxx gpu-profile-case".to_owned(),
            cases,
            non_kernel_boundaries: vec![
                boundary("routing", "retained host-clock samples", "runner-ready"),
                boundary(
                    "swiglu",
                    "included in FC1 core range; isolated split requires a reviewed kernel ABI",
                    "fused-boundary",
                ),
                boundary(
                    "collective",
                    "CUDA events plus NCCL trace at TP4 layer replay",
                    "blocked-by-layer-replay-review",
                ),
                boundary(
                    "host-and-end-to-end",
                    "retained host enqueue and wall-clock samples",
                    "runner-ready",
                ),
            ],
        })
    }
}

fn grouped_routing_cases() -> [ProfileRouting; 5] {
    [
        ProfileRouting::EmptyExperts,
        ProfileRouting::OneHot,
        ProfileRouting::Uniform,
        ProfileRouting::Zipf,
        ProfileRouting::MaximallySkewed,
    ]
}

fn push_plan_case(
    cases: &mut Vec<ProfilePlanCase>,
    backend: ProfileBackend,
    mode: ProfileMode,
    phase: ProfilePhase,
    routing: ProfileRouting,
    rows: u32,
) -> Result<(), ProfileError> {
    let spec = ProfileCaseSpec {
        backend,
        mode,
        phase,
        routing,
        rows,
        warmup_iterations: 20,
        measured_iterations: 200,
    }
    .validate()?;
    let representative_row = matches!(rows, 1 | 128 | 256 | 3072);
    cases.push(ProfilePlanCase {
        spec,
        cuda_event_timing: true,
        nsys_trace: representative_row,
        ncu_counter_replay: representative_row,
    });
    Ok(())
}

fn boundary(name: &str, measurement: &str, status: &str) -> ProfileBoundary {
    ProfileBoundary {
        name: name.to_owned(),
        measurement: measurement.to_owned(),
        status: status.to_owned(),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProfileError {
    Backend,
    Mode,
    Phase,
    Routing,
    Iterations,
    Combination,
    #[cfg(any(feature = "cuda-ffi", test))]
    Samples,
    #[cfg(any(feature = "cuda-ffi", test))]
    Arithmetic,
    Plan,
}

impl fmt::Display for ProfileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::Backend => "unknown profile backend",
            Self::Mode => "unknown profile execution mode",
            Self::Phase => "unknown profile phase",
            Self::Routing => "unknown profile routing case",
            Self::Iterations => "profile iteration count is outside the fixed bounds",
            Self::Combination => "unsupported or internally inconsistent profile case",
            #[cfg(any(feature = "cuda-ffi", test))]
            Self::Samples => "latency samples are empty, non-finite, negative, or oversized",
            #[cfg(any(feature = "cuda-ffi", test))]
            Self::Arithmetic => "profile byte arithmetic overflowed",
            Self::Plan => "deterministic profile plan is incomplete or contains duplicate cases",
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for ProfileError {}

#[cfg(any(feature = "cuda-ffi", test))]
fn nearest_rank(ordered: &[f64], percentile: usize) -> f64 {
    let rank = percentile
        .checked_mul(ordered.len())
        .and_then(|value| value.checked_add(99))
        .map(|value| value / 100)
        .unwrap_or(ordered.len());
    ordered[rank.saturating_sub(1).min(ordered.len() - 1)]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_case() -> ProfileCaseSpec {
        ProfileCaseSpec {
            backend: ProfileBackend::Nvfp4GroupedFc1,
            mode: ProfileMode::Eager,
            phase: ProfilePhase::Inclusive,
            routing: ProfileRouting::Zipf,
            rows: 3072,
            warmup_iterations: 20,
            measured_iterations: 200,
        }
    }

    #[test]
    fn profile_matrix_covers_exact_decode_and_prefill_contract() {
        assert_eq!(PROFILE_DECODE_ROWS, [1, 2, 4, 8, 16, 32, 64, 128]);
        assert_eq!(PROFILE_PREFILL_ROWS, [256, 512, 1024, 2048, 3072]);
        for rows in PROFILE_DECODE_ROWS.into_iter().chain(PROFILE_PREFILL_ROWS) {
            assert!(
                ProfileCaseSpec {
                    rows,
                    ..base_case()
                }
                .validate()
                .is_ok()
            );
        }
    }

    #[test]
    fn invalid_backend_mode_phase_and_routing_combinations_fail_closed() {
        assert!(
            ProfileCaseSpec {
                backend: ProfileBackend::Nvfp4GroupedFc1,
                mode: ProfileMode::Graph,
                phase: ProfilePhase::GraphInclusive,
                ..base_case()
            }
            .validate()
            .is_err()
        );
        assert!(
            ProfileCaseSpec {
                backend: ProfileBackend::Nvfp4DirectFc1,
                routing: ProfileRouting::Uniform,
                ..base_case()
            }
            .validate()
            .is_err()
        );
        assert!(
            ProfileCaseSpec {
                backend: ProfileBackend::Exl3Gate,
                phase: ProfilePhase::Projection,
                routing: ProfileRouting::NotApplicable,
                rows: 16,
                ..base_case()
            }
            .validate()
            .is_err()
        );
    }

    #[test]
    fn latency_distribution_retains_samples_and_uses_nearest_rank() {
        let distribution =
            LatencyDistribution::from_microseconds((1_u32..=100).rev().map(f64::from).collect())
                .unwrap();
        assert_eq!(distribution.sample_count, 100);
        assert_eq!(distribution.samples[0], 100.0);
        assert_eq!(distribution.minimum, 1.0);
        assert_eq!(distribution.p50, 50.0);
        assert_eq!(distribution.p90, 90.0);
        assert_eq!(distribution.p95, 95.0);
        assert_eq!(distribution.p99, 99.0);
        assert_eq!(distribution.maximum, 100.0);
        assert_eq!(distribution.mean, 50.5);
    }

    #[test]
    fn invalid_latency_samples_fail_closed() {
        assert!(LatencyDistribution::from_microseconds(Vec::new()).is_err());
        assert!(LatencyDistribution::from_microseconds(vec![-1.0]).is_err());
        assert!(LatencyDistribution::from_microseconds(vec![f64::NAN]).is_err());
    }

    #[test]
    fn deterministic_plan_has_all_unique_case_ids_and_fixed_replay_subset() {
        let first = ProfilePlan::deterministic().unwrap();
        let second = ProfilePlan::deterministic().unwrap();
        assert_eq!(first, second);
        assert_eq!(first.cases.len(), 571);
        assert!(first.cases.iter().all(|case| case.cuda_event_timing));
        assert!(first.cases.iter().any(|case| case.ncu_counter_replay));
        assert!(first.cases.iter().any(|case| !case.ncu_counter_replay));
        assert_eq!(
            serde_json::to_vec_pretty(&first).unwrap(),
            serde_json::to_vec_pretty(&second).unwrap()
        );
    }

    #[test]
    fn byte_ledger_overflow_fails_closed() {
        let ledger = ByteLedger {
            input_bytes: 0,
            packed_value_bytes: 0,
            packed_scale_bytes: 0,
            metadata_bytes: 0,
            output_bytes: 0,
            temporary_bytes: 0,
            contract_read_bytes: u64::MAX,
            contract_write_bytes: 1,
        };
        assert_eq!(
            ledger.contract_traffic_bytes(),
            Err(ProfileError::Arithmetic)
        );
    }

    #[test]
    fn evidence_manifest_detects_any_artifact_drift() {
        let root = std::env::temp_dir().join(format!(
            "glmaxx-profile-manifest-test-{}",
            std::process::id()
        ));
        fs::create_dir(&root).unwrap();
        fs::write(root.join("timing.json"), b"timing-v1\n").unwrap();
        fs::create_dir(root.join("ncu")).unwrap();
        fs::write(root.join("ncu/counters.csv"), b"counter-v1\n").unwrap();
        let manifest =
            build_evidence_manifest(&root, "0123456789abcdef0123456789abcdef01234567").unwrap();
        fs::write(
            root.join(EVIDENCE_MANIFEST_NAME),
            serde_json::to_vec_pretty(&manifest).unwrap(),
        )
        .unwrap();
        assert_eq!(validate_evidence_manifest(&root).unwrap(), manifest);
        fs::write(root.join("timing.json"), b"timing-v2\n").unwrap();
        assert!(validate_evidence_manifest(&root).is_err());
        fs::remove_dir_all(root).unwrap();
    }
}
