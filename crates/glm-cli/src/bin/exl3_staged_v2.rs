use std::ffi::{CStr, c_char, c_void};
use std::fs;
use std::path::{Path, PathBuf};

use glm_cuda::{
    Exl3Descriptor, Exl3KernelProjection, KernelError, exl3_workspace_bytes,
    validate_exl3_descriptor,
};
use glm_format::{EXL3_MCG_MULTIPLIER, Exl3Metadata, Exl3Projection, Exl3Trellis, f32_to_f16_bits};
use serde::Serialize;
use sha2::{Digest, Sha256};

const STAGED_KERNEL_ABI: &[u8] = b"glmaxx.sm120.exl3.warp_staged_projection.v2";
const WARMUP_ITERATIONS: u32 = 50;
const MEASURED_ITERATIONS: u32 = 1_000;

unsafe extern "C" {
    fn glmaxx_exl3_projection_launch(
        descriptor: *const Exl3Descriptor,
        stream: *mut c_void,
        asynchronous_error: *mut i32,
    ) -> i32;
    fn glmaxx_exl3_staged_projection_launch(
        descriptor: *const Exl3Descriptor,
        stream: *mut c_void,
        asynchronous_error: *mut i32,
    ) -> i32;
    fn glmaxx_exl3_staged_kernel_abi() -> *const c_char;
    fn glmaxx_device_alloc(bytes: u64, pointer: *mut u64) -> i32;
    fn glmaxx_device_free(pointer: u64) -> i32;
    fn glmaxx_stream_create(stream: *mut u64) -> i32;
    fn glmaxx_stream_destroy(stream: u64) -> i32;
    fn glmaxx_stream_synchronize(stream: u64) -> i32;
    fn glmaxx_memcpy_h2d(destination: u64, source: *const c_void, bytes: u64, stream: u64) -> i32;
    fn glmaxx_memcpy_d2h(destination: *mut c_void, source: u64, bytes: u64, stream: u64) -> i32;
    fn glmaxx_event_create(event: *mut u64) -> i32;
    fn glmaxx_event_record(event: u64, stream: u64) -> i32;
    fn glmaxx_event_elapsed_ms(start: u64, end: u64, milliseconds: *mut f32) -> i32;
    fn glmaxx_event_destroy(event: u64) -> i32;
}

#[derive(Clone, Copy)]
enum Route {
    Scalar,
    Staged,
}

struct Stream(u64);

impl Stream {
    fn create() -> Result<Self, KernelError> {
        let mut stream = 0_u64;
        // SAFETY: `stream` is a valid out-parameter.
        check(unsafe { glmaxx_stream_create(std::ptr::from_mut(&mut stream)) })?;
        if stream == 0 {
            return Err(KernelError::Null);
        }
        Ok(Self(stream))
    }

    fn synchronize(&self) -> Result<(), KernelError> {
        // SAFETY: this object owns a live stream.
        check(unsafe { glmaxx_stream_synchronize(self.0) })
    }
}

impl Drop for Stream {
    fn drop(&mut self) {
        // SAFETY: this object owns the stream and drops exactly once.
        let _ = unsafe { glmaxx_stream_destroy(self.0) };
        self.0 = 0;
    }
}

struct DeviceBuffer {
    pointer: u64,
    bytes: u64,
}

impl DeviceBuffer {
    fn allocate(bytes: u64) -> Result<Self, KernelError> {
        if bytes == 0 {
            return Err(KernelError::Shape);
        }
        let mut pointer = 0_u64;
        // SAFETY: `pointer` is a valid out-parameter.
        check(unsafe { glmaxx_device_alloc(bytes, std::ptr::from_mut(&mut pointer)) })?;
        if pointer == 0 {
            return Err(KernelError::Null);
        }
        Ok(Self { pointer, bytes })
    }

    fn upload(bytes: &[u8], stream: &Stream) -> Result<Self, KernelError> {
        let buffer =
            Self::allocate(u64::try_from(bytes.len()).map_err(|_| KernelError::Overflow)?)?;
        // SAFETY: the host slice and device allocation are valid for the
        // exact byte count until the caller synchronizes the stream.
        check(unsafe {
            glmaxx_memcpy_h2d(
                buffer.pointer,
                bytes.as_ptr().cast(),
                buffer.bytes,
                stream.0,
            )
        })?;
        Ok(buffer)
    }

