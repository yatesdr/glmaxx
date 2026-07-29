use std::env;
use std::fs;
use std::path::Path;

use glm_cache::{Budget, CacheCapacity, MODEL_POSITIONS};
use glm_cuda::{Fc1Descriptor, KernelPath, LaunchGeometry, workspace_bytes};
use glm_engine::{
    AttentionTransport, CollectiveKind, CollectiveOp, CollectiveSchedule, GIB, GraphEntry,
    GraphKey, GraphProfile, ProfileClass, RankMemoryInput, STEP_PLAN_ABI, STEP_PLAN_RECORD_BYTES,
    StepMode, StepPlan, StepPlanRequest, SystemMemoryPlan, TP_RANK_MASK, plan_system_memory,
};
use glm_format::{Codec, KERNEL_ABI, PackedNvfp4, RankFile, RankFileBuilder, TensorRecord};
use glm_reference::{
    DECODE_ROWS, ModelConstants, NUMERICAL_CASES, PREFILL_ROWS, ROUTING_CASES, compact_routes,
    generate_numerical_fixture, generate_routes, operation_manifest_json,
};
#[cfg(feature = "cuda-ffi")]
use glm_reference::{NumericalCase, RoutingCase, bf16_round, routed_fc1_oracle};
use serde::Serialize;
use sha2::{Digest, Sha256};

const ACTUAL_PACKED_SHA256: &str =
    "a84be06b6bf6192eb51324ee57a1b6a4c57924c78709bcbe275b9f56b547cab5";
const ACTUAL_RANK0_SHA256: &str =
    "ea706d83c4aa89fda26f977f03e7fa72862b71cf36c2c77cead70d68bc7b3093";

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
        _ => {
            return Err(
                "usage: glmaxx <manifest [path]|cpu-proof|matrix-proof [path]|pack-actual path|inspect path|budget|abi-check|engine-proof [path]|gpu-smoke [rows]|gpu-matrix evidence-dir>"
                    .into(),
            );
        }
    }
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
            packed,
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
    glm_cuda::validate_native_abi(assignments)?;
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
        "m128_workspace_bytes": workspace_bytes(assignments)?,
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
