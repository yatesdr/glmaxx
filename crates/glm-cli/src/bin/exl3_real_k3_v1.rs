//! Fail-closed qualification of the accepted scalar EXL3 K=3 source kernel
//! against one real GLM-5.2 TR3 safetensors projection.

use std::env;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;
use std::time::Instant;

use glm_cuda::{EXL3_KERNEL_ABI, Fc1BenchmarkConfig, NativeExl3Fixture};
use glm_format::{
    EXL3_MODEL_REVISION, EXL3_SOURCE_REVISION, EXL3_SOURCE_VERSION, Exl3Metadata, Exl3Projection,
    SafeTensorFile, f16_bits_to_f32, f32_to_f16_bits, load_exl3_projection,
};
use serde::Serialize;
use sha2::{Digest, Sha256};

const EXPECTED_KERNEL_ABI: &str = "glmaxx.sm120.exl3.source_projection.v1";
const WARMUP_ITERATIONS: u32 = 50;
const MEASURED_ITERATIONS: u32 = 1_000;
const REPLAY_COUNT: u32 = 3;
const ROW_COUNTS: [u32; 4] = [1, 2, 4, 8];

#[derive(Serialize)]
struct ComponentReport {
    name: String,
    dtype: &'static str,
    shape: Vec<u64>,
    bytes: u64,
    sha256: String,
}

#[derive(Serialize)]
struct TimingSummary {
    samples: usize,
    minimum_us: f64,
    p50_us: f64,
    p95_us: f64,
    p99_us: f64,
    maximum_us: f64,
    mean_us: f64,
}

#[derive(Serialize)]
struct CaseReport {
    rows: u32,
    input_values: usize,
    output_values: usize,
    input_sha256: String,
    cpu_output_sha256: String,
    gpu_output_sha256: String,
    maximum_absolute_error: f32,
    maximum_relative_error: f32,
    failed_elements: usize,
    repeat_count: u32,
    repeat_bitwise_deterministic: bool,
    cpu_reference_wall_us: u128,
    projection_timing: TimingSummary,
    host_enqueue_timing: TimingSummary,
}

#[derive(Serialize)]
struct QualificationReport {
    schema: &'static str,
    verdict: &'static str,
    performance_status: &'static str,
    model_revision: &'static str,
    source_revision: &'static str,
    source_version: &'static str,
    kernel_abi: &'static str,
    kernel_route: &'static str,
    source_kind: &'static str,
    source_file: String,
    source_file_bytes: u64,
    source_file_sha256: String,
    source_header_sha256: String,
    tensor_stem: String,
    projection: &'static str,
    layer: u16,
    expert: u16,
    rank: u8,
    logical_shape_k_n: [u32; 2],
    bits: u8,
    components: Vec<ComponentReport>,
    source_payload_bytes: usize,
    source_payload_sha256: String,
    native_metadata_sha256: String,
    trellis_sha256: String,
    suh_sha256: String,
    svh_sha256: String,
    gpu_uploaded_packed_weight_bytes: usize,
    gpu_runtime_weight_repack_bytes: u64,
    gpu_persistent_reconstructed_weight_bytes: u64,
    cpu_reference_reconstructed_weight_bytes: u64,
    benchmark_warmup_iterations: u32,
    benchmark_measured_iterations: u32,
    timing_percentile_method: &'static str,
    tolerance: &'static str,
    cases: Vec<CaseReport>,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("glmaxx-exl3-real-k3-v1: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let arguments: Vec<String> = env::args().collect();
    if arguments.len() != 7 {
        return Err(
            "usage: glmaxx-exl3-real-k3-v1 SHARD EXPECTED_SHA256 LAYER EXPERT RANK gate|up|down"
                .into(),
        );
    }
    let shard = Path::new(&arguments[1]);
    let expected_file_sha256 = &arguments[2];
    if expected_file_sha256.len() != 64
        || !expected_file_sha256
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err("EXPECTED_SHA256 must be 64 lowercase hexadecimal characters".into());
    }
    if shard.extension().and_then(|value| value.to_str()) != Some("safetensors") {
        return Err("SHARD must be one explicit .safetensors file".into());
    }
    if EXL3_KERNEL_ABI != EXPECTED_KERNEL_ABI {
        return Err(format!(
            "refusing non-scalar route: expected {EXPECTED_KERNEL_ABI}, found {EXL3_KERNEL_ABI}"
        )
        .into());
    }

    let layer: u16 = arguments[3].parse()?;
    let expert: u16 = arguments[4].parse()?;
    let rank: u8 = arguments[5].parse()?;
    let (projection, projection_name, logical_k, logical_n) = match arguments[6].as_str() {
        "gate" => (Exl3Projection::Gate, "gate", 6_144_u32, 512_u32),
        "up" => (Exl3Projection::Up, "up", 6_144_u32, 512_u32),
        "down" => (Exl3Projection::Down, "down", 512_u32, 6_144_u32),
        _ => return Err("projection must be gate, up, or down".into()),
    };