    fn copy_to(&self, bytes: &mut [u8], stream: &Stream) -> Result<(), KernelError> {
        if u64::try_from(bytes.len()).map_err(|_| KernelError::Overflow)? != self.bytes {
            return Err(KernelError::Shape);
        }
        // SAFETY: the output slice and device allocation are valid for the
        // exact byte count until stream synchronization completes.
        check(unsafe {
            glmaxx_memcpy_d2h(
                bytes.as_mut_ptr().cast(),
                self.pointer,
                self.bytes,
                stream.0,
            )
        })
    }
}

impl Drop for DeviceBuffer {
    fn drop(&mut self) {
        // SAFETY: this object owns the allocation and drops exactly once.
        let _ = unsafe { glmaxx_device_free(self.pointer) };
        self.pointer = 0;
    }
}

struct Event(u64);

impl Event {
    fn create() -> Result<Self, KernelError> {
        let mut event = 0_u64;
        // SAFETY: `event` is a valid out-parameter.
        check(unsafe { glmaxx_event_create(std::ptr::from_mut(&mut event)) })?;
        if event == 0 {
            return Err(KernelError::Null);
        }
        Ok(Self(event))
    }

    fn record(&self, stream: &Stream) -> Result<(), KernelError> {
        // SAFETY: both event and stream are live and caller-owned.
        check(unsafe { glmaxx_event_record(self.0, stream.0) })
    }

    fn elapsed_us(&self, end: &Self) -> Result<f64, KernelError> {
        let mut milliseconds = 0_f32;
        // SAFETY: both events have completed on the same synchronized stream.
        check(unsafe {
            glmaxx_event_elapsed_ms(self.0, end.0, std::ptr::from_mut(&mut milliseconds))
        })?;
        Ok(f64::from(milliseconds) * 1_000.0)
    }
}

impl Drop for Event {
    fn drop(&mut self) {
        // SAFETY: this object owns the event and drops exactly once.
        let _ = unsafe { glmaxx_event_destroy(self.0) };
        self.0 = 0;
    }
}

struct CaseBuffers {
    input: DeviceBuffer,
    rotated_input: DeviceBuffer,
    projected: DeviceBuffer,
    output: DeviceBuffer,
    validation: DeviceBuffer,
    descriptor: Exl3Descriptor,
}

struct ProjectionFixture {
    stream: Stream,
    trellis: DeviceBuffer,
    suh: DeviceBuffer,
    svh: DeviceBuffer,
    projection: Exl3KernelProjection,
    source_projection: Exl3Projection,
    logical_k: u32,
    logical_n: u32,
    trellis_sha256: String,
    suh_sha256: String,
    svh_sha256: String,
}

impl ProjectionFixture {
    fn new(source_projection: Exl3Projection) -> Result<Self, Box<dyn std::error::Error>> {
        let projection = kernel_projection(source_projection);
        let (logical_k, logical_n) = projection_shape(source_projection);
        let tensor = synthetic_tensor(source_projection, logical_k, logical_n)?;
        let stream = Stream::create()?;
        let trellis = DeviceBuffer::upload(words_as_bytes(&tensor.trellis), &stream)?;
        let suh = DeviceBuffer::upload(words_as_bytes(&tensor.suh), &stream)?;
        let svh = DeviceBuffer::upload(words_as_bytes(&tensor.svh), &stream)?;
        stream.synchronize()?;
        Ok(Self {
            stream,
            trellis,
            suh,
            svh,
            projection,
            source_projection,
            logical_k,
            logical_n,
            trellis_sha256: u16_sha256(&tensor.trellis),
            suh_sha256: u16_sha256(&tensor.suh),
            svh_sha256: u16_sha256(&tensor.svh),
        })
    }

