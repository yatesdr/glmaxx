use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs::{self, File};
use std::io::Read;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use glm_cache::{
    Budget, CacheCapacity, DurablePageRequest, FileTierStore, MODEL_POSITIONS, NamespaceInputs,
    PagePieceBytes, PageTableConfig, PrefixIndex, PrefixNamespace, ResidencyConfig, TierPiece,
};
use glm_cuda::{
    EXL3_KERNEL_ABI, Exl3Descriptor, Exl3KernelProjection, Fc1Descriptor, Fc2Descriptor,
    KernelPath, LaunchGeometry, exl3_workspace_bytes, fc2_grouped_workspace_bytes,
    fc2_workspace_bytes, workspace_bytes,
};
use glm_engine::{
    AttentionTransport, CollectiveKind, CollectiveOp, CollectiveSchedule, GIB, GraphEntry,
    GraphKey, GraphProfile, MIN_MTP_TENTATIVE_SLOTS_PER_RANK, MIN_PAGE_SLACK_SLOTS_PER_RANK,
    ProfileBudgetArtifact, ProfileClass, RankMemoryInput, STEP_PLAN_ABI, STEP_PLAN_RECORD_BYTES,
    StepMode, StepPlan, StepPlanRequest, SystemMemoryPlan, TP_RANK_MASK, Tp4WorkerPool,
    plan_system_memory,
};
#[cfg(feature = "cuda-ffi")]
use glm_engine::{
    LoadProfile, LoadVerificationMode, NativeCheckpointStartupConfig, READER_CHUNK_BYTES,
    load_native_checkpoint,
};
use glm_format::{
    CUTLASS_COMMIT, Codec, EXL3_MODEL_REVISION, EXL3_SOURCE_REVISION, Exl3Metadata, Exl3Projection,
    Exl3Trellis, KERNEL_ABI, NativeRankReader, PINNED_EXL3_REPOSITORY, PINNED_RANK_TENSOR_COUNT,
    PINNED_SOURCE_MANIFEST_SHA256, PackedNvfp4, PinnedRankPlan, PinnedSourceVerification, RankFile,
    RankFileBuilder, RankPayloadProof, RankWeightProfile, SafeTensorFile, ShardedSafetensors,
    StreamingRankConfig, StreamingRankSet, StreamingRankSummary, TensorPayload, TensorRecord,
    pinned_exl3_rank_plan, pinned_exl3_weight_policy_sha256, validate_pinned_exl3_checkpoint,
    verify_pinned_source_files,
};
use glm_reference::{
    DECODE_ROWS, ModelConstants, NUMERICAL_CASES, PREFILL_ROWS, ROUTING_CASES, compact_routes,
    generate_numerical_fixture, generate_routes, operation_manifest_json,
};
#[cfg(feature = "cuda-ffi")]
use glm_reference::{
    NumericalCase, Route, RoutedExpertWeights, RoutingCase, bf16_round, routed_fc1_oracle,
    routed_fc2_oracle,
};
use glm_scheduler::{
    RequestSpec, RequestState, RouteCatalog, SamplingCollective, SchedulerConfig, TenantConfig,
};
use glm_serving::{PrefixRestoreCoordinator, RequestEvent, ServingConfig, ServingCoordinator};
use glm_tokenizer::{
    CHAT_TEMPLATE_SHA256, ChatMessage, ChatRole, ChatTemplateOptions, GENERATION_CONFIG_SHA256,
    MODEL_VOCABULARY, PinnedTokenizer, ReasoningEffort, TOKEN_OUTPUT_TABLE_SHA256,
    TOKENIZER_CONFIG_SHA256, TOKENIZER_SHA256, TOKENIZER_VOCABULARY, render_chat,
};
use serde::Serialize;
use sha2::{Digest, Sha256};

mod cache_proof;
mod profile;
mod review;

const ACTUAL_PACKED_SHA256: &str =
    "a84be06b6bf6192eb51324ee57a1b6a4c57924c78709bcbe275b9f56b547cab5";
const ACTUAL_RANK0_SHA256: &str =
    "aa9df44c04d503b58fcd861c27b434b4fe0908233333b92bd8c4cc133bb7c392";
const REVIEW_ACCEPTANCE_TOKEN: &str = "manifest-abi-v0.2.2-accepted";
const CONVERSION_REPOSITORY: &str = "https://github.com/yatesdr/glmaxx.git";
const CN4_CONVERTER_CONTAINER_DIGEST: &str =
    "sha256:4eb9de29e5a532c672697762ca7b24e5e82316d6795dfdf616f129d35b794109";
const FORMAT_SPEC_BYTES: &[u8] = include_bytes!("../../../spec/format-v0.md");
const ENGINE_SPEC_BYTES: &[u8] = include_bytes!("../../../spec/engine-v0.md");

fn main() {
    if let Err(error) = run() {
        eprintln!("glmaxx: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let arguments: Vec<String> = env::args().collect();
    match arguments.get(1).map(String::as_str) {
        Some("manifest") => {
            let json = operation_manifest_json()?;
            if let Some(path) = arguments.get(2) {
                fs::write(path, &json)?;
                println!("wrote {} bytes to {path}", json.len());
            } else {
                println!("{}", String::from_utf8(json)?);
            }
        }
        Some("cpu-proof") => cpu_proof()?,
        Some("direct-tier-proof") => {
            let report = direct_tier_proof()?;
            let mut json = serde_json::to_vec_pretty(&report)?;
            json.push(b'\n');
            if let Some(path) = arguments.get(2) {
                fs::write(path, &json)?;
                println!("wrote {} bytes to {path}", json.len());
            } else {
                println!("{}", String::from_utf8(json)?);
            }
        }
        Some("direct-tier-state-proof") => {
            let report = direct_tier_state_proof()?;
            let mut json = serde_json::to_vec_pretty(&report)?;
            json.push(b'\n');
            if let Some(path) = arguments.get(2) {
                fs::write(path, &json)?;
                println!("wrote {} bytes to {path}", json.len());
            } else {
                println!("{}", String::from_utf8(json)?);
            }
        }
        Some("direct-tier-checksum-proof") => {
            let report = direct_tier_checksum_proof()?;
            let mut json = serde_json::to_vec_pretty(&report)?;
            json.push(b'\n');
            if let Some(path) = arguments.get(2) {
                fs::write(path, &json)?;
                println!("wrote {} bytes to {path}", json.len());
            } else {
                println!("{}", String::from_utf8(json)?);
            }
        }
        Some("direct-tier-checksum-worker-proof") => {
            let report = direct_tier_checksum_worker_proof()?;
            let mut json = serde_json::to_vec_pretty(&report)?;
            json.push(b'\n');
            if let Some(path) = arguments.get(2) {
                fs::write(path, &json)?;
                println!("wrote {} bytes to {path}", json.len());
            } else {
                println!("{}", String::from_utf8(json)?);
            }
        }
        Some("exl3-warp-proof") => {
            let report = glm_format::prove_exl3_warp_staging_v2()?;
            let mut json = serde_json::to_vec_pretty(&report)?;
            json.push(b'\n');
            if let Some(path) = arguments.get(2) {
                fs::write(path, &json)?;
                println!("wrote {} bytes to {path}", json.len());
            } else {
                println!("{}", String::from_utf8(json)?);
            }
        }
        Some("matrix-proof") => {
            let report = matrix_proof()?;
            let json = serde_json::to_vec_pretty(&report)?;
            if let Some(path) = arguments.get(2) {
                fs::write(path, &json)?;
                println!("wrote {} bytes to {path}", json.len());
            } else {
                println!("{}", String::from_utf8(json)?);
            }
        }
        Some("pack-actual") => {
            let path = arguments
                .get(2)
                .ok_or("pack-actual requires an output path")?;
            pack_actual(Path::new(path))?;
        }
        Some("inspect") => {
            let path = arguments.get(2).ok_or("inspect requires a rank file")?;
            inspect(Path::new(path))?;
        }
        Some("budget") => print_budget()?,
        Some("abi-check") => abi_check()?,
        Some("engine-proof") => {
            let report = engine_proof()?;
            let mut json = serde_json::to_vec_pretty(&report)?;
            if let Some(path) = arguments.get(2) {
                json.push(b'\n');
                fs::write(path, &json)?;
                println!("wrote {} bytes to {path}", json.len());
            } else {
                println!("{}", String::from_utf8(json)?);
            }
        }
        Some("serving-proof") => {
            let path = arguments
                .get(2)
                .ok_or("serving-proof requires an external evidence directory")?;
            serving_proof(Path::new(path))?;
        }
        Some("cache-lifecycle-proof") => {
            let path = arguments
                .get(2)
                .ok_or("cache-lifecycle-proof requires an external evidence directory")?;
            cache_proof::write_cache_lifecycle_proof(Path::new(path))?;
        }
        Some("tokenizer-proof") => {
            let bundle = arguments
                .get(2)
                .ok_or("tokenizer-proof requires the pinned tokenizer directory")?;
            let report = tokenizer_proof(Path::new(bundle))?;
            let mut json = serde_json::to_vec_pretty(&report)?;
            json.push(b'\n');
            if let Some(path) = arguments.get(3) {
                fs::write(path, &json)?;
                println!("wrote {} bytes to {path}", json.len());
            } else {
                println!("{}", String::from_utf8(json)?);
            }
        }
        Some("exl3-proof") => {
            let path = arguments
                .get(2)
                .ok_or("exl3-proof requires a source payload path")?;
            exl3_proof(Path::new(path))?;
        }
        Some("safetensors-inventory") => {
            let path = arguments
                .get(2)
                .ok_or("safetensors-inventory requires a safetensors file or index")?;
            safetensors_inventory(Path::new(path))?;
        }
        Some("exl3-safetensors-proof") => {
            let path = arguments
                .get(2)
                .ok_or("exl3-safetensors-proof requires a safetensors file or index")?;
            let layer = parse_argument::<u16>(&arguments, 3, "layer")?;
            let expert = parse_argument::<u16>(&arguments, 4, "expert")?;
            let rank = parse_argument::<u8>(&arguments, 5, "rank")?;
            let projection = arguments
                .get(6)
                .ok_or("exl3-safetensors-proof requires gate, up, or down")?;
            exl3_safetensors_proof(Path::new(path), layer, expert, rank, projection.as_str())?;
        }
        Some("checkpoint-proof") => {
            let path = arguments
                .get(2)
                .ok_or("checkpoint-proof requires the pinned safetensors index")?;
            checkpoint_proof(Path::new(path))?;
        }
        Some("checkpoint-source-proof") => {
            let path = arguments
                .get(2)
                .ok_or("checkpoint-source-proof requires the pinned safetensors index")?;
            checkpoint_source_proof(Path::new(path))?;
        }
        Some("native-rank-proof") => {
            if arguments.len() > 4 {
                return Err(
                    "native-rank-proof accepts a rank-set directory and optional output path"
                        .into(),
                );
            }
            let directory = arguments
                .get(2)
                .ok_or("native-rank-proof requires the native rank-set directory")?;
            let proof = native_rank_proof(Path::new(directory))?;
            let mut json = serde_json::to_vec_pretty(&proof)?;
            json.push(b'\n');
            if let Some(path) = arguments.get(3) {
                fs::write(path, &json)?;
                println!("wrote {} bytes to {path}", json.len());
            } else {
                println!("{}", String::from_utf8(json)?);
            }
        }
        Some("convert-pinned-exl3") => {
            let index = arguments
                .get(2)
                .ok_or("convert-pinned-exl3 requires the pinned safetensors index")?;
            let output = arguments
                .get(3)
                .ok_or("convert-pinned-exl3 requires an output directory")?;
            let conversion_commit = arguments
                .get(4)
                .ok_or("convert-pinned-exl3 requires an exact conversion commit")?;
            let profile_budget = arguments
                .get(5)
                .ok_or("convert-pinned-exl3 requires profile-budget-v0.json")?;
            let review = arguments
                .get(6)
                .ok_or("convert-pinned-exl3 requires the independent review artifact")?;
            convert_pinned_exl3(
                Path::new(index),
                Path::new(output),
                conversion_commit,
                Path::new(profile_budget),
                Path::new(review),
            )?;
        }
        Some("review-proof") => {
            if arguments.len() > 4 {
                return Err("review-proof accepts a handoff and optional review artifact".into());
            }
            let handoff = arguments
                .get(2)
                .ok_or("review-proof requires a handoff path")?;
            let review = arguments.get(3).map(Path::new);
            let proof = review::verify_review_handoff(Path::new("."), Path::new(handoff), review)?;
            println!("{}", serde_json::to_string_pretty(&proof)?);
        }
        Some("review-acceptance-lint") => {
            if arguments.len() != 4 {
                return Err(
                    "review-acceptance-lint requires a handoff and staged review artifact".into(),
                );
            }
            let handoff = arguments
                .get(2)
                .ok_or("review-acceptance-lint requires a handoff path")?;
            let staged_review = arguments
                .get(3)
                .ok_or("review-acceptance-lint requires a staged review artifact")?;
            let proof = review::verify_staged_review_acceptance(
                Path::new("."),
                Path::new(handoff),
                Path::new(staged_review),
            )?;
            println!("{}", serde_json::to_string_pretty(&proof)?);
        }
        Some("review-acceptance-lint-all") => {
            if !(3..=4).contains(&arguments.len()) {
                return Err(
                    "review-acceptance-lint-all requires a staging directory and accepts an \
                     optional output path"
                        .into(),
                );
            }
            let staging_directory = arguments
                .get(2)
                .ok_or("review-acceptance-lint-all requires a staging directory")?;
            let proof = review::verify_all_staged_review_acceptances(
                Path::new("."),
                Path::new(staging_directory),
            )?;
            let mut json = serde_json::to_vec_pretty(&proof)?;
            json.push(b'\n');
            if let Some(path) = arguments.get(3) {
                fs::write(path, &json)?;
                println!(
                    "linted {} staged reviews: {} ready, {} rejected, {} absent; \
                     wrote {} bytes to {path}",
                    proof.present_staged_reviews,
                    proof.ready_staged_reviews,
                    proof.rejected_staged_reviews,
                    proof.absent_staged_reviews,
                    json.len()
                );
            } else {
                println!("{}", String::from_utf8(json)?);
            }
            if proof.rejected_staged_reviews != 0 {
                return Err(format!(
                    "{} staged review artifacts failed acceptance lint",
                    proof.rejected_staged_reviews
                )
                .into());
            }
        }
        Some("profile-plan") => {
            if arguments.len() > 3 {
                return Err("profile-plan accepts only an optional output path".into());
            }
            let plan = profile::ProfilePlan::deterministic()?;
            let mut json = serde_json::to_vec_pretty(&plan)?;
            json.push(b'\n');
            if let Some(path) = arguments.get(2) {
                fs::write(path, &json)?;
                println!("wrote {} bytes to {path}", json.len());
            } else {
                println!("{}", String::from_utf8(json)?);
            }
        }
        Some("profile-plan-validate") => {
            if arguments.len() != 3 {
                return Err("profile-plan-validate requires exactly one plan path".into());
            }
            let path = arguments
                .get(2)
                .ok_or("profile-plan-validate requires a plan path")?;
            let bytes = fs::read(path)?;
            let actual: profile::ProfilePlan = serde_json::from_slice(&bytes)?;
            let expected = profile::ProfilePlan::deterministic()?;
            if actual != expected {
                return Err("profile plan differs from the deterministic in-tree contract".into());
            }
            println!("profile-plan-valid cases={}", actual.cases.len());
        }
        Some("profile-evidence-manifest") => {
            if arguments.len() != 4 {
                return Err(
                    "profile-evidence-manifest requires evidence-root source-commit".into(),
                );
            }
            let root = Path::new(
                arguments
                    .get(2)
                    .ok_or("profile-evidence-manifest requires an evidence root")?,
            );
            validate_external_profile_evidence_root(root)?;
            let output = root.join(profile::EVIDENCE_MANIFEST_NAME);
            if output.exists() {
                return Err("evidence manifest already exists".into());
            }
            let manifest = profile::build_evidence_manifest(
                root,
                arguments
                    .get(3)
                    .ok_or("profile-evidence-manifest requires a source commit")?,
            )?;
            let mut json = serde_json::to_vec_pretty(&manifest)?;
            json.push(b'\n');
            fs::write(&output, &json)?;
            println!(
                "profile-evidence-manifest artifacts={} path={}",
                manifest.artifacts.len(),
                output.display()
            );
        }
        Some("profile-evidence-validate") => {
            if arguments.len() != 3 {
                return Err("profile-evidence-validate requires one evidence-root".into());
            }
            let root = Path::new(
                arguments
                    .get(2)
                    .ok_or("profile-evidence-validate requires an evidence root")?,
            );
            validate_external_profile_evidence_root(root)?;
            let manifest = profile::validate_evidence_manifest(root)?;
            println!(
                "profile-evidence-valid source_commit={} artifacts={}",
                manifest.source_commit,
                manifest.artifacts.len()
            );
        }
        Some("review-proof-all") => {
            if arguments.len() > 4 {
                return Err(
                    "review-proof-all accepts a repository and optional output path".into(),
                );
            }
            let repository = arguments.get(2).map_or_else(|| Path::new("."), Path::new);
            let proof = review::verify_all_review_handoffs(repository)?;
            let mut json = serde_json::to_vec_pretty(&proof)?;
            json.push(b'\n');
            if let Some(path) = arguments.get(3) {
                fs::write(path, &json)?;
                println!(
                    "verified {} review handoffs and {}/{} configured results \
                     ({} accepted, {} withheld); wrote {} bytes to {path}",
                    proof.verified_handoffs.len(),
                    proof.present_review_results,
                    proof.configured_review_results,
                    proof.accepted_review_results,
                    proof.withheld_review_results,
                    json.len()
                );
            } else {
                println!("{}", String::from_utf8(json)?);
            }
        }
        #[cfg(feature = "cuda-ffi")]
        Some("gpu-smoke") => {
            let rows = arguments
                .get(2)
                .map(|value| value.parse::<u32>())
                .transpose()?
                .unwrap_or(1);
            gpu_smoke(rows)?;
        }
        #[cfg(feature = "cuda-ffi")]
        Some("gpu-fc2-smoke") => {
            let rows = arguments
                .get(2)
                .map(|value| value.parse::<u32>())
                .transpose()?
                .unwrap_or(1);
            gpu_fc2_smoke(rows)?;
        }
        #[cfg(feature = "cuda-ffi")]
        Some("gpu-exl3-smoke") => {
            let projection = arguments.get(2).map(String::as_str).unwrap_or("gate");
            let rows = arguments
                .get(3)
                .map(|value| value.parse::<u32>())
                .transpose()?
                .unwrap_or(1);
            gpu_exl3_smoke(projection, rows)?;
        }
        #[cfg(feature = "cuda-ffi")]
        Some("gpu-rank-bind-smoke") => gpu_rank_bind_smoke()?,
        #[cfg(feature = "cuda-ffi")]
        Some("gpu-rank-memory-baseline") => gpu_rank_memory_baseline()?,
        #[cfg(feature = "cuda-ffi")]
        Some("gpu-checkpoint-load-smoke") => {
            if arguments.len() > 6 {
                return Err(
                    "gpu-checkpoint-load-smoke accepts rank-set-dir, profile-budget-v0.json, \
                     evidence-dir, and optional phase-timeout-seconds"
                        .into(),
                );
            }
            let rank_set = arguments
                .get(2)
                .ok_or("gpu-checkpoint-load-smoke requires the native rank-set directory")?;
            let profile_budget = arguments
                .get(3)
                .ok_or("gpu-checkpoint-load-smoke requires profile-budget-v0.json")?;
            let evidence = arguments
                .get(4)
                .ok_or("gpu-checkpoint-load-smoke requires an external empty evidence directory")?;
            let phase_timeout_seconds = arguments
                .get(5)
                .map(|value| value.parse::<u64>())
                .transpose()?
                .unwrap_or(900);
            gpu_checkpoint_load_smoke(
                Path::new(rank_set),
                Path::new(profile_budget),
                Path::new(evidence),
                phase_timeout_seconds,
            )?;
        }
        #[cfg(feature = "cuda-ffi")]
        Some("gpu-matrix") => {
            let path = arguments
                .get(2)
                .ok_or("gpu-matrix requires an external evidence directory")?;
            gpu_matrix(Path::new(path))?;
        }
        #[cfg(feature = "cuda-ffi")]
        Some("gpu-graph") => {
            let path = arguments
                .get(2)
                .ok_or("gpu-graph requires an external evidence directory")?;
            gpu_graph(Path::new(path))?;
        }
        #[cfg(feature = "cuda-ffi")]
        Some("gpu-dense-control") => {
            let path = arguments
                .get(2)
                .ok_or("gpu-dense-control requires an external evidence directory")?;
            gpu_dense_control(Path::new(path))?;
        }
        #[cfg(feature = "cuda-ffi")]
        Some("gpu-grouped-control") => {
            let path = arguments
                .get(2)
                .ok_or("gpu-grouped-control requires an external evidence directory")?;
            gpu_grouped_control(Path::new(path))?;
        }
        #[cfg(feature = "cuda-ffi")]
        Some("gpu-grouped-bench") => {
            let path = arguments
                .get(2)
                .ok_or("gpu-grouped-bench requires an external evidence directory")?;
            gpu_grouped_bench(Path::new(path))?;
        }
        #[cfg(feature = "cuda-ffi")]
        Some("gpu-bench") => {
            let path = arguments
                .get(2)
                .ok_or("gpu-bench requires an external evidence directory")?;
            gpu_bench(Path::new(path))?;
        }
        #[cfg(feature = "cuda-ffi")]
        Some("gpu-time-case") => {
            let (case, evidence) = parse_gpu_profile_case(&arguments)?;
            gpu_profile_case(case, &evidence, false)?;
        }
        #[cfg(feature = "cuda-ffi")]
        Some("gpu-profile-case") => {
            let (case, evidence) = parse_gpu_profile_case(&arguments)?;
            gpu_profile_case(case, &evidence, true)?;
        }
        _ => {
            return Err(
                "usage: glmaxx <manifest [path]|cpu-proof|direct-tier-proof [path]|direct-tier-state-proof [path]|direct-tier-checksum-proof [path]|direct-tier-checksum-worker-proof [path]|exl3-warp-proof [path]|matrix-proof [path]|pack-actual path|inspect path|budget|abi-check|engine-proof [path]|serving-proof evidence-dir|cache-lifecycle-proof evidence-dir|tokenizer-proof pinned-tokenizer-dir [path]|exl3-proof source-payload|safetensors-inventory file-or-index|exl3-safetensors-proof file-or-index layer expert rank gate|up|down|checkpoint-proof pinned-index|checkpoint-source-proof pinned-index|native-rank-proof rank-set-dir [path]|convert-pinned-exl3 pinned-index output-dir conversion-commit profile-budget-v0.json review-artifact|review-proof handoff [review-artifact]|review-acceptance-lint handoff staged-review-artifact|review-acceptance-lint-all staging-directory [path]|review-proof-all [repository] [path]|profile-plan [path]|profile-plan-validate path|profile-evidence-manifest root source-commit|profile-evidence-validate root|gpu-rank-bind-smoke|gpu-rank-memory-baseline|gpu-checkpoint-load-smoke rank-set-dir profile-budget-v0.json evidence-dir [phase-timeout-seconds]|gpu-smoke [rows]|gpu-fc2-smoke [rows]|gpu-exl3-smoke [gate|up|down] [rows]|gpu-matrix evidence-dir|gpu-graph evidence-dir|gpu-dense-control evidence-dir|gpu-grouped-control evidence-dir|gpu-bench evidence-dir|gpu-grouped-bench evidence-dir|gpu-time-case backend mode phase routing rows warmups iterations evidence-dir|gpu-profile-case backend mode phase routing rows warmups iterations evidence-dir>"
                    .into(),
            );
        }
    }
    Ok(())
}

#[derive(Serialize)]
struct CheckpointProof {
    schema: &'static str,
    repository: &'static str,
    revision: &'static str,
    source: String,
    structure_sha256: String,
    tensor_count: usize,
    shard_count: usize,
    payload_bytes: u64,
    exl3_component_count: usize,
    protected_tensor_count: usize,
    verdict: &'static str,
}

fn checkpoint_proof(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    if !path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.ends_with(".safetensors.index.json"))
    {
        return Err("checkpoint-proof requires the pinned standard index, not a directory".into());
    }
    let checkpoint = ShardedSafetensors::open(path)?;
    let inventory = validate_pinned_exl3_checkpoint(&checkpoint, EXL3_MODEL_REVISION)?;
    let proof = CheckpointProof {
        schema: "glmaxx.pinned-checkpoint-proof.v1",
        repository: PINNED_EXL3_REPOSITORY,
        revision: EXL3_MODEL_REVISION,
        source: path.display().to_string(),
        structure_sha256: hex(&inventory.structure_sha256),
        tensor_count: inventory.tensor_count,
        shard_count: inventory.shard_count,
        payload_bytes: inventory.payload_bytes,
        exl3_component_count: inventory.exl3_component_count,
        protected_tensor_count: inventory.protected_tensor_count,
        verdict: "PINNED_CHECKPOINT_STRUCTURE_PASS",
    };
    println!("{}", serde_json::to_string_pretty(&proof)?);
    Ok(())
}

#[derive(Serialize)]
struct CheckpointSourceProof {
    schema: &'static str,
    repository: &'static str,
    revision: &'static str,
    identity_basis: &'static str,
    source_markers_verified: bool,
    source: String,
    structure_sha256: String,
    manifest_sha256: String,
    verified_file_count: usize,
    verified_file_bytes: u64,
    file_sha256: BTreeMap<String, String>,
    publisher_manifest_exception_count: usize,
    publisher_manifest_exceptions: BTreeMap<String, PublisherManifestExceptionProof>,
    verdict: &'static str,
}

#[derive(Serialize)]
struct PublisherManifestExceptionProof {
    manifest_sha256: String,
    revision_sha256: String,
    reason: &'static str,
}

fn checkpoint_source_proof(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    if !path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.ends_with(".safetensors.index.json"))
    {
        return Err(
            "checkpoint-source-proof requires the pinned standard index, not a directory".into(),
        );
    }
    let checkpoint = ShardedSafetensors::open(path)?;
    let inventory = validate_pinned_exl3_checkpoint(&checkpoint, EXL3_MODEL_REVISION)?;
    let source =
        verify_pinned_source_files(&checkpoint, |completed, total, verified_bytes, name| {
            eprintln!("source-verify {completed}/{total} bytes={verified_bytes} file={name}");
        })?;
    let file_sha256 = source
        .files()
        .iter()
        .map(|(name, digest)| (name.clone(), hex(digest)))
        .collect();
    let publisher_manifest_exceptions: BTreeMap<_, _> = source
        .manifest_exceptions()
        .iter()
        .map(|(name, exception)| {
            (
                name.clone(),
                PublisherManifestExceptionProof {
                    manifest_sha256: hex(&exception.manifest_sha256),
                    revision_sha256: hex(&exception.revision_sha256),
                    reason: "pinned upstream manifest is self-inconsistent for non-model metadata",
                },
            )
        })
        .collect();
    let publisher_manifest_exception_count = publisher_manifest_exceptions.len();
    let proof = CheckpointSourceProof {
        schema: "glmaxx.pinned-checkpoint-source-proof.v3",
        repository: PINNED_EXL3_REPOSITORY,
        revision: EXL3_MODEL_REVISION,
        identity_basis: if source.source_markers_verified() {
            "exact-manifest-and-optional-source-markers"
        } else {
            "exact-content-addressed-manifest"
        },
        source_markers_verified: source.source_markers_verified(),
        source: path.display().to_string(),
        structure_sha256: hex(&inventory.structure_sha256),
        manifest_sha256: hex(&source.manifest_sha256()),
        verified_file_count: source.file_count(),
        verified_file_bytes: source.verified_file_bytes(),
        file_sha256,
        publisher_manifest_exception_count,
        publisher_manifest_exceptions,
        verdict: "PINNED_CHECKPOINT_SOURCE_PASS",
    };
    println!("{}", serde_json::to_string_pretty(&proof)?);
    Ok(())
}