    let source_file_sha256 = hash_file(shard)?;
    if source_file_sha256 != *expected_file_sha256 {
        return Err(format!(
            "source shard SHA-256 mismatch: expected {expected_file_sha256}, found {source_file_sha256}"
        )
        .into());
    }

    let metadata = Exl3Metadata::new(projection, layer, expert, rank, 3, logical_k, logical_n)?;
    let stem =
        format!("model.layers.{layer}.mlp.experts.{expert}.{projection_name}_proj.rank{rank}");
    let component_names = [
        format!("{stem}.mcg"),
        format!("{stem}.suh"),
        format!("{stem}.svh"),
        format!("{stem}.trellis"),
    ];
    let source = SafeTensorFile::open(shard)?;
    let mut components = Vec::with_capacity(component_names.len());
    let mut source_payload = Vec::new();
    for name in &component_names {
        let descriptor = source
            .tensor(name)
            .ok_or_else(|| format!("missing component {name}"))?;
        let bytes = source.read_tensor(name)?;
        components.push(ComponentReport {
            name: name.clone(),
            dtype: descriptor.dtype.name(),
            shape: descriptor.shape.clone(),
            bytes: descriptor.bytes,
            sha256: hash_bytes(&bytes),
        });
        source_payload.extend_from_slice(&bytes);
    }
    let tensor = load_exl3_projection(&source, &stem, metadata)?;
    if tensor.metadata.bits != 3 {
        return Err("K=3 qualifier loaded a non-K=3 tensor".into());
    }

    let fixture = NativeExl3Fixture::from_source(&tensor)?;
    let mut cases = Vec::with_capacity(ROW_COUNTS.len());
    for rows in ROW_COUNTS {
        let input_f16 =
            deterministic_input(rows, logical_k, layer, expert, rank, projection as u8)?;
        let cpu_started = Instant::now();
        let reference = tensor.matmul_reference_f16(&input_f16, usize::try_from(rows)?)?;
        let cpu_reference_wall_us = cpu_started.elapsed().as_micros();
        let replay = fixture.run_repeated(&input_f16, rows, REPLAY_COUNT)?;
        let (maximum_absolute_error, maximum_relative_error, failed_elements) =
            compare_f16_output(&reference, &replay.output_f16);
        if failed_elements != 0 || !replay.bitwise_deterministic {
            return Err(format!(
                "correctness gate failed at rows={rows}: failures={failed_elements}, deterministic={}",
                replay.bitwise_deterministic
            )
            .into());
        }
        let timing = fixture.benchmark(
            &input_f16,
            rows,
            Fc1BenchmarkConfig {
                warmup_iterations: WARMUP_ITERATIONS,
                measured_iterations: MEASURED_ITERATIONS,
            },
        )?;
        cases.push(CaseReport {
            rows,
            input_values: input_f16.len(),
            output_values: reference.len(),
            input_sha256: hash_words(&input_f16),
            cpu_output_sha256: hash_words(&reference),
            gpu_output_sha256: hash_words(&replay.output_f16),
            maximum_absolute_error,
            maximum_relative_error,
            failed_elements,
            repeat_count: replay.repeat_count,
            repeat_bitwise_deterministic: replay.bitwise_deterministic,
            cpu_reference_wall_us,
            projection_timing: summarize(&timing.projection_samples_us)?,
            host_enqueue_timing: summarize(&timing.host_enqueue_samples_us)?,
        });
    }

