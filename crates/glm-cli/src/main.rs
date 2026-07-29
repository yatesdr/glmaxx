use std::collections::BTreeMap;
use std::env;
use std::fs::{self, File};
use std::io::Read;
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::str::FromStr;

use glm_cache::{
    Budget, CacheCapacity, DurablePageRequest, FileTierStore, MODEL_POSITIONS, NamespaceInputs,
    PagePieceBytes, PrefixIndex, PrefixNamespace, ResidencyConfig, TierPiece,
};
use glm_cuda::{
    Fc1Descriptor, Fc2Descriptor, KernelPath, LaunchGeometry, fc2_workspace_bytes, workspace_bytes,
};
use glm_engine::{
    AttentionTransport, CollectiveKind, CollectiveOp, CollectiveSchedule, CpuWorkerPool, GIB,
    GraphEntry, GraphKey, GraphProfile, ProfileBudgetArtifact, ProfileClass, RankMemoryInput,
    STEP_PLAN_ABI, STEP_PLAN_RECORD_BYTES, StepMode, StepPlan, StepPlanRequest, SystemMemoryPlan,
    TP_RANK_MASK, plan_system_memory,
};
use glm_format::{
    CUTLASS_COMMIT, Codec, EXL3_MODEL_REVISION, EXL3_SOURCE_REVISION, Exl3Metadata, Exl3Projection,
    Exl3Trellis, KERNEL_ABI, PINNED_EXL3_REPOSITORY, PINNED_SOURCE_MANIFEST_SHA256, PackedNvfp4,
    PinnedRankPlan, PinnedSourceVerification, RankFile, RankFileBuilder, SafeTensorFile,
    ShardedSafetensors, StreamingRankConfig, StreamingRankSet, StreamingRankSummary, TensorPayload,
    TensorRecord, pinned_exl3_rank_plan, pinned_exl3_weight_policy_sha256,
    validate_pinned_exl3_checkpoint, verify_pinned_source_files,
};
use glm_reference::{
    DECODE_ROWS, ModelConstants, NUMERICAL_CASES, PREFILL_ROWS, ROUTING_CASES, compact_routes,
    generate_numerical_fixture, generate_routes, operation_manifest_json,
};
#[cfg(feature = "cuda-ffi")]
use glm_reference::{NumericalCase, RoutingCase, bf16_round, routed_fc1_oracle};
use glm_scheduler::{
    RequestSpec, RequestState, RouteCatalog, SamplingCollective, SchedulerConfig, TenantConfig,
};
use glm_serving::{PrefixRestoreCoordinator, RequestEvent, ServingConfig, ServingCoordinator};
use serde::Serialize;
use sha2::{Digest, Sha256};

const ACTUAL_PACKED_SHA256: &str =
    "a84be06b6bf6192eb51324ee57a1b6a4c57924c78709bcbe275b9f56b547cab5";
const ACTUAL_RANK0_SHA256: &str =
    "ea706d83c4aa89fda26f977f03e7fa72862b71cf36c2c77cead70d68bc7b3093";
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
        _ => {
            return Err(
                "usage: glmaxx <manifest [path]|cpu-proof|matrix-proof [path]|pack-actual path|inspect path|budget|abi-check|engine-proof [path]|serving-proof evidence-dir|exl3-proof source-payload|safetensors-inventory file-or-index|exl3-safetensors-proof file-or-index layer expert rank gate|up|down|checkpoint-proof pinned-index|convert-pinned-exl3 pinned-index output-dir conversion-commit profile-budget-v0.json review-artifact|gpu-smoke [rows]|gpu-matrix evidence-dir|gpu-graph evidence-dir|gpu-dense-control evidence-dir|gpu-grouped-control evidence-dir|gpu-bench evidence-dir|gpu-grouped-bench evidence-dir>"
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
            sampling: SamplingCollective::Greedy,
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
        CpuWorkerPool::spawn(2, None)?,
    )?;
    serving.attach_prefix_cache(prefix)?;
    serving.admit_tokens(
        RequestSpec {
            id: 101,
            tenant: 1,
            prompt_tokens: 128,
            maximum_new_tokens: 4,
            mtp_depth: 0,
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
    activation_quantization_us: f32,
    core_swiglu_us: f32,
    inclusive_operator_us: f32,
    graph_inclusive_us: f32,
    host_enqueue_us: f64,
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
    activation_quantization_us: f32,
    grouped_core_swiglu_us: f32,
    inclusive_operator_us: f32,
    host_enqueue_us: f64,
    route_compaction: &'static str,
    grouped_metadata_preparation: &'static str,
    runtime_weight_repack_bytes: u64,
    persistent_dequant_bytes: u64,
    materialized_gate_up_control: bool,
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
    let max_rows = *PREFILL_ROWS.last().ok_or("no prefill row bucket")?;
    let row_buckets: Vec<usize> = DECODE_ROWS.into_iter().chain(PREFILL_ROWS).collect();
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
                schema: "glmaxx.sm120-fc1-grouped-benchmark-case.v1",
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
                activation_quantization_us: timing.activation_quantization_us,
                grouped_core_swiglu_us: timing.grouped_core_swiglu_us,
                inclusive_operator_us: timing.inclusive_operator_us,
                host_enqueue_us: timing.host_enqueue_us,
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
        schema: "glmaxx.sm120-fc1-grouped-benchmark-summary.v1",
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
    let max_rows = *PREFILL_ROWS.last().ok_or("no prefill row bucket")?;
    let row_buckets: Vec<usize> = DECODE_ROWS.into_iter().chain(PREFILL_ROWS).collect();
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
            schema: "glmaxx.sm120-fc1-benchmark-case.v1",
            kernel_abi: KERNEL_ABI,
            backend: "direct-nvfp4-cuda-core-baseline",
            rows,
            assignments: compacted.len(),
            routing: RoutingCase::OneHotExpert0.id(),
            packed_weight_sha256: packed_hash(&packed),
            output_sha256: u16_hash(&output),
            warmup_iterations: timing.warmup_iterations,
            measured_iterations: timing.measured_iterations,
            activation_quantization_us: timing.activation_quantization_us,
            core_swiglu_us: timing.core_swiglu_us,
            inclusive_operator_us: timing.inclusive_operator_us,
            graph_inclusive_us: timing.graph_inclusive_us,
            host_enqueue_us: timing.host_enqueue_us,
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
        schema: "glmaxx.sm120-fc1-benchmark-summary.v1",
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
        "m128_workspace_bytes": workspace_bytes(assignments)?,
        "m128_fc2_workspace_bytes": fc2_workspace_bytes(rows, assignments)?,
        "cuda_ffi_feature": cfg!(feature = "cuda-ffi"),
        "native_abi_verified": native_abi_verified,
        "gpu_launched": false,
        "reason": reason,
    });
    let _ = descriptor;
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
            target_slack_slots: 0,
            draft_committed_slots: 262_144,
            draft_tentative_slots: 448,
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