#[derive(Debug, Serialize)]
struct NativeRankEvidence {
    rank: u32,
    file_uuid: String,
    tensor_count: usize,
    payload_bytes: u64,
    payload_sha256: String,
    tensor_contract_sha256: String,
    stream_chunks: u64,
    maximum_reader_scratch_bytes: usize,
}

#[derive(Debug, Serialize)]
struct NativeRankSetProof {
    schema: &'static str,
    conversion_uuid: String,
    model_config_sha256: String,
    tokenizer_bundle_sha256: String,
    chat_template_sha256: String,
    weight_policy_sha256: String,
    kernel_abi_sha256: String,
    operation_manifest_sha256: String,
    profile: RankWeightProfile,
    profile_budget_sha256: String,
    ranks: Vec<NativeRankEvidence>,
    verdict: &'static str,
}

fn native_rank_paths(directory: &Path) -> Result<[PathBuf; 4], Box<dyn std::error::Error>> {
    let metadata = directory.symlink_metadata()?;
    if !metadata.file_type().is_dir() || metadata.file_type().is_symlink() {
        return Err("native rank set requires a real directory".into());
    }
    let expected_names: BTreeSet<String> = (0..4).map(|rank| format!("rank-{rank}.g5n")).collect();
    let mut actual_names = BTreeSet::new();
    for entry in directory.read_dir()? {
        let entry = entry?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| "native rank-set directory contains a non-UTF-8 entry")?;
        actual_names.insert(name);
    }
    if actual_names != expected_names {
        return Err(format!(
            "native rank-set directory must contain exactly rank-0.g5n through rank-3.g5n; found {actual_names:?}"
        )
        .into());
    }

    Ok(std::array::from_fn(|rank| {
        directory.join(format!("rank-{rank}.g5n"))
    }))
}

fn native_rank_proof(directory: &Path) -> Result<NativeRankSetProof, Box<dyn std::error::Error>> {
    let paths = native_rank_paths(directory)?;
    let readers: Vec<NativeRankReader> = paths
        .iter()
        .map(NativeRankReader::open)
        .collect::<Result<_, _>>()?;
    let readers: [NativeRankReader; 4] = readers
        .try_into()
        .map_err(|_| "native rank-set reader count was not four")?;
    NativeRankReader::validate_rank_set([&readers[0], &readers[1], &readers[2], &readers[3]])?;
    let manifests = readers
        .iter()
        .map(|reader| {
            reader
                .validated_manifest()
                .ok_or("native rank file does not contain the production rank manifest schema")
        })
        .collect::<Result<Vec<_>, _>>()?;
    let operation_manifest_sha256 = sha256(&operation_manifest_json()?);
    if manifests[0].operation_manifest_sha256 != operation_manifest_sha256 {
        return Err(format!(
            "native rank-set operation manifest does not match this binary: file={}, binary={}",
            hex(&manifests[0].operation_manifest_sha256),
            hex(&operation_manifest_sha256)
        )
        .into());
    }
    let compiled_weight_policy_sha256 = pinned_exl3_weight_policy_sha256();
    if readers[0].weight_policy_sha256 != compiled_weight_policy_sha256 {
        return Err(format!(
            "native rank-set weight policy does not match this binary: file={}, binary={}",
            hex(&readers[0].weight_policy_sha256),
            hex(&compiled_weight_policy_sha256)
        )
        .into());
    }
    if readers[0].tensor_count() != PINNED_RANK_TENSOR_COUNT {
        return Err(format!(
            "capacity-exl3 rank-set tensor count is {}, expected {PINNED_RANK_TENSOR_COUNT}",
            readers[0].tensor_count()
        )
        .into());
    }
    let compiled_kernel_abi_sha256 = sha256(KERNEL_ABI.as_bytes());
    if readers[0].kernel_abi_sha256 != compiled_kernel_abi_sha256 {
        return Err(format!(
            "native rank-set kernel ABI does not match this binary: file={}, binary={}",
            hex(&readers[0].kernel_abi_sha256),
            hex(&compiled_kernel_abi_sha256)
        )
        .into());
    }

    let payload_proofs: Vec<RankPayloadProof> = thread::scope(|scope| {
        let handles: Vec<_> = readers
            .iter()
            .map(|reader| scope.spawn(|| reader.verify()))
            .collect();
        handles
            .into_iter()
            .map(|handle| {
                handle
                    .join()
                    .map_err(|_| "native rank verifier thread panicked".to_owned())?
                    .map_err(|error| error.to_string())
            })
            .collect::<Result<Vec<_>, String>>()
    })?;
    let ranks = payload_proofs
        .iter()
        .zip(&readers)
        .map(|(proof, reader)| NativeRankEvidence {
            rank: proof.rank,
            file_uuid: hex(&reader.file_uuid),
            tensor_count: proof.tensor_count,
            payload_bytes: proof.payload_bytes,
            payload_sha256: hex(&proof.payload_sha256),
            tensor_contract_sha256: hex(&reader
                .validated_manifest()
                .unwrap()
                .tensor_contract_sha256),
            stream_chunks: proof.stream_chunks,
            maximum_reader_scratch_bytes: proof.maximum_reader_scratch_bytes,
        })
        .collect();
    Ok(NativeRankSetProof {
        schema: "glmaxx.native-rank-set-proof.v1",
        conversion_uuid: hex(&readers[0].conversion_uuid),
        model_config_sha256: hex(&readers[0].model_config_sha256),
        tokenizer_bundle_sha256: hex(&readers[0].tokenizer_bundle_sha256),
        chat_template_sha256: hex(&readers[0].chat_template_sha256),
        weight_policy_sha256: hex(&readers[0].weight_policy_sha256),
        kernel_abi_sha256: hex(&readers[0].kernel_abi_sha256),
        operation_manifest_sha256: hex(&manifests[0].operation_manifest_sha256),
        profile: manifests[0].profile,
        profile_budget_sha256: hex(&manifests[0].profile_budget_sha256),
        ranks,
        verdict: "NATIVE_RANK_SET_PASS",
    })
}

#[cfg(feature = "cuda-ffi")]
#[derive(Serialize)]
struct NativeCheckpointLoadRankReport {
    rank: u8,
    device_identity_sha256: String,
    file_uuid: String,
    manifest_sha256: String,
    descriptor_sha256: String,
    payload_sha256: String,
    tensor_contract_sha256: String,
    tensor_count: u32,
    file_payload_bytes: u64,
    device_weight_arena_bytes: u64,
    device_metadata_arena_bytes: u64,
    arena_layout_sha256: String,
    required_hbm_bytes: u64,
    owner_allocation_generation: u64,
    verification_evidence_sha256: String,
    verified_file_payload_bytes: u64,
    uploaded_plane_bytes: u64,
    finalized_adopted_rank_set_sha256: String,
    cleanup_load_attempt_generation: u64,
    cleanup_acknowledged: bool,
}

#[cfg(feature = "cuda-ffi")]
#[derive(Serialize)]
struct NativeCheckpointLoadSmokeReport {
    schema: &'static str,
    verdict: &'static str,
    source_commit: String,
    executable_sha256: String,
    rank_set_directory: String,
    profile_budget_path: String,
    profile_budget_sha256: String,
    memory_plan_artifact: &'static str,
    memory_plan_sha256: String,
    operation_manifest_sha256: String,
    codec_capability_sha256: String,
    profile: &'static str,
    verification_mode: &'static str,
    plan_sha256: String,
    conversion_uuid: String,
    tensor_count_per_rank: u32,
    staging_slot_bytes: u32,
    staging_slots_per_rank: u16,
    load_attempt_generation: u64,
    adopted_rank_set_sha256: String,
    rank_set_receipt_sha256: String,
    phase_timeout_seconds: u64,
    startup_and_load_elapsed_nanoseconds: u128,
    shutdown_elapsed_nanoseconds: u128,
    total_elapsed_nanoseconds: u128,
    full_payload_sha256_verified: bool,
    full_arena_readback_verified: bool,
    model_kernel_launched: bool,
    ranks: Vec<NativeCheckpointLoadRankReport>,
}

#[cfg(feature = "cuda-ffi")]
fn gpu_checkpoint_load_smoke(
    rank_set_directory: &Path,
    profile_budget_path: &Path,
    evidence_directory: &Path,
    phase_timeout_seconds: u64,
) -> Result<(), Box<dyn std::error::Error>> {
    if !(1..=3_600).contains(&phase_timeout_seconds) {
        return Err("phase timeout must be in 1..=3600 seconds".into());
    }
    validate_empty_external_gpu_directory(evidence_directory, "gpu-checkpoint-load-smoke")?;
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .ok_or("cannot resolve repository root")?
        .canonicalize()?;
    if rank_set_directory.canonicalize()?.starts_with(&repository) {
        return Err("native checkpoint weights must be outside the Git repository".into());
    }
    let source_commit = option_env!("GLMAXX_SOURCE_COMMIT")
        .ok_or("binary lacks GLMAXX_SOURCE_COMMIT build provenance")?;
    validate_conversion_commit(source_commit)?;

    let rank_files = native_rank_paths(rank_set_directory)?;
    let readers: Vec<NativeRankReader> = rank_files
        .iter()
        .map(NativeRankReader::open)
        .collect::<Result<_, _>>()?;
    let readers: [NativeRankReader; 4] = readers
        .try_into()
        .map_err(|_| "native rank-set reader count was not four")?;
    NativeRankReader::validate_rank_set([&readers[0], &readers[1], &readers[2], &readers[3]])?;

    let profile_budget_bytes = read_bounded_regular(profile_budget_path, 4 * 1024 * 1024)?;
    let profile_budget_sha256 = sha256(&profile_budget_bytes);
    let profile_budget: ProfileBudgetArtifact = serde_json::from_slice(&profile_budget_bytes)?;
    let system_memory_plan = profile_budget.system_memory_plan()?;
    let memory_plan_bytes = system_memory_plan.canonical_artifact_bytes()?;
    let memory_plan_sha256 = system_memory_plan.artifact_sha256()?;

    let operation_manifest_sha256 = sha256(&operation_manifest_json()?);
    for reader in &readers {
        let manifest = reader
            .validated_manifest()
            .ok_or("native rank file lacks the production manifest")?;
        if manifest.profile != RankWeightProfile::CapacityExl3
            || manifest.profile_budget_sha256 != profile_budget_sha256
            || manifest.operation_manifest_sha256 != operation_manifest_sha256
        {
            return Err(format!(
                "rank {} does not bind the supplied completed budget and compiled operation manifest",
                reader.rank
            )
            .into());
        }
    }

    let codec_capability_sha256 = glm_cuda::native_checkpoint_codec_capability_sha256()?;
    let executable = env::current_exe()?;
    let executable_bytes = read_bounded_regular(&executable, GIB)?;
    let executable_sha256 = sha256(&executable_bytes);
    fs::write(
        evidence_directory.join("memory-plan.json"),
        &memory_plan_bytes,
    )?;

    let phase_timeout = Duration::from_secs(phase_timeout_seconds);
    let load_attempt_generation = 1;
    let owner_allocation_generations = [1, 2, 3, 4];
    let required_hbm_bytes: [u64; 4] = system_memory_plan
        .ranks
        .iter()
        .map(|rank| rank.required_bytes)
        .collect::<Vec<_>>()
        .try_into()
        .map_err(|_| "validated memory plan did not contain four ranks")?;
    let total_started = Instant::now();
    let load_started = Instant::now();
    let loaded = load_native_checkpoint(
        rank_files,
        NativeCheckpointStartupConfig {
            maximum_outstanding: 1,
            verification_mode: LoadVerificationMode::FullSha256,
            profile: LoadProfile::CapacityExl3,
            memory_plan: system_memory_plan,
            codec_capability_sha256,
            operation_manifest_sha256,
            profile_budget_sha256,
            staging_slot_bytes: READER_CHUNK_BYTES,
            staging_slots_per_rank: 2,
            software_provenance_sha256: executable_sha256,
            load_attempt_generation,
            owner_allocation_generations,
            phase_timeout,
        },
    )?;
    let startup_and_load_elapsed_nanoseconds = load_started.elapsed().as_nanos();

    let plan_sha256 = loaded.plan().plan_sha256();
    let plan_header = loaded.plan().header();
    let plan_ranks = loaded.plan().ranks();
    let load_outcome = loaded.load_outcome().clone();
    let device_identity_sha256 = loaded.device_identity_sha256();
    let shutdown_started = Instant::now();
    let shutdown = loaded.shutdown(phase_timeout)?;
    let shutdown_elapsed_nanoseconds = shutdown_started.elapsed().as_nanos();

    let ranks = (0..4)
        .map(|rank| {
            let entry = plan_ranks[rank];
            let prepared = load_outcome.prepared_receipts[rank];
            let finalized = load_outcome.finalize_acknowledgements[rank];
            let cleanup = shutdown.cleanup_acknowledgements[rank];
            NativeCheckpointLoadRankReport {
                rank: u8::try_from(rank).expect("four ranks fit u8"),
                device_identity_sha256: hex(&device_identity_sha256[rank]),
                file_uuid: hex(&entry.file_uuid),
                manifest_sha256: hex(&entry.manifest_sha256),
                descriptor_sha256: hex(&entry.descriptor_sha256),
                payload_sha256: hex(&entry.payload_sha256),
                tensor_contract_sha256: hex(&entry.tensor_contract_sha256),
                tensor_count: entry.tensor_count,
                file_payload_bytes: entry.file_payload_bytes,
                device_weight_arena_bytes: entry.device_weight_arena_bytes,
                device_metadata_arena_bytes: entry.device_metadata_arena_bytes,
                arena_layout_sha256: hex(&entry.arena_layout_sha256),
                required_hbm_bytes: required_hbm_bytes[rank],
                owner_allocation_generation: finalized.owner_allocation_generation(),
                verification_evidence_sha256: hex(&prepared.verification_evidence_sha256),
                verified_file_payload_bytes: prepared.verified_file_payload_bytes,
                uploaded_plane_bytes: prepared.uploaded_plane_metadata_bytes,
                finalized_adopted_rank_set_sha256: hex(&finalized.adopted_rank_set_sha256()),
                cleanup_load_attempt_generation: cleanup.load_attempt_generation(),
                cleanup_acknowledged: cleanup.rank() == entry.rank
                    && cleanup.plan_sha256() == plan_sha256
                    && cleanup.owner_allocation_generation()
                        == finalized.owner_allocation_generation(),
            }
        })
        .collect::<Vec<_>>();
    if ranks.iter().any(|rank| !rank.cleanup_acknowledged) {
        return Err("rank-exact cleanup acknowledgement validation failed".into());
    }

    let report = NativeCheckpointLoadSmokeReport {
        schema: "glmaxx.sm120-tp4-native-checkpoint-load-smoke.v1",
        verdict: "SM120_TP4_CHECKPOINT_LOAD_PASS",
        source_commit: source_commit.to_owned(),
        executable_sha256: hex(&executable_sha256),
        rank_set_directory: rank_set_directory.canonicalize()?.display().to_string(),
        profile_budget_path: profile_budget_path.canonicalize()?.display().to_string(),
        profile_budget_sha256: hex(&profile_budget_sha256),
        memory_plan_artifact: "memory-plan.json",
        memory_plan_sha256: hex(&memory_plan_sha256),
        operation_manifest_sha256: hex(&operation_manifest_sha256),
        codec_capability_sha256: hex(&codec_capability_sha256),
        profile: "capacity-exl3",
        verification_mode: "full-sha256",
        plan_sha256: hex(&plan_sha256),
        conversion_uuid: hex(&plan_header.conversion_uuid),
        tensor_count_per_rank: plan_header.tensor_count,
        staging_slot_bytes: plan_header.staging_slot_bytes,
        staging_slots_per_rank: plan_header.staging_slots_per_rank,
        load_attempt_generation,
        adopted_rank_set_sha256: hex(&load_outcome.adopted_receipt.adopted_rank_set_sha256()),
        rank_set_receipt_sha256: hex(&load_outcome.adopted_receipt.rank_set_receipt_sha256()),
        phase_timeout_seconds,
        startup_and_load_elapsed_nanoseconds,
        shutdown_elapsed_nanoseconds,
        total_elapsed_nanoseconds: total_started.elapsed().as_nanos(),
        full_payload_sha256_verified: true,
        full_arena_readback_verified: true,
        model_kernel_launched: false,
        ranks,
    };
    let mut report_bytes = serde_json::to_vec_pretty(&report)?;
    report_bytes.push(b'\n');
    fs::write(evidence_directory.join("summary.json"), report_bytes)?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

fn convert_pinned_exl3(
    index: &Path,
    output: &Path,
    conversion_commit: &str,
    profile_budget_path: &Path,
    review_path: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    validate_conversion_commit(conversion_commit)?;
    let embedded_commit = option_env!("GLMAXX_SOURCE_COMMIT")
        .ok_or("converter binary lacks GLMAXX_SOURCE_COMMIT build provenance")?;
    if embedded_commit != conversion_commit {
        return Err(format!(
            "converter commit mismatch: embedded {embedded_commit}, requested {conversion_commit}"
        )
        .into());
    }

    let profile_budget = read_bounded_regular(profile_budget_path, 4 * 1024 * 1024)?;
    let profile_budget_contract: ProfileBudgetArtifact = serde_json::from_slice(&profile_budget)?;
    profile_budget_contract.validate()?;
    if profile_budget_contract.measurement_status != "complete"
        || !profile_budget_contract.conversion_allowed
    {
        return Err(
            "profile budget is not a complete, conversion-approved capacity-exl3 v0 contract"
                .into(),
        );
    }
    let profile_budget_sha256 = sha256(&profile_budget);
    let operation_manifest = operation_manifest_json()?;
    let operation_manifest_sha256 = sha256(&operation_manifest);
    let format_spec_sha256 = sha256(FORMAT_SPEC_BYTES);
    let engine_spec_sha256 = sha256(ENGINE_SPEC_BYTES);
    let review = read_bounded_regular(review_path, 4 * 1024 * 1024)?;
    require_review_line(&review, REVIEW_ACCEPTANCE_TOKEN)?;
    require_review_line(
        &review,
        &format!("profile-budget-v0-sha256={}", hex(&profile_budget_sha256)),
    )?;
    require_review_line(
        &review,
        &format!(
            "operation-manifest-sha256={}",
            hex(&operation_manifest_sha256)
        ),
    )?;
    require_review_line(
        &review,
        &format!("format-v0-sha256={}", hex(&format_spec_sha256)),
    )?;
    require_review_line(
        &review,
        &format!("engine-v0-sha256={}", hex(&engine_spec_sha256)),
    )?;
    let review_sha256 = sha256(&review);

    eprintln!("opening and structurally validating pinned checkpoint");
    let checkpoint = ShardedSafetensors::open(index)?;
    validate_pinned_exl3_checkpoint(&checkpoint, EXL3_MODEL_REVISION)?;
    eprintln!("recomputing all pinned source-file SHA-256 digests");
    let source =
        verify_pinned_source_files(&checkpoint, |completed, total, verified_bytes, name| {
            eprintln!("source-verify {completed}/{total} bytes={verified_bytes} file={name}");
        })?;

    let plans: [PinnedRankPlan; 4] = (0_u8..4)
        .map(pinned_exl3_rank_plan)
        .collect::<Result<Vec<_>, _>>()?
        .try_into()
        .map_err(|_| "rank-plan count mismatch")?;
    let tokenizer_bundle_sha256 = tokenizer_bundle_sha256(&source)?;
    let model_config_sha256 = required_source_sha256(&source, "config.json")?;
    let chat_template_sha256 = required_source_sha256(&source, "chat_template.jinja")?;
    let weight_policy_sha256 = pinned_exl3_weight_policy_sha256();
    let kernel_abi_sha256 = sha256(KERNEL_ABI.as_bytes());
    let manifests: [(Vec<u8>, usize); 4] = plans
        .iter()
        .map(|plan| {
            rank_conversion_manifest(
                plan,
                &source,
                conversion_commit,
                profile_budget_sha256,
                review_sha256,
                operation_manifest_sha256,
                format_spec_sha256,
                engine_spec_sha256,
                weight_policy_sha256,
            )
        })
        .collect::<Result<Vec<_>, _>>()?
        .try_into()
        .map_err(|_| "rank-manifest count mismatch")?;
    let configs: [StreamingRankConfig; 4] = plans
        .iter()
        .zip(manifests)
        .map(
            |(plan, (manifest, manifest_payload_sha256_slot))| StreamingRankConfig {
                rank: u32::from(plan.rank()),
                manifest,
                manifest_payload_sha256_slot: Some(manifest_payload_sha256_slot),
                model_config_sha256,
                tokenizer_bundle_sha256,
                chat_template_sha256,
                weight_policy_sha256,
                kernel_abi_sha256,
                tensors: plan.tensor_specs(),
            },
        )
        .collect::<Vec<_>>()
        .try_into()
        .map_err(|_| "rank-config count mismatch")?;

    let summaries = if output.exists() {
        eprintln!("published rank set exists; performing full verification");
        StreamingRankSet::verify_published(output, configs.clone())?
    } else {
        let rank_set = StreamingRankSet::create_or_resume(output, configs.clone())?;
        eprintln!(
            "rank staging directory: {}",
            rank_set.staging_path().display()
        );
        for plan in &plans {
            let rank = plan.rank();
            let Some(mut writer) = rank_set.open_rank_writer(rank)? else {
                eprintln!("rank {rank} was already finalized; publication will verify it");
                continue;
            };
            eprintln!(
                "rank {rank} resume: {}/{} tensors durable",
                writer.completed_tensors(),
                plan.tensor_count()
            );
            let mut last_reported = writer.completed_tensors() / 1_024;
            plan.write_incomplete_with_progress(&checkpoint, &mut writer, |progress| {
                let report_bucket = progress.completed_tensors / 1_024;
                if report_bucket != last_reported
                    || progress.completed_tensors == progress.total_tensors
                {
                    eprintln!(
                        "rank {rank} tensors={}/{} payload_bytes={}/{}",
                        progress.completed_tensors,
                        progress.total_tensors,
                        progress.completed_payload_bytes,
                        progress.total_payload_bytes
                    );
                    last_reported = report_bucket;
                }
            })?;
            if writer.completed_tensors() != plan.tensor_count() {
                return Err(format!("rank {rank} conversion stopped incomplete").into());
            }
        }
        eprintln!("all four rank bodies are durable; auditing and publishing atomically");
        rank_set.publish()?
    };
    print_conversion_result(
        output,
        &summaries,
        &source,
        profile_budget_sha256,
        review_sha256,
    )?;
    Ok(())
}

fn validate_conversion_commit(commit: &str) -> Result<(), Box<dyn std::error::Error>> {
    if commit.len() != 40
        || !commit
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err("conversion commit must be an exact lowercase 40-hex Git object ID".into());
    }
    Ok(())
}

fn read_bounded_regular(
    path: &Path,
    maximum_bytes: u64,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let before = path.symlink_metadata()?;
    if !before.file_type().is_file()
        || before.file_type().is_symlink()
        || before.nlink() != 1
        || before.len() == 0
        || before.len() > maximum_bytes
    {
        return Err(format!("unsafe or invalid provenance file: {}", path.display()).into());
    }
    let mut file = File::open(path)?;
    let opened = file.metadata()?;
    if before.dev() != opened.dev()
        || before.ino() != opened.ino()
        || before.len() != opened.len()
        || before.mtime() != opened.mtime()
        || before.mtime_nsec() != opened.mtime_nsec()
    {
        return Err(format!("provenance file changed while opening: {}", path.display()).into());
    }
    let mut bytes = Vec::with_capacity(usize::try_from(before.len())?);
    file.read_to_end(&mut bytes)?;
    let after = path.symlink_metadata()?;
    let after_opened = file.metadata()?;
    if before.dev() != after.dev()
        || before.ino() != after.ino()
        || before.len() != after.len()
        || before.mtime() != after.mtime()
        || before.mtime_nsec() != after.mtime_nsec()
        || opened.dev() != after_opened.dev()
        || opened.ino() != after_opened.ino()
        || opened.len() != after_opened.len()
        || opened.mtime() != after_opened.mtime()
        || opened.mtime_nsec() != after_opened.mtime_nsec()
        || bytes.len() as u64 != before.len()
    {
        return Err(format!("provenance file changed while reading: {}", path.display()).into());
    }
    Ok(bytes)
}

fn require_review_line(review: &[u8], required: &str) -> Result<(), Box<dyn std::error::Error>> {
    let text = std::str::from_utf8(review)?;
    if !text.lines().any(|line| line == required) {
        return Err(format!("independent review is missing exact line: {required}").into());
    }
    Ok(())
}

fn required_source_sha256(
    source: &PinnedSourceVerification,
    name: &str,
) -> Result<[u8; 32], Box<dyn std::error::Error>> {
    source
        .file_sha256(name)
        .ok_or_else(|| format!("pinned source manifest lacks {name}").into())
}

fn tokenizer_bundle_sha256(
    source: &PinnedSourceVerification,
) -> Result<[u8; 32], Box<dyn std::error::Error>> {
    let mut hasher = Sha256::new();
    hasher.update(b"glmaxx-tokenizer-bundle-v0\0");
    for name in [
        "tokenizer.json",
        "tokenizer_config.json",
        "generation_config.json",
    ] {
        hasher.update(name.as_bytes());
        hasher.update([0]);
        hasher.update(required_source_sha256(source, name)?);
    }
    Ok(hasher.finalize().into())
}