    let native_metadata = tensor.metadata.encode();
    let gpu_uploaded_packed_weight_bytes = tensor
        .trellis
        .len()
        .checked_add(tensor.suh.len())
        .and_then(|words| words.checked_add(tensor.svh.len()))
        .and_then(|words| words.checked_mul(2))
        .ok_or("GPU upload byte count overflow")?;
    let cpu_reference_reconstructed_weight_bytes = u64::from(logical_k)
        .checked_mul(u64::from(logical_n))
        .and_then(|values| values.checked_mul(2))
        .ok_or("reconstructed byte count overflow")?;
    let report = QualificationReport {
        schema: "glmaxx.sm120-exl3-real-k3-qualification.v1",
        verdict: "passed",
        performance_status: "scalar correctness control; not an optimized-route claim",
        model_revision: EXL3_MODEL_REVISION,
        source_revision: EXL3_SOURCE_REVISION,
        source_version: EXL3_SOURCE_VERSION,
        kernel_abi: EXL3_KERNEL_ABI,
        kernel_route: "accepted scalar source projection v1",
        source_kind: "single-file safetensors",
        source_file: shard.display().to_string(),
        source_file_bytes: source.file_bytes(),
        source_file_sha256,
        source_header_sha256: hex(&source.header_sha256()),
        tensor_stem: stem,
        projection: projection_name,
        layer,
        expert,
        rank,
        logical_shape_k_n: [logical_k, logical_n],
        bits: tensor.metadata.bits,
        components,
        source_payload_bytes: source_payload.len(),
        source_payload_sha256: hash_bytes(&source_payload),
        native_metadata_sha256: hash_bytes(&native_metadata),
        trellis_sha256: hash_words(&tensor.trellis),
        suh_sha256: hash_words(&tensor.suh),
        svh_sha256: hash_words(&tensor.svh),
        gpu_uploaded_packed_weight_bytes,
        gpu_runtime_weight_repack_bytes: 0,
        gpu_persistent_reconstructed_weight_bytes: 0,
        cpu_reference_reconstructed_weight_bytes,
        benchmark_warmup_iterations: WARMUP_ITERATIONS,
        benchmark_measured_iterations: MEASURED_ITERATIONS,
        timing_percentile_method: "nearest-rank over sorted CUDA-event microseconds",
        tolerance: "finite(gpu) and abs(gpu-cpu) <= 0.5 + 0.03 * abs(cpu)",
        cases,
    };
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

fn deterministic_input(
    rows: u32,
    logical_k: u32,
    layer: u16,
    expert: u16,
    rank: u8,
    projection: u8,
) -> Result<Vec<u16>, Box<dyn std::error::Error>> {
    let values = usize::try_from(rows)?
        .checked_mul(usize::try_from(logical_k)?)
        .ok_or("input length overflow")?;
    let identity = usize::from(layer)
        .checked_mul(131)
        .and_then(|value| value.checked_add(usize::from(expert) * 17))
        .and_then(|value| value.checked_add(usize::from(rank) * 7))
        .and_then(|value| value.checked_add(usize::from(projection)))
        .ok_or("input identity overflow")?;
    Ok((0..values)
        .map(|index| {
            let signed = i32::try_from((index * 29 + 17 + identity) % 257)
                .expect("modulo 257 fits i32")
                - 128;
            f32_to_f16_bits(signed as f32 / 512.0)
        })
        .collect())
}

fn compare_f16_output(reference: &[u16], actual: &[u16]) -> (f32, f32, usize) {
    if reference.len() != actual.len() {
        return (f32::INFINITY, f32::INFINITY, usize::MAX);
    }
    let mut maximum_absolute = 0.0_f32;
    let mut maximum_relative = 0.0_f32;
    let mut failures = 0_usize;
    for (&reference, &actual) in reference.iter().zip(actual) {
        let reference = f16_bits_to_f32(reference);
        let actual = f16_bits_to_f32(actual);
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

fn summarize(samples: &[f64]) -> Result<TimingSummary, Box<dyn std::error::Error>> {
    if samples.is_empty()
        || samples
            .iter()
            .any(|sample| !sample.is_finite() || *sample < 0.0)
    {
        return Err("timing samples must be non-empty, finite, and non-negative".into());
    }
    let mut ordered = samples.to_vec();
    ordered.sort_by(f64::total_cmp);
    let sum: f64 = ordered.iter().sum();
    Ok(TimingSummary {
        samples: ordered.len(),
        minimum_us: ordered[0],
        p50_us: nearest_rank(&ordered, 50),
        p95_us: nearest_rank(&ordered, 95),
        p99_us: nearest_rank(&ordered, 99),
        maximum_us: ordered[ordered.len() - 1],
        mean_us: sum / ordered.len() as f64,
    })
}

fn nearest_rank(ordered: &[f64], percentile: usize) -> f64 {
    let rank = percentile
        .checked_mul(ordered.len())
        .expect("sample count is bounded")
        .div_ceil(100)
        .saturating_sub(1);
    ordered[rank.min(ordered.len() - 1)]
}

fn hash_file(path: &Path) -> Result<String, Box<dyn std::error::Error>> {
    let file = File::open(path)?;
    let mut reader = BufReader::with_capacity(8 * 1024 * 1024, file);
    let mut digest = Sha256::new();
    let mut buffer = vec![0_u8; 1024 * 1024];
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
    }
    Ok(hex(&digest.finalize()))
}

fn hash_bytes(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

fn hash_words(words: &[u16]) -> String {
    let mut digest = Sha256::new();
    for word in words {
        digest.update(word.to_le_bytes());
    }
    hex(&digest.finalize())
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(char::from(DIGITS[usize::from(byte >> 4)]));
        output.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    output
}

#[cfg(test)]
mod tests {
    use super::{nearest_rank, summarize};

    #[test]
    fn nearest_rank_is_pinned() {
        let ordered = [1.0, 2.0, 3.0, 4.0, 5.0];
        assert_eq!(nearest_rank(&ordered, 50), 3.0);
        assert_eq!(nearest_rank(&ordered, 95), 5.0);
        assert_eq!(nearest_rank(&ordered, 99), 5.0);
    }

    #[test]
    fn summary_rejects_non_finite_samples() {
        assert!(summarize(&[]).is_err());
        assert!(summarize(&[f64::NAN]).is_err());
        assert!(summarize(&[-1.0]).is_err());
    }
}
