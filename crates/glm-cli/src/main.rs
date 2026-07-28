use std::env;
use std::fs;
use std::path::Path;

use glm_cache::{Budget, CacheCapacity, MODEL_POSITIONS};
use glm_cuda::{Fc1Descriptor, KernelPath, LaunchGeometry, workspace_bytes};
use glm_format::{Codec, KERNEL_ABI, PackedNvfp4, RankFile, RankFileBuilder, TensorRecord};
use glm_reference::{ModelConstants, operation_manifest_json};
#[cfg(all(feature = "cuda-ffi", target_os = "linux"))]
use glm_reference::{bf16_round, routed_fc1_oracle};
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
        #[cfg(all(feature = "cuda-ffi", target_os = "linux"))]
        Some("gpu-smoke") => {
            let rows = arguments
                .get(2)
                .map(|value| value.parse::<u32>())
                .transpose()?
                .unwrap_or(1);
            gpu_smoke(rows)?;
        }
        _ => {
            return Err(
                "usage: glmaxx <manifest [path]|cpu-proof|pack-actual path|inspect path|budget|abi-check|gpu-smoke>"
                    .into(),
            );
        }
    }
    Ok(())
}

#[cfg(all(feature = "cuda-ffi", target_os = "linux"))]
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
    let report = serde_json::json!({
        "kernel_abi": KERNEL_ABI,
        "descriptor_bytes": std::mem::size_of::<Fc1Descriptor>(),
        "descriptor_alignment": std::mem::align_of::<Fc1Descriptor>(),
        "m128_workspace_bytes": workspace_bytes(assignments)?,
        "cuda_compiled": false,
        "reason": "Phase A host has no nvcc; run the pinned cn4 harness after authorization"
    });
    let _ = descriptor;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
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