#[allow(clippy::too_many_arguments)]
fn rank_conversion_manifest(
    plan: &PinnedRankPlan,
    source: &PinnedSourceVerification,
    conversion_commit: &str,
    profile_budget_sha256: [u8; 32],
    review_sha256: [u8; 32],
    operation_manifest_sha256: [u8; 32],
    format_spec_sha256: [u8; 32],
    engine_spec_sha256: [u8; 32],
    weight_policy_sha256: [u8; 32],
) -> Result<(Vec<u8>, usize), Box<dyn std::error::Error>> {
    let tensors = plan.manifest_tensors()?;
    let tensor_contract = canonical_json(&tensors)?;
    let tensor_contract_sha256 = sha256(&tensor_contract);
    let source_files: BTreeMap<_, _> = source
        .files()
        .iter()
        .map(|(name, digest)| (name.clone(), hex(digest)))
        .collect();
    let manifest = serde_json::json!({
        "calibration": {
            "manifest_file": "calibration_manifest.json",
            "manifest_sha256": hex(&required_source_sha256(source, "calibration_manifest.json")?),
            "source_revision": EXL3_MODEL_REVISION
        },
        "codec": {
            "exl3_source_revision": EXL3_SOURCE_REVISION,
            "format": "g5n-v0.2.2",
            "profile": "capacity-exl3-v0"
        },
        "conversion": {
            "commit": conversion_commit,
            "repository": CONVERSION_REPOSITORY
        },
        "integrity": {
            "output_hash_location": "rank-header.payload_sha256-and-descriptor-plane-sha256",
            "output_payload_sha256": "0000000000000000000000000000000000000000000000000000000000000000",
            "source_file_sha256": source_files,
            "source_verification": "FULL_SHA256",
            "source_verified_file_bytes": source.verified_file_bytes
        },
        "license_provenance": {
            "license_sha256": hex(&required_source_sha256(source, "LICENSE")?),
            "readme_sha256": hex(&required_source_sha256(source, "README.md")?),
            "source_repository": PINNED_EXL3_REPOSITORY
        },
        "model": {
            "config_sha256": hex(&required_source_sha256(source, "config.json")?),
            "operation_manifest_sha256": hex(&operation_manifest_sha256),
            "repository": PINNED_EXL3_REPOSITORY,
            "revision": EXL3_MODEL_REVISION,
            "source_index_sha256": hex(&required_source_sha256(source, "model.safetensors.index.json")?),
            "source_manifest_sha256": hex(&PINNED_SOURCE_MANIFEST_SHA256)
        },
        "profile": {
            "name": "capacity-exl3",
            "profile_budget_sha256": hex(&profile_budget_sha256),
            "weight_policy_sha256": hex(&weight_policy_sha256)
        },
        "rank": plan.rank(),
        "review": {
            "acceptance_token": REVIEW_ACCEPTANCE_TOKEN,
            "artifact_sha256": hex(&review_sha256),
            "engine_spec_sha256": hex(&engine_spec_sha256),
            "format_spec_sha256": hex(&format_spec_sha256)
        },
        "schema": "glmaxx.rank-manifest.v0.2.2",
        "tensor_contract_sha256": hex(&tensor_contract_sha256),
        "tensor_count": plan.tensor_count(),
        "tensor_source_payload_bytes": plan.source_payload_bytes(),
        "tensors": tensors,
        "tokenizer": {
            "chat_template_sha256": hex(&required_source_sha256(source, "chat_template.jinja")?),
            "generation_config_sha256": hex(&required_source_sha256(source, "generation_config.json")?),
            "tokenizer_config_sha256": hex(&required_source_sha256(source, "tokenizer_config.json")?),
            "tokenizer_sha256": hex(&required_source_sha256(source, "tokenizer.json")?)
        },
        "toolchain": {
            "container_digest": CN4_CONVERTER_CONTAINER_DIGEST,
            "cuda": "13.3",
            "cutlass_commit": CUTLASS_COMMIT,
            "kernel_abi": KERNEL_ABI,
            "rust": "1.92.0"
        },
        "tp_degree": 4
    });
    let bytes = canonical_json(&manifest)?;
    let prefix = b"\"output_payload_sha256\":\"";
    let matches = bytes
        .windows(prefix.len())
        .enumerate()
        .filter_map(|(offset, window)| (window == prefix).then_some(offset + prefix.len()))
        .collect::<Vec<_>>();
    if matches.len() != 1
        || bytes.get(matches[0]..matches[0] + 64)
            != Some(&b"0000000000000000000000000000000000000000000000000000000000000000"[..])
    {
        return Err("canonical rank manifest payload-hash slot is not unique".into());
    }
    Ok((bytes, matches[0]))
}

fn canonical_json(value: &impl Serialize) -> Result<Vec<u8>, serde_json::Error> {
    let value = serde_json::to_value(value)?;
    serde_json::to_vec(&value)
}

#[derive(Serialize)]
struct ConversionRankResult {
    descriptor_sha256: String,
    manifest_sha256: String,
    metadata_sha256: String,
    payload_sha256: String,
    rank: u32,
    string_sha256: String,
    tensor_count: usize,
    total_file_bytes: u64,
}

#[derive(Serialize)]
struct ConversionResult {
    conversion_uuid: String,
    output: String,
    profile_budget_sha256: String,
    ranks: Vec<ConversionRankResult>,
    review_sha256: String,
    schema: &'static str,
    source_manifest_sha256: String,
    source_verified_file_bytes: u64,
    verdict: &'static str,
}

fn print_conversion_result(
    output: &Path,
    summaries: &[StreamingRankSummary; 4],
    source: &PinnedSourceVerification,
    profile_budget_sha256: [u8; 32],
    review_sha256: [u8; 32],
) -> Result<(), Box<dyn std::error::Error>> {
    let conversion_uuid = StreamingRankSummary::derive_conversion_uuid(summaries)?;
    let ranks = summaries
        .iter()
        .map(|summary| ConversionRankResult {
            descriptor_sha256: hex(&summary.descriptor_sha256),
            manifest_sha256: hex(&summary.manifest_sha256),
            metadata_sha256: hex(&summary.metadata_sha256),
            payload_sha256: hex(&summary.payload_sha256),
            rank: summary.rank,
            string_sha256: hex(&summary.string_sha256),
            tensor_count: summary.tensor_count,
            total_file_bytes: summary.total_file_bytes,
        })
        .collect();
    let result = ConversionResult {
        conversion_uuid: hex(&conversion_uuid),
        output: output.display().to_string(),
        profile_budget_sha256: hex(&profile_budget_sha256),
        ranks,
        review_sha256: hex(&review_sha256),
        schema: "glmaxx.pinned-conversion-result.v1",
        source_manifest_sha256: hex(&source.manifest_sha256),
        source_verified_file_bytes: source.verified_file_bytes,
        verdict: "PINNED_EXL3_FOUR_RANK_CONVERSION_PASS",
    };
    println!("{}", String::from_utf8(canonical_json(&result)?)?);
    Ok(())
}

fn parse_argument<T: FromStr>(
    arguments: &[String],
    index: usize,
    name: &str,
) -> Result<T, Box<dyn std::error::Error>> {
    let value = arguments
        .get(index)
        .ok_or_else(|| format!("missing {name} argument"))?;
    value
        .parse()
        .map_err(|_| format!("invalid {name} argument: {value}").into())
}

#[derive(Serialize)]
struct SafetensorsInventory {
    schema: &'static str,
    kind: &'static str,
    source: String,
    structure_sha256: String,
    tensor_count: usize,
    shard_count: usize,
    tensor_payload_bytes: u64,
    dtype_counts: BTreeMap<&'static str, usize>,
    dtype_bytes: BTreeMap<&'static str, u64>,
}

fn safetensors_inventory(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let is_directory = path.is_dir();
    let is_index = path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.ends_with(".safetensors.index.json"));
    let mut dtype_counts = BTreeMap::new();
    let mut dtype_bytes = BTreeMap::new();
    let (kind, identity, tensor_count, shard_count, tensor_payload_bytes) =
        if is_directory || is_index {
            let files = ShardedSafetensors::open_auto(path)?;
            let mut total = 0_u64;
            for name in files.tensor_names() {
                let descriptor = files
                    .tensor(name)
                    .ok_or("validated sharded tensor disappeared")?;
                *dtype_counts.entry(descriptor.dtype.name()).or_insert(0) += 1;
                *dtype_bytes.entry(descriptor.dtype.name()).or_insert(0) += descriptor.bytes;
                total = total
                    .checked_add(descriptor.bytes)
                    .ok_or("safetensors byte total overflow")?;
            }
            (
                if is_directory {
                    "shard-directory"
                } else {
                    "sharded-index"
                },
                files.structure_sha256(),
                files.tensor_names().len(),
                files.shards().len(),
                total,
            )
        } else {
            let file = SafeTensorFile::open(path)?;
            let mut total = 0_u64;
            for descriptor in file.tensors().values() {
                *dtype_counts.entry(descriptor.dtype.name()).or_insert(0) += 1;
                *dtype_bytes.entry(descriptor.dtype.name()).or_insert(0) += descriptor.bytes;
                total = total
                    .checked_add(descriptor.bytes)
                    .ok_or("safetensors byte total overflow")?;
            }
            (
                "single-file",
                file.header_sha256(),
                file.tensors().len(),
                1,
                total,
            )
        };
    let report = SafetensorsInventory {
        schema: "glmaxx.safetensors-inventory.v1",
        kind,
        source: path.display().to_string(),
        structure_sha256: hex(&identity),
        tensor_count,
        shard_count,
        tensor_payload_bytes,
        dtype_counts,
        dtype_bytes,
    };
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

#[derive(Serialize)]
struct Exl3SafetensorsComponent {
    name: String,
    dtype: &'static str,
    shape: Vec<u64>,
    bytes: u64,
    sha256: String,
}

#[derive(Serialize)]
struct Exl3SafetensorsProof {
    schema: &'static str,
    model_revision: &'static str,
    source_revision: &'static str,
    source_version: &'static str,
    source: String,
    source_kind: &'static str,
    structure_sha256: String,
    tensor_stem: String,
    projection: &'static str,
    layer: u16,
    expert: u16,
    rank: u8,
    logical_shape_k_n: [u32; 2],
    components: Vec<Exl3SafetensorsComponent>,
    source_payload_bytes: usize,
    source_payload_sha256: String,
    native_metadata_sha256: String,
    native_primary_sha256: String,
    native_aux_sha256: String,
    reconstructed_f16_bytes: usize,
    reconstructed_sha256: String,
}

fn exl3_safetensors_proof(
    path: &Path,
    layer: u16,
    expert: u16,
    rank: u8,
    projection_name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let (projection, canonical_projection, logical_k, logical_n) = match projection_name {
        "gate" => (Exl3Projection::Gate, "gate", 6_144, 512),
        "up" => (Exl3Projection::Up, "up", 6_144, 512),
        "down" => (Exl3Projection::Down, "down", 512, 6_144),
        _ => return Err("projection must be gate, up, or down".into()),
    };
    let metadata = Exl3Metadata::new(projection, layer, expert, rank, 3, logical_k, logical_n)?;
    let stem =
        format!("model.layers.{layer}.mlp.experts.{expert}.{canonical_projection}_proj.rank{rank}");
    let names = [
        format!("{stem}.mcg"),
        format!("{stem}.suh"),
        format!("{stem}.svh"),
        format!("{stem}.trellis"),
    ];
    let is_directory = path.is_dir();
    let is_index = path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.ends_with(".safetensors.index.json"));

    let (source_kind, structure_hash, tensor, components, source_bytes) =
        if is_directory || is_index {
            let source = ShardedSafetensors::open_auto(path)?;
            let mut components = Vec::with_capacity(names.len());
            let mut source_bytes = Vec::new();
            for name in &names {
                let descriptor = source
                    .tensor(name)
                    .ok_or_else(|| format!("missing validated component {name}"))?;
                let bytes = source.read_tensor(name)?;
                components.push(Exl3SafetensorsComponent {
                    name: name.clone(),
                    dtype: descriptor.dtype.name(),
                    shape: descriptor.shape.clone(),
                    bytes: descriptor.bytes,
                    sha256: hex(&sha256(&bytes)),
                });
                source_bytes.extend_from_slice(&bytes);
            }
            (
                if is_directory {
                    "shard-directory"
                } else {
                    "sharded-index"
                },
                source.structure_sha256(),
                glm_format::load_exl3_projection_sharded(&source, &stem, metadata)?,
                components,
                source_bytes,
            )
        } else {
            let source = SafeTensorFile::open(path)?;
            let mut components = Vec::with_capacity(names.len());
            let mut source_bytes = Vec::new();
            for name in &names {
                let descriptor = source
                    .tensor(name)
                    .ok_or_else(|| format!("missing validated component {name}"))?;
                let bytes = source.read_tensor(name)?;
                components.push(Exl3SafetensorsComponent {
                    name: name.clone(),
                    dtype: descriptor.dtype.name(),
                    shape: descriptor.shape.clone(),
                    bytes: descriptor.bytes,
                    sha256: hex(&sha256(&bytes)),
                });
                source_bytes.extend_from_slice(&bytes);
            }
            (
                "single-file",
                source.header_sha256(),
                glm_format::load_exl3_projection(&source, &stem, metadata)?,
                components,
                source_bytes,
            )
        };

    let primary = tensor.primary_plane()?;
    let aux = tensor.aux_plane()?;
    let native_metadata = tensor.metadata.encode();
    let reconstructed = tensor.reconstruct_native_f16()?;
    let mut reconstructed_bytes = Vec::with_capacity(reconstructed.len() * 2);
    for word in reconstructed {
        reconstructed_bytes.extend_from_slice(&word.to_le_bytes());
    }
    let report = Exl3SafetensorsProof {
        schema: "glmaxx.exl3-safetensors-proof.v1",
        model_revision: glm_format::EXL3_MODEL_REVISION,
        source_revision: glm_format::EXL3_SOURCE_REVISION,
        source_version: glm_format::EXL3_SOURCE_VERSION,
        source: path.display().to_string(),
        source_kind,
        structure_sha256: hex(&structure_hash),
        tensor_stem: stem,
        projection: canonical_projection,
        layer,
        expert,
        rank,
        logical_shape_k_n: [logical_k, logical_n],
        components,
        source_payload_bytes: source_bytes.len(),
        source_payload_sha256: hex(&sha256(&source_bytes)),
        native_metadata_sha256: hex(&sha256(&native_metadata)),
        native_primary_sha256: hex(&sha256(&primary)),
        native_aux_sha256: hex(&sha256(&aux)),
        reconstructed_f16_bytes: reconstructed_bytes.len(),
        reconstructed_sha256: hex(&sha256(&reconstructed_bytes)),
    };
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

#[derive(Serialize)]
struct ServingProof {
    schema: &'static str,
    backend: &'static str,
    tp_ranks: u8,
    admitted_requests: u32,
    completed_steps: u64,
    prefix_restored_tokens: u32,
    prefill_progress_events: u32,
    token_events: u32,
    speculative_token_events: u32,
    finished_requests: u32,
    failed_requests: u32,
    final_states: Vec<&'static str>,
}

fn serving_proof(evidence_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    fs::create_dir_all(evidence_dir)?;
    let store_root = evidence_dir.join("kv-store");
    let namespace = PrefixNamespace::new(NamespaceInputs {
        model_revision_sha256: [1; 32],
        tokenizer_sha256: [2; 32],
        chat_template_sha256: [3; 32],
        weight_policy_hash: [4; 32],
        target_kv_abi_sha256: [5; 32],
        draft_kv_abi_sha256: [6; 32],
        rope_parameters_sha256: [7; 32],
    })?;
    let cached_tokens: Vec<u32> = (0..64).collect();
    let mut first_tokens: Vec<u32> = (0..128).collect();
    let second_tokens: Vec<u32> = (1_000..1_064).collect();
    let index = PrefixIndex::new(namespace);
    let page_key = index.derive_keys(&cached_tokens)[0];
    let mut store = FileTierStore::open(&store_root)?;
    let record = store.publish(DurablePageRequest {
        namespace: namespace.0,
        page_key: page_key.0,
        generation: 1,
        mtp: false,
        pieces: [TierPiece::TargetKv, TierPiece::TargetIndexer]
            .into_iter()
            .map(|piece| PagePieceBytes {
                piece,
                bytes: vec![piece as u8; piece.expected_bytes() as usize],
            })
            .collect(),
    })?;
    let page_bytes = record.pieces.iter().try_fold(0_u64, |total, piece| {
        total
            .checked_add(piece.byte_length)
            .ok_or("prefix page byte overflow")
    })?;
    drop(store);

    let mut prefix = PrefixRestoreCoordinator::new(
        index,
        &store_root,
        ResidencyConfig {
            hbm_bytes: page_bytes * 2,
            dram_bytes: page_bytes * 2,
        },
        2,
    )?;
    prefix.register_prefix(&cached_tokens, vec![record])?;

    let profile = GraphProfile::new(vec![
        proof_graph(1, StepMode::Prefill, 4, 64, 0),
        proof_graph(2, StepMode::Decode, 4, 4, 0),
        proof_graph(3, StepMode::Verify, 4, 28, 6),
    ])?;
    let routes = RouteCatalog {
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
    };
    let mut serving = ServingCoordinator::new(
        ServingConfig {
            epoch: 1,
            event_capacity: 1024,
            maximum_retained_prompt_bytes: 64 * 1024 * 1024,
            page_table: PageTableConfig {
                target_pages_per_rank: 256,
                draft_pages_per_rank: 256,
            },
        },
        SchedulerConfig {
            maximum_batch_sequences: 4,
            maximum_prefill_tokens: 64,
            maximum_decode_burst: 2,
        },
        profile,
        vec![
            TenantConfig {
                tenant: 1,
                weight: 1,
                maximum_active_requests: 4,
            },
            TenantConfig {
                tenant: 2,
                weight: 2,
                maximum_active_requests: 4,
            },
        ],
        routes,
        Tp4WorkerPool::spawn_cpu(2, None)?,
    )?;
    serving.attach_prefix_cache(prefix)?;
    serving.admit_tokens(
        RequestSpec {
            id: 101,
            tenant: 1,
            prompt_tokens: 128,
            maximum_new_tokens: 4,
            mtp_depth: 0,
            sampling: SamplingCollective::Greedy,
        },
        &first_tokens,
    )?;
    serving.admit_tokens(
        RequestSpec {
            id: 202,
            tenant: 2,
            prompt_tokens: 64,
            maximum_new_tokens: 7,
            mtp_depth: 6,
            sampling: SamplingCollective::Greedy,
        },
        &second_tokens,
    )?;
    // Make it explicit that inputs are no longer needed after admission.
    first_tokens.clear();

    let mut events = serving.drain_events();
    let mut completed_steps = 0_u64;
    while completed_steps < 32 && serving.tick()? {
        completed_steps += 1;
        events.extend(serving.drain_events());
    }
    events.extend(serving.drain_events());
    let final_states = [101, 202]
        .into_iter()
        .map(
            |id| match serving.request_progress(id).map(|progress| progress.state) {
                Some(RequestState::Finished) => Ok("finished"),
                Some(RequestState::Cancelled) => Ok("cancelled"),
                Some(RequestState::Failed) => Ok("failed"),
                _ => Err("serving proof did not reach a terminal request state"),
            },
        )
        .collect::<Result<Vec<_>, _>>()?;
    let report = ServingProof {
        schema: "glmaxx.cpu-serving-proof.v1",
        backend: "four-rank-cpu-contract",
        tp_ranks: 4,
        admitted_requests: count_events(&events, |event| {
            matches!(event, RequestEvent::Admitted { .. })
        }),
        completed_steps,
        prefix_restored_tokens: events
            .iter()
            .find_map(|event| match event {
                RequestEvent::Admitted {
                    request_id: 101,
                    cached_prompt_tokens,
                } => Some(*cached_prompt_tokens),
                _ => None,
            })
            .ok_or("missing cached-prefix admission event")?,
        prefill_progress_events: count_events(&events, |event| {
            matches!(event, RequestEvent::PrefillProgress { .. })
        }),
        token_events: count_events(&events, |event| matches!(event, RequestEvent::Token { .. })),
        speculative_token_events: count_events(&events, |event| {
            matches!(
                event,
                RequestEvent::Token {
                    speculative: true,
                    ..
                }
            )
        }),
        finished_requests: count_events(&events, |event| {
            matches!(event, RequestEvent::Finished { .. })
        }),
        failed_requests: count_events(&events, |event| {
            matches!(event, RequestEvent::Failed { .. })
        }),
        final_states,
    };
    let report_path = evidence_dir.join("serving-proof.json");
    let mut json = serde_json::to_vec_pretty(&report)?;
    json.push(b'\n');
    fs::write(&report_path, &json)?;
    println!("wrote {} bytes to {}", json.len(), report_path.display());
    Ok(())
}

#[derive(Serialize)]
struct TokenizerCaseProof {
    name: &'static str,
    utf8_sha256: String,
    token_ids: Vec<u32>,
    round_trip_exact: bool,
}

#[derive(Serialize)]
struct TokenizerProof {
    schema: &'static str,
    base_model_revision: &'static str,
    checkpoint_revision: &'static str,
    tokenizer_sha256: String,
    tokenizer_config_sha256: String,
    generation_config_sha256: String,
    chat_template_sha256: String,
    token_output_table_sha256: String,
    tokenizer_backend: &'static str,
    model_vocabulary: u32,
    mapped_vocabulary: u32,
    masked_padding_ids: [u32; 2],
    cases: Vec<TokenizerCaseProof>,
    incremental_utf8_exact: bool,
    cross_token_stop_exact: bool,
    verdict: &'static str,
}

fn tokenizer_proof(root: &Path) -> Result<TokenizerProof, Box<dyn std::error::Error>> {
    let tokenizer = Arc::new(PinnedTokenizer::open(root)?);
    let output_table_sha256 = tokenizer.output_table_sha256();
    if output_table_sha256 != TOKEN_OUTPUT_TABLE_SHA256 {
        return Err("pinned tokenizer output table changed after load".into());
    }
    let user_prompt = render_chat(
        &[tokenizer_message(ChatRole::User, "Hello")],
        None,
        ChatTemplateOptions::default(),
    )?;
    let history_prompt = render_chat(
        &[
            tokenizer_message(ChatRole::User, "Q"),
            tokenizer_message(ChatRole::Assistant, "<think>r</think>A"),
            tokenizer_message(ChatRole::User, "Next"),
        ],
        None,
        ChatTemplateOptions {
            reasoning_effort: ReasoningEffort::High,
            ..ChatTemplateOptions::default()
        },
    )?;
    let cases = [
        ("ascii", "Hello, world!".to_owned(), vec![9703, 11, 1879, 0]),
        (
            "unicode",
            "北京 café 👋🏽\nline 2".to_owned(),
            vec![99_334, 51_609, 61_370, 233, 151_821, 198, 1056, 220, 17],
        ),
        (
            "code",
            "fn main() { println!(\"hi\"); }".to_owned(),
            vec![8821, 1887, 368, 314, 13_742, 17_203, 6023, 5038, 335],
        ),
        (
            "user_prompt",
            user_prompt,
            vec![
                154_822, 154_824, 154_826, 25_062, 287, 29_905, 371, 25, 7487, 154_827, 9703,
                154_828, 154_841,
            ],
        ),
        (
            "history_prompt",
            history_prompt,
            vec![
                154_822, 154_824, 154_826, 25_062, 287, 29_905, 371, 25, 5124, 154_827, 48,
                154_828, 154_841, 154_842, 32, 154_827, 5847, 154_828, 154_841,
            ],
        ),
    ];
    let mut case_proofs = Vec::with_capacity(cases.len());
    for (name, text, expected) in cases {
        let token_ids = tokenizer.encode(&text)?;
        if token_ids != expected {
            return Err(format!(
                "pinned tokenizer case {name} changed: expected {expected:?}, observed {token_ids:?}"
            )
            .into());
        }
        let decoded = tokenizer.decode_reference(&token_ids, false)?;
        case_proofs.push(TokenizerCaseProof {
            name,
            utf8_sha256: hex(&sha256(text.as_bytes())),
            token_ids,
            round_trip_exact: decoded == text,
        });
    }
    if case_proofs.iter().any(|proof| !proof.round_trip_exact) {
        return Err("pinned tokenizer reference round trip changed".into());
    }

    let unicode = "北京 café 👋🏽\nline 2";
    let unicode_ids = tokenizer.encode(unicode)?;
    let mut stream = tokenizer.stream(Vec::new())?;
    let mut streamed = String::new();
    for token in unicode_ids {
        streamed.push_str(&stream.push(token)?.text);
    }
    streamed.push_str(&stream.finish()?.text);
    let incremental_utf8_exact = streamed == unicode;

    let stop_ids = tokenizer.encode("alpha STOP hidden")?;
    let mut stream = tokenizer.stream(vec!["STOP".to_owned()])?;
    let mut stopped = String::new();
    let mut saw_stop = false;
    for token in stop_ids {
        let delta = stream.push(token)?;
        stopped.push_str(&delta.text);
        if delta.finish.is_some() {
            saw_stop = true;
            break;
        }
    }
    let cross_token_stop_exact = saw_stop && stopped == "alpha ";
    if !incremental_utf8_exact || !cross_token_stop_exact {
        return Err("incremental tokenizer contract changed".into());
    }
    Ok(TokenizerProof {
        schema: "glmaxx.tokenizer-proof.v1",
        base_model_revision: "b4734de4facf877f85769a911abafc5283eab3d9",
        checkpoint_revision: EXL3_MODEL_REVISION,
        tokenizer_sha256: hex(&TOKENIZER_SHA256),
        tokenizer_config_sha256: hex(&TOKENIZER_CONFIG_SHA256),
        generation_config_sha256: hex(&GENERATION_CONFIG_SHA256),
        chat_template_sha256: hex(&CHAT_TEMPLATE_SHA256),
        token_output_table_sha256: hex(&output_table_sha256),
        tokenizer_backend: "tokenizers-rs-0.23.1-pinned-files-only",
        model_vocabulary: MODEL_VOCABULARY,
        mapped_vocabulary: TOKENIZER_VOCABULARY,
        masked_padding_ids: [TOKENIZER_VOCABULARY, MODEL_VOCABULARY - 1],
        cases: case_proofs,
        incremental_utf8_exact,
        cross_token_stop_exact,
        verdict: "PINNED_TOKENIZER_TEMPLATE_STREAM_PASS",
    })
}

fn tokenizer_message(role: ChatRole, content: &str) -> ChatMessage {
    ChatMessage {
        role,
        content: content.to_owned(),
        reasoning_content: None,
        tool_calls: Vec::new(),
        name: None,
        tool_call_id: None,
    }
}

fn proof_graph(
    graph_id: u32,
    mode: StepMode,
    sequence_bucket: u16,
    rows: u32,
    depth: u8,
) -> GraphEntry {
    GraphEntry {
        graph_id,
        key: GraphKey {
            mode,
            sequence_bucket,
            verifier_row_bucket: if mode == StepMode::Prefill { 0 } else { rows },
            mtp_depth: depth,
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
            vec![6, 7, 8]
        },
        maximum_scratch_bytes: 1,
        argument_bytes: 1,
        graph_object_bytes: 1,
        resident_module_bytes: 1,
        admission_slo_class: 1,
    }
}

fn count_events(events: &[RequestEvent], predicate: impl Fn(&RequestEvent) -> bool) -> u32 {
    u32::try_from(events.iter().filter(|event| predicate(event)).count()).unwrap_or(u32::MAX)
}

#[derive(Serialize)]
struct Exl3Proof {
    schema: &'static str,
    model_revision: &'static str,
    source_revision: &'static str,
    source_version: &'static str,
    tensor: &'static str,
    logical_shape_k_n: [u32; 2],
    payload_bytes: usize,
    payload_sha256: String,
    reconstructed_f16_bytes: usize,
    reconstructed_sha256: String,
}

#[derive(Serialize)]
struct DirectTierProof {
    schema: &'static str,
    format_version: u16,
    alignment: u64,
    target_only: DirectTierExtentProof,
    mtp: DirectTierExtentProof,
    target_padding_ranges: [[u64; 2]; 2],
    mtp_padding_ranges: [[u64; 2]; 3],
    blocking_store_migration_rejected: bool,
    gpu_evidence: &'static str,
    verdict: &'static str,
}

#[derive(Serialize)]
struct DirectTierExtentProof {
    capability: &'static str,
    logical_bytes: u64,
    physical_bytes: u64,
    physical_blocks: u64,
    address_aligned: bool,
    physical_sha256: String,
    piece_sha256: Vec<String>,
    decoded_exact: bool,
}