    fn prepare(&self, rows: u32) -> Result<(CaseBuffers, String), KernelError> {
        let input = synthetic_input(rows, self.logical_k, self.source_projection)?;
        let input_hash = u16_sha256(&input);
        let input_buffer = DeviceBuffer::upload(words_as_bytes(&input), &self.stream)?;
        let rotated_input =
            DeviceBuffer::allocate(u64::from(rows) * u64::from(self.logical_k) * 2)?;
        let projected = DeviceBuffer::allocate(u64::from(rows) * u64::from(self.logical_n) * 2)?;
        let output = DeviceBuffer::allocate(u64::from(rows) * u64::from(self.logical_n) * 2)?;
        let validation = DeviceBuffer::allocate(4)?;
        let mut descriptor = Exl3Descriptor::new(rows, self.projection);
        descriptor.input_f16 = input_buffer.pointer;
        descriptor.trellis_u16 = self.trellis.pointer;
        descriptor.suh_f16 = self.suh.pointer;
        descriptor.svh_f16 = self.svh.pointer;
        descriptor.rotated_input_f16 = rotated_input.pointer;
        descriptor.projected_f16 = projected.pointer;
        descriptor.output_f16 = output.pointer;
        descriptor.validation_error_u32 = validation.pointer;
        descriptor.workspace_bytes = exl3_workspace_bytes(rows, self.logical_k, self.logical_n)?;
        descriptor.sequence = 1;
        validate_exl3_descriptor(&descriptor)?;
        self.stream.synchronize()?;
        Ok((
            CaseBuffers {
                input: input_buffer,
                rotated_input,
                projected,
                output,
                validation,
                descriptor,
            },
            input_hash,
        ))
    }

    fn run_case(&self, rows: u32) -> Result<CaseReport, Box<dyn std::error::Error>> {
        let (case, input_sha256) = self.prepare(rows)?;
        let (scalar_output, scalar_validation) = run_and_copy(Route::Scalar, &case, &self.stream)?;
        let (staged_output, staged_validation) = run_and_copy(Route::Staged, &case, &self.stream)?;
        let mismatched_elements = scalar_output
            .iter()
            .zip(&staged_output)
            .filter(|(scalar, staged)| scalar != staged)
            .count();
        let (scalar_samples_us, staged_samples_us) =
            benchmark_pair(&case.descriptor, &self.stream)?;
        let scalar_latency = Latency::new(scalar_samples_us)?;
        let staged_latency = Latency::new(staged_samples_us)?;
        let speedup = scalar_latency.p50 / staged_latency.p50;
        let result = CaseReport {
            schema: "glmaxx.sm120-exl3-warp-staged-v2-case.v1",
            kernel_abi: std::str::from_utf8(STAGED_KERNEL_ABI)?,
            projection: projection_id(self.source_projection),
            rows,
            logical_k: self.logical_k,
            logical_n: self.logical_n,
            input_sha256,
            trellis_sha256: self.trellis_sha256.clone(),
            suh_sha256: self.suh_sha256.clone(),
            svh_sha256: self.svh_sha256.clone(),
            scalar_output_sha256: u16_sha256(&scalar_output),
            staged_output_sha256: u16_sha256(&staged_output),
            mismatched_elements,
            scalar_validation,
            staged_validation,
            warmup_iterations: WARMUP_ITERATIONS,
            measured_iterations: MEASURED_ITERATIONS,
            scalar_latency,
            staged_latency,
            p50_speedup: speedup,
            staged_source_load_bytes: 1_179_648,
            staged_static_shared_bytes: 768,
            runtime_weight_repack_bytes: 0,
            persistent_reconstructed_weight_bytes: 0,
        };
        // Keep every allocation live through the final stream completion.
        self.stream.synchronize()?;
        std::hint::black_box((
            case.input.pointer,
            case.rotated_input.pointer,
            case.projected.pointer,
        ));
        Ok(result)
    }
}

#[derive(Serialize)]
struct Latency {
    unit: &'static str,
    sample_count: usize,
    samples: Vec<f64>,
    minimum: f64,
    p50: f64,
    p95: f64,
    p99: f64,
    maximum: f64,
    mean: f64,
    population_stddev: f64,
}

impl Latency {
    fn new(samples: Vec<f64>) -> Result<Self, KernelError> {
        if samples.is_empty()
            || samples
                .iter()
                .any(|sample| !sample.is_finite() || *sample < 0.0)
        {
            return Err(KernelError::Shape);
        }
        let mut ordered = samples.clone();
        ordered.sort_by(f64::total_cmp);
        let mean = samples.iter().sum::<f64>() / samples.len() as f64;
        let variance = samples
            .iter()
            .map(|sample| {
                let delta = *sample - mean;
                delta * delta
            })
            .sum::<f64>()
            / samples.len() as f64;
        Ok(Self {
            unit: "microseconds",
            sample_count: samples.len(),
            minimum: ordered[0],
            p50: nearest_rank(&ordered, 50),
            p95: nearest_rank(&ordered, 95),
            p99: nearest_rank(&ordered, 99),
            maximum: *ordered.last().ok_or(KernelError::Shape)?,
            mean,
            population_stddev: variance.sqrt(),
            samples,
        })
    }
}