fn direct_tier_proof() -> Result<DirectTierProof, Box<dyn std::error::Error>> {
    let target = direct_tier_case(false)?;
    let mtp = direct_tier_case(true)?;
    let legacy = glm_cache::TierRecord {
        namespace: [1; 32],
        page_key: [2; 32],
        generation: 1,
        tier: glm_cache::Tier::Nvme,
        mtp: false,
        pieces: vec![
            glm_cache::TierPieceRecord {
                piece: TierPiece::TargetKv,
                byte_length: glm_cache::TARGET_KV_EXTENT_LENGTH,
                storage_offset: 0,
                sha256: [3; 32],
            },
            glm_cache::TierPieceRecord {
                piece: TierPiece::TargetIndexer,
                byte_length: glm_cache::TARGET_INDEXER_EXTENT_LENGTH,
                storage_offset: glm_cache::TARGET_INDEXER_EXTENT_OFFSET,
                sha256: [4; 32],
            },
        ],
    };
    let blocking_store_migration_rejected = matches!(
        glm_cache::DirectExtentRecord::try_from_blocking_store(&legacy),
        Err(glm_cache::DirectExtentError::MigrationRequired)
    );
    if !target.decoded_exact || !mtp.decoded_exact || !blocking_store_migration_rejected {
        return Err("direct-tier CPU proof failed".into());
    }
    Ok(DirectTierProof {
        schema: "glmaxx.direct-tier-extent-cpu-proof.v1",
        format_version: glm_cache::DIRECT_TIER_FORMAT_VERSION,
        alignment: glm_cache::DIRECT_IO_ALIGNMENT,
        target_only: target,
        mtp,
        target_padding_ranges: [
            [
                glm_cache::TARGET_KV_EXTENT_LENGTH,
                glm_cache::TARGET_INDEXER_EXTENT_OFFSET,
            ],
            [
                glm_cache::TARGET_INDEXER_EXTENT_OFFSET + glm_cache::TARGET_INDEXER_EXTENT_LENGTH,
                glm_cache::TARGET_ONLY_PHYSICAL_BYTES,
            ],
        ],
        mtp_padding_ranges: [
            [
                glm_cache::TARGET_KV_EXTENT_LENGTH,
                glm_cache::TARGET_INDEXER_EXTENT_OFFSET,
            ],
            [
                glm_cache::TARGET_INDEXER_EXTENT_OFFSET + glm_cache::TARGET_INDEXER_EXTENT_LENGTH,
                glm_cache::DRAFT_SIDECAR_EXTENT_OFFSET,
            ],
            [
                glm_cache::DRAFT_SIDECAR_EXTENT_OFFSET + glm_cache::DRAFT_SIDECAR_EXTENT_LENGTH,
                glm_cache::MTP_PHYSICAL_BYTES,
            ],
        ],
        blocking_store_migration_rejected,
        gpu_evidence: "none: CPU extent codec only",
        verdict: "DIRECT_EXTENT_LAYOUT_PADDING_AND_DIGEST_PASS",
    })
}

fn direct_tier_case(mtp: bool) -> Result<DirectTierExtentProof, Box<dyn std::error::Error>> {
    let target_kv = direct_tier_pattern(glm_cache::TARGET_KV_EXTENT_LENGTH, 3);
    let target_indexer = direct_tier_pattern(glm_cache::TARGET_INDEXER_EXTENT_LENGTH, 5);
    let draft = mtp.then(|| direct_tier_pattern(glm_cache::DRAFT_SIDECAR_EXTENT_LENGTH, 7));
    let (record, buffer) = glm_cache::encode_direct_extent(
        [0x11; 32],
        [0x22; 32],
        3,
        5,
        glm_cache::DIRECT_IO_ALIGNMENT * 7,
        glm_cache::DirectPagePieces {
            target_kv: &target_kv,
            target_indexer: &target_indexer,
            draft_sidecar: draft.as_deref(),
        },
    )?;
    let decoded = glm_cache::decode_direct_extent(&record, buffer.as_slice())?;
    let decoded_exact = decoded.target_kv == target_kv
        && decoded.target_indexer == target_indexer
        && decoded.draft_sidecar == draft.as_deref();
    Ok(DirectTierExtentProof {
        capability: if mtp { "mtp" } else { "target" },
        logical_bytes: record.capability.logical_bytes(),
        physical_bytes: record.physical_length,
        physical_blocks: record.physical_length / glm_cache::DIRECT_IO_ALIGNMENT,
        address_aligned: (buffer.as_slice().as_ptr() as usize)
            .is_multiple_of(glm_cache::DIRECT_IO_ALIGNMENT as usize),
        physical_sha256: hex(&record.physical_sha256),
        piece_sha256: record
            .pieces
            .iter()
            .map(|piece| hex(&piece.sha256))
            .collect(),
        decoded_exact,
    })
}

fn direct_tier_pattern(length: u64, seed: u8) -> Vec<u8> {
    (0..length)
        .map(|index| seed.wrapping_add((index % 251) as u8))
        .collect()
}

#[derive(Serialize)]
struct DirectTierStateProof {
    schema: &'static str,
    buffer_alignment: u64,
    buffer_generations: [u64; 2],
    stale_buffer_generation_rejected: bool,
    descriptor_user_data: [u64; 2],
    descriptor_binding_exact: bool,
    stale_descriptor_generation_rejected: bool,
    shared_ticket_waiter_order: Vec<u64>,
    shared_ticket_physical_bytes: u64,
    tenant_logical_bytes: [[u64; 2]; 2],
    mtp_satisfies_target: bool,
    target_rejected_for_mtp: bool,
    original_cancel_both_orders_pass: bool,
    logical_abandonment_pass: bool,
    catalog_before_submit: &'static str,
    catalog_after_submit: &'static str,
    cq_entries: u32,
    nodrop_independent: bool,
    final_tickets: usize,
    final_waiters: usize,
    final_active_buffers: usize,
    final_descriptors: usize,
    final_cqes: u32,
    final_physical_bytes: u64,
    gpu_evidence: &'static str,
    verdict: &'static str,
}

fn direct_tier_state_proof() -> Result<DirectTierStateProof, Box<dyn std::error::Error>> {
    let mut buffers = glm_cache::DirectBufferPool::new(1)?;
    let first = buffers.reserve(glm_cache::DirectBufferUse::CpuRead)?;
    buffers.transition(first, glm_cache::DirectBufferState::ReadInflight)?;
    buffers.release_abandoned_read(first)?;
    let second = buffers.reserve(glm_cache::DirectBufferUse::CpuRead)?;
    let stale_buffer_generation_rejected = matches!(
        buffers.state(first),
        Err(glm_cache::DirectBufferStateError::StaleGeneration)
    );
    buffers.transition(second, glm_cache::DirectBufferState::ReadInflight)?;
    buffers.transition(second, glm_cache::DirectBufferState::HashingForRead)?;
    buffers.transition(second, glm_cache::DirectBufferState::HostReady)?;
    buffers.transition(second, glm_cache::DirectBufferState::Free)?;

    let binding = glm_cache::DirectDescriptorBinding {
        buffer: second,
        operation_generation: 9,
        operation: glm_cache::DirectOperationKind::Read,
    };
    let mut descriptors = glm_cache::DirectDescriptorTable::new(1)?;
    let original = descriptors.allocate(binding)?;
    let cancel = descriptors.issue_cancel(original)?;
    let descriptor_binding_exact =
        descriptors.resolve(original)? == binding && descriptors.resolve(cancel)? == binding;
    descriptors.complete(cancel)?;
    descriptors.complete(original)?;
    let reused_descriptor = descriptors.allocate(glm_cache::DirectDescriptorBinding {
        operation_generation: 10,
        ..binding
    })?;
    let stale_descriptor_generation_rejected = matches!(
        descriptors.resolve(original),
        Err(glm_cache::DirectDescriptorError::StaleGeneration)
    );
    descriptors.complete(reused_descriptor)?;

    let config = direct_tier_state_config();
    let mut table = glm_cache::DirectRestoreTable::new(config, false)?;
    let mtp_record = direct_tier_state_record(glm_cache::DirectTierCapability::Mtp, 1);
    let ticket = table
        .plan(
            direct_tier_state_request(30, 1, glm_cache::DirectTierCapability::Target),
            mtp_record.clone(),
            5,
            [0x51; 32],
        )?
        .ticket();
    let mtp_satisfies_target = matches!(
        table.plan(
            direct_tier_state_request(10, 1, glm_cache::DirectTierCapability::Mtp),
            mtp_record.clone(),
            5,
            [0x51; 32],
        )?,
        glm_cache::DirectRestoreAdmission::Joined(joined) if joined == ticket
    ) && matches!(
        table.plan(
            direct_tier_state_request(20, 2, glm_cache::DirectTierCapability::Target),
            mtp_record,
            5,
            [0x51; 32],
        )?,
        glm_cache::DirectRestoreAdmission::Joined(joined) if joined == ticket
    );
    let shared_ticket_waiter_order = table.waiter_order(ticket)?;
    let shared_ticket_physical_bytes = table.physical_bytes();
    let tenant_logical_bytes = [
        [1, table.tenant_logical_bytes(1)],
        [2, table.tenant_logical_bytes(2)],
    ];
    table.reserve_buffer(ticket)?;
    table.submit_read(ticket)?;
    table.complete_original(ticket, glm_cache::DirectReadCompletion::Exact)?;
    let hash_job = table
        .next_hash_job()?
        .ok_or("direct-tier checksum job was not queued")?;
    if hash_job.ticket() != ticket {
        return Err("direct-tier checksum job ticket drift".into());
    }
    let hash_result = table.run_hash_job(hash_job)?;
    if !hash_result.verified() {
        return Err("direct-tier checksum rejected the canonical zero extent".into());
    }
    table.complete_hash(hash_result)?;
    let delivered = table.finish_cpu_delivery(ticket)?;
    if delivered != shared_ticket_waiter_order {
        return Err("direct-tier waiter delivery order drift".into());
    }

    let target_rejected_for_mtp = matches!(
        glm_cache::DirectRestoreTable::new(config, false)?.plan(
            direct_tier_state_request(1, 1, glm_cache::DirectTierCapability::Mtp),
            direct_tier_state_record(glm_cache::DirectTierCapability::Target, 2),
            5,
            [0x51; 32],
        ),
        Err(glm_cache::DirectRestoreError::Capability)
    );
    let original_cancel_both_orders_pass =
        direct_tier_cancellation_order(false)? && direct_tier_cancellation_order(true)?;
    let logical_abandonment_pass = direct_tier_logical_abandonment()?;
    let (catalog_before_submit, catalog_after_submit) = direct_tier_catalog_binding()?;
    let (cq_entries, nodrop_independent) = direct_tier_cq_proof()?;

    let all_pass = stale_buffer_generation_rejected
        && descriptor_binding_exact
        && stale_descriptor_generation_rejected
        && mtp_satisfies_target
        && target_rejected_for_mtp
        && original_cancel_both_orders_pass
        && logical_abandonment_pass
        && catalog_before_submit == "REPLAN_REQUIRED"
        && catalog_after_submit == "SUBMITTED_RECORD_PINNED"
        && nodrop_independent
        && table.ticket_count() == 0
        && table.waiter_count() == 0
        && table.active_buffers() == 0
        && table.outstanding_descriptors() == 0
        && table.outstanding_cqes() == 0
        && table.physical_bytes() == 0;
    if !all_pass {
        return Err("direct-tier state CPU proof failed".into());
    }
    Ok(DirectTierStateProof {
        schema: "glmaxx.direct-tier-state-cpu-proof.v1",
        buffer_alignment: glm_cache::DIRECT_IO_ALIGNMENT,
        buffer_generations: [first.generation, second.generation],
        stale_buffer_generation_rejected,
        descriptor_user_data: [original.user_data(), cancel.user_data()],
        descriptor_binding_exact,
        stale_descriptor_generation_rejected,
        shared_ticket_waiter_order,
        shared_ticket_physical_bytes,
        tenant_logical_bytes,
        mtp_satisfies_target,
        target_rejected_for_mtp,
        original_cancel_both_orders_pass,
        logical_abandonment_pass,
        catalog_before_submit,
        catalog_after_submit,
        cq_entries,
        nodrop_independent,
        final_tickets: table.ticket_count(),
        final_waiters: table.waiter_count(),
        final_active_buffers: table.active_buffers(),
        final_descriptors: table.outstanding_descriptors(),
        final_cqes: table.outstanding_cqes(),
        final_physical_bytes: table.physical_bytes(),
        gpu_evidence: "none: deterministic CPU state machine only",
        verdict: "DIRECT_BUFFER_DESCRIPTOR_RESTORE_STATE_PASS",
    })
}

#[derive(Serialize)]
struct DirectTierChecksumProof {
    schema: &'static str,
    maximum_hash_jobs: u32,
    hash_wait_before_read_submission: bool,
    wait_preserved_buffer_reservation: bool,
    descriptors_after_wait: usize,
    cqes_after_wait: u32,
    queued_hash_jobs: usize,
    running_hash_jobs: usize,
    hash_job_ticket: u64,
    hash_job_buffer_slot: u32,
    hash_job_buffer_generation: u64,
    canonical_extent_verified: bool,
    replayed_result_rejected: bool,
    corrupt_extent_rejected: bool,
    corrupt_buffer_quarantined: bool,
    final_tickets: usize,
    final_waiters: usize,
    final_active_hash_jobs: u32,
    final_active_buffers: usize,
    final_descriptors: usize,
    final_cqes: u32,
    final_physical_bytes: u64,
    gpu_evidence: &'static str,
    verdict: &'static str,
}

fn direct_tier_checksum_proof() -> Result<DirectTierChecksumProof, Box<dyn std::error::Error>> {
    let mut config = direct_tier_state_config();
    config.maximum_hash_jobs = 1;
    let mut table = glm_cache::DirectRestoreTable::new(config, false)?;
    let first = table
        .plan(
            direct_tier_state_request(1, 1, glm_cache::DirectTierCapability::Target),
            direct_tier_state_record(glm_cache::DirectTierCapability::Target, 31),
            5,
            [0x51; 32],
        )?
        .ticket();
    let second = table
        .plan(
            direct_tier_state_request(2, 2, glm_cache::DirectTierCapability::Target),
            direct_tier_state_record(glm_cache::DirectTierCapability::Target, 32),
            5,
            [0x51; 32],
        )?
        .ticket();
    table.reserve_buffer(first)?;
    table.reserve_buffer(second)?;
    table.submit_read(first)?;
    let hash_wait_before_read_submission = matches!(
        table.submit_read(second),
        Err(glm_cache::DirectRestoreError::HashWait)
    );
    let wait_preserved_buffer_reservation =
        table.state(second)? == glm_cache::DirectRestoreState::BufferReserved;
    let descriptors_after_wait = table.outstanding_descriptors();
    let cqes_after_wait = table.outstanding_cqes();
    table.validate_invariants()?;

    table.complete_original(first, glm_cache::DirectReadCompletion::Exact)?;
    let queued_hash_jobs = table.queued_hash_jobs();
    let hash_job = table
        .next_hash_job()?
        .ok_or("direct-tier checksum proof did not dequeue the first job")?;
    let running_hash_jobs = table.running_hash_jobs();
    let hash_result = table.run_hash_job(hash_job)?;
    let canonical_extent_verified = hash_result.verified();
    table.complete_hash(hash_result)?;
    let replayed_result_rejected = matches!(
        table.complete_hash(hash_result),
        Err(glm_cache::DirectRestoreError::HashBinding)
    );
    table.finish_cpu_delivery(first)?;

    table.submit_read(second)?;
    table.complete_original(second, glm_cache::DirectReadCompletion::Exact)?;
    let second_job = table
        .next_hash_job()?
        .ok_or("direct-tier checksum proof did not dequeue the second job")?;
    let second_result = table.run_hash_job(second_job)?;
    table.complete_hash(second_result)?;
    table.finish_cpu_delivery(second)?;
    table.validate_invariants()?;

    let mut corrupt = glm_cache::DirectRestoreTable::new(config, false)?;
    let corrupt_ticket = corrupt
        .plan(
            direct_tier_state_request(3, 3, glm_cache::DirectTierCapability::Target),
            direct_tier_state_record(glm_cache::DirectTierCapability::Target, 33),
            5,
            [0x51; 32],
        )?
        .ticket();
    corrupt.reserve_buffer(corrupt_ticket)?;
    corrupt.submit_read(corrupt_ticket)?;
    corrupt.copy_into_read_destination(corrupt_ticket, 0, &[1])?;
    corrupt.complete_original(corrupt_ticket, glm_cache::DirectReadCompletion::Exact)?;
    let corrupt_job = corrupt
        .next_hash_job()?
        .ok_or("direct-tier corrupt checksum job was not queued")?;
    let corrupt_result = corrupt.run_hash_job(corrupt_job)?;
    let corrupt_extent_rejected = !corrupt_result.verified()
        && matches!(
            corrupt.complete_hash(corrupt_result),
            Err(glm_cache::DirectRestoreError::Integrity)
        );
    let corrupt_buffer_quarantined = corrupt.quarantined_buffers() == 1
        && corrupt.active_buffers() == 0
        && corrupt.ticket_count() == 0
        && corrupt.active_hash_jobs() == 0;
    corrupt.validate_invariants()?;

    let all_pass = hash_wait_before_read_submission
        && wait_preserved_buffer_reservation
        && descriptors_after_wait == 1
        && cqes_after_wait == 1
        && queued_hash_jobs == 1
        && running_hash_jobs == 1
        && canonical_extent_verified
        && replayed_result_rejected
        && corrupt_extent_rejected
        && corrupt_buffer_quarantined
        && table.ticket_count() == 0
        && table.waiter_count() == 0
        && table.active_hash_jobs() == 0
        && table.active_buffers() == 0
        && table.outstanding_descriptors() == 0
        && table.outstanding_cqes() == 0
        && table.physical_bytes() == 0;
    if !all_pass {
        return Err("direct-tier checksum authority CPU proof failed".into());
    }

    Ok(DirectTierChecksumProof {
        schema: "glmaxx.direct-tier-checksum-authority-cpu-proof.v1",
        maximum_hash_jobs: config.maximum_hash_jobs,
        hash_wait_before_read_submission,
        wait_preserved_buffer_reservation,
        descriptors_after_wait,
        cqes_after_wait,
        queued_hash_jobs,
        running_hash_jobs,
        hash_job_ticket: hash_job.ticket().0,
        hash_job_buffer_slot: hash_job.buffer().slot,
        hash_job_buffer_generation: hash_job.buffer().generation,
        canonical_extent_verified,
        replayed_result_rejected,
        corrupt_extent_rejected,
        corrupt_buffer_quarantined,
        final_tickets: table.ticket_count(),
        final_waiters: table.waiter_count(),
        final_active_hash_jobs: table.active_hash_jobs(),
        final_active_buffers: table.active_buffers(),
        final_descriptors: table.outstanding_descriptors(),
        final_cqes: table.outstanding_cqes(),
        final_physical_bytes: table.physical_bytes(),
        gpu_evidence: "none: deterministic CPU checksum authority only",
        verdict: "DIRECT_TIER_CHECKSUM_AUTHORITY_PASS",
    })
}

#[derive(Serialize)]
struct DirectTierChecksumWorkerProof {
    schema: &'static str,
    maximum_hash_jobs: u32,
    worker_count: u32,
    command_queue_capacity: u32,
    completion_queue_capacity: u32,
    workers_started_before_read_admission: bool,
    queued_before_dispatch: usize,
    running_after_dispatch: usize,
    manual_execution_rejected: bool,
    live_shutdown_rejected: bool,
    completed_ticket_ids: Vec<u64>,
    target_extent_verified: bool,
    mtp_extent_verified: bool,
    zero_copy_shared_allocation_verified: bool,
    abandoned_hash_acknowledged: bool,
    corrupt_extent_rejected: bool,
    corrupt_buffer_quarantined: bool,
    worker_restart_rejected: bool,
    post_shutdown_read_rejected: bool,
    final_worker_count: u32,
    final_tickets: usize,
    final_waiters: usize,
    final_active_hash_jobs: u32,
    final_active_buffers: usize,
    final_quarantined_buffers: usize,
    final_descriptors: usize,
    final_cqes: u32,
    final_physical_bytes: u64,
    gpu_evidence: &'static str,
    verdict: &'static str,
}

fn direct_tier_checksum_worker_proof()
-> Result<DirectTierChecksumWorkerProof, Box<dyn std::error::Error>> {
    let config = direct_tier_state_config();
    let mut table = glm_cache::DirectRestoreTable::new(config, false)?;
    table.start_checksum_workers(2)?;
    let worker_count = table.checksum_worker_count();
    let command_queue_capacity = table.checksum_worker_capacity();
    let completion_queue_capacity = table.checksum_worker_capacity();
    let workers_started_before_read_admission =
        table.ticket_count() == 0 && table.active_hash_jobs() == 0 && worker_count == 2;

    let target = table
        .plan(
            direct_tier_state_request(1, 1, glm_cache::DirectTierCapability::Target),
            direct_tier_state_record(glm_cache::DirectTierCapability::Target, 41),
            5,
            [0x51; 32],
        )?
        .ticket();
    let mtp = table
        .plan(
            direct_tier_state_request(2, 2, glm_cache::DirectTierCapability::Mtp),
            direct_tier_state_record(glm_cache::DirectTierCapability::Mtp, 42),
            5,
            [0x51; 32],
        )?
        .ticket();
    for ticket in [target, mtp] {
        table.reserve_buffer(ticket)?;
        table.submit_read(ticket)?;
        table.complete_original(ticket, glm_cache::DirectReadCompletion::Exact)?;
    }
    let queued_before_dispatch = table.queued_hash_jobs();
    let manual_execution_rejected = matches!(
        table.next_hash_job(),
        Err(glm_cache::DirectRestoreError::HashExecutionMode)
    );
    for expected in [target, mtp] {
        let dispatched = table
            .dispatch_next_checksum()?
            .ok_or("direct-tier checksum worker proof lost a queued task")?;
        if dispatched.ticket() != expected {
            return Err("direct-tier checksum worker dispatch order drift".into());
        }
    }
    let running_after_dispatch = table.running_hash_jobs();
    let live_shutdown_rejected = matches!(
        table.shutdown_checksum_workers(),
        Err(glm_cache::DirectRestoreError::CompletionOutstanding)
    );

    let mut completed_ticket_ids = BTreeSet::new();
    let mut target_extent_verified = false;
    let mut mtp_extent_verified = false;
    let mut zero_copy_shared_allocation_verified = true;
    for _ in 0..2 {
        let result = wait_direct_checksum_result(&mut table)?;
        zero_copy_shared_allocation_verified &= result.worker_shared_allocation() == Some(true);
        if result.job().ticket() == target {
            target_extent_verified = result.verified();
        } else if result.job().ticket() == mtp {
            mtp_extent_verified = result.verified();
        } else {
            return Err("direct-tier checksum worker returned an unknown ticket".into());
        }
        completed_ticket_ids.insert(result.job().ticket().0);
        let ticket = result.job().ticket();
        table.complete_hash(result)?;
        table.finish_cpu_delivery(ticket)?;
    }

    let abandoned = table
        .plan(
            direct_tier_state_request(3, 3, glm_cache::DirectTierCapability::Target),
            direct_tier_state_record(glm_cache::DirectTierCapability::Target, 43),
            5,
            [0x51; 32],
        )?
        .ticket();
    table.reserve_buffer(abandoned)?;
    table.submit_read(abandoned)?;
    table.complete_original(abandoned, glm_cache::DirectReadCompletion::Exact)?;
    let abandonment_waited_for_hash = matches!(
        table.cancel_waiter(3, true)?,
        glm_cache::DirectCancellation::WaitingForHashAcknowledgement
    );
    table
        .dispatch_next_checksum()?
        .ok_or("direct-tier abandoned checksum task was not dispatched")?;
    let abandoned_result = wait_direct_checksum_result(&mut table)?;
    zero_copy_shared_allocation_verified &=
        abandoned_result.worker_shared_allocation() == Some(true);
    let abandoned_result_verified = abandoned_result.verified();
    table.complete_hash(abandoned_result)?;
    let abandoned_hash_acknowledged = abandonment_waited_for_hash
        && abandoned_result_verified
        && table.ticket_count() == 0
        && table.active_hash_jobs() == 0;

    let corrupt = table
        .plan(
            direct_tier_state_request(4, 4, glm_cache::DirectTierCapability::Target),
            direct_tier_state_record(glm_cache::DirectTierCapability::Target, 44),
            5,
            [0x51; 32],
        )?
        .ticket();
    table.reserve_buffer(corrupt)?;
    table.submit_read(corrupt)?;
    table.copy_into_read_destination(corrupt, 0, &[1])?;
    table.complete_original(corrupt, glm_cache::DirectReadCompletion::Exact)?;
    table
        .dispatch_next_checksum()?
        .ok_or("direct-tier corrupt checksum task was not dispatched")?;
    let corrupt_result = wait_direct_checksum_result(&mut table)?;
    zero_copy_shared_allocation_verified &= corrupt_result.worker_shared_allocation() == Some(true);
    let corrupt_extent_rejected = !corrupt_result.verified()
        && matches!(
            table.complete_hash(corrupt_result),
            Err(glm_cache::DirectRestoreError::Integrity)
        );
    let corrupt_buffer_quarantined = table.quarantined_buffers() == 1
        && table.ticket_count() == 0
        && table.active_hash_jobs() == 0;

    table.shutdown_checksum_workers()?;
    let final_worker_count = table.checksum_worker_count();
    let worker_restart_rejected = matches!(
        table.start_checksum_workers(1),
        Err(glm_cache::DirectRestoreError::WorkerConfig)
    );
    let after_shutdown = table
        .plan(
            direct_tier_state_request(5, 5, glm_cache::DirectTierCapability::Target),
            direct_tier_state_record(glm_cache::DirectTierCapability::Target, 45),
            5,
            [0x51; 32],
        )?
        .ticket();
    table.reserve_buffer(after_shutdown)?;
    let post_shutdown_read_rejected = matches!(
        table.submit_read(after_shutdown),
        Err(glm_cache::DirectRestoreError::WorkerUnavailable)
    );
    table.cancel_waiter(5, true)?;
    table.validate_invariants()?;

    let completed_ticket_ids: Vec<_> = completed_ticket_ids.into_iter().collect();
    let all_pass = worker_count == 2
        && command_queue_capacity == config.maximum_hash_jobs
        && completion_queue_capacity == config.maximum_hash_jobs
        && workers_started_before_read_admission
        && queued_before_dispatch == 2
        && running_after_dispatch == 2
        && manual_execution_rejected
        && live_shutdown_rejected
        && completed_ticket_ids == [target.0, mtp.0]
        && target_extent_verified
        && mtp_extent_verified
        && zero_copy_shared_allocation_verified
        && abandoned_hash_acknowledged
        && corrupt_extent_rejected
        && corrupt_buffer_quarantined
        && worker_restart_rejected
        && post_shutdown_read_rejected
        && final_worker_count == 0
        && table.ticket_count() == 0
        && table.waiter_count() == 0
        && table.active_hash_jobs() == 0
        && table.active_buffers() == 0
        && table.quarantined_buffers() == 1
        && table.outstanding_descriptors() == 0
        && table.outstanding_cqes() == 0
        && table.physical_bytes() == 0;
    if !all_pass {
        return Err("direct-tier checksum worker CPU proof failed".into());
    }

    Ok(DirectTierChecksumWorkerProof {
        schema: "glmaxx.direct-tier-checksum-workers-cpu-proof.v1",
        maximum_hash_jobs: config.maximum_hash_jobs,
        worker_count,
        command_queue_capacity,
        completion_queue_capacity,
        workers_started_before_read_admission,
        queued_before_dispatch,
        running_after_dispatch,
        manual_execution_rejected,
        live_shutdown_rejected,
        completed_ticket_ids,
        target_extent_verified,
        mtp_extent_verified,
        zero_copy_shared_allocation_verified,
        abandoned_hash_acknowledged,
        corrupt_extent_rejected,
        corrupt_buffer_quarantined,
        worker_restart_rejected,
        post_shutdown_read_rejected,
        final_worker_count,
        final_tickets: table.ticket_count(),
        final_waiters: table.waiter_count(),
        final_active_hash_jobs: table.active_hash_jobs(),
        final_active_buffers: table.active_buffers(),
        final_quarantined_buffers: table.quarantined_buffers(),
        final_descriptors: table.outstanding_descriptors(),
        final_cqes: table.outstanding_cqes(),
        final_physical_bytes: table.physical_bytes(),
        gpu_evidence: "none: deterministic fixed CPU checksum workers only",
        verdict: "DIRECT_TIER_FIXED_CHECKSUM_WORKERS_PASS",
    })
}

fn wait_direct_checksum_result(
    table: &mut glm_cache::DirectRestoreTable,
) -> Result<glm_cache::DirectHashResult, Box<dyn std::error::Error>> {
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        if let Some(result) = table.poll_checksum_result()? {
            return Ok(result);
        }
        thread::yield_now();
    }
    Err("direct-tier checksum worker exceeded the bounded CPU proof deadline".into())
}

fn direct_tier_state_config() -> glm_cache::DirectRestoreConfig {
    glm_cache::DirectRestoreConfig {
        maximum_tickets: 4,
        maximum_waiters_per_ticket: 4,
        maximum_hash_jobs: 2,
        maximum_physical_bytes: glm_cache::MTP_PHYSICAL_BYTES * 4,
        maximum_logical_bytes_per_tenant: glm_cache::MTP_LOGICAL_BYTES * 4,
        buffer_slots: 2,
        descriptor_capacity: 2,
    }
}

fn direct_tier_state_request(
    request_id: u64,
    tenant_id: u64,
    required_capability: glm_cache::DirectTierCapability,
) -> glm_cache::DirectRestoreRequest {
    glm_cache::DirectRestoreRequest {
        request_id,
        tenant_id,
        required_capability,
    }
}

fn direct_tier_state_record(
    capability: glm_cache::DirectTierCapability,
    key: u8,
) -> glm_cache::DirectExtentRecord {
    let mut pieces = vec![
        glm_cache::DirectPieceRecord {
            piece: TierPiece::TargetKv,
            extent_offset: glm_cache::TARGET_KV_EXTENT_OFFSET,
            logical_length: glm_cache::TARGET_KV_EXTENT_LENGTH,
            sha256: direct_tier_zero_sha256(glm_cache::TARGET_KV_EXTENT_LENGTH),
        },
        glm_cache::DirectPieceRecord {
            piece: TierPiece::TargetIndexer,
            extent_offset: glm_cache::TARGET_INDEXER_EXTENT_OFFSET,
            logical_length: glm_cache::TARGET_INDEXER_EXTENT_LENGTH,
            sha256: direct_tier_zero_sha256(glm_cache::TARGET_INDEXER_EXTENT_LENGTH),
        },
    ];
    if capability == glm_cache::DirectTierCapability::Mtp {
        pieces.push(glm_cache::DirectPieceRecord {
            piece: TierPiece::DraftSidecar,
            extent_offset: glm_cache::DRAFT_SIDECAR_EXTENT_OFFSET,
            logical_length: glm_cache::DRAFT_SIDECAR_EXTENT_LENGTH,
            sha256: direct_tier_zero_sha256(glm_cache::DRAFT_SIDECAR_EXTENT_LENGTH),
        });
    }
    glm_cache::DirectExtentRecord {
        format_version: glm_cache::DIRECT_TIER_FORMAT_VERSION,
        namespace: [0x11; 32],
        page_key: [key; 32],
        durable_revision: 7,
        capability,
        segment_id: 3,
        physical_offset: glm_cache::DIRECT_IO_ALIGNMENT * u64::from(key),
        physical_length: capability.physical_bytes(),
        physical_sha256: direct_tier_zero_sha256(capability.physical_bytes()),
        pieces,
    }
}

fn direct_tier_zero_sha256(length: u64) -> [u8; 32] {
    let zeros = [0_u8; 4_096];
    let mut remaining = length;
    let mut hasher = Sha256::new();
    while remaining != 0 {
        let bytes = usize::try_from(remaining.min(zeros.len() as u64))
            .expect("zero digest chunk fits usize");
        hasher.update(&zeros[..bytes]);
        remaining -= bytes as u64;
    }
    hasher.finalize().into()
}

fn direct_tier_cancellation_order(
    original_first: bool,
) -> Result<bool, Box<dyn std::error::Error>> {
    let mut table = glm_cache::DirectRestoreTable::new(direct_tier_state_config(), false)?;
    let ticket = table
        .plan(
            direct_tier_state_request(1, 1, glm_cache::DirectTierCapability::Target),
            direct_tier_state_record(
                glm_cache::DirectTierCapability::Target,
                if original_first { 3 } else { 4 },
            ),
            5,
            [0x51; 32],
        )?
        .ticket();
    table.reserve_buffer(ticket)?;
    table.submit_read(ticket)?;
    if table.cancel_waiter(1, true)? != glm_cache::DirectCancellation::AsyncCancelSubmitted {
        return Ok(false);
    }
    if original_first {
        table.complete_original(ticket, glm_cache::DirectReadCompletion::Cancelled)?;
        if table.active_buffers() != 1 {
            return Ok(false);
        }
        table.complete_cancel(ticket)?;
    } else {
        table.complete_cancel(ticket)?;
        if table.active_buffers() != 1 {
            return Ok(false);
        }
        table.complete_original(ticket, glm_cache::DirectReadCompletion::Cancelled)?;
    }
    Ok(table.ticket_count() == 0
        && table.active_buffers() == 0
        && table.outstanding_descriptors() == 0
        && table.outstanding_cqes() == 0
        && table.physical_bytes() == 0)
}

fn direct_tier_logical_abandonment() -> Result<bool, Box<dyn std::error::Error>> {
    let mut table = glm_cache::DirectRestoreTable::new(direct_tier_state_config(), false)?;
    let ticket = table
        .plan(
            direct_tier_state_request(1, 1, glm_cache::DirectTierCapability::Target),
            direct_tier_state_record(glm_cache::DirectTierCapability::Target, 5),
            5,
            [0x51; 32],
        )?
        .ticket();
    table.reserve_buffer(ticket)?;
    table.submit_read(ticket)?;
    if table.cancel_waiter(1, false)? != glm_cache::DirectCancellation::AbandonedWithoutAsyncCancel
    {
        return Ok(false);
    }
    let retained = table.active_buffers() == 1 && table.physical_bytes() != 0;
    table.complete_original(ticket, glm_cache::DirectReadCompletion::Exact)?;
    Ok(retained
        && table.ticket_count() == 0
        && table.active_buffers() == 0
        && table.outstanding_descriptors() == 0)
}

fn direct_tier_catalog_binding() -> Result<(&'static str, &'static str), Box<dyn std::error::Error>>
{
    let mut table = glm_cache::DirectRestoreTable::new(direct_tier_state_config(), false)?;
    let ticket = table
        .plan(
            direct_tier_state_request(1, 1, glm_cache::DirectTierCapability::Target),
            direct_tier_state_record(glm_cache::DirectTierCapability::Target, 6),
            5,
            [0x51; 32],
        )?
        .ticket();
    let before = match table.catalog_binding(ticket, 6, [0x52; 32])? {
        glm_cache::DirectCatalogBinding::ReplanRequired => "REPLAN_REQUIRED",
        _ => "WRONG",
    };
    table.reserve_buffer(ticket)?;
    table.submit_read(ticket)?;
    let after = match table.catalog_binding(ticket, 6, [0x52; 32])? {
        glm_cache::DirectCatalogBinding::SubmittedRecordPinned => "SUBMITTED_RECORD_PINNED",
        _ => "WRONG",
    };
    table.cancel_waiter(1, false)?;
    table.complete_original(ticket, glm_cache::DirectReadCompletion::Exact)?;
    Ok((before, after))
}

fn direct_tier_cq_proof() -> Result<(u32, bool), Box<dyn std::error::Error>> {
    let mut traces = Vec::new();
    for nodrop in [false, true] {
        let mut tracker = glm_cache::DirectCqTracker::new(2, 4, nodrop)?;
        tracker.try_submit(glm_cache::DirectCqKind::Original)?;
        tracker.try_submit(glm_cache::DirectCqKind::Original)?;
        tracker.try_submit(glm_cache::DirectCqKind::AsyncCancel)?;
        tracker.try_submit(glm_cache::DirectCqKind::Fsync)?;
        let saturated = matches!(
            tracker.try_submit(glm_cache::DirectCqKind::AsyncCancel),
            Err(glm_cache::DirectRestoreError::CqWait)
        );
        let high = tracker.outstanding();
        for kind in [
            glm_cache::DirectCqKind::AsyncCancel,
            glm_cache::DirectCqKind::Original,
            glm_cache::DirectCqKind::Original,
            glm_cache::DirectCqKind::Fsync,
        ] {
            tracker.complete(kind)?;
        }
        traces.push((saturated, high, tracker.outstanding()));
    }
    Ok((4, traces[0] == traces[1] && traces[0] == (true, 4, 0)))
}