#[derive(Serialize)]
struct CaseReport {
    schema: &'static str,
    kernel_abi: &'static str,
    projection: &'static str,
    rows: u32,
    logical_k: u32,
    logical_n: u32,
    input_sha256: String,
    trellis_sha256: String,
    suh_sha256: String,
    svh_sha256: String,
    scalar_output_sha256: String,
    staged_output_sha256: String,
    mismatched_elements: usize,
    scalar_validation: u32,
    staged_validation: u32,
    warmup_iterations: u32,
    measured_iterations: u32,
    scalar_latency: Latency,
    staged_latency: Latency,
    p50_speedup: f64,
    staged_source_load_bytes: u64,
    staged_static_shared_bytes: u64,
    runtime_weight_repack_bytes: u64,
    persistent_reconstructed_weight_bytes: u64,
}

#[derive(Serialize)]
struct SuiteReport {
    schema: &'static str,
    kernel_abi: &'static str,
    cases: usize,
    failed_cases: usize,
    minimum_p50_speedup: f64,
    maximum_p50_speedup: f64,
    rows: [u32; 4],
    projections: [&'static str; 3],
    claim: &'static str,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut arguments = std::env::args_os();
    let _program = arguments.next();
    let evidence = PathBuf::from(
        arguments
            .next()
            .ok_or("glmaxx-exl3-staged-v2 requires an empty evidence directory")?,
    );
    if arguments.next().is_some() {
        return Err("glmaxx-exl3-staged-v2 accepts exactly one evidence directory".into());
    }
    validate_evidence_directory(&evidence)?;
    validate_native_abi()?;

    let mut case_reports = Vec::with_capacity(12);
    for projection in [
        Exl3Projection::Gate,
        Exl3Projection::Up,
        Exl3Projection::Down,
    ] {
        let fixture = ProjectionFixture::new(projection)?;
        for rows in [1_u32, 2, 4, 8] {
            let report = fixture.run_case(rows)?;
            let mut json = serde_json::to_vec_pretty(&report)?;
            json.push(b'\n');
            fs::write(
                evidence.join(format!("{}-m{rows}.json", projection_id(projection))),
                json,
            )?;
            case_reports.push(report);
        }
    }

    let failed_cases = case_reports
        .iter()
        .filter(|report| {
            report.mismatched_elements != 0
                || report.scalar_validation != 0
                || report.staged_validation != 0
        })
        .count();
    let minimum_p50_speedup = case_reports
        .iter()
        .map(|report| report.p50_speedup)
        .reduce(f64::min)
        .ok_or("no EXL3 staged cases")?;
    let maximum_p50_speedup = case_reports
        .iter()
        .map(|report| report.p50_speedup)
        .reduce(f64::max)
        .ok_or("no EXL3 staged cases")?;
    let suite = SuiteReport {
        schema: "glmaxx.sm120-exl3-warp-staged-v2-suite.v1",
        kernel_abi: std::str::from_utf8(STAGED_KERNEL_ABI)?,
        cases: case_reports.len(),
        failed_cases,
        minimum_p50_speedup,
        maximum_p50_speedup,
        rows: [1, 2, 4, 8],
        projections: ["gate", "up", "down"],
        claim: "synthetic scalar-v1 versus staged-v2 bitwise and CUDA-event evidence only",
    };
    let mut json = serde_json::to_vec_pretty(&suite)?;
    json.push(b'\n');
    fs::write(evidence.join("summary.json"), &json)?;
    println!("{}", String::from_utf8(json)?);
    if failed_cases != 0 {
        return Err("EXL3 staged v2 failed bitwise validation".into());
    }
    Ok(())
}

fn validate_evidence_directory(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    if !path.is_dir() {
        return Err("evidence directory must already exist".into());
    }
    if path.read_dir()?.next().is_some() {
        return Err("evidence directory must be empty".into());
    }
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .ok_or("cannot resolve repository root")?
        .canonicalize()?;
    if path.canonicalize()?.starts_with(repository) {
        return Err("raw GPU evidence must be outside the Git repository".into());
    }
    Ok(())
}

fn validate_native_abi() -> Result<(), Box<dyn std::error::Error>> {
    // SAFETY: the native function returns immutable process-lifetime bytes.
    let pointer = unsafe { glmaxx_exl3_staged_kernel_abi() };
    if pointer.is_null() || unsafe { CStr::from_ptr(pointer) }.to_bytes() != STAGED_KERNEL_ABI {
        return Err("EXL3 staged v2 native ABI mismatch".into());
    }
    Ok(())
}

fn synthetic_tensor(
    projection: Exl3Projection,
    logical_k: u32,
    logical_n: u32,
) -> Result<Exl3Trellis, Box<dyn std::error::Error>> {
    let metadata = Exl3Metadata::new(projection, 3, 0, 0, 3, logical_k, logical_n)?;
    let mut state =
        0x0002_c026_0721_u64 ^ projection_seed(projection) ^ (u64::from(logical_k) << 32);
    let mut trellis = Vec::with_capacity(usize::try_from(metadata.trellis_words)?);
    for _ in 0..metadata.trellis_words {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        trellis.push(state as u16);
    }
    let suh = (0..logical_k)
        .map(|index| {
            let offset = i32::try_from((index * 13 + 5) % 17).expect("bounded offset") - 8;
            f32_to_f16_bits(1.0 + offset as f32 / 64.0)
        })
        .collect();
    let svh = (0..logical_n)
        .map(|index| {
            let offset = i32::try_from((index * 7 + 3) % 13).expect("bounded offset") - 6;
            f32_to_f16_bits(1.0 + offset as f32 / 64.0)
        })
        .collect();
    let tensor = Exl3Trellis {
        metadata,
        trellis,
        suh,
        svh,
        mcg_marker: EXL3_MCG_MULTIPLIER,
    };
    tensor.validate()?;
    Ok(tensor)
}

fn synthetic_input(
    rows: u32,
    logical_k: u32,
    projection: Exl3Projection,
) -> Result<Vec<u16>, KernelError> {
    let elements = usize::try_from(rows)
        .map_err(|_| KernelError::Overflow)?
        .checked_mul(usize::try_from(logical_k).map_err(|_| KernelError::Overflow)?)
        .ok_or(KernelError::Overflow)?;
    let seed = usize::try_from(projection_seed(projection)).map_err(|_| KernelError::Overflow)?;
    Ok((0..elements)
        .map(|index| {
            let signed =
                i32::try_from((index * 29 + 17 + seed) % 257).expect("bounded input") - 128;
            f32_to_f16_bits(signed as f32 / 512.0)
        })
        .collect())
}

fn run_and_copy(
    route: Route,
    case: &CaseBuffers,
    stream: &Stream,
) -> Result<(Vec<u16>, u32), KernelError> {
    launch(route, &case.descriptor, stream)?;
    stream.synchronize()?;
    let output_elements = usize::try_from(case.descriptor.rows)
        .map_err(|_| KernelError::Overflow)?
        .checked_mul(usize::try_from(case.descriptor.logical_n).map_err(|_| KernelError::Overflow)?)
        .ok_or(KernelError::Overflow)?;
    let mut output = vec![0_u16; output_elements];
    case.output
        .copy_to(words_as_bytes_mut(&mut output), stream)?;
    let mut validation = 0_u32;
    // SAFETY: the mutable u32 is live and exactly four bytes long.
    let validation_bytes = unsafe {
        std::slice::from_raw_parts_mut(std::ptr::from_mut(&mut validation).cast::<u8>(), 4)
    };
    case.validation.copy_to(validation_bytes, stream)?;
    stream.synchronize()?;
    Ok((output, validation))
}

fn benchmark_pair(
    descriptor: &Exl3Descriptor,
    stream: &Stream,
) -> Result<(Vec<f64>, Vec<f64>), KernelError> {
    for iteration in 0..WARMUP_ITERATIONS {
        let routes = if iteration % 2 == 0 {
            [Route::Scalar, Route::Staged]
        } else {
            [Route::Staged, Route::Scalar]
        };
        for route in routes {
            launch(route, descriptor, stream)?;
        }
    }
    stream.synchronize()?;

    let mut scalar_pairs = Vec::with_capacity(MEASURED_ITERATIONS as usize);
    let mut staged_pairs = Vec::with_capacity(MEASURED_ITERATIONS as usize);
    for iteration in 0..MEASURED_ITERATIONS {
        let routes = if iteration % 2 == 0 {
            [Route::Scalar, Route::Staged]
        } else {
            [Route::Staged, Route::Scalar]
        };
        for route in routes {
            let start = Event::create()?;
            let end = Event::create()?;
            start.record(stream)?;
            launch(route, descriptor, stream)?;
            end.record(stream)?;
            match route {
                Route::Scalar => scalar_pairs.push((start, end)),
                Route::Staged => staged_pairs.push((start, end)),
            }
        }
    }
    stream.synchronize()?;
    let scalar = scalar_pairs
        .iter()
        .map(|(start, end)| start.elapsed_us(end))
        .collect::<Result<Vec<_>, _>>()?;
    let staged = staged_pairs
        .iter()
        .map(|(start, end)| start.elapsed_us(end))
        .collect::<Result<Vec<_>, _>>()?;
    Ok((scalar, staged))
}

fn launch(route: Route, descriptor: &Exl3Descriptor, stream: &Stream) -> Result<(), KernelError> {
    let mut asynchronous_error = 0_i32;
    // SAFETY: the POD descriptor and every referenced allocation remain live
    // through stream synchronization owned by the caller.
    let status = unsafe {
        match route {
            Route::Scalar => glmaxx_exl3_projection_launch(
                std::ptr::from_ref(descriptor),
                stream.0 as *mut c_void,
                std::ptr::from_mut(&mut asynchronous_error),
            ),
            Route::Staged => glmaxx_exl3_staged_projection_launch(
                std::ptr::from_ref(descriptor),
                stream.0 as *mut c_void,
                std::ptr::from_mut(&mut asynchronous_error),
            ),
        }
    };
    check(status)?;
    if asynchronous_error == 0 {
        Ok(())
    } else {
        Err(KernelError::Async(asynchronous_error))
    }
}

fn check(status: i32) -> Result<(), KernelError> {
    if status == 0 {
        Ok(())
    } else {
        Err(KernelError::Driver(status))
    }
}

fn nearest_rank(ordered: &[f64], percentile: usize) -> f64 {
    let rank = percentile
        .checked_mul(ordered.len())
        .and_then(|value| value.checked_add(99))
        .expect("bounded percentile rank")
        / 100;
    ordered[rank.saturating_sub(1)]
}

fn projection_shape(projection: Exl3Projection) -> (u32, u32) {
    match projection {
        Exl3Projection::Gate | Exl3Projection::Up => (6_144, 512),
        Exl3Projection::Down => (512, 6_144),
    }
}

fn kernel_projection(projection: Exl3Projection) -> Exl3KernelProjection {
    match projection {
        Exl3Projection::Gate => Exl3KernelProjection::Gate,
        Exl3Projection::Up => Exl3KernelProjection::Up,
        Exl3Projection::Down => Exl3KernelProjection::Down,
    }
}

fn projection_id(projection: Exl3Projection) -> &'static str {
    match projection {
        Exl3Projection::Gate => "gate",
        Exl3Projection::Up => "up",
        Exl3Projection::Down => "down",
    }
}

fn projection_seed(projection: Exl3Projection) -> u64 {
    match projection {
        Exl3Projection::Gate => 0,
        Exl3Projection::Up => 1,
        Exl3Projection::Down => 2,
    }
}

fn words_as_bytes(words: &[u16]) -> &[u8] {
    // SAFETY: u16 has no invalid bit patterns and byte length is exact.
    unsafe { std::slice::from_raw_parts(words.as_ptr().cast(), std::mem::size_of_val(words)) }
}

fn words_as_bytes_mut(words: &mut [u16]) -> &mut [u8] {
    // SAFETY: u16 has no invalid bit patterns and byte length is exact.
    unsafe {
        std::slice::from_raw_parts_mut(words.as_mut_ptr().cast(), std::mem::size_of_val(words))
    }
}

fn u16_sha256(words: &[u16]) -> String {
    hex_digest(Sha256::digest(words_as_bytes(words)))
}

fn hex_digest(bytes: impl AsRef<[u8]>) -> String {
    bytes
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