fn exl3_proof(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let payload = fs::read(path)?;
    let metadata = Exl3Metadata::new(Exl3Projection::Gate, 3, 0, 0, 3, 6_144, 512)?;
    let tensor = Exl3Trellis::from_source_payload(metadata, &payload)?;
    let reconstructed = tensor.reconstruct_native_f16()?;
    let mut reconstructed_bytes = Vec::with_capacity(reconstructed.len() * 2);
    for word in reconstructed {
        reconstructed_bytes.extend_from_slice(&word.to_le_bytes());
    }
    let report = Exl3Proof {
        schema: "glmaxx.exl3-source-payload-proof.v1",
        model_revision: glm_format::EXL3_MODEL_REVISION,
        source_revision: glm_format::EXL3_SOURCE_REVISION,
        source_version: glm_format::EXL3_SOURCE_VERSION,
        tensor: "model.layers.3.mlp.experts.0.gate_proj.rank0",
        logical_shape_k_n: [6_144, 512],
        payload_bytes: payload.len(),
        payload_sha256: hex(&sha256(&payload)),
        reconstructed_f16_bytes: reconstructed_bytes.len(),
        reconstructed_sha256: hex(&sha256(&reconstructed_bytes)),
    };
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

#[derive(Serialize)]
struct MatrixProof {
    schema: &'static str,
    model_shape: [usize; 3],
    row_buckets: Vec<usize>,
    routing: Vec<RoutingProof>,
    numerical: Vec<NumericalProof>,
    positive_gpu_cases: usize,
    negative_route_cases: usize,
    expansion: &'static str,
    gpu_evidence: &'static str,
}

#[derive(Serialize)]
struct RoutingProof {
    case: &'static str,
    expected: &'static str,
    assignments_by_rows: Vec<[usize; 2]>,
}

#[derive(Serialize)]
struct NumericalProof {
    case: &'static str,
    activation_rows: usize,
    activation_sha256: String,
    packed_weight_sha256: String,
    value_bytes: usize,
    scale_bytes: usize,
}

fn matrix_proof() -> Result<MatrixProof, Box<dyn std::error::Error>> {
    let constants = ModelConstants::default();
    let n = constants.local_gate_up_rows as usize;
    let k = constants.hidden as usize;
    let row_buckets: Vec<usize> = DECODE_ROWS.into_iter().chain(PREFILL_ROWS).collect();
    let mut routing = Vec::with_capacity(ROUTING_CASES.len());
    for case in ROUTING_CASES {
        let mut assignments_by_rows = Vec::with_capacity(row_buckets.len());
        for &rows in &row_buckets {
            let routes = generate_routes(case, rows)?;
            let compacted = compact_routes(&routes, rows);
            if compacted.is_err() != case.expects_rejection() {
                return Err(
                    format!("routing case {} has wrong outcome at M={rows}", case.id()).into(),
                );
            }
            assignments_by_rows.push([rows, compacted.map_or(0, |value| value.len())]);
        }
        routing.push(RoutingProof {
            case: case.id(),
            expected: if case.expects_rejection() {
                "reject"
            } else {
                "launch"
            },
            assignments_by_rows,
        });
    }

    let mut numerical = Vec::with_capacity(NUMERICAL_CASES.len());
    let activation_rows = *PREFILL_ROWS.last().ok_or("no prefill rows declared")?;
    for case in NUMERICAL_CASES {
        let fixture = generate_numerical_fixture(case, activation_rows, n, k)?;
        let activation_sha256 = f32_hash(&fixture.activations);
        let packed = PackedNvfp4::pack(&fixture.weights, n, k, Codec::OneDimensional)?;
        numerical.push(NumericalProof {
            case: case.id(),
            activation_rows,
            activation_sha256,
            packed_weight_sha256: packed_hash(&packed),
            value_bytes: packed.values.len(),
            scale_bytes: packed.scales.len(),
        });
    }

    let positive_routes = ROUTING_CASES
        .iter()
        .filter(|case| !case.expects_rejection())
        .count();
    let row_count = DECODE_ROWS.len() + PREFILL_ROWS.len();
    Ok(MatrixProof {
        schema: "glmaxx.sm120-fc1-matrix-proof.v1",
        model_shape: [n, k, constants.routed_experts as usize],
        row_buckets,
        routing,
        numerical,
        positive_gpu_cases: row_count * (positive_routes + NUMERICAL_CASES.len()),
        negative_route_cases: row_count,
        expansion: "routing rows x routing cases at deterministic-random-v1, plus numerical rows x numerical cases at one-hot-expert-0; not a cross product",
        gpu_evidence: "none: deterministic CPU fixture expansion only",
    })
}

#[cfg(feature = "cuda-ffi")]
fn gpu_rank_bind_smoke() -> Result<(), Box<dyn std::error::Error>> {
    let workers: Vec<_> = (0_u8..4)
        .map(|rank| {
            std::thread::Builder::new()
                .name(format!("glmaxx-bind-rank-{rank}"))
                .spawn(move || {
                    let context = glm_cuda::NativeRankContext::bind(rank)?;
                    let identity = context.identity();
                    if context.stream()? == 0 {
                        return Err(glm_cuda::KernelError::Null);
                    }
                    context.synchronize()?;
                    Ok::<_, glm_cuda::KernelError>(identity)
                })
        })
        .collect::<Result<_, _>>()?;
    let mut identities = Vec::with_capacity(4);
    for worker in workers {
        identities.push(worker.join().map_err(|_| "rank bind worker panicked")??);
    }
    identities.sort_by_key(|identity| identity.device_index);
    if identities.len() != 4
        || identities.iter().enumerate().any(|(rank, identity)| {
            identity.visible_devices != 4
                || usize::try_from(identity.device_index).ok() != Some(rank)
                || identity.compute_capability != 120
                || identity.multiprocessor_count == 0
                || identity.total_memory_bytes == 0
        })
    {
        return Err("native TP4 device binding did not produce the exact SM120 rank set".into());
    }
    let devices: Vec<_> = identities
        .iter()
        .map(|identity| {
            serde_json::json!({
                "rank": identity.device_index,
                "device_index": identity.device_index,
                "compute_capability": identity.compute_capability,
                "multiprocessor_count": identity.multiprocessor_count,
                "total_memory_bytes": identity.total_memory_bytes,
            })
        })
        .collect();
    let report = serde_json::json!({
        "schema": "glmaxx.sm120-tp4-rank-bind.v1",
        "visible_devices": 4,
        "devices": devices,
        "streams": "one nonblocking stream created, synchronized, and destroyed on each persistent-rank test thread",
        "kernel_launched": false,
        "verdict": "SM120_TP4_RANK_BIND_PASS",
    });
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

#[cfg(feature = "cuda-ffi")]
fn gpu_rank_memory_baseline() -> Result<(), Box<dyn std::error::Error>> {
    use std::sync::{Arc, Barrier};

    let contexts_ready = Arc::new(Barrier::new(4));
    let measurements_complete = Arc::new(Barrier::new(4));
    let workers: Vec<_> = (0_u8..4)
        .map(|rank| {
            let contexts_ready = Arc::clone(&contexts_ready);
            let measurements_complete = Arc::clone(&measurements_complete);
            std::thread::spawn(move || {
                let bound = (|| {
                    let context = glm_cuda::NativeRankContext::bind(rank)?;
                    if context.stream()? == 0 {
                        return Err(glm_cuda::KernelError::Null);
                    }
                    context.synchronize()?;
                    Ok(context)
                })();
                contexts_ready.wait();
                let measurement = bound.and_then(|context| {
                    let identity = context.identity();
                    let post_context_free_memory_bytes = context.free_memory_bytes()?;
                    Ok::<_, glm_cuda::KernelError>((identity, post_context_free_memory_bytes))
                });
                measurements_complete.wait();
                measurement
            })
        })
        .collect();
    let mut measurements = Vec::with_capacity(4);
    for worker in workers {
        measurements.push(worker.join().map_err(|_| "rank memory worker panicked")??);
    }
    measurements.sort_by_key(|(identity, _)| identity.device_index);
    if measurements.len() != 4
        || measurements
            .iter()
            .enumerate()
            .any(|(rank, (identity, free_bytes))| {
                identity.visible_devices != 4
                    || usize::try_from(identity.device_index).ok() != Some(rank)
                    || identity.compute_capability != 120
                    || identity.total_memory_bytes == 0
                    || *free_bytes == 0
                    || *free_bytes > identity.total_memory_bytes
            })
    {
        return Err("native TP4 memory baseline did not produce the exact SM120 rank set".into());
    }
    let minimum_post_context_free_memory_bytes = measurements
        .iter()
        .map(|(_, free_bytes)| *free_bytes)
        .min()
        .ok_or("rank memory measurements are empty")?;
    let devices: Vec<_> = measurements
        .iter()
        .map(|(identity, free_bytes)| {
            serde_json::json!({
                "rank": identity.device_index,
                "device_index": identity.device_index,
                "compute_capability": identity.compute_capability,
                "multiprocessor_count": identity.multiprocessor_count,
                "total_memory_bytes": identity.total_memory_bytes,
                "post_context_free_memory_bytes": free_bytes,
                "unavailable_after_context_bytes": identity.total_memory_bytes - free_bytes,
            })
        })
        .collect();
    let report = serde_json::json!({
        "schema": "glmaxx.sm120-tp4-memory-baseline.v1",
        "visible_devices": 4,
        "devices": devices,
        "minimum_post_context_free_memory_bytes": minimum_post_context_free_memory_bytes,
        "contexts": "four contexts and one nonblocking stream per rank held simultaneously through all measurements",
        "allocation_posture": "no GLMAXX device allocation other than CUDA context/runtime-owned state and one stream per rank",
        "kernel_launched": false,
        "capacity_claim": false,
        "verdict": "SM120_TP4_MEMORY_BASELINE_DIAGNOSTIC",
    });
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

#[cfg(feature = "cuda-ffi")]
fn gpu_smoke(rows: u32) -> Result<(), Box<dyn std::error::Error>> {
    if rows == 0 || rows > 3072 {
        return Err("gpu-smoke rows must be in 1..=3072".into());
    }
    let constants = ModelConstants::default();
    let n = constants.local_gate_up_rows as usize;
    let k = constants.hidden as usize;
    let packed = PackedNvfp4::pack(&actual_shape_values(n, k), n, k, Codec::OneDimensional)?;
    let activation: Vec<f32> = (0..rows as usize * k)
        .map(|index| {
            let signed = i32::try_from((index * 13) % 257).unwrap() - 128;
            signed as f32 / 128.0
        })
        .collect();
    let activation_bf16: Vec<u16> = activation
        .iter()
        .map(|&value| (bf16_round(value).to_bits() >> 16) as u16)
        .collect();
    let cpu = routed_fc1_oracle(&activation, rows as usize, k, &packed)?;
    let gpu_bits = glm_cuda::run_single_expert(&activation_bf16, rows, &packed)?;
    let gpu: Vec<f32> = gpu_bits
        .into_iter()
        .map(|bits| f32::from_bits(u32::from(bits) << 16))
        .collect();
    let mut maximum_absolute = 0.0_f32;
    let mut maximum_relative = 0.0_f32;
    let mut failures = 0_u32;
    for (&reference, &actual) in cpu.iter().zip(&gpu) {
        let absolute = (reference - actual).abs();
        let relative = absolute / reference.abs().max(1.0e-6);
        maximum_absolute = maximum_absolute.max(absolute);
        maximum_relative = maximum_relative.max(relative);
        if !actual.is_finite() || absolute > 0.5 + 0.02 * reference.abs() {
            failures += 1;
        }
    }
    let report = serde_json::json!({
        "schema": "glmaxx.sm120-fc1-smoke.v1",
        "shape": [rows, 6144, 1024],
        "fixture_sha256": packed_hash(&packed),
        "kernel_abi": KERNEL_ABI,
        "tolerance": "abs <= 0.5 + 0.02 * abs(reference)",
        "maximum_absolute_error": maximum_absolute,
        "maximum_relative_error": maximum_relative,
        "failures": failures,
    });
    println!("{}", serde_json::to_string_pretty(&report)?);
    if failures != 0 {
        return Err("SM120 output exceeded predeclared tolerance".into());
    }
    Ok(())
}

#[cfg(feature = "cuda-ffi")]
fn gpu_fc2_smoke(rows: u32) -> Result<(), Box<dyn std::error::Error>> {
    if rows == 0 || rows > 8 {
        return Err("gpu-fc2-smoke CPU-control rows must be in 1..=8".into());
    }
    let hidden = 6_144_usize;
    let local_intermediate = 512_usize;
    let experts: Vec<u16> = (0_u16..8).collect();
    let packed = PackedNvfp4::pack(
        &actual_shape_values(hidden, local_intermediate),
        hidden,
        local_intermediate,
        Codec::OneDimensional,
    )?;
    let mut routes = Vec::with_capacity(rows as usize * experts.len());
    for token in 0..rows {
        for slot in 0_u8..8 {
            routes.push(Route {
                token,
                expert: u16::from(slot),
                slot,
                weight: f32::from(slot + 1) / 36.0,
            });
        }
    }
    let compacted = compact_routes(&routes, rows as usize)?;
    let activated: Vec<f32> = (0..compacted.len() * local_intermediate)
        .map(|index| {
            let signed = i32::try_from((index * 29 + 17) % 257).unwrap() - 128;
            bf16_round(signed as f32 / 96.0)
        })
        .collect();
    let weights: Vec<RoutedExpertWeights> = experts
        .iter()
        .map(|&expert| RoutedExpertWeights {
            expert,
            down: packed.clone(),
        })
        .collect();
    let reference = routed_fc2_oracle(
        &activated,
        &routes,
        rows as usize,
        local_intermediate,
        hidden,
        &weights,
    )?;
    let route_experts: Vec<u16> = compacted.iter().map(|route| route.expert).collect();
    let route_tokens: Vec<u32> = compacted.iter().map(|route| route.token).collect();
    let route_slots: Vec<u8> = compacted.iter().map(|route| route.slot).collect();
    let route_weights: Vec<f32> = compacted
        .iter()
        .map(|route| f32::from(route.slot + 1) / 36.0)
        .collect();
    let input_bf16 = to_bf16_bits(&activated);
    let device = glm_cuda::NativeFc2Fixture::replicated(&packed, &experts)?;
    let direct = device.run(
        &input_bf16,
        rows,
        &route_experts,
        &route_tokens,
        &route_slots,
        &route_weights,
    )?;
    let grouped = device.run_grouped_control(
        &input_bf16,
        rows,
        &route_experts,
        &route_tokens,
        &route_slots,
        &route_weights,
    )?;
    let grouped_repeat = device.run_grouped_control(
        &input_bf16,
        rows,
        &route_experts,
        &route_tokens,
        &route_slots,
        &route_weights,
    )?;
    let (direct_max_abs, direct_max_rel, direct_failures) = compare_fc2_output(&reference, &direct);
    let (grouped_max_abs, grouped_max_rel, grouped_failures) =
        compare_fc2_output(&reference, &grouped);
    let grouped_deterministic = grouped == grouped_repeat;
    let report = serde_json::json!({
        "schema": "glmaxx.sm120-fc2-smoke.v1",
        "shape": [rows, rows * 8, local_intermediate, hidden],
        "active_experts": experts.len(),
        "packed_weight_sha256": packed_hash(&packed),
        "kernel_abi": KERNEL_ABI,
        "route_weight_placement": "after down projection",
        "scatter_order": "token slot 0..7",
        "tolerance": "finite(gpu) and abs(gpu-cpu) <= 0.5 + 0.03 * abs(cpu)",
        "direct_output_sha256": f32_hash(&direct),
        "direct_maximum_absolute_error": direct_max_abs,
        "direct_maximum_relative_error": direct_max_rel,
        "direct_failed_elements": direct_failures,
        "grouped_output_sha256": f32_hash(&grouped),
        "grouped_maximum_absolute_error": grouped_max_abs,
        "grouped_maximum_relative_error": grouped_max_rel,
        "grouped_failed_elements": grouped_failures,
        "grouped_repeat_bitwise_deterministic": grouped_deterministic,
        "runtime_weight_repack_bytes": 0,
        "persistent_dequant_bytes": 0,
    });
    println!("{}", serde_json::to_string_pretty(&report)?);
    if direct_failures != 0 || grouped_failures != 0 || !grouped_deterministic {
        return Err("SM120 FC2 smoke did not satisfy the frozen gate".into());
    }
    Ok(())
}

#[cfg(feature = "cuda-ffi")]
fn compare_fc2_output(reference: &[f32], actual: &[f32]) -> (f32, f32, usize) {
    if reference.len() != actual.len() {
        return (f32::INFINITY, f32::INFINITY, usize::MAX);
    }
    let mut maximum_absolute = 0.0_f32;
    let mut maximum_relative = 0.0_f32;
    let mut failures = 0_usize;
    for (&reference, &actual) in reference.iter().zip(actual) {
        let absolute = (reference - actual).abs();
        let relative = absolute / reference.abs().max(1.0e-6);
        if absolute.is_finite() {
            maximum_absolute = maximum_absolute.max(absolute);
        } else {
            maximum_absolute = f32::INFINITY;
        }
        if relative.is_finite() {
            maximum_relative = maximum_relative.max(relative);
        } else {
            maximum_relative = f32::INFINITY;
        }
        if !reference.is_finite() || !actual.is_finite() || absolute > 0.5 + 0.03 * reference.abs()
        {
            failures = failures.saturating_add(1);
        }
    }
    (maximum_absolute, maximum_relative, failures)
}

#[cfg(feature = "cuda-ffi")]
fn gpu_exl3_smoke(projection_name: &str, rows: u32) -> Result<(), Box<dyn std::error::Error>> {
    if rows == 0 || rows > 8 {
        return Err("gpu-exl3-smoke CPU-control rows must be in 1..=8".into());
    }
    let projection = match projection_name {
        "gate" => Exl3Projection::Gate,
        "up" => Exl3Projection::Up,
        "down" => Exl3Projection::Down,
        _ => return Err("gpu-exl3-smoke projection must be gate, up, or down".into()),
    };
    let (logical_k, logical_n) = match projection {
        Exl3Projection::Gate | Exl3Projection::Up => (6_144_u32, 512_u32),
        Exl3Projection::Down => (512_u32, 6_144_u32),
    };
    let metadata = Exl3Metadata::new(projection, 3, 0, 0, 3, logical_k, logical_n)?;
    let mut state = 0x0002_c026_0721_u64 ^ u64::from(projection as u8) ^ (u64::from(rows) << 32);
    let mut trellis = Vec::with_capacity(
        usize::try_from(metadata.trellis_words).map_err(|_| "EXL3 trellis is too large")?,
    );
    for _ in 0..metadata.trellis_words {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        trellis.push(state as u16);
    }
    let suh: Vec<u16> = (0..logical_k)
        .map(|index| {
            let offset = i32::try_from((index * 13 + 5) % 17).unwrap() - 8;
            glm_format::f32_to_f16_bits(1.0 + offset as f32 / 64.0)
        })
        .collect();
    let svh: Vec<u16> = (0..logical_n)
        .map(|index| {
            let offset = i32::try_from((index * 7 + 3) % 13).unwrap() - 6;
            glm_format::f32_to_f16_bits(1.0 + offset as f32 / 64.0)
        })
        .collect();
    let tensor = Exl3Trellis {
        metadata,
        trellis,
        suh,
        svh,
        mcg_marker: glm_format::EXL3_MCG_MULTIPLIER,
    };
    tensor.validate()?;
    let input_f16: Vec<u16> = (0..usize::try_from(rows)? * usize::try_from(logical_k)?)
        .map(|index| {
            let signed = i32::try_from((index * 29 + 17) % 257).unwrap() - 128;
            glm_format::f32_to_f16_bits(signed as f32 / 512.0)
        })
        .collect();
    let reference = tensor.matmul_reference_f16(&input_f16, usize::try_from(rows)?)?;
    let fixture = glm_cuda::NativeExl3Fixture::from_source(&tensor)?;
    let replay = fixture.run_repeated(&input_f16, rows, 2)?;
    let (maximum_absolute, maximum_relative, failures) =
        compare_f16_output(&reference, &replay.output_f16);
    let report = serde_json::json!({
        "schema": "glmaxx.sm120-exl3-source-smoke.v1",
        "projection": projection_name,
        "shape": [rows, logical_k, logical_n],
        "bits": 3,
        "kernel_abi": EXL3_KERNEL_ABI,
        "trellis_sha256": u16_hash(&tensor.trellis),
        "suh_sha256": u16_hash(&tensor.suh),
        "svh_sha256": u16_hash(&tensor.svh),
        "input_sha256": u16_hash(&input_f16),
        "cpu_output_sha256": u16_hash(&reference),
        "gpu_output_sha256": u16_hash(&replay.output_f16),
        "maximum_absolute_error": maximum_absolute,
        "maximum_relative_error": maximum_relative,
        "failed_elements": failures,
        "repeat_count": replay.repeat_count,
        "repeat_bitwise_deterministic": replay.bitwise_deterministic,
        "runtime_weight_repack_bytes": 0,
        "persistent_reconstructed_weight_bytes": 0,
        "tolerance": "finite(gpu) and abs(gpu-cpu) <= 0.5 + 0.03 * abs(cpu)",
    });
    println!("{}", serde_json::to_string_pretty(&report)?);
    if failures != 0 || !replay.bitwise_deterministic {
        return Err("SM120 EXL3 source-projection smoke did not satisfy the frozen gate".into());
    }
    Ok(())
}

#[cfg(feature = "cuda-ffi")]
fn compare_f16_output(reference: &[u16], actual: &[u16]) -> (f32, f32, usize) {
    if reference.len() != actual.len() {
        return (f32::INFINITY, f32::INFINITY, usize::MAX);
    }
    let mut maximum_absolute = 0.0_f32;
    let mut maximum_relative = 0.0_f32;
    let mut failures = 0_usize;
    for (&reference, &actual) in reference.iter().zip(actual) {
        let reference = glm_format::f16_bits_to_f32(reference);
        let actual = glm_format::f16_bits_to_f32(actual);
        let absolute = (reference - actual).abs();
        let relative = absolute / reference.abs().max(1.0e-6);
        maximum_absolute = if absolute.is_finite() {
            maximum_absolute.max(absolute)
        } else {
            f32::INFINITY
        };
        maximum_relative = if relative.is_finite() {
            maximum_relative.max(relative)
        } else {
            f32::INFINITY
        };
        if !reference.is_finite() || !actual.is_finite() || absolute > 0.5 + 0.03 * reference.abs()
        {
            failures = failures.saturating_add(1);
        }
    }
    (maximum_absolute, maximum_relative, failures)
}

#[cfg(feature = "cuda-ffi")]
#[derive(Serialize)]
struct GpuMatrixSummary {
    schema: &'static str,
    kernel_abi: &'static str,
    positive_cases: usize,
    negative_route_cases: usize,
    failed_elements: usize,
    eager_deterministic_cases: usize,
    evidence_directory: String,
}

#[cfg(feature = "cuda-ffi")]
#[derive(Serialize)]
struct GpuGraphSummary {
    schema: &'static str,
    kernel_abi: &'static str,
    graph_cases: usize,
    graph_repeat_count: u32,
    failed_elements: usize,
    bitwise_deterministic_cases: usize,
    evidence_directory: String,
}

#[cfg(feature = "cuda-ffi")]
#[derive(Serialize)]
struct GpuGraphCaseReport {
    schema: &'static str,
    rows: usize,
    assignments: usize,
    packed_weight_sha256: String,
    output_sha256: String,
    tolerance: &'static str,
    maximum_absolute_error: Option<f32>,
    maximum_relative_error: Option<f32>,
    graph_repeat_count: u32,
    graph_bitwise_deterministic: bool,
    failures: Vec<GpuFailure>,
}

#[cfg(feature = "cuda-ffi")]
#[derive(Serialize)]
struct GpuDenseControlSummary {
    schema: &'static str,
    kernel_abi: &'static str,
    backend: &'static str,
    cases: usize,
    repeat_count: usize,
    failed_elements: usize,
    bitwise_deterministic_cases: usize,
    runtime_weight_repack_bytes: u64,
    persistent_dequant_bytes: u64,
    materialized_gate_up_control: bool,
    evidence_directory: String,
}

#[cfg(feature = "cuda-ffi")]
#[derive(Serialize)]
struct GpuGroupedControlSummary {
    schema: &'static str,
    kernel_abi: &'static str,
    backend: &'static str,
    positive_cases: usize,
    negative_route_cases: usize,
    repeat_count: usize,
    bitwise_deterministic_cases: usize,
    failed_elements: usize,
    runtime_weight_repack_bytes: u64,
    persistent_dequant_bytes: u64,
    materialized_gate_up_control: bool,
    evidence_directory: String,
}

#[cfg(feature = "cuda-ffi")]
#[derive(Serialize)]
struct GpuBenchmarkSummary {
    schema: &'static str,
    kernel_abi: &'static str,
    backend: &'static str,
    cases: usize,
    warmup_iterations: u32,
    measured_iterations: u32,
    evidence_directory: String,
    verdict: &'static str,
}

#[cfg(feature = "cuda-ffi")]
#[derive(Serialize)]
struct GpuBenchmarkCase {
    schema: &'static str,
    kernel_abi: &'static str,
    backend: &'static str,
    rows: usize,
    assignments: usize,
    routing: &'static str,
    packed_weight_sha256: String,
    output_sha256: String,
    warmup_iterations: u32,
    measured_iterations: u32,
    activation_quantization: profile::LatencyDistribution,
    core_swiglu: profile::LatencyDistribution,
    inclusive_operator: profile::LatencyDistribution,
    graph_inclusive: profile::LatencyDistribution,
    host_enqueue: profile::LatencyDistribution,
    route_compaction: &'static str,
    runtime_weight_repack_bytes: u64,
    persistent_dequant_bytes: u64,
}

#[cfg(feature = "cuda-ffi")]
#[derive(Serialize)]
struct GpuGroupedBenchmarkSummary {
    schema: &'static str,
    kernel_abi: &'static str,
    backend: &'static str,
    cases: usize,
    routing_cases: usize,
    warmup_iterations: u32,
    measured_iterations: u32,
    evidence_directory: String,
    verdict: &'static str,
}

#[cfg(feature = "cuda-ffi")]
#[derive(Serialize)]
struct GpuGroupedBenchmarkCase {
    schema: &'static str,
    kernel_abi: &'static str,
    backend: &'static str,
    rows: usize,
    assignments: usize,
    active_experts: u32,
    routing: &'static str,
    packed_weight_sha256: String,
    output_sha256: String,
    warmup_iterations: u32,
    measured_iterations: u32,
    activation_quantization: profile::LatencyDistribution,
    grouped_core_swiglu: profile::LatencyDistribution,
    inclusive_operator: profile::LatencyDistribution,
    host_enqueue: profile::LatencyDistribution,
    route_compaction: &'static str,
    grouped_metadata_preparation: &'static str,
    runtime_weight_repack_bytes: u64,
    persistent_dequant_bytes: u64,
    materialized_gate_up_control: bool,
}

#[cfg(feature = "cuda-ffi")]
#[derive(Serialize)]
struct GpuProfileCaseReport {
    schema: &'static str,
    kernel_abi: &'static str,
    case: profile::ProfileCaseSpec,
    execution: &'static str,
    assignments: usize,
    active_experts: usize,
    output_sha256: String,
    routing_host: Option<profile::LatencyDistribution>,
    latency: Option<profile::LatencyDistribution>,
    byte_ledger: profile::ByteLedger,
    p50_contract_gib_per_second: Option<f64>,
    nvtx_root_range: &'static str,
    cuda_profiler_api_capture: bool,
    runtime_weight_repack_bytes: u64,
    persistent_dequant_bytes: u64,
}

#[cfg(feature = "cuda-ffi")]
fn parse_gpu_profile_case(
    arguments: &[String],
) -> Result<(profile::ProfileCaseSpec, PathBuf), Box<dyn std::error::Error>> {
    if arguments.len() != 10 {
        return Err("GPU profile case requires backend mode phase routing rows warmups iterations evidence-dir".into());
    }
    let case = profile::ProfileCaseSpec {
        backend: arguments.get(2).ok_or("missing profile backend")?.parse()?,
        mode: arguments.get(3).ok_or("missing profile mode")?.parse()?,
        phase: arguments.get(4).ok_or("missing profile phase")?.parse()?,
        routing: arguments.get(5).ok_or("missing profile routing")?.parse()?,
        rows: parse_argument(arguments, 6, "profile rows")?,
        warmup_iterations: parse_argument(arguments, 7, "profile warmups")?,
        measured_iterations: parse_argument(arguments, 8, "profile iterations")?,
    }
    .validate()?;
    Ok((
        case,
        PathBuf::from(
            arguments
                .get(9)
                .ok_or("missing profile evidence directory")?,
        ),
    ))
}

#[cfg(feature = "cuda-ffi")]
struct PreparedProfileRoutes {
    experts: Vec<u16>,
    tokens: Vec<u32>,
    slots: Vec<u8>,
    weights: Vec<f32>,
    active_experts: Vec<u16>,
}

#[cfg(feature = "cuda-ffi")]
fn prepare_profile_routes(
    routing: profile::ProfileRouting,
    rows: u32,
) -> Result<PreparedProfileRoutes, Box<dyn std::error::Error>> {
    let rows_usize = usize::try_from(rows)?;
    let mut routes = match routing {
        profile::ProfileRouting::OneHot => generate_routes(RoutingCase::OneHotExpert0, rows_usize)?,
        profile::ProfileRouting::Uniform => {
            generate_routes(RoutingCase::UniformAllExperts, rows_usize)?
        }
        profile::ProfileRouting::Zipf => generate_routes(RoutingCase::ZipfSkew, rows_usize)?,
        profile::ProfileRouting::EmptyExperts => {
            generate_routes(RoutingCase::EmptyExperts, rows_usize)?
        }
        profile::ProfileRouting::MaximallySkewed => {
            let mut routes = Vec::with_capacity(
                rows_usize
                    .checked_mul(8)
                    .ok_or("profile route capacity overflow")?,
            );
            for token in 0..rows {
                for slot in 0_u8..8 {
                    routes.push(Route {
                        token,
                        expert: u16::from(slot),
                        slot,
                        weight: 0.125,
                    });
                }
            }
            routes
        }
        profile::ProfileRouting::NotApplicable => {
            return Err("EXL3 does not have routed-expert metadata".into());
        }
    };
    if routing != profile::ProfileRouting::OneHot {
        for route in &mut routes {
            route.weight = 0.125;
        }
    }
    routes.sort_by_key(|route| (route.expert, route.token, route.slot));
    let compacted = compact_routes(&routes, rows_usize)?;
    if compacted.len() != routes.len()
        || compacted.iter().zip(&routes).any(|(compact, route)| {
            (compact.expert, compact.token, compact.slot) != (route.expert, route.token, route.slot)
        })
    {
        return Err("profile route compaction was not deterministic".into());
    }
    let mut active_experts = Vec::new();
    for route in &routes {
        if active_experts.last().copied() != Some(route.expert) {
            active_experts.push(route.expert);
        }
    }
    Ok(PreparedProfileRoutes {
        experts: routes.iter().map(|route| route.expert).collect(),
        tokens: routes.iter().map(|route| route.token).collect(),
        slots: routes.iter().map(|route| route.slot).collect(),
        weights: routes.iter().map(|route| route.weight).collect(),
        active_experts,
    })
}

#[cfg(feature = "cuda-ffi")]
fn time_profile_routing(
    case: profile::ProfileCaseSpec,
) -> Result<profile::LatencyDistribution, Box<dyn std::error::Error>> {
    for _ in 0..case.warmup_iterations {
        std::hint::black_box(prepare_profile_routes(case.routing, case.rows)?);
    }
    let mut samples = Vec::with_capacity(usize::try_from(case.measured_iterations)?);
    for _ in 0..case.measured_iterations {
        let start = Instant::now();
        std::hint::black_box(prepare_profile_routes(case.routing, case.rows)?);
        samples.push(start.elapsed().as_secs_f64() * 1_000_000.0);
    }
    Ok(profile::LatencyDistribution::from_microseconds(samples)?)
}

#[cfg(feature = "cuda-ffi")]
fn gpu_profile_case(
    case: profile::ProfileCaseSpec,
    evidence_directory: &Path,
    profiler_capture: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    validate_empty_external_gpu_directory(
        evidence_directory,
        if profiler_capture {
            "gpu-profile-case"
        } else {
            "gpu-time-case"
        },
    )?;
    let report = if case.backend.is_exl3() {
        execute_exl3_profile_case(case, profiler_capture)?
    } else if matches!(
        case.backend,
        profile::ProfileBackend::Nvfp4DirectFc1 | profile::ProfileBackend::Nvfp4GroupedFc1
    ) {
        execute_fc1_profile_case(case, profiler_capture)?
    } else {
        execute_fc2_profile_case(case, profiler_capture)?
    };
    let mut json = serde_json::to_vec_pretty(&report)?;
    json.push(b'\n');
    fs::write(evidence_directory.join("case.json"), &json)?;
    println!("{}", String::from_utf8(json)?);
    Ok(())
}

#[cfg(feature = "cuda-ffi")]
fn execute_fc1_profile_case(
    case: profile::ProfileCaseSpec,
    profiler_capture: bool,
) -> Result<GpuProfileCaseReport, Box<dyn std::error::Error>> {
    let rows = usize::try_from(case.rows)?;
    let constants = ModelConstants::default();
    let n = constants.local_gate_up_rows as usize;
    let k = constants.hidden as usize;
    let numerical = generate_numerical_fixture(NumericalCase::DeterministicRandom, rows, n, k)?;
    let packed = PackedNvfp4::pack(&numerical.weights, n, k, Codec::OneDimensional)?;
    let input = to_bf16_bits(&numerical.activations);
    let routes = prepare_profile_routes(case.routing, case.rows)?;
    let routing_host = if profiler_capture {
        None
    } else {
        Some(time_profile_routing(case)?)
    };
    let device = glm_cuda::NativeFc1Fixture::replicated(&packed, &routes.active_experts)?;
    let config = glm_cuda::Fc1BenchmarkConfig {
        warmup_iterations: case.warmup_iterations,
        measured_iterations: case.measured_iterations,
    };
    let phase = match case.phase {
        profile::ProfilePhase::Quantize => glm_cuda::Fc1ProfilePhase::Quantize,
        profile::ProfilePhase::Core => glm_cuda::Fc1ProfilePhase::CoreSwiglu,
        profile::ProfilePhase::Inclusive => glm_cuda::Fc1ProfilePhase::Inclusive,
        profile::ProfilePhase::GraphInclusive => glm_cuda::Fc1ProfilePhase::GraphInclusive,
        profile::ProfilePhase::Reduce | profile::ProfilePhase::Projection => {
            return Err("invalid FC1 profile phase".into());
        }
    };
    let latency = if profiler_capture {
        if case.backend.is_grouped() {
            device.profile_grouped_control(
                &input,
                case.rows,
                &routes.experts,
                &routes.tokens,
                &routes.slots,
                phase,
                config,
            )?;
        } else {
            device.profile_direct(
                &input,
                case.rows,
                &routes.experts,
                &routes.tokens,
                &routes.slots,
                phase,
                config,
            )?;
        }
        None
    } else {
        let samples = if case.backend.is_grouped() {
            device.time_grouped_phase(
                &input,
                case.rows,
                &routes.experts,
                &routes.tokens,
                &routes.slots,
                phase,
                config,
            )?
        } else {
            device.time_direct_phase(
                &input,
                case.rows,
                &routes.experts,
                &routes.tokens,
                &routes.slots,
                phase,
                config,
            )?
        };
        Some(profile::LatencyDistribution::from_microseconds(samples)?)
    };
    let output = if case.backend.is_grouped() {
        device.run_grouped_control(
            &input,
            case.rows,
            &routes.experts,
            &routes.tokens,
            &routes.slots,
        )?
    } else {
        device.run(
            &input,
            case.rows,
            &routes.experts,
            &routes.tokens,
            &routes.slots,
        )?
    };
    let byte_ledger = nvfp4_profile_byte_ledger(
        &input,
        &packed,
        routes.experts.len(),
        routes.active_experts.len(),
        output.len().checked_mul(2).ok_or("output byte overflow")?,
        usize::try_from(glm_cuda::grouped_workspace_bytes(u32::try_from(
            routes.experts.len(),
        )?)?)?,
    )?;
    build_profile_report(
        case,
        profiler_capture,
        routes.experts.len(),
        routes.active_experts.len(),
        u16_hash(&output),
        routing_host,
        latency,
        byte_ledger,
        KERNEL_ABI,
    )
}

#[cfg(feature = "cuda-ffi")]
fn execute_fc2_profile_case(
    case: profile::ProfileCaseSpec,
    profiler_capture: bool,
) -> Result<GpuProfileCaseReport, Box<dyn std::error::Error>> {
    let routes = prepare_profile_routes(case.routing, case.rows)?;
    let routing_host = if profiler_capture {
        None
    } else {
        Some(time_profile_routing(case)?)
    };
    let assignments = routes.experts.len();
    let n = usize::try_from(glm_cuda::HIDDEN)?;
    let k = usize::try_from(glm_cuda::LOCAL_INTERMEDIATE)?;
    let numerical =
        generate_numerical_fixture(NumericalCase::DeterministicRandom, assignments, n, k)?;
    let packed = PackedNvfp4::pack(&numerical.weights, n, k, Codec::OneDimensional)?;
    let input = to_bf16_bits(&numerical.activations);
    let device = glm_cuda::NativeFc2Fixture::replicated(&packed, &routes.active_experts)?;
    let config = glm_cuda::Fc1BenchmarkConfig {
        warmup_iterations: case.warmup_iterations,
        measured_iterations: case.measured_iterations,
    };
    let phase = match case.phase {
        profile::ProfilePhase::Quantize => glm_cuda::Fc2ProfilePhase::Quantize,
        profile::ProfilePhase::Core => glm_cuda::Fc2ProfilePhase::Core,
        profile::ProfilePhase::Reduce => glm_cuda::Fc2ProfilePhase::Reduce,
        profile::ProfilePhase::Inclusive => glm_cuda::Fc2ProfilePhase::Inclusive,
        profile::ProfilePhase::GraphInclusive | profile::ProfilePhase::Projection => {
            return Err("invalid FC2 profile phase".into());
        }
    };
    let latency = if profiler_capture {
        if case.backend.is_grouped() {
            device.profile_grouped_control(
                &input,
                case.rows,
                &routes.experts,
                &routes.tokens,
                &routes.slots,
                &routes.weights,
                phase,
                config,
            )?;
        } else {
            device.profile(
                &input,
                case.rows,
                &routes.experts,
                &routes.tokens,
                &routes.slots,
                &routes.weights,
                phase,
                config,
            )?;
        }
        None
    } else {
        let samples = if case.backend.is_grouped() {
            device.time_grouped_phase(
                &input,
                case.rows,
                &routes.experts,
                &routes.tokens,
                &routes.slots,
                &routes.weights,
                phase,
                config,
            )?
        } else {
            device.time_phase(
                &input,
                case.rows,
                &routes.experts,
                &routes.tokens,
                &routes.slots,
                &routes.weights,
                phase,
                config,
            )?
        };
        Some(profile::LatencyDistribution::from_microseconds(samples)?)
    };
    let output = if case.backend.is_grouped() {
        device.run_grouped_control(
            &input,
            case.rows,
            &routes.experts,
            &routes.tokens,
            &routes.slots,
            &routes.weights,
        )?
    } else {
        device.run(
            &input,
            case.rows,
            &routes.experts,
            &routes.tokens,
            &routes.slots,
            &routes.weights,
        )?
    };
    let byte_ledger = nvfp4_profile_byte_ledger(
        &input,
        &packed,
        assignments,
        routes.active_experts.len(),
        output.len().checked_mul(4).ok_or("output byte overflow")?,
        usize::try_from(fc2_grouped_workspace_bytes(
            case.rows,
            u32::try_from(assignments)?,
        )?)?,
    )?;
    build_profile_report(
        case,
        profiler_capture,
        assignments,
        routes.active_experts.len(),
        f32_hash(&output),
        routing_host,
        latency,
        byte_ledger,
        KERNEL_ABI,
    )
}

#[cfg(feature = "cuda-ffi")]
fn execute_exl3_profile_case(
    case: profile::ProfileCaseSpec,
    profiler_capture: bool,
) -> Result<GpuProfileCaseReport, Box<dyn std::error::Error>> {
    let projection = match case.backend {
        profile::ProfileBackend::Exl3Gate => Exl3Projection::Gate,
        profile::ProfileBackend::Exl3Up => Exl3Projection::Up,
        profile::ProfileBackend::Exl3Down => Exl3Projection::Down,
        _ => return Err("non-EXL3 backend passed to EXL3 runner".into()),
    };
    let (logical_k, logical_n) = match projection {
        Exl3Projection::Gate | Exl3Projection::Up => (6_144_u32, 512_u32),
        Exl3Projection::Down => (512_u32, 6_144_u32),
    };
    let metadata = Exl3Metadata::new(projection, 3, 0, 0, 3, logical_k, logical_n)?;
    let mut state =
        0x0002_c026_0721_u64 ^ u64::from(projection as u8) ^ (u64::from(case.rows) << 32);
    let mut trellis = Vec::with_capacity(usize::try_from(metadata.trellis_words)?);
    for _ in 0..metadata.trellis_words {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        trellis.push(state as u16);
    }
    let suh: Vec<u16> = (0..logical_k)
        .map(|index| {
            let offset = i32::try_from((index * 13 + 5) % 17).expect("bounded offset") - 8;
            glm_format::f32_to_f16_bits(1.0 + offset as f32 / 64.0)
        })
        .collect();
    let svh: Vec<u16> = (0..logical_n)
        .map(|index| {
            let offset = i32::try_from((index * 7 + 3) % 13).expect("bounded offset") - 6;
            glm_format::f32_to_f16_bits(1.0 + offset as f32 / 64.0)
        })
        .collect();
    let tensor = Exl3Trellis {
        metadata,
        trellis,
        suh,
        svh,
        mcg_marker: glm_format::EXL3_MCG_MULTIPLIER,
    };
    tensor.validate()?;
    let input_elements = usize::try_from(case.rows)?
        .checked_mul(usize::try_from(logical_k)?)
        .ok_or("EXL3 input length overflow")?;
    let input: Vec<u16> = (0..input_elements)
        .map(|index| {
            let signed = i32::try_from((index * 29 + 17) % 257).expect("bounded input") - 128;
            glm_format::f32_to_f16_bits(signed as f32 / 512.0)
        })
        .collect();
    let fixture = glm_cuda::NativeExl3Fixture::from_source(&tensor)?;
    let config = glm_cuda::Fc1BenchmarkConfig {
        warmup_iterations: case.warmup_iterations,
        measured_iterations: case.measured_iterations,
    };
    let latency = if profiler_capture {
        fixture.profile(&input, case.rows, config)?;
        None
    } else {
        Some(profile::LatencyDistribution::from_microseconds(
            fixture
                .benchmark(&input, case.rows, config)?
                .projection_samples_us,
        )?)
    };
    let output = fixture.run(&input, case.rows)?;
    let input_bytes = bytes_for_elements(input.len(), 2)?;
    let packed_value_bytes = bytes_for_elements(tensor.trellis.len(), 2)?;
    let packed_scale_bytes = bytes_for_elements(
        tensor
            .suh
            .len()
            .checked_add(tensor.svh.len())
            .ok_or("EXL3 rotation length overflow")?,
        2,
    )?;
    let output_bytes = bytes_for_elements(output.len(), 2)?;
    let temporary_bytes = exl3_workspace_bytes(case.rows, logical_k, logical_n)?;
    let contract_read_bytes = checked_sum(&[
        input_bytes,
        packed_value_bytes,
        packed_scale_bytes,
        temporary_bytes,
    ])?;
    let contract_write_bytes = checked_sum(&[output_bytes, temporary_bytes])?;
    let ledger = profile::ByteLedger {
        input_bytes,
        packed_value_bytes,
        packed_scale_bytes,
        metadata_bytes: 0,
        output_bytes,
        temporary_bytes,
        contract_read_bytes,
        contract_write_bytes,
    };
    build_profile_report(
        case,
        profiler_capture,
        usize::try_from(case.rows)?,
        1,
        u16_hash(&output),
        None,
        latency,
        ledger,
        EXL3_KERNEL_ABI,
    )
}

#[cfg(feature = "cuda-ffi")]
fn nvfp4_profile_byte_ledger(
    input: &[u16],
    packed: &PackedNvfp4,
    assignments: usize,
    active_experts: usize,
    output_bytes: usize,
    temporary_bytes: usize,
) -> Result<profile::ByteLedger, Box<dyn std::error::Error>> {
    let input_bytes = bytes_for_elements(input.len(), 2)?;
    let packed_value_bytes = bytes_for_elements(packed.values.len(), active_experts)?;
    let packed_scale_bytes = bytes_for_elements(packed.scales.len(), active_experts)?;
    let metadata_bytes = bytes_for_elements(assignments, 11)?
        .checked_add(bytes_for_elements(257, 4)?)
        .ok_or("route metadata byte overflow")?;
    let output_bytes = u64::try_from(output_bytes)?;
    let temporary_bytes = u64::try_from(temporary_bytes)?;
    let contract_read_bytes = checked_sum(&[
        input_bytes,
        packed_value_bytes,
        packed_scale_bytes,
        metadata_bytes,
        temporary_bytes,
    ])?;
    let contract_write_bytes = checked_sum(&[output_bytes, temporary_bytes])?;
    Ok(profile::ByteLedger {
        input_bytes,
        packed_value_bytes,
        packed_scale_bytes,
        metadata_bytes,
        output_bytes,
        temporary_bytes,
        contract_read_bytes,
        contract_write_bytes,
    })
}

#[cfg(feature = "cuda-ffi")]
fn bytes_for_elements(
    elements: usize,
    bytes_each: usize,
) -> Result<u64, Box<dyn std::error::Error>> {
    Ok(u64::try_from(
        elements
            .checked_mul(bytes_each)
            .ok_or("profile byte multiplication overflow")?,
    )?)
}

#[cfg(feature = "cuda-ffi")]
fn checked_sum(values: &[u64]) -> Result<u64, Box<dyn std::error::Error>> {
    values.iter().try_fold(0_u64, |sum, value| {
        sum.checked_add(*value)
            .ok_or_else(|| "profile byte addition overflow".into())
    })
}

#[cfg(feature = "cuda-ffi")]
#[allow(clippy::too_many_arguments)]
fn build_profile_report(
    case: profile::ProfileCaseSpec,
    profiler_capture: bool,
    assignments: usize,
    active_experts: usize,
    output_sha256: String,
    routing_host: Option<profile::LatencyDistribution>,
    latency: Option<profile::LatencyDistribution>,
    byte_ledger: profile::ByteLedger,
    kernel_abi: &'static str,
) -> Result<GpuProfileCaseReport, Box<dyn std::error::Error>> {
    let p50_contract_gib_per_second = latency
        .as_ref()
        .map(|distribution| byte_ledger.p50_gib_per_second(distribution))
        .transpose()?;
    Ok(GpuProfileCaseReport {
        schema: "glmaxx.sm120-profile-case.v1",
        kernel_abi,
        case,
        execution: if profiler_capture {
            "counter-or-trace-replay-without-CUDA-event-timing"
        } else {
            "retained-per-launch-CUDA-event-timing-without-profiler"
        },
        assignments,
        active_experts,
        output_sha256,
        routing_host,
        latency,
        byte_ledger,
        p50_contract_gib_per_second,
        nvtx_root_range: "glmaxx-profile",
        cuda_profiler_api_capture: profiler_capture,
        runtime_weight_repack_bytes: 0,
        persistent_dequant_bytes: 0,
    })
}

#[cfg(feature = "cuda-ffi")]
#[derive(Serialize)]
struct GpuCaseReport {
    schema: &'static str,
    suite: &'static str,
    rows: usize,
    routing: &'static str,
    numerical: &'static str,
    assignments: usize,
    packed_weight_sha256: String,
    output_sha256: String,
    tolerance: &'static str,
    maximum_absolute_error: Option<f32>,
    maximum_relative_error: Option<f32>,
    eager_repeat_count: usize,
    eager_bitwise_deterministic: bool,
    failures: Vec<GpuFailure>,
}

#[cfg(feature = "cuda-ffi")]
#[derive(Serialize)]
struct GpuFailure {
    assignment: usize,
    column: usize,
    reference_f32_bits: u32,
    actual_bf16_bits: u16,
    reason: &'static str,
}

#[cfg(feature = "cuda-ffi")]
fn gpu_matrix(evidence_directory: &Path) -> Result<(), Box<dyn std::error::Error>> {
    if !evidence_directory.is_dir() {
        return Err("gpu-matrix evidence directory must already exist".into());
    }
    if evidence_directory.read_dir()?.next().is_some() {
        return Err("gpu-matrix evidence directory must be empty".into());
    }
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .ok_or("cannot resolve repository root")?
        .canonicalize()?;
    if evidence_directory.canonicalize()?.starts_with(repository) {
        return Err("raw GPU evidence must be outside the Git repository".into());
    }
    let constants = ModelConstants::default();
    let n = constants.local_gate_up_rows as usize;
    let k = constants.hidden as usize;
    let max_rows = usize::try_from(
        *profile::PROFILE_PREFILL_ROWS
            .last()
            .ok_or("no prefill row bucket")?,
    )?;
    let row_buckets: Vec<usize> = profile::PROFILE_DECODE_ROWS
        .into_iter()
        .chain(profile::PROFILE_PREFILL_ROWS)
        .map(|rows| usize::try_from(rows).expect("profile rows fit usize"))
        .collect();
    let mut positive_cases = 0_usize;
    let mut negative_route_cases = 0_usize;
    let mut failed_elements = 0_usize;
    let mut eager_deterministic_cases = 0_usize;

    let routing_fixture =
        generate_numerical_fixture(NumericalCase::DeterministicRandom, max_rows, n, k)?;
    let routing_packed = PackedNvfp4::pack(&routing_fixture.weights, n, k, Codec::OneDimensional)?;
    let all_experts: Vec<u16> = (0_u16..=255).collect();
    let routing_device = glm_cuda::NativeFc1Fixture::replicated(&routing_packed, &all_experts)?;
    for &rows in &row_buckets {
        let activation = &routing_fixture.activations[..rows * k];
        let reference = routed_fc1_oracle(activation, rows, k, &routing_packed)?;
        let activation_bf16 = to_bf16_bits(activation);
        for routing in ROUTING_CASES {
            let routes = generate_routes(routing, rows)?;
            let compacted = compact_routes(&routes, rows);
            if routing.expects_rejection() {
                if compacted.is_ok() {
                    return Err(format!(
                        "negative routing case {} was accepted at M={rows}",
                        routing.id()
                    )
                    .into());
                }
                negative_route_cases += 1;
                continue;
            }
            let compacted = compacted?;
            let repeats = if routing == RoutingCase::OneHotExpert0
                && (rows == DECODE_ROWS[0] || rows == max_rows)
            {
                eager_deterministic_cases += 1;
                20
            } else {
                1
            };
            let report = execute_gpu_case(
                &routing_device,
                "routing",
                rows,
                routing,
                NumericalCase::DeterministicRandom,
                &activation_bf16,
                &reference,
                &routing_packed,
                &compacted,
                repeats,
            )?;
            failed_elements = failed_elements
                .checked_add(report.failures.len())
                .ok_or("failure count overflow")?;
            write_gpu_case(evidence_directory, &report)?;
            positive_cases += 1;
        }
    }

    for numerical in NUMERICAL_CASES {
        let fixture = generate_numerical_fixture(numerical, max_rows, n, k)?;
        let packed = PackedNvfp4::pack(&fixture.weights, n, k, Codec::OneDimensional)?;
        let device = glm_cuda::NativeFc1Fixture::replicated(&packed, &[0])?;
        for &rows in &row_buckets {
            let activation = &fixture.activations[..rows * k];
            let reference = routed_fc1_oracle(activation, rows, k, &packed)?;
            let activation_bf16 = to_bf16_bits(activation);
            let routes = generate_routes(RoutingCase::OneHotExpert0, rows)?;
            let compacted = compact_routes(&routes, rows)?;
            let report = execute_gpu_case(
                &device,
                "numerical",
                rows,
                RoutingCase::OneHotExpert0,
                numerical,
                &activation_bf16,
                &reference,
                &packed,
                &compacted,
                1,
            )?;
            failed_elements = failed_elements
                .checked_add(report.failures.len())
                .ok_or("failure count overflow")?;
            write_gpu_case(evidence_directory, &report)?;
            positive_cases += 1;
        }
    }

    let summary = GpuMatrixSummary {
        schema: "glmaxx.sm120-fc1-matrix-result.v1",
        kernel_abi: KERNEL_ABI,
        positive_cases,
        negative_route_cases,
        failed_elements,
        eager_deterministic_cases,
        evidence_directory: evidence_directory.display().to_string(),
    };
    let summary_bytes = serde_json::to_vec_pretty(&summary)?;
    fs::write(evidence_directory.join("summary.json"), &summary_bytes)?;
    println!("{}", String::from_utf8(summary_bytes)?);
    if positive_cases != 135
        || negative_route_cases != 9
        || eager_deterministic_cases != 2
        || failed_elements != 0
    {
        return Err("SM120 correctness matrix did not satisfy the frozen gate".into());
    }
    Ok(())
}

#[cfg(feature = "cuda-ffi")]
fn gpu_graph(evidence_directory: &Path) -> Result<(), Box<dyn std::error::Error>> {
    if !evidence_directory.is_dir() {
        return Err("gpu-graph evidence directory must already exist".into());
    }
    if evidence_directory.read_dir()?.next().is_some() {
        return Err("gpu-graph evidence directory must be empty".into());
    }
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .ok_or("cannot resolve repository root")?
        .canonicalize()?;
    if evidence_directory.canonicalize()?.starts_with(repository) {
        return Err("raw GPU evidence must be outside the Git repository".into());
    }

    const GRAPH_ROWS: [usize; 2] = [1, 256];
    const REPEAT_COUNT: u32 = 20;
    let constants = ModelConstants::default();
    let n = constants.local_gate_up_rows as usize;
    let k = constants.hidden as usize;
    let fixture =
        generate_numerical_fixture(NumericalCase::DeterministicRandom, GRAPH_ROWS[1], n, k)?;
    let packed = PackedNvfp4::pack(&fixture.weights, n, k, Codec::OneDimensional)?;
    let device = glm_cuda::NativeFc1Fixture::replicated(&packed, &[0])?;
    let mut failed_elements = 0_usize;
    let mut bitwise_deterministic_cases = 0_usize;

    for rows in GRAPH_ROWS {
        let activation = &fixture.activations[..rows * k];
        let activation_bf16 = to_bf16_bits(activation);
        let reference = routed_fc1_oracle(activation, rows, k, &packed)?;
        let routes = generate_routes(RoutingCase::OneHotExpert0, rows)?;
        let compacted = compact_routes(&routes, rows)?;
        let route_experts: Vec<u16> = compacted.iter().map(|route| route.expert).collect();
        let route_tokens: Vec<u32> = compacted.iter().map(|route| route.token).collect();
        let route_slots: Vec<u8> = compacted.iter().map(|route| route.slot).collect();
        let replay = device.run_graph_repeated(
            &activation_bf16,
            u32::try_from(rows)?,
            &route_experts,
            &route_tokens,
            &route_slots,
            REPEAT_COUNT,
        )?;
        if replay.bitwise_deterministic {
            bitwise_deterministic_cases += 1;
        }

        let local_intermediate = constants.local_intermediate as usize;
        let mut maximum_absolute = 0.0_f32;
        let mut maximum_relative = 0.0_f32;
        let mut finite_errors = true;
        let mut failures = Vec::new();
        for (assignment, route) in compacted.iter().enumerate() {
            let token = usize::try_from(route.token)?;
            for column in 0..local_intermediate {
                let reference_value = reference[token * local_intermediate + column];
                let actual_bits = replay.output_bf16[assignment * local_intermediate + column];
                let actual = f32::from_bits(u32::from(actual_bits) << 16);
                let absolute = (reference_value - actual).abs();
                let relative = absolute / reference_value.abs().max(1.0e-6);
                if absolute.is_finite() && relative.is_finite() {
                    maximum_absolute = maximum_absolute.max(absolute);
                    maximum_relative = maximum_relative.max(relative);
                } else {
                    finite_errors = false;
                }
                let reason = if !reference_value.is_finite() {
                    Some("non-finite CPU reference")
                } else if !actual.is_finite() {
                    Some("non-finite GPU output")
                } else if absolute > 0.5 + 0.02 * reference_value.abs() {
                    Some("element tolerance exceeded")
                } else {
                    None
                };
                if let Some(reason) = reason {
                    failures.push(GpuFailure {
                        assignment,
                        column,
                        reference_f32_bits: reference_value.to_bits(),
                        actual_bf16_bits: actual_bits,
                        reason,
                    });
                }
            }
        }
        if !replay.bitwise_deterministic {
            failures.push(GpuFailure {
                assignment: usize::MAX,
                column: usize::MAX,
                reference_f32_bits: 0,
                actual_bf16_bits: 0,
                reason: "CUDA graph replay output was not bitwise deterministic",
            });
        }
        failed_elements = failed_elements
            .checked_add(failures.len())
            .ok_or("failure count overflow")?;
        let report = GpuGraphCaseReport {
            schema: "glmaxx.sm120-fc1-graph-case-result.v1",
            rows,
            assignments: compacted.len(),
            packed_weight_sha256: packed_hash(&packed),
            output_sha256: u16_hash(&replay.output_bf16),
            tolerance: "finite(gpu) and abs(gpu-cpu) <= 0.5 + 0.02 * abs(cpu)",
            maximum_absolute_error: finite_errors.then_some(maximum_absolute),
            maximum_relative_error: finite_errors.then_some(maximum_relative),
            graph_repeat_count: replay.repeat_count,
            graph_bitwise_deterministic: replay.bitwise_deterministic,
            failures,
        };
        fs::write(
            evidence_directory.join(format!("graph-m{rows:03}.json")),
            serde_json::to_vec_pretty(&report)?,
        )?;
    }

    let summary = GpuGraphSummary {
        schema: "glmaxx.sm120-fc1-graph-result.v1",
        kernel_abi: KERNEL_ABI,
        graph_cases: GRAPH_ROWS.len(),
        graph_repeat_count: REPEAT_COUNT,
        failed_elements,
        bitwise_deterministic_cases,
        evidence_directory: evidence_directory.display().to_string(),
    };
    let summary_bytes = serde_json::to_vec_pretty(&summary)?;
    fs::write(evidence_directory.join("summary.json"), &summary_bytes)?;
    println!("{}", String::from_utf8(summary_bytes)?);
    if failed_elements != 0 || bitwise_deterministic_cases != GRAPH_ROWS.len() {
        return Err("SM120 CUDA graph correctness gate failed".into());
    }
    Ok(())
}

#[cfg(feature = "cuda-ffi")]
fn gpu_dense_control(evidence_directory: &Path) -> Result<(), Box<dyn std::error::Error>> {
    validate_empty_external_gpu_directory(evidence_directory, "gpu-dense-control")?;

    const CONTROL_ROWS: [usize; 2] = [1, 256];
    const REPEAT_COUNT: usize = 20;
    let constants = ModelConstants::default();
    let n = constants.local_gate_up_rows as usize;
    let k = constants.hidden as usize;
    let fixture =
        generate_numerical_fixture(NumericalCase::DeterministicRandom, CONTROL_ROWS[1], n, k)?;
    let packed = PackedNvfp4::pack(&fixture.weights, n, k, Codec::OneDimensional)?;
    let device = glm_cuda::NativeFc1Fixture::replicated(&packed, &[0])?;
    let mut failed_elements = 0_usize;
    let mut bitwise_deterministic_cases = 0_usize;

    for rows in CONTROL_ROWS {
        let activation = &fixture.activations[..rows * k];
        let activation_bf16 = to_bf16_bits(activation);
        let reference = routed_fc1_oracle(activation, rows, k, &packed)?;
        let routes = generate_routes(RoutingCase::OneHotExpert0, rows)?;
        let compacted = compact_routes(&routes, rows)?;
        let report = execute_dense_control_case(
            &device,
            rows,
            &activation_bf16,
            &reference,
            &packed,
            &compacted,
            REPEAT_COUNT,
        )?;
        if report.eager_bitwise_deterministic {
            bitwise_deterministic_cases += 1;
        }
        failed_elements = failed_elements
            .checked_add(report.failures.len())
            .ok_or("failure count overflow")?;
        write_gpu_case(evidence_directory, &report)?;
    }

    let summary = GpuDenseControlSummary {
        schema: "glmaxx.sm120-fc1-dense-control-summary.v1",
        kernel_abi: KERNEL_ABI,
        backend: "cutlass-sm120-nvfp4-materialized-gate-up-control",
        cases: CONTROL_ROWS.len(),
        repeat_count: REPEAT_COUNT,
        failed_elements,
        bitwise_deterministic_cases,
        runtime_weight_repack_bytes: 0,
        persistent_dequant_bytes: 0,
        materialized_gate_up_control: true,
        evidence_directory: evidence_directory.display().to_string(),
    };
    let summary_bytes = serde_json::to_vec_pretty(&summary)?;
    fs::write(evidence_directory.join("summary.json"), &summary_bytes)?;
    println!("{}", String::from_utf8(summary_bytes)?);
    if failed_elements != 0 || bitwise_deterministic_cases != CONTROL_ROWS.len() {
        return Err("SM120 CUTLASS dense control correctness gate failed".into());
    }
    Ok(())
}

#[cfg(feature = "cuda-ffi")]
fn gpu_grouped_control(evidence_directory: &Path) -> Result<(), Box<dyn std::error::Error>> {
    validate_empty_external_gpu_directory(evidence_directory, "gpu-grouped-control")?;

    const CONTROL_ROWS: [usize; 2] = [1, 256];
    const REPEAT_COUNT: usize = 20;
    let constants = ModelConstants::default();
    let n = constants.local_gate_up_rows as usize;
    let k = constants.hidden as usize;
    let fixture =
        generate_numerical_fixture(NumericalCase::DeterministicRandom, CONTROL_ROWS[1], n, k)?;
    let packed = PackedNvfp4::pack(&fixture.weights, n, k, Codec::OneDimensional)?;
    let all_experts: Vec<u16> = (0_u16..=255).collect();
    let device = glm_cuda::NativeFc1Fixture::replicated(&packed, &all_experts)?;
    let mut positive_cases = 0_usize;
    let mut negative_route_cases = 0_usize;
    let mut failed_elements = 0_usize;
    let mut bitwise_deterministic_cases = 0_usize;

    for rows in CONTROL_ROWS {
        let activation = &fixture.activations[..rows * k];
        let activation_bf16 = to_bf16_bits(activation);
        let reference = routed_fc1_oracle(activation, rows, k, &packed)?;
        for routing in ROUTING_CASES {
            let routes = generate_routes(routing, rows)?;
            let compacted = compact_routes(&routes, rows);
            if routing.expects_rejection() {
                if compacted.is_ok() {
                    return Err(format!(
                        "negative grouped routing case {} was accepted at M={rows}",
                        routing.id()
                    )
                    .into());
                }
                negative_route_cases += 1;
                continue;
            }
            let compacted = compacted?;
            let repeats = if routing == RoutingCase::UniformAllExperts {
                REPEAT_COUNT
            } else {
                1
            };
            let report = execute_grouped_control_case(
                &device,
                rows,
                routing,
                &activation_bf16,
                &reference,
                &packed,
                &compacted,
                repeats,
            )?;
            if repeats == REPEAT_COUNT && report.eager_bitwise_deterministic {
                bitwise_deterministic_cases += 1;
            }
            failed_elements = failed_elements
                .checked_add(report.failures.len())
                .ok_or("failure count overflow")?;
            write_gpu_case(evidence_directory, &report)?;
            positive_cases += 1;
        }
    }

    let summary = GpuGroupedControlSummary {
        schema: "glmaxx.sm120-fc1-grouped-control-summary.v1",
        kernel_abi: KERNEL_ABI,
        backend: "cutlass-sm120-nvfp4-expert-grouped-materialized-gate-up-control",
        positive_cases,
        negative_route_cases,
        repeat_count: REPEAT_COUNT,
        bitwise_deterministic_cases,
        failed_elements,
        runtime_weight_repack_bytes: 0,
        persistent_dequant_bytes: 0,
        materialized_gate_up_control: true,
        evidence_directory: evidence_directory.display().to_string(),
    };
    let summary_bytes = serde_json::to_vec_pretty(&summary)?;
    fs::write(evidence_directory.join("summary.json"), &summary_bytes)?;
    println!("{}", String::from_utf8(summary_bytes)?);
    if positive_cases != 14
        || negative_route_cases != 2
        || failed_elements != 0
        || bitwise_deterministic_cases != CONTROL_ROWS.len()
    {
        return Err("SM120 CUTLASS grouped control correctness gate failed".into());
    }
    Ok(())
}

#[cfg(feature = "cuda-ffi")]
fn gpu_grouped_bench(evidence_directory: &Path) -> Result<(), Box<dyn std::error::Error>> {
    validate_empty_external_gpu_directory(evidence_directory, "gpu-grouped-bench")?;

    const WARMUP: u32 = 20;
    const ITERATIONS: u32 = 200;
    const BENCHMARK_ROUTES: [RoutingCase; 3] = [
        RoutingCase::OneHotExpert0,
        RoutingCase::UniformAllExperts,
        RoutingCase::ZipfSkew,
    ];
    let constants = ModelConstants::default();
    let n = constants.local_gate_up_rows as usize;
    let k = constants.hidden as usize;
    let max_rows = *PREFILL_ROWS.last().ok_or("no prefill row bucket")?;
    let row_buckets: Vec<usize> = DECODE_ROWS.into_iter().chain(PREFILL_ROWS).collect();
    let fixture = generate_numerical_fixture(NumericalCase::DeterministicRandom, max_rows, n, k)?;
    let packed = PackedNvfp4::pack(&fixture.weights, n, k, Codec::OneDimensional)?;
    let all_experts: Vec<u16> = (0_u16..=255).collect();
    let device = glm_cuda::NativeFc1Fixture::replicated(&packed, &all_experts)?;

    for &rows in &row_buckets {
        let activation = &fixture.activations[..rows * k];
        let activation_bf16 = to_bf16_bits(activation);
        for routing in BENCHMARK_ROUTES {
            let routes = generate_routes(routing, rows)?;
            let compacted = compact_routes(&routes, rows)?;
            let route_experts: Vec<u16> = compacted.iter().map(|route| route.expert).collect();
            let route_tokens: Vec<u32> = compacted.iter().map(|route| route.token).collect();
            let route_slots: Vec<u8> = compacted.iter().map(|route| route.slot).collect();
            let timing = device.benchmark_grouped_control(
                &activation_bf16,
                u32::try_from(rows)?,
                &route_experts,
                &route_tokens,
                &route_slots,
                glm_cuda::Fc1BenchmarkConfig {
                    warmup_iterations: WARMUP,
                    measured_iterations: ITERATIONS,
                },
            )?;
            let output = device.run_grouped_control(
                &activation_bf16,
                u32::try_from(rows)?,
                &route_experts,
                &route_tokens,
                &route_slots,
            )?;
            let report = GpuGroupedBenchmarkCase {
                schema: "glmaxx.sm120-fc1-grouped-benchmark-case.v2",
                kernel_abi: KERNEL_ABI,
                backend: "cutlass-sm120-nvfp4-expert-grouped-materialized-gate-up-control",
                rows,
                assignments: compacted.len(),
                active_experts: timing.active_experts,
                routing: routing.id(),
                packed_weight_sha256: packed_hash(&packed),
                output_sha256: u16_hash(&output),
                warmup_iterations: timing.warmup_iterations,
                measured_iterations: timing.measured_iterations,
                activation_quantization: profile::LatencyDistribution::from_microseconds(
                    timing.activation_quantization_samples_us,
                )?,
                grouped_core_swiglu: profile::LatencyDistribution::from_microseconds(
                    timing.grouped_core_swiglu_samples_us,
                )?,
                inclusive_operator: profile::LatencyDistribution::from_microseconds(
                    timing.inclusive_operator_samples_us,
                )?,
                host_enqueue: profile::LatencyDistribution::from_microseconds(
                    timing.host_enqueue_samples_us,
                )?,
                route_compaction: "CPU fixture control outside timed CUDA boundary",
                grouped_metadata_preparation: "one host-to-device active-expert copy and device metadata build before warmup",
                runtime_weight_repack_bytes: 0,
                persistent_dequant_bytes: 0,
                materialized_gate_up_control: true,
            };
            fs::write(
                evidence_directory.join(format!("grouped-m{rows:03}-{}.json", routing.id())),
                serde_json::to_vec_pretty(&report)?,
            )?;
        }
    }

    let summary = GpuGroupedBenchmarkSummary {
        schema: "glmaxx.sm120-fc1-grouped-benchmark-summary.v2",
        kernel_abi: KERNEL_ABI,
        backend: "cutlass-sm120-nvfp4-expert-grouped-materialized-gate-up-control",
        cases: row_buckets.len() * BENCHMARK_ROUTES.len(),
        routing_cases: BENCHMARK_ROUTES.len(),
        warmup_iterations: WARMUP,
        measured_iterations: ITERATIONS,
        evidence_directory: evidence_directory.display().to_string(),
        verdict: "PROVISIONAL_MATERIALIZED_CONTROL_ONLY",
    };
    let summary_bytes = serde_json::to_vec_pretty(&summary)?;
    fs::write(evidence_directory.join("summary.json"), &summary_bytes)?;
    println!("{}", String::from_utf8(summary_bytes)?);
    Ok(())
}

#[cfg(feature = "cuda-ffi")]
fn gpu_bench(evidence_directory: &Path) -> Result<(), Box<dyn std::error::Error>> {
    if !evidence_directory.is_dir() {
        return Err("gpu-bench evidence directory must already exist".into());
    }
    if evidence_directory.read_dir()?.next().is_some() {
        return Err("gpu-bench evidence directory must be empty".into());
    }
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .ok_or("cannot resolve repository root")?
        .canonicalize()?;
    if evidence_directory.canonicalize()?.starts_with(repository) {
        return Err("raw GPU evidence must be outside the Git repository".into());
    }

    const WARMUP: u32 = 20;
    const ITERATIONS: u32 = 200;
    let constants = ModelConstants::default();
    let n = constants.local_gate_up_rows as usize;
    let k = constants.hidden as usize;
    let max_rows = usize::try_from(
        *profile::PROFILE_PREFILL_ROWS
            .last()
            .ok_or("no prefill row bucket")?,
    )?;
    let row_buckets: Vec<usize> = profile::PROFILE_DECODE_ROWS
        .into_iter()
        .chain(profile::PROFILE_PREFILL_ROWS)
        .map(|rows| usize::try_from(rows).expect("profile rows fit usize"))
        .collect();
    let fixture = generate_numerical_fixture(NumericalCase::DeterministicRandom, max_rows, n, k)?;
    let packed = PackedNvfp4::pack(&fixture.weights, n, k, Codec::OneDimensional)?;
    let device = glm_cuda::NativeFc1Fixture::replicated(&packed, &[0])?;

    for &rows in &row_buckets {
        let activation = &fixture.activations[..rows * k];
        let activation_bf16 = to_bf16_bits(activation);
        let routes = generate_routes(RoutingCase::OneHotExpert0, rows)?;
        let compacted = compact_routes(&routes, rows)?;
        let route_experts: Vec<u16> = compacted.iter().map(|route| route.expert).collect();
        let route_tokens: Vec<u32> = compacted.iter().map(|route| route.token).collect();
        let route_slots: Vec<u8> = compacted.iter().map(|route| route.slot).collect();
        let timing = device.benchmark(
            &activation_bf16,
            u32::try_from(rows)?,
            &route_experts,
            &route_tokens,
            &route_slots,
            glm_cuda::Fc1BenchmarkConfig {
                warmup_iterations: WARMUP,
                measured_iterations: ITERATIONS,
            },
        )?;
        let output = device.run(
            &activation_bf16,
            u32::try_from(rows)?,
            &route_experts,
            &route_tokens,
            &route_slots,
        )?;
        let report = GpuBenchmarkCase {
            schema: "glmaxx.sm120-fc1-benchmark-case.v2",
            kernel_abi: KERNEL_ABI,
            backend: "direct-nvfp4-cuda-core-baseline",
            rows,
            assignments: compacted.len(),
            routing: RoutingCase::OneHotExpert0.id(),
            packed_weight_sha256: packed_hash(&packed),
            output_sha256: u16_hash(&output),
            warmup_iterations: timing.warmup_iterations,
            measured_iterations: timing.measured_iterations,
            activation_quantization: profile::LatencyDistribution::from_microseconds(
                timing.activation_quantization_samples_us,
            )?,
            core_swiglu: profile::LatencyDistribution::from_microseconds(
                timing.core_swiglu_samples_us,
            )?,
            inclusive_operator: profile::LatencyDistribution::from_microseconds(
                timing.inclusive_operator_samples_us,
            )?,
            graph_inclusive: profile::LatencyDistribution::from_microseconds(
                timing.graph_inclusive_samples_us,
            )?,
            host_enqueue: profile::LatencyDistribution::from_microseconds(
                timing.host_enqueue_samples_us,
            )?,
            route_compaction: "CPU fixture control outside timed CUDA boundary",
            runtime_weight_repack_bytes: 0,
            persistent_dequant_bytes: 0,
        };
        fs::write(
            evidence_directory.join(format!("direct-m{rows:03}.json")),
            serde_json::to_vec_pretty(&report)?,
        )?;
    }

    let summary = GpuBenchmarkSummary {
        schema: "glmaxx.sm120-fc1-benchmark-summary.v2",
        kernel_abi: KERNEL_ABI,
        backend: "direct-nvfp4-cuda-core-baseline",
        cases: row_buckets.len(),
        warmup_iterations: WARMUP,
        measured_iterations: ITERATIONS,
        evidence_directory: evidence_directory.display().to_string(),
        verdict: "PROVISIONAL_CONTROL_ONLY",
    };
    let summary_bytes = serde_json::to_vec_pretty(&summary)?;
    fs::write(evidence_directory.join("summary.json"), &summary_bytes)?;
    println!("{}", String::from_utf8(summary_bytes)?);
    Ok(())
}

#[cfg(feature = "cuda-ffi")]
#[allow(clippy::too_many_arguments)]
fn execute_gpu_case(
    device: &glm_cuda::NativeFc1Fixture,
    suite: &'static str,
    rows: usize,
    routing: RoutingCase,
    numerical: NumericalCase,
    activation_bf16: &[u16],
    reference: &[f32],
    packed: &PackedNvfp4,
    compacted: &[glm_reference::CompactedRoute],
    repeat_count: usize,
) -> Result<GpuCaseReport, Box<dyn std::error::Error>> {
    let route_experts: Vec<u16> = compacted.iter().map(|route| route.expert).collect();
    let route_tokens: Vec<u32> = compacted.iter().map(|route| route.token).collect();
    let route_slots: Vec<u8> = compacted.iter().map(|route| route.slot).collect();
    let rows_u32 = u32::try_from(rows)?;
    let first = device.run(
        activation_bf16,
        rows_u32,
        &route_experts,
        &route_tokens,
        &route_slots,
    )?;
    let mut eager_bitwise_deterministic = true;
    for _ in 1..repeat_count {
        let repeated = device.run(
            activation_bf16,
            rows_u32,
            &route_experts,
            &route_tokens,
            &route_slots,
        )?;
        eager_bitwise_deterministic &= repeated == first;
    }
    let local_intermediate = ModelConstants::default().local_intermediate as usize;
    let mut maximum_absolute = 0.0_f32;
    let mut maximum_relative = 0.0_f32;
    let mut finite_errors = true;
    let mut failures = Vec::new();
    for (assignment, route) in compacted.iter().enumerate() {
        let token = usize::try_from(route.token)?;
        for column in 0..local_intermediate {
            let reference_value = reference[token * local_intermediate + column];
            let actual_bits = first[assignment * local_intermediate + column];
            let actual = f32::from_bits(u32::from(actual_bits) << 16);
            let absolute = (reference_value - actual).abs();
            let relative = absolute / reference_value.abs().max(1.0e-6);
            if absolute.is_finite() && relative.is_finite() {
                maximum_absolute = maximum_absolute.max(absolute);
                maximum_relative = maximum_relative.max(relative);
            } else {
                finite_errors = false;
            }
            let reason = if !reference_value.is_finite() {
                Some("non-finite CPU reference")
            } else if !actual.is_finite() {
                Some("non-finite GPU output")
            } else if absolute > 0.5 + 0.02 * reference_value.abs() {
                Some("element tolerance exceeded")
            } else {
                None
            };
            if let Some(reason) = reason {
                failures.push(GpuFailure {
                    assignment,
                    column,
                    reference_f32_bits: reference_value.to_bits(),
                    actual_bf16_bits: actual_bits,
                    reason,
                });
            }
        }
    }
    if !eager_bitwise_deterministic {
        failures.push(GpuFailure {
            assignment: usize::MAX,
            column: usize::MAX,
            reference_f32_bits: 0,
            actual_bf16_bits: 0,
            reason: "eager repeat output was not bitwise deterministic",
        });
    }
    Ok(GpuCaseReport {
        schema: "glmaxx.sm120-fc1-case-result.v1",
        suite,
        rows,
        routing: routing.id(),
        numerical: numerical.id(),
        assignments: compacted.len(),
        packed_weight_sha256: packed_hash(packed),
        output_sha256: u16_hash(&first),
        tolerance: "finite(gpu) and abs(gpu-cpu) <= 0.5 + 0.02 * abs(cpu)",
        maximum_absolute_error: finite_errors.then_some(maximum_absolute),
        maximum_relative_error: finite_errors.then_some(maximum_relative),
        eager_repeat_count: repeat_count,
        eager_bitwise_deterministic,
        failures,
    })
}

#[cfg(feature = "cuda-ffi")]
#[allow(clippy::too_many_arguments)]
fn execute_dense_control_case(
    device: &glm_cuda::NativeFc1Fixture,
    rows: usize,
    activation_bf16: &[u16],
    reference: &[f32],
    packed: &PackedNvfp4,
    compacted: &[glm_reference::CompactedRoute],
    repeat_count: usize,
) -> Result<GpuCaseReport, Box<dyn std::error::Error>> {
    let route_experts: Vec<u16> = compacted.iter().map(|route| route.expert).collect();
    let route_tokens: Vec<u32> = compacted.iter().map(|route| route.token).collect();
    let route_slots: Vec<u8> = compacted.iter().map(|route| route.slot).collect();
    let rows_u32 = u32::try_from(rows)?;
    let first = device.run_dense_control(
        activation_bf16,
        rows_u32,
        &route_experts,
        &route_tokens,
        &route_slots,
    )?;
    let mut eager_bitwise_deterministic = true;
    for _ in 1..repeat_count {
        let repeated = device.run_dense_control(
            activation_bf16,
            rows_u32,
            &route_experts,
            &route_tokens,
            &route_slots,
        )?;
        eager_bitwise_deterministic &= repeated == first;
    }
    let local_intermediate = ModelConstants::default().local_intermediate as usize;
    let mut maximum_absolute = 0.0_f32;
    let mut maximum_relative = 0.0_f32;
    let mut finite_errors = true;
    let mut failures = Vec::new();
    for (assignment, route) in compacted.iter().enumerate() {
        let token = usize::try_from(route.token)?;
        for column in 0..local_intermediate {
            let reference_value = reference[token * local_intermediate + column];
            let actual_bits = first[assignment * local_intermediate + column];
            let actual = f32::from_bits(u32::from(actual_bits) << 16);
            let absolute = (reference_value - actual).abs();
            let relative = absolute / reference_value.abs().max(1.0e-6);
            if absolute.is_finite() && relative.is_finite() {
                maximum_absolute = maximum_absolute.max(absolute);
                maximum_relative = maximum_relative.max(relative);
            } else {
                finite_errors = false;
            }
            let reason = if !reference_value.is_finite() {
                Some("non-finite CPU reference")
            } else if !actual.is_finite() {
                Some("non-finite GPU output")
            } else if absolute > 0.5 + 0.02 * reference_value.abs() {
                Some("element tolerance exceeded")
            } else {
                None
            };
            if let Some(reason) = reason {
                failures.push(GpuFailure {
                    assignment,
                    column,
                    reference_f32_bits: reference_value.to_bits(),
                    actual_bf16_bits: actual_bits,
                    reason,
                });
            }
        }
    }
    if !eager_bitwise_deterministic {
        failures.push(GpuFailure {
            assignment: usize::MAX,
            column: usize::MAX,
            reference_f32_bits: 0,
            actual_bf16_bits: 0,
            reason: "dense control repeat output was not bitwise deterministic",
        });
    }
    Ok(GpuCaseReport {
        schema: "glmaxx.sm120-fc1-case-result.v1",
        suite: "dense-control",
        rows,
        routing: RoutingCase::OneHotExpert0.id(),
        numerical: NumericalCase::DeterministicRandom.id(),
        assignments: compacted.len(),
        packed_weight_sha256: packed_hash(packed),
        output_sha256: u16_hash(&first),
        tolerance: "finite(gpu) and abs(gpu-cpu) <= 0.5 + 0.02 * abs(cpu)",
        maximum_absolute_error: finite_errors.then_some(maximum_absolute),
        maximum_relative_error: finite_errors.then_some(maximum_relative),
        eager_repeat_count: repeat_count,
        eager_bitwise_deterministic,
        failures,
    })
}

#[cfg(feature = "cuda-ffi")]
#[allow(clippy::too_many_arguments)]
fn execute_grouped_control_case(
    device: &glm_cuda::NativeFc1Fixture,
    rows: usize,
    routing: RoutingCase,
    activation_bf16: &[u16],
    reference: &[f32],
    packed: &PackedNvfp4,
    compacted: &[glm_reference::CompactedRoute],
    repeat_count: usize,
) -> Result<GpuCaseReport, Box<dyn std::error::Error>> {
    let route_experts: Vec<u16> = compacted.iter().map(|route| route.expert).collect();
    let route_tokens: Vec<u32> = compacted.iter().map(|route| route.token).collect();
    let route_slots: Vec<u8> = compacted.iter().map(|route| route.slot).collect();
    let rows_u32 = u32::try_from(rows)?;
    let first = device.run_grouped_control(
        activation_bf16,
        rows_u32,
        &route_experts,
        &route_tokens,
        &route_slots,
    )?;
    let mut eager_bitwise_deterministic = true;
    for _ in 1..repeat_count {
        let repeated = device.run_grouped_control(
            activation_bf16,
            rows_u32,
            &route_experts,
            &route_tokens,
            &route_slots,
        )?;
        eager_bitwise_deterministic &= repeated == first;
    }
    let local_intermediate = ModelConstants::default().local_intermediate as usize;
    let mut maximum_absolute = 0.0_f32;
    let mut maximum_relative = 0.0_f32;
    let mut finite_errors = true;
    let mut failures = Vec::new();
    for (assignment, route) in compacted.iter().enumerate() {
        let token = usize::try_from(route.token)?;
        for column in 0..local_intermediate {
            let reference_value = reference[token * local_intermediate + column];
            let actual_bits = first[assignment * local_intermediate + column];
            let actual = f32::from_bits(u32::from(actual_bits) << 16);
            let absolute = (reference_value - actual).abs();
            let relative = absolute / reference_value.abs().max(1.0e-6);
            if absolute.is_finite() && relative.is_finite() {
                maximum_absolute = maximum_absolute.max(absolute);
                maximum_relative = maximum_relative.max(relative);
            } else {
                finite_errors = false;
            }
            let reason = if !reference_value.is_finite() {
                Some("non-finite CPU reference")
            } else if !actual.is_finite() {
                Some("non-finite GPU output")
            } else if absolute > 0.5 + 0.02 * reference_value.abs() {
                Some("element tolerance exceeded")
            } else {
                None
            };
            if let Some(reason) = reason {
                failures.push(GpuFailure {
                    assignment,
                    column,
                    reference_f32_bits: reference_value.to_bits(),
                    actual_bf16_bits: actual_bits,
                    reason,
                });
            }
        }
    }
    if !eager_bitwise_deterministic {
        failures.push(GpuFailure {
            assignment: usize::MAX,
            column: usize::MAX,
            reference_f32_bits: 0,
            actual_bf16_bits: 0,
            reason: "grouped control repeat output was not bitwise deterministic",
        });
    }
    Ok(GpuCaseReport {
        schema: "glmaxx.sm120-fc1-case-result.v1",
        suite: "grouped-control",
        rows,
        routing: routing.id(),
        numerical: NumericalCase::DeterministicRandom.id(),
        assignments: compacted.len(),
        packed_weight_sha256: packed_hash(packed),
        output_sha256: u16_hash(&first),
        tolerance: "finite(gpu) and abs(gpu-cpu) <= 0.5 + 0.02 * abs(cpu)",
        maximum_absolute_error: finite_errors.then_some(maximum_absolute),
        maximum_relative_error: finite_errors.then_some(maximum_relative),
        eager_repeat_count: repeat_count,
        eager_bitwise_deterministic,
        failures,
    })
}

#[cfg(feature = "cuda-ffi")]
fn validate_empty_external_gpu_directory(
    evidence_directory: &Path,
    command: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if !evidence_directory.is_dir() {
        return Err(format!("{command} evidence directory must already exist").into());
    }
    if evidence_directory.read_dir()?.next().is_some() {
        return Err(format!("{command} evidence directory must be empty").into());
    }
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .ok_or("cannot resolve repository root")?
        .canonicalize()?;
    if evidence_directory.canonicalize()?.starts_with(repository) {
        return Err("raw GPU evidence must be outside the Git repository".into());
    }
    Ok(())
}

fn validate_external_profile_evidence_root(
    evidence_root: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    if !evidence_root.is_dir() {
        return Err("profile evidence root must already be a directory".into());
    }
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .ok_or("cannot resolve repository root")?
        .canonicalize()?;
    if evidence_root.canonicalize()?.starts_with(repository) {
        return Err("profile evidence root must be outside the Git repository".into());
    }
    Ok(())
}

#[cfg(feature = "cuda-ffi")]
fn write_gpu_case(
    evidence_directory: &Path,
    report: &GpuCaseReport,
) -> Result<(), Box<dyn std::error::Error>> {
    let filename = format!(
        "{}-m{:03}-{}-{}.json",
        report.suite, report.rows, report.routing, report.numerical
    );
    fs::write(
        evidence_directory.join(filename),
        serde_json::to_vec_pretty(report)?,
    )?;
    Ok(())
}

#[cfg(feature = "cuda-ffi")]
fn to_bf16_bits(values: &[f32]) -> Vec<u16> {
    values
        .iter()
        .map(|&value| (bf16_round(value).to_bits() >> 16) as u16)
        .collect()
}

#[derive(Serialize)]
struct CpuProof {
    schema: &'static str,
    model_shape: [usize; 2],
    codec: &'static str,
    value_bytes: usize,
    scale_bytes: usize,
    metadata_bytes: usize,
    fixture_sha256: String,
    zero_is_canonical: bool,
    reconstruction_finite: bool,
    deterministic_repack: bool,
    gpu_evidence: &'static str,
}

fn cpu_proof() -> Result<(), Box<dyn std::error::Error>> {
    let constants = ModelConstants::default();
    let n = constants.local_gate_up_rows as usize;
    let k = constants.hidden as usize;
    let input = actual_shape_values(n, k);
    let packed = PackedNvfp4::pack(&input, n, k, Codec::OneDimensional)?;
    let second = PackedNvfp4::pack(&input, n, k, Codec::OneDimensional)?;
    let reconstruction = packed.dequantize()?;
    let zero = PackedNvfp4::pack(&vec![0.0; 128 * 64], 128, 64, Codec::OneDimensional)?;
    let fixture_sha256 = packed_hash(&packed);
    if fixture_sha256 != ACTUAL_PACKED_SHA256 {
        return Err(
            format!("actual-shape packed fixture digest changed to {fixture_sha256}").into(),
        );
    }
    let report = CpuProof {
        schema: "glmaxx.cpu-proof.v1",
        model_shape: [n, k],
        codec: "sm120-nvfp4-1d-block16-direct-v1",
        value_bytes: packed.values.len(),
        scale_bytes: packed.scales.len(),
        metadata_bytes: 128,
        fixture_sha256,
        zero_is_canonical: zero.values.iter().all(|&value| value == 0)
            && zero.scales.iter().all(|&value| value == 0)
            && zero.metadata.global_scale == 1.0,
        reconstruction_finite: reconstruction.iter().all(|value| value.is_finite()),
        deterministic_repack: packed == second,
        gpu_evidence: "none: Phase A is CPU-only and no CUDA toolchain is present",
    };
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

fn pack_actual(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let constants = ModelConstants::default();
    let n = constants.local_gate_up_rows as usize;
    let k = constants.hidden as usize;
    let packed = PackedNvfp4::pack(&actual_shape_values(n, k), n, k, Codec::OneDimensional)?;
    let builders: [RankFileBuilder; 4] =
        std::array::from_fn(|rank| make_builder(u32::try_from(rank).unwrap(), packed.clone()));
    let conversion_uuid = RankFileBuilder::derive_conversion_uuid(&builders)?;
    let bytes = builders[0].build(conversion_uuid)?;
    let rank_sha256 = hex(&sha256(&bytes));
    if rank_sha256 != ACTUAL_RANK0_SHA256 {
        return Err(format!("actual-shape rank-file digest changed to {rank_sha256}").into());
    }
    fs::write(path, &bytes)?;
    let parsed = RankFile::read(bytes)?;
    println!(
        "rank={} tensor={} fixture_sha256={} conversion_uuid={}",
        parsed.rank,
        parsed.tensor_name(0)?,
        packed_hash(&packed),
        hex(&parsed.conversion_uuid)
    );
    Ok(())
}

fn make_builder(rank: u32, packed: PackedNvfp4) -> RankFileBuilder {
    let manifest = format!(
        "{{\"kernel_abi\":\"{KERNEL_ABI}\",\"profile\":\"nvfp4-laboratory\",\"rank\":{rank},\"schema\":\"glmaxx.rank-fixture.v1\"}}"
    );
    RankFileBuilder {
        rank,
        manifest: manifest.into_bytes(),
        model_config_sha256: sha256(b"zai-org/GLM-5.2 config placeholder: fixture only"),
        tokenizer_bundle_sha256: sha256(b"fixture-no-tokenizer"),
        chat_template_sha256: sha256(b"fixture-no-template"),
        weight_policy_sha256: sha256(b"nvfp4-laboratory-fc1-only-v1"),
        kernel_abi_sha256: sha256(KERNEL_ABI.as_bytes()),
        tensors: vec![TensorRecord {
            tensor_id: 0,
            name: "model.layers.3.mlp.experts.0.gate_up_proj.weight".into(),
            role_id: 0x0501,
            layer_id: 3,
            expert_id: 0,
            tp_shard_axis: 0,
            flags: 0b0000_1010,
            payload: TensorPayload::Nvfp4(packed),
        }],
    }
}

fn inspect(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let file = RankFile::read(fs::read(path)?)?;
    println!(
        "rank={} tensors={} file_uuid={} conversion_uuid={}",
        file.rank,
        file.descriptors.len(),
        hex(&file.file_uuid),
        hex(&file.conversion_uuid)
    );
    for index in 0..file.descriptors.len() {
        let descriptor = &file.descriptors[index];
        println!(
            "tensor={} name={} logical={:?} padded={:?} values={} scales={}",
            descriptor.tensor_id,
            file.tensor_name(index)?,
            descriptor.logical_shape,
            descriptor.padded_shape,
            descriptor.payload_bytes,
            descriptor.aux_bytes
        );
    }
    Ok(())
}

fn print_budget() -> Result<(), Box<dyn std::error::Error>> {
    let capacity = CacheCapacity::at_positions(MODEL_POSITIONS, true)?;
    let per_rank = capacity.total()? / 4;
    let report = serde_json::json!({
        "schema": "glmaxx.cache-budget.v1",
        "positions": MODEL_POSITIONS,
        "target_kv_bytes": capacity.target_kv_bytes,
        "draft_kv_bytes": capacity.draft_kv_bytes,
        "indexer_key_bytes": capacity.indexer_key_bytes,
        "draft_indexer_key_bytes": capacity.draft_indexer_key_bytes,
        "aggregate_bytes": capacity.total()?,
        "per_rank_bytes": per_rank,
        "explicit_budget_terms": std::mem::size_of::<Budget>(),
    });
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

fn abi_check() -> Result<(), Box<dyn std::error::Error>> {
    let rows = 128;
    let assignments = rows * 8;
    let descriptor = Fc1Descriptor::new(LaunchGeometry {
        rows,
        assignments,
        path: KernelPath::DecodePersistent,
    });
    #[cfg(feature = "cuda-ffi")]
    glm_cuda::validate_native_moe_abi(rows, assignments)?;
    #[cfg(feature = "cuda-ffi")]
    glm_cuda::validate_native_exl3_abi(1, 6_144, 512)?;
    let native_abi_verified = cfg!(feature = "cuda-ffi");
    let reason = if native_abi_verified {
        "native library linked; ABI and workspace formula verified without a GPU launch"
    } else {
        "CUDA FFI not linked; build with --features cuda-ffi and GLMAXX_KERNEL_LIB_DIR"
    };
    let report = serde_json::json!({
        "kernel_abi": KERNEL_ABI,
        "descriptor_bytes": std::mem::size_of::<Fc1Descriptor>(),
        "descriptor_alignment": std::mem::align_of::<Fc1Descriptor>(),
        "fc2_descriptor_bytes": std::mem::size_of::<Fc2Descriptor>(),
        "fc2_descriptor_alignment": std::mem::align_of::<Fc2Descriptor>(),
        "exl3_kernel_abi": EXL3_KERNEL_ABI,
        "exl3_descriptor_bytes": std::mem::size_of::<Exl3Descriptor>(),
        "exl3_descriptor_alignment": std::mem::align_of::<Exl3Descriptor>(),
        "exl3_gate_m1_workspace_bytes": exl3_workspace_bytes(1, 6_144, 512)?,
        "m128_workspace_bytes": workspace_bytes(assignments)?,
        "m128_fc2_workspace_bytes": fc2_workspace_bytes(rows, assignments)?,
        "m128_grouped_fc2_workspace_bytes": fc2_grouped_workspace_bytes(rows, assignments)?,
        "cuda_ffi_feature": cfg!(feature = "cuda-ffi"),
        "native_abi_verified": native_abi_verified,
        "gpu_launched": false,
        "reason": reason,
    });
    let _ = descriptor;
    let _ = Exl3Descriptor::new(1, Exl3KernelProjection::Gate);
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

#[derive(Serialize)]
struct EngineProof {
    schema: &'static str,
    step_plan_abi: &'static str,
    step_plan_record_bytes: usize,
    plan_hash: String,
    collective_schedule_hash: String,
    collective_operations: usize,
    graph_profile_hash: String,
    graph_entries: usize,
    full_context_positions: u64,
    system_memory_plan: SystemMemoryPlan,
    memory_evidence: &'static str,
    mixed_mode_posture: &'static str,
    gpu_evidence: &'static str,
}

fn engine_proof() -> Result<EngineProof, Box<dyn std::error::Error>> {
    let schedule = CollectiveSchedule::new(vec![
        CollectiveOp {
            ordinal: 0,
            kind: CollectiveKind::DcpQueryGather,
            route_id: 3,
            payload_bytes: 32_768,
            participant_mask: TP_RANK_MASK,
        },
        CollectiveOp {
            ordinal: 1,
            kind: CollectiveKind::DcpPartialStateReturn,
            route_id: 4,
            payload_bytes: 98_304,
            participant_mask: TP_RANK_MASK,
        },
        CollectiveOp {
            ordinal: 2,
            kind: CollectiveKind::TpReduce,
            route_id: 9,
            payload_bytes: 98_304,
            participant_mask: TP_RANK_MASK,
        },
        CollectiveOp {
            ordinal: 3,
            kind: CollectiveKind::LogitsArgmax,
            route_id: 12,
            payload_bytes: 128,
            participant_mask: TP_RANK_MASK,
        },
    ])?;
    let plan = StepPlan::build(
        StepPlanRequest {
            epoch: 7,
            step_id: 42,
            mode: StepMode::Decode,
            active_sequences: 8,
            sequence_bucket: 8,
            scheduled_prompt_tokens: 0,
            query_rows: 8,
            verifier_row_bucket: 8,
            mtp_depth: 0,
            graph_id: 11,
            tp_route_id: 9,
            dcp_route_id: 3,
            attention_transport: AttentionTransport::DecodeQueryLse,
            sampling_route_id: 12,
            sequence_table_generation: 99,
        },
        &schedule,
    )?;
    let profile = GraphProfile::new(vec![GraphEntry {
        graph_id: 11,
        key: GraphKey {
            mode: StepMode::Decode,
            sequence_bucket: 8,
            verifier_row_bucket: 8,
            mtp_depth: 0,
            attention_transport: AttentionTransport::DecodeQueryLse,
        },
        maximum_active_sequences: 8,
        maximum_prompt_tokens: 0,
        maximum_query_rows: 8,
        compatible_tp_routes: vec![9],
        compatible_dcp_routes: vec![3, 4],
        compatible_sampling_routes: vec![12],
        maximum_scratch_bytes: 64 << 20,
        argument_bytes: 64 << 10,
        graph_object_bytes: 2 << 20,
        resident_module_bytes: 8 << 20,
        admission_slo_class: 1,
    }])?;
    profile.admit(&plan)?;

    let memory_inputs = (0..4)
        .map(|rank| RankMemoryInput {
            rank,
            profile: ProfileClass::HybridServe,
            mtp_enabled: true,
            // These are deliberately synthetic proof values. The production
            // artifact consumes measured cn4 free bytes and converted payload
            // bytes; this fixture only proves deterministic accounting.
            measured_usable_hbm_bytes: 95 * GIB,
            weight_bytes: 82 * GIB,
            module_and_context_bytes: GIB,
            graph_resident_bytes: 256 << 20,
            maximum_prefill_workspace_bytes: 512 << 20,
            maximum_verifier_workspace_bytes: 128 << 20,
            collective_bytes: 256 << 20,
            staging_bytes: 256 << 20,
            model_metadata_bytes: 64 << 20,
            page_table_bytes: 64 << 20,
            allocator_padding_bytes: 256 << 20,
            escrow_bytes: GIB,
            target_committed_slots: 262_144,
            target_page_slack_slots: MIN_PAGE_SLACK_SLOTS_PER_RANK,
            target_tentative_slots: MIN_MTP_TENTATIVE_SLOTS_PER_RANK,
            draft_committed_slots: 262_144,
            draft_page_slack_slots: MIN_PAGE_SLACK_SLOTS_PER_RANK,
            draft_tentative_slots: MIN_MTP_TENTATIVE_SLOTS_PER_RANK,
        })
        .collect();
    let system_memory_plan = plan_system_memory(memory_inputs)?;

    Ok(EngineProof {
        schema: "glmaxx.engine-contract-proof.v1",
        step_plan_abi: STEP_PLAN_ABI,
        step_plan_record_bytes: STEP_PLAN_RECORD_BYTES,
        plan_hash: hex(&plan.plan_hash),
        collective_schedule_hash: hex(&plan.collective_schedule_hash),
        collective_operations: schedule.operations().len(),
        graph_profile_hash: hex(&profile.profile_hash),
        graph_entries: profile.entries.len(),
        full_context_positions: MODEL_POSITIONS,
        system_memory_plan,
        memory_evidence: "synthetic accounting fixture; not measured cn4 HBM and not a converted weight policy",
        mixed_mode_posture: "fail-closed pending reviewed dual-attention transport contract",
        gpu_evidence: "none: deterministic CPU control-plane proof only",
    })
}

fn actual_shape_values(n: usize, k: usize) -> Vec<f32> {
    (0..n * k)
        .map(|index| {
            let mixed = (index as u64)
                .wrapping_mul(0x9e37_79b9_7f4a_7c15)
                .rotate_left(17);
            let signed = i32::try_from((mixed >> 48) & 0xffff).unwrap() - 32768;
            if index % 4093 == 0 {
                (signed as f32) / 128.0
            } else {
                (signed as f32) / 8192.0
            }
        })
        .collect()
}

fn packed_hash(packed: &PackedNvfp4) -> String {
    let mut hasher = Sha256::new();
    hasher.update(packed.metadata.encode());
    hasher.update(&packed.values);
    hasher.update(&packed.scales);
    hex(&hasher.finalize())
}

fn f32_hash(values: &[f32]) -> String {
    let mut hasher = Sha256::new();
    for value in values {
        hasher.update(value.to_bits().to_le_bytes());
    }
    hex(&hasher.finalize())
}

#[cfg(feature = "cuda-ffi")]
fn u16_hash(values: &[u16]) -> String {
    let mut hasher = Sha256::new();
    for value in values {
        hasher.update(value.to_le_bytes());
    }
    hex(&hasher.finalize())
}

fn sha256(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for &byte in bytes {
        output.push(char::from(DIGITS[usize::from(byte >> 4)]));
        output.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    output
}
