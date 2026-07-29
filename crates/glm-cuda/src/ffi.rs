use std::ffi::{CStr, c_char, c_void};
use std::thread::ThreadId;
use std::time::Instant;

use glm_format::{Codec, Exl3Projection, Exl3Trellis, KERNEL_ABI, PackedNvfp4};

use crate::abi::active_experts_for_grouped;
use crate::{
    CudaDriver, EXL3_KERNEL_ABI, Exl3Descriptor, Exl3KernelProjection, Fc1Descriptor,
    Fc2Descriptor, HIDDEN, KernelError, KernelPath, LOCAL_INTERMEDIATE, LaunchGeometry,
    exl3_trellis_bytes, exl3_workspace_bytes, fc2_grouped_sfa_capacity_bytes,
    fc2_grouped_workspace_bytes, fc2_workspace_bytes, grouped_sfa_capacity_bytes, grouped_sfa_plan,
    grouped_workspace_bytes, validate_descriptor, validate_exl3_descriptor, workspace_bytes,
};

unsafe extern "C" {
    fn glmaxx_nvfp4_routed_fc1_launch(
        descriptor: *const Fc1Descriptor,
        stream: *mut c_void,
        error_code: *mut i32,
    ) -> i32;
    fn glmaxx_nvfp4_routed_fc1_graph_instantiate(
        descriptor: *const Fc1Descriptor,
        stream: *mut c_void,
        graph_exec: *mut u64,
    ) -> i32;
    fn glmaxx_nvfp4_quantize_launch(descriptor: *const Fc1Descriptor, stream: *mut c_void) -> i32;
    fn glmaxx_nvfp4_grouped_quantize_launch(
        descriptor: *const Fc1Descriptor,
        stream: *mut c_void,
    ) -> i32;
    fn glmaxx_nvfp4_core_swiglu_launch(
        descriptor: *const Fc1Descriptor,
        stream: *mut c_void,
    ) -> i32;
    fn glmaxx_nvfp4_dense_control_launch(
        descriptor: *const Fc1Descriptor,
        expert: u32,
        stream: *mut c_void,
        error_code: *mut i32,
    ) -> i32;
    fn glmaxx_nvfp4_grouped_control_launch(
        descriptor: *const Fc1Descriptor,
        active_experts: *const u16,
        active_expert_count: u32,
        stream: *mut c_void,
        error_code: *mut i32,
    ) -> i32;
    fn glmaxx_nvfp4_grouped_core_swiglu_launch(
        descriptor: *const Fc1Descriptor,
        active_experts: *const u16,
        active_expert_count: u32,
        stream: *mut c_void,
        error_code: *mut i32,
    ) -> i32;
    fn glmaxx_nvfp4_grouped_prepare_launch(
        descriptor: *const Fc1Descriptor,
        active_experts: *const u16,
        active_expert_count: u32,
        stream: *mut c_void,
    ) -> i32;
    fn glmaxx_nvfp4_grouped_prepared_control_launch(
        descriptor: *const Fc1Descriptor,
        active_expert_count: u32,
        stream: *mut c_void,
        error_code: *mut i32,
    ) -> i32;
    fn glmaxx_nvfp4_grouped_prepared_core_swiglu_launch(
        descriptor: *const Fc1Descriptor,
        active_expert_count: u32,
        stream: *mut c_void,
        error_code: *mut i32,
    ) -> i32;
    fn glmaxx_nvfp4_routed_fc2_launch(
        descriptor: *const Fc2Descriptor,
        stream: *mut c_void,
        error_code: *mut i32,
    ) -> i32;
    fn glmaxx_nvfp4_fc2_dense_control_launch(
        descriptor: *const Fc2Descriptor,
        expert: u32,
        stream: *mut c_void,
        error_code: *mut i32,
    ) -> i32;
    fn glmaxx_nvfp4_fc2_grouped_control_launch(
        descriptor: *const Fc2Descriptor,
        active_experts: *const u16,
        active_expert_count: u32,
        stream: *mut c_void,
        error_code: *mut i32,
    ) -> i32;
    fn glmaxx_exl3_projection_launch(
        descriptor: *const Exl3Descriptor,
        stream: *mut c_void,
        error_code: *mut i32,
    ) -> i32;
    fn glmaxx_graph_exec_launch(graph_exec: u64, stream: u64) -> i32;
    fn glmaxx_graph_exec_destroy(graph_exec: u64) -> i32;
    fn glmaxx_event_create(event: *mut u64) -> i32;
    fn glmaxx_event_record(event: u64, stream: u64) -> i32;
    fn glmaxx_event_synchronize(event: u64) -> i32;
    fn glmaxx_event_elapsed_ms(start: u64, end: u64, milliseconds: *mut f32) -> i32;
    fn glmaxx_event_destroy(event: u64) -> i32;
    fn glmaxx_nvfp4_routed_fc1_workspace_bytes(assignments: u32) -> u64;
    fn glmaxx_nvfp4_grouped_workspace_bytes(assignments: u32) -> u64;
    fn glmaxx_nvfp4_routed_fc2_workspace_bytes(rows: u32, assignments: u32) -> u64;
    fn glmaxx_nvfp4_grouped_fc2_workspace_bytes(rows: u32, assignments: u32) -> u64;
    fn glmaxx_exl3_projection_workspace_bytes(rows: u32, logical_k: u32, logical_n: u32) -> u64;
    fn glmaxx_kernel_abi() -> *const c_char;
    fn glmaxx_exl3_kernel_abi() -> *const c_char;
    fn glmaxx_device_count(count: *mut i32) -> i32;
    fn glmaxx_device_bind(
        device_index: i32,
        compute_capability: *mut i32,
        multiprocessor_count: *mut i32,
        total_memory_bytes: *mut u64,
    ) -> i32;
    fn glmaxx_device_alloc(bytes: u64, pointer: *mut u64) -> i32;
    fn glmaxx_device_free(pointer: u64) -> i32;
    fn glmaxx_stream_create(stream: *mut u64) -> i32;
    fn glmaxx_stream_destroy(stream: u64) -> i32;
    fn glmaxx_stream_query(stream: u64, complete: *mut i32) -> i32;
    fn glmaxx_stream_synchronize(stream: u64) -> i32;
    fn glmaxx_memcpy_h2d(destination: u64, source: *const c_void, bytes: u64, stream: u64) -> i32;
    fn glmaxx_memcpy_d2d(destination: u64, source: u64, bytes: u64, stream: u64) -> i32;
    fn glmaxx_memcpy_d2h(destination: *mut c_void, source: u64, bytes: u64, stream: u64) -> i32;
}

pub struct NativeKernelDriver;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeDeviceIdentity {
    pub visible_devices: u32,
    pub device_index: u32,
    pub compute_capability: u32,
    pub multiprocessor_count: u32,
    pub total_memory_bytes: u64,
}

/// One rank's thread-owned CUDA device and nonblocking execution stream.
///
/// Construct this object on the persistent rank thread. Every method verifies
/// that it is still called by that same thread.
pub struct NativeRankContext {
    identity: NativeDeviceIdentity,
    stream: NativeStream,
    owner: ThreadId,
}

impl NativeRankContext {
    pub fn bind(rank: u8) -> Result<Self, KernelError> {
        let mut visible_devices = 0_i32;
        // SAFETY: `visible_devices` is a valid out-parameter.
        check(unsafe { glmaxx_device_count(std::ptr::from_mut(&mut visible_devices)) })?;
        if visible_devices != 4 || rank >= 4 {
            return Err(KernelError::Topology);
        }
        let mut compute_capability = 0_i32;
        let mut multiprocessor_count = 0_i32;
        let mut total_memory_bytes = 0_u64;
        // SAFETY: all outputs are valid and the rank is within the verified
        // four-device visible set.
        check(unsafe {
            glmaxx_device_bind(
                i32::from(rank),
                std::ptr::from_mut(&mut compute_capability),
                std::ptr::from_mut(&mut multiprocessor_count),
                std::ptr::from_mut(&mut total_memory_bytes),
            )
        })?;
        if compute_capability != 120 || multiprocessor_count <= 0 || total_memory_bytes == 0 {
            return Err(KernelError::Topology);
        }
        let stream = NativeStream::create()?;
        Ok(Self {
            identity: NativeDeviceIdentity {
                visible_devices: u32::try_from(visible_devices)
                    .map_err(|_| KernelError::Topology)?,
                device_index: u32::from(rank),
                compute_capability: u32::try_from(compute_capability)
                    .map_err(|_| KernelError::Topology)?,
                multiprocessor_count: u32::try_from(multiprocessor_count)
                    .map_err(|_| KernelError::Topology)?,
                total_memory_bytes,
            },
            stream,
            owner: std::thread::current().id(),
        })
    }

    #[must_use]
    pub const fn identity(&self) -> NativeDeviceIdentity {
        self.identity
    }

    pub fn stream(&self) -> Result<u64, KernelError> {
        self.require_owner()?;
        Ok(self.stream.0)
    }

    pub fn synchronize(&self) -> Result<(), KernelError> {
        self.require_owner()?;
        // SAFETY: this context owns the stream and the owner-thread check
        // prevents cross-thread rank-state access.
        check(unsafe { glmaxx_stream_synchronize(self.stream.0) })
    }

    fn require_owner(&self) -> Result<(), KernelError> {
        if std::thread::current().id() == self.owner {
            Ok(())
        } else {
            Err(KernelError::Topology)
        }
    }
}

impl CudaDriver for NativeKernelDriver {
    fn allocate(&self, bytes: u64, alignment: u64) -> Result<u64, KernelError> {
        if alignment > 256 {
            return Err(KernelError::Alignment);
        }
        let mut pointer = 0_u64;
        // SAFETY: `pointer` is a valid out-parameter.
        let status = unsafe { glmaxx_device_alloc(bytes, std::ptr::from_mut(&mut pointer)) };
        check(status)?;
        Ok(pointer)
    }

    fn free(&self, pointer: u64) -> Result<(), KernelError> {
        // SAFETY: the caller's RAII object owns this native allocation.
        check(unsafe { glmaxx_device_free(pointer) })
    }

    fn launch_fc1(&self, descriptor: &Fc1Descriptor, stream: u64) -> Result<(), KernelError> {
        validate_native_library(descriptor.assignments)?;
        let mut async_error = 0_i32;
        // SAFETY: validation occurs in `LaunchTicket`; the native function only
        // reads the POD descriptor and enqueues work on the caller-owned stream.
        let status = unsafe {
            glmaxx_nvfp4_routed_fc1_launch(
                std::ptr::from_ref(descriptor),
                stream as *mut c_void,
                std::ptr::from_mut(&mut async_error),
            )
        };
        if status != 0 {
            Err(KernelError::Driver(status))
        } else if async_error != 0 {
            Err(KernelError::Async(async_error))
        } else {
            Ok(())
        }
    }

    fn launch_fc2(&self, descriptor: &Fc2Descriptor, stream: u64) -> Result<(), KernelError> {
        validate_native_fc2_library(descriptor.rows, descriptor.assignments)?;
        let mut async_error = 0_i32;
        // SAFETY: `Fc2LaunchTicket` validates the POD descriptor and all
        // referenced allocations remain caller-owned through stream completion.
        let status = unsafe {
            glmaxx_nvfp4_routed_fc2_launch(
                std::ptr::from_ref(descriptor),
                stream as *mut c_void,
                std::ptr::from_mut(&mut async_error),
            )
        };
        if status != 0 {
            Err(KernelError::Driver(status))
        } else if async_error != 0 {
            Err(KernelError::Async(async_error))
        } else {
            Ok(())
        }
    }

    fn launch_exl3(&self, descriptor: &Exl3Descriptor, stream: u64) -> Result<(), KernelError> {
        validate_native_exl3_library(descriptor.rows, descriptor.logical_k, descriptor.logical_n)?;
        let mut async_error = 0_i32;
        // SAFETY: the safe launch ticket validates the POD descriptor and
        // caller-owned allocations remain live through stream completion.
        let status = unsafe {
            glmaxx_exl3_projection_launch(
                std::ptr::from_ref(descriptor),
                stream as *mut c_void,
                std::ptr::from_mut(&mut async_error),
            )
        };
        if status != 0 {
            Err(KernelError::Driver(status))
        } else if async_error != 0 {
            Err(KernelError::Async(async_error))
        } else {
            Ok(())
        }
    }

    fn query_stream(&self, stream: u64) -> Result<bool, KernelError> {
        let mut complete = 0_i32;
        // SAFETY: `complete` is a valid out-parameter and the caller owns the
        // stream for the lifetime of the launch ticket.
        check(unsafe { glmaxx_stream_query(stream, std::ptr::from_mut(&mut complete)) })?;
        Ok(complete != 0)
    }
}

struct NativeBuffer {
    pointer: u64,
    bytes: u64,
}

impl NativeBuffer {
    fn allocate(bytes: u64) -> Result<Self, KernelError> {
        if bytes == 0 {
            return Err(KernelError::Shape);
        }
        let mut pointer = 0_u64;
        // SAFETY: `pointer` is a valid out-parameter.
        check(unsafe { glmaxx_device_alloc(bytes, std::ptr::from_mut(&mut pointer)) })?;
        Ok(Self { pointer, bytes })
    }

    fn upload(bytes: &[u8], stream: u64) -> Result<Self, KernelError> {
        let buffer = Self::allocate(bytes.len() as u64)?;
        buffer.upload_at(bytes, 0, stream)?;
        Ok(buffer)
    }

    fn upload_at(&self, bytes: &[u8], offset: u64, stream: u64) -> Result<(), KernelError> {
        let byte_count = u64::try_from(bytes.len()).map_err(|_| KernelError::Overflow)?;
        if offset
            .checked_add(byte_count)
            .ok_or(KernelError::Overflow)?
            > self.bytes
        {
            return Err(KernelError::Shape);
        }
        let destination = self
            .pointer
            .checked_add(offset)
            .ok_or(KernelError::Overflow)?;
        // SAFETY: both buffers are valid for `bytes.len()` and stream is owned.
        check(unsafe { glmaxx_memcpy_h2d(destination, bytes.as_ptr().cast(), byte_count, stream) })
    }

    fn copy_within(
        &self,
        source_offset: u64,
        destination_offset: u64,
        bytes: u64,
        stream: u64,
    ) -> Result<(), KernelError> {
        if source_offset
            .checked_add(bytes)
            .ok_or(KernelError::Overflow)?
            > self.bytes
            || destination_offset
                .checked_add(bytes)
                .ok_or(KernelError::Overflow)?
                > self.bytes
        {
            return Err(KernelError::Shape);
        }
        let source = self
            .pointer
            .checked_add(source_offset)
            .ok_or(KernelError::Overflow)?;
        let destination = self
            .pointer
            .checked_add(destination_offset)
            .ok_or(KernelError::Overflow)?;
        // SAFETY: the checked ranges are live within this device allocation.
        check(unsafe { glmaxx_memcpy_d2d(destination, source, bytes, stream) })
    }
}

impl Drop for NativeBuffer {
    fn drop(&mut self) {
        // SAFETY: this object owns the allocation and drops once.
        let _ = unsafe { glmaxx_device_free(self.pointer) };
    }
}

struct NativeStream(u64);

impl NativeStream {
    fn create() -> Result<Self, KernelError> {
        let mut stream = 0_u64;
        // SAFETY: `stream` is a valid out-parameter.
        check(unsafe { glmaxx_stream_create(std::ptr::from_mut(&mut stream)) })?;
        Ok(Self(stream))
    }
}

impl Drop for NativeStream {
    fn drop(&mut self) {
        // SAFETY: this object owns the stream and drops once.
        let _ = unsafe { glmaxx_stream_destroy(self.0) };
    }
}

struct NativeGraph(u64);

impl Drop for NativeGraph {
    fn drop(&mut self) {
        // SAFETY: this object owns the executable graph and drops once.
        let _ = unsafe { glmaxx_graph_exec_destroy(self.0) };
    }
}

struct NativeEvent(u64);

impl NativeEvent {
    fn create() -> Result<Self, KernelError> {
        let mut event = 0_u64;
        // SAFETY: `event` is a valid out-parameter.
        check(unsafe { glmaxx_event_create(std::ptr::from_mut(&mut event)) })?;
        if event == 0 {
            return Err(KernelError::Driver(-1));
        }
        Ok(Self(event))
    }

    fn record(&self, stream: u64) -> Result<(), KernelError> {
        // SAFETY: the event and stream are caller-owned and live.
        check(unsafe { glmaxx_event_record(self.0, stream) })
    }

    fn synchronize(&self) -> Result<(), KernelError> {
        // SAFETY: the event is caller-owned and live.
        check(unsafe { glmaxx_event_synchronize(self.0) })
    }

    fn elapsed_ms(&self, end: &Self) -> Result<f32, KernelError> {
        let mut milliseconds = 0.0_f32;
        // SAFETY: both events are complete and `milliseconds` is a valid
        // out-parameter.
        check(unsafe {
            glmaxx_event_elapsed_ms(self.0, end.0, std::ptr::from_mut(&mut milliseconds))
        })?;
        Ok(milliseconds)
    }
}

impl Drop for NativeEvent {
    fn drop(&mut self) {
        // SAFETY: this object owns the event and drops once.
        let _ = unsafe { glmaxx_event_destroy(self.0) };
    }
}

struct NativeFc1Case {
    _input: NativeBuffer,
    _route_expert: NativeBuffer,
    _route_token: NativeBuffer,
    _route_slot: NativeBuffer,
    _route_weight: NativeBuffer,
    _offsets: NativeBuffer,
    _compacted: NativeBuffer,
    _activation_values: NativeBuffer,
    _activation_scales: NativeBuffer,
    _activation_globals: NativeBuffer,
    _gate_up: NativeBuffer,
    output: NativeBuffer,
    descriptor: Fc1Descriptor,
}

struct NativeFc2Case {
    _input: NativeBuffer,
    _route_expert: NativeBuffer,
    _route_token: NativeBuffer,
    _route_slot: NativeBuffer,
    _route_weight: NativeBuffer,
    _offsets: NativeBuffer,
    _activation_values: NativeBuffer,
    _activation_scales: NativeBuffer,
    _activation_globals: NativeBuffer,
    _assignment_down: NativeBuffer,
    output: NativeBuffer,
    _slot_assignment: NativeBuffer,
    validation_error: NativeBuffer,
    descriptor: Fc2Descriptor,
}

struct NativeExl3Case {
    _input: NativeBuffer,
    _rotated_input: NativeBuffer,
    _projected: NativeBuffer,
    output: NativeBuffer,
    validation_error: NativeBuffer,
    descriptor: Exl3Descriptor,
}

impl NativeExl3Case {
    fn download(&self, stream: u64) -> Result<Vec<u16>, KernelError> {
        let output_words = u64::from(self.descriptor.rows)
            .checked_mul(u64::from(self.descriptor.logical_n))
            .ok_or(KernelError::Overflow)?;
        let output_bytes = output_words.checked_mul(2).ok_or(KernelError::Overflow)?;
        let mut host_output =
            vec![0_u16; usize::try_from(output_words).map_err(|_| KernelError::Overflow)?];
        let mut validation_error = 0_u32;
        // SAFETY: both host destinations and device sources cover the exact
        // requested ranges and remain live through stream synchronization.
        check(unsafe {
            glmaxx_memcpy_d2h(
                host_output.as_mut_ptr().cast(),
                self.output.pointer,
                output_bytes,
                stream,
            )
        })?;
        check(unsafe {
            glmaxx_memcpy_d2h(
                std::ptr::from_mut(&mut validation_error).cast(),
                self.validation_error.pointer,
                4,
                stream,
            )
        })?;
        check(unsafe { glmaxx_stream_synchronize(stream) })?;
        if validation_error == 0 {
            Ok(host_output)
        } else {
            Err(KernelError::DeviceValidation(validation_error))
        }
    }
}

impl NativeFc2Case {
    fn download(&self, stream: u64) -> Result<Vec<f32>, KernelError> {
        let output_words = u64::from(self.descriptor.rows)
            .checked_mul(u64::from(HIDDEN))
            .ok_or(KernelError::Overflow)?;
        let output_bytes = output_words.checked_mul(4).ok_or(KernelError::Overflow)?;
        let mut host_output =
            vec![0.0_f32; usize::try_from(output_words).map_err(|_| KernelError::Overflow)?];
        let mut validation_error = 0_u32;
        // SAFETY: both destination slices and both source allocations cover
        // the exact requested byte ranges and remain live through sync.
        check(unsafe {
            glmaxx_memcpy_d2h(
                host_output.as_mut_ptr().cast(),
                self.output.pointer,
                output_bytes,
                stream,
            )
        })?;
        check(unsafe {
            glmaxx_memcpy_d2h(
                std::ptr::from_mut(&mut validation_error).cast(),
                self.validation_error.pointer,
                4,
                stream,
            )
        })?;
        check(unsafe { glmaxx_stream_synchronize(stream) })?;
        if validation_error == 0 {
            Ok(host_output)
        } else {
            Err(KernelError::DeviceValidation(validation_error))
        }
    }
}

impl NativeFc1Case {
    fn download(&self, stream: u64) -> Result<Vec<u16>, KernelError> {
        let output_words = u64::from(self.descriptor.assignments)
            .checked_mul(u64::from(LOCAL_INTERMEDIATE))
            .ok_or(KernelError::Overflow)?;
        let output_bytes = output_words.checked_mul(2).ok_or(KernelError::Overflow)?;
        let mut host_output =
            vec![0_u16; usize::try_from(output_words).map_err(|_| KernelError::Overflow)?];
        // SAFETY: the source and destination cover exactly the output allocation.
        check(unsafe {
            glmaxx_memcpy_d2h(
                host_output.as_mut_ptr().cast(),
                self.output.pointer,
                output_bytes,
                stream,
            )
        })?;
        check(unsafe { glmaxx_stream_synchronize(stream) })?;
        Ok(host_output)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GraphReplay {
    pub output_bf16: Vec<u16>,
    pub repeat_count: u32,
    pub bitwise_deterministic: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Fc1Timing {
    pub warmup_iterations: u32,
    pub measured_iterations: u32,
    pub activation_quantization_us: f32,
    pub core_swiglu_us: f32,
    pub inclusive_operator_us: f32,
    pub graph_inclusive_us: f32,
    pub host_enqueue_us: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GroupedFc1Timing {
    pub warmup_iterations: u32,
    pub measured_iterations: u32,
    pub active_experts: u32,
    pub activation_quantization_us: f32,
    pub grouped_core_swiglu_us: f32,
    pub inclusive_operator_us: f32,
    pub host_enqueue_us: f64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Fc1BenchmarkConfig {
    pub warmup_iterations: u32,
    pub measured_iterations: u32,
}

pub struct NativeFc1Fixture {
    stream: NativeStream,
    weight_values: NativeBuffer,
    weight_scales: NativeBuffer,
    weight_globals: NativeBuffer,
    initialized_experts: [bool; 256],
}

impl NativeFc1Fixture {
    pub fn replicated(weights: &PackedNvfp4, experts: &[u16]) -> Result<Self, KernelError> {
        validate_weights(weights)?;
        if experts.is_empty() {
            return Err(KernelError::Shape);
        }
        let mut initialized_experts = [false; 256];
        for &expert in experts {
            *initialized_experts
                .get_mut(usize::from(expert))
                .ok_or(KernelError::Shape)? = true;
        }
        let first_expert = initialized_experts
            .iter()
            .position(|&initialized| initialized)
            .ok_or(KernelError::Shape)?;
        let allocated_experts = initialized_experts
            .iter()
            .rposition(|&initialized| initialized)
            .and_then(|index| index.checked_add(1))
            .ok_or(KernelError::Shape)?;
        let value_stride =
            u64::try_from(weights.values.len()).map_err(|_| KernelError::Overflow)?;
        let scale_stride =
            u64::try_from(weights.scales.len()).map_err(|_| KernelError::Overflow)?;
        let value_bytes = value_stride
            .checked_mul(allocated_experts as u64)
            .ok_or(KernelError::Overflow)?;
        let scale_bytes = scale_stride
            .checked_mul(allocated_experts as u64)
            .ok_or(KernelError::Overflow)?;
        let stream = NativeStream::create()?;
        let weight_values = NativeBuffer::allocate(value_bytes)?;
        let weight_scales = NativeBuffer::allocate(scale_bytes)?;
        let first_value_offset = value_stride
            .checked_mul(first_expert as u64)
            .ok_or(KernelError::Overflow)?;
        let first_scale_offset = scale_stride
            .checked_mul(first_expert as u64)
            .ok_or(KernelError::Overflow)?;
        weight_values.upload_at(&weights.values, first_value_offset, stream.0)?;
        weight_scales.upload_at(&weights.scales, first_scale_offset, stream.0)?;
        for (expert, &initialized) in initialized_experts.iter().enumerate() {
            if initialized && expert != first_expert {
                weight_values.copy_within(
                    first_value_offset,
                    value_stride
                        .checked_mul(expert as u64)
                        .ok_or(KernelError::Overflow)?,
                    value_stride,
                    stream.0,
                )?;
                weight_scales.copy_within(
                    first_scale_offset,
                    scale_stride
                        .checked_mul(expert as u64)
                        .ok_or(KernelError::Overflow)?,
                    scale_stride,
                    stream.0,
                )?;
            }
        }
        let mut globals = vec![0.0_f32; allocated_experts];
        for (expert, &initialized) in initialized_experts.iter().enumerate() {
            if initialized {
                globals[expert] = weights.metadata.global_scale;
            }
        }
        let weight_globals = NativeBuffer::upload(floats_as_bytes(&globals), stream.0)?;
        // The source slices may be dropped as soon as this constructor returns.
        check(unsafe { glmaxx_stream_synchronize(stream.0) })?;
        Ok(Self {
            stream,
            weight_values,
            weight_scales,
            weight_globals,
            initialized_experts,
        })
    }

    pub fn run(
        &self,
        input_bf16: &[u16],
        rows: u32,
        route_experts: &[u16],
        route_tokens: &[u32],
        route_slots: &[u8],
    ) -> Result<Vec<u16>, KernelError> {
        let expert_offsets =
            self.validate_case(input_bf16, rows, route_experts, route_tokens, route_slots)?;
        self.run_validated(
            input_bf16,
            rows,
            route_experts,
            route_tokens,
            route_slots,
            &expert_offsets,
        )
    }

    pub fn run_graph_repeated(
        &self,
        input_bf16: &[u16],
        rows: u32,
        route_experts: &[u16],
        route_tokens: &[u32],
        route_slots: &[u8],
        repeat_count: u32,
    ) -> Result<GraphReplay, KernelError> {
        if repeat_count == 0 || repeat_count > 100 {
            return Err(KernelError::Shape);
        }
        let expert_offsets =
            self.validate_case(input_bf16, rows, route_experts, route_tokens, route_slots)?;
        let execution = self.prepare_case(
            input_bf16,
            rows,
            route_experts,
            route_tokens,
            route_slots,
            &expert_offsets,
        )?;
        let mut graph_exec = 0_u64;
        // SAFETY: the descriptor and every referenced allocation remain live
        // for graph instantiation and every replay below.
        check(unsafe {
            glmaxx_nvfp4_routed_fc1_graph_instantiate(
                std::ptr::from_ref(&execution.descriptor),
                self.stream.0 as *mut c_void,
                std::ptr::from_mut(&mut graph_exec),
            )
        })?;
        if graph_exec == 0 {
            return Err(KernelError::Driver(-1));
        }
        let graph = NativeGraph(graph_exec);
        let mut first_output = None;
        let mut bitwise_deterministic = true;
        for _ in 0..repeat_count {
            // SAFETY: graph and stream are caller-owned and live.
            check(unsafe { glmaxx_graph_exec_launch(graph.0, self.stream.0) })?;
            let output = execution.download(self.stream.0)?;
            if let Some(first) = &first_output {
                bitwise_deterministic &= first == &output;
            } else {
                first_output = Some(output);
            }
        }
        Ok(GraphReplay {
            output_bf16: first_output.ok_or(KernelError::Shape)?,
            repeat_count,
            bitwise_deterministic,
        })
    }

    pub fn run_dense_control(
        &self,
        input_bf16: &[u16],
        rows: u32,
        route_experts: &[u16],
        route_tokens: &[u32],
        route_slots: &[u8],
    ) -> Result<Vec<u16>, KernelError> {
        let &expert = route_experts.first().ok_or(KernelError::Shape)?;
        if route_experts.iter().any(|&candidate| candidate != expert) {
            return Err(KernelError::Shape);
        }
        let expert_offsets =
            self.validate_case(input_bf16, rows, route_experts, route_tokens, route_slots)?;
        let execution = self.prepare_case(
            input_bf16,
            rows,
            route_experts,
            route_tokens,
            route_slots,
            &expert_offsets,
        )?;
        let mut async_error = 0_i32;
        // SAFETY: the prepared descriptor and every referenced allocation
        // remain live until the output download synchronizes the stream. The
        // one-expert restriction is checked above.
        let status = unsafe {
            glmaxx_nvfp4_dense_control_launch(
                std::ptr::from_ref(&execution.descriptor),
                u32::from(expert),
                self.stream.0 as *mut c_void,
                std::ptr::from_mut(&mut async_error),
            )
        };
        if status != 0 {
            return Err(KernelError::Driver(status));
        }
        if async_error != 0 {
            return Err(KernelError::Async(async_error));
        }
        execution.download(self.stream.0)
    }

    pub fn run_grouped_control(
        &self,
        input_bf16: &[u16],
        rows: u32,
        route_experts: &[u16],
        route_tokens: &[u32],
        route_slots: &[u8],
    ) -> Result<Vec<u16>, KernelError> {
        let expert_offsets =
            self.validate_case(input_bf16, rows, route_experts, route_tokens, route_slots)?;
        let active_experts = active_experts_for_grouped(route_experts, &expert_offsets)?;
        let execution = self.prepare_case(
            input_bf16,
            rows,
            route_experts,
            route_tokens,
            route_slots,
            &expert_offsets,
        )?;
        launch_native_grouped_control(&execution.descriptor, &active_experts, self.stream.0, true)?;
        execution.download(self.stream.0)
    }

    pub fn benchmark(
        &self,
        input_bf16: &[u16],
        rows: u32,
        route_experts: &[u16],
        route_tokens: &[u32],
        route_slots: &[u8],
        config: Fc1BenchmarkConfig,
    ) -> Result<Fc1Timing, KernelError> {
        if config.warmup_iterations == 0
            || config.warmup_iterations > 100
            || config.measured_iterations == 0
            || config.measured_iterations > 10_000
        {
            return Err(KernelError::Shape);
        }
        let expert_offsets =
            self.validate_case(input_bf16, rows, route_experts, route_tokens, route_slots)?;
        let execution = self.prepare_case(
            input_bf16,
            rows,
            route_experts,
            route_tokens,
            route_slots,
            &expert_offsets,
        )?;

        for _ in 0..config.warmup_iterations {
            launch_native_fc1(&execution.descriptor, self.stream.0)?;
        }
        check(unsafe { glmaxx_stream_synchronize(self.stream.0) })?;

        let activation_quantization_us =
            time_cuda_launches(self.stream.0, config.measured_iterations, || {
                // SAFETY: the descriptor and all referenced allocations are
                // live for the complete benchmark.
                check(unsafe {
                    glmaxx_nvfp4_quantize_launch(
                        std::ptr::from_ref(&execution.descriptor),
                        self.stream.0 as *mut c_void,
                    )
                })
            })?;
        let core_swiglu_us = time_cuda_launches(self.stream.0, config.measured_iterations, || {
            // SAFETY: quantization above populated the activation buffers and
            // the descriptor remains live.
            check(unsafe {
                glmaxx_nvfp4_core_swiglu_launch(
                    std::ptr::from_ref(&execution.descriptor),
                    self.stream.0 as *mut c_void,
                )
            })
        })?;
        let inclusive_operator_us =
            time_cuda_launches(self.stream.0, config.measured_iterations, || {
                launch_native_fc1(&execution.descriptor, self.stream.0)
            })?;

        let mut graph_exec = 0_u64;
        // SAFETY: the fixed descriptor and buffers remain live for all graph
        // timing iterations.
        check(unsafe {
            glmaxx_nvfp4_routed_fc1_graph_instantiate(
                std::ptr::from_ref(&execution.descriptor),
                self.stream.0 as *mut c_void,
                std::ptr::from_mut(&mut graph_exec),
            )
        })?;
        if graph_exec == 0 {
            return Err(KernelError::Driver(-1));
        }
        let graph = NativeGraph(graph_exec);
        let graph_inclusive_us =
            time_cuda_launches(self.stream.0, config.measured_iterations, || {
                // SAFETY: graph and stream are caller-owned and live.
                check(unsafe { glmaxx_graph_exec_launch(graph.0, self.stream.0) })
            })?;

        let enqueue_start = Instant::now();
        for _ in 0..config.measured_iterations {
            launch_native_fc1(&execution.descriptor, self.stream.0)?;
        }
        let host_enqueue_us = enqueue_start.elapsed().as_secs_f64() * 1_000_000.0
            / f64::from(config.measured_iterations);
        check(unsafe { glmaxx_stream_synchronize(self.stream.0) })?;

        Ok(Fc1Timing {
            warmup_iterations: config.warmup_iterations,
            measured_iterations: config.measured_iterations,
            activation_quantization_us,
            core_swiglu_us,
            inclusive_operator_us,
            graph_inclusive_us,
            host_enqueue_us,
        })
    }

    pub fn benchmark_grouped_control(
        &self,
        input_bf16: &[u16],
        rows: u32,
        route_experts: &[u16],
        route_tokens: &[u32],
        route_slots: &[u8],
        config: Fc1BenchmarkConfig,
    ) -> Result<GroupedFc1Timing, KernelError> {
        if config.warmup_iterations == 0
            || config.warmup_iterations > 100
            || config.measured_iterations == 0
            || config.measured_iterations > 10_000
        {
            return Err(KernelError::Shape);
        }
        let expert_offsets =
            self.validate_case(input_bf16, rows, route_experts, route_tokens, route_slots)?;
        let active_experts = active_experts_for_grouped(route_experts, &expert_offsets)?;
        let active_expert_count =
            u32::try_from(active_experts.len()).map_err(|_| KernelError::Overflow)?;
        let execution = self.prepare_case(
            input_bf16,
            rows,
            route_experts,
            route_tokens,
            route_slots,
            &expert_offsets,
        )?;
        prepare_native_grouped_control(&execution.descriptor, &active_experts, self.stream.0)?;
        check(unsafe { glmaxx_stream_synchronize(self.stream.0) })?;

        for _ in 0..config.warmup_iterations {
            launch_native_grouped_prepared(
                &execution.descriptor,
                active_expert_count,
                self.stream.0,
                true,
            )?;
        }
        check(unsafe { glmaxx_stream_synchronize(self.stream.0) })?;

        let activation_quantization_us =
            time_cuda_launches(self.stream.0, config.measured_iterations, || {
                // SAFETY: the descriptor and all referenced allocations are
                // live for the complete benchmark.
                check(unsafe {
                    glmaxx_nvfp4_grouped_quantize_launch(
                        std::ptr::from_ref(&execution.descriptor),
                        self.stream.0 as *mut c_void,
                    )
                })
            })?;
        let grouped_core_swiglu_us =
            time_cuda_launches(self.stream.0, config.measured_iterations, || {
                launch_native_grouped_prepared(
                    &execution.descriptor,
                    active_expert_count,
                    self.stream.0,
                    false,
                )
            })?;
        let inclusive_operator_us =
            time_cuda_launches(self.stream.0, config.measured_iterations, || {
                launch_native_grouped_prepared(
                    &execution.descriptor,
                    active_expert_count,
                    self.stream.0,
                    true,
                )
            })?;

        let enqueue_start = Instant::now();
        for _ in 0..config.measured_iterations {
            launch_native_grouped_prepared(
                &execution.descriptor,
                active_expert_count,
                self.stream.0,
                true,
            )?;
        }
        let host_enqueue_us = enqueue_start.elapsed().as_secs_f64() * 1_000_000.0
            / f64::from(config.measured_iterations);
        check(unsafe { glmaxx_stream_synchronize(self.stream.0) })?;

        Ok(GroupedFc1Timing {
            warmup_iterations: config.warmup_iterations,
            measured_iterations: config.measured_iterations,
            active_experts: active_expert_count,
            activation_quantization_us,
            grouped_core_swiglu_us,
            inclusive_operator_us,
            host_enqueue_us,
        })
    }

    fn validate_case(
        &self,
        input_bf16: &[u16],
        rows: u32,
        route_experts: &[u16],
        route_tokens: &[u32],
        route_slots: &[u8],
    ) -> Result<[u32; 257], KernelError> {
        let assignments = u32::try_from(route_experts.len()).map_err(|_| KernelError::Overflow)?;
        if rows == 0
            || rows > 65_536
            || input_bf16.len()
                != (rows as usize)
                    .checked_mul(HIDDEN as usize)
                    .ok_or(KernelError::Overflow)?
            || assignments == 0
            || assignments > 65_535
            || assignments > rows.checked_mul(8).ok_or(KernelError::Overflow)?
            || route_tokens.len() != route_experts.len()
            || route_slots.len() != route_experts.len()
        {
            return Err(KernelError::Shape);
        }
        let mut slot_masks = vec![0_u8; rows as usize];
        let mut expert_masks = vec![[0_u64; 4]; rows as usize];
        let mut counts = [0_u32; 257];
        for ((&expert, &token), &slot) in route_experts.iter().zip(route_tokens).zip(route_slots) {
            let token_index = usize::try_from(token).map_err(|_| KernelError::Shape)?;
            let expert_index = usize::from(expert);
            if token >= rows
                || slot >= 8
                || !self
                    .initialized_experts
                    .get(expert_index)
                    .copied()
                    .unwrap_or(false)
            {
                return Err(KernelError::Shape);
            }
            let slot_bit = 1_u8 << slot;
            let expert_word = expert_index / 64;
            let expert_bit = 1_u64 << (expert_index % 64);
            if slot_masks[token_index] & slot_bit != 0
                || expert_masks[token_index][expert_word] & expert_bit != 0
            {
                return Err(KernelError::Shape);
            }
            slot_masks[token_index] |= slot_bit;
            expert_masks[token_index][expert_word] |= expert_bit;
            counts[expert_index + 1] = counts[expert_index + 1]
                .checked_add(1)
                .ok_or(KernelError::Overflow)?;
        }
        for expert in 1..counts.len() {
            counts[expert] = counts[expert]
                .checked_add(counts[expert - 1])
                .ok_or(KernelError::Overflow)?;
        }
        Ok(counts)
    }

    fn run_validated(
        &self,
        input_bf16: &[u16],
        rows: u32,
        route_experts: &[u16],
        route_tokens: &[u32],
        route_slots: &[u8],
        expert_offsets: &[u32; 257],
    ) -> Result<Vec<u16>, KernelError> {
        let execution = self.prepare_case(
            input_bf16,
            rows,
            route_experts,
            route_tokens,
            route_slots,
            expert_offsets,
        )?;
        let mut async_error = 0_i32;
        // SAFETY: all device allocations remain alive through synchronization.
        check(unsafe {
            glmaxx_nvfp4_routed_fc1_launch(
                std::ptr::from_ref(&execution.descriptor),
                self.stream.0 as *mut c_void,
                std::ptr::from_mut(&mut async_error),
            )
        })?;
        if async_error != 0 {
            return Err(KernelError::Async(async_error));
        }
        execution.download(self.stream.0)
    }

    fn prepare_case(
        &self,
        input_bf16: &[u16],
        rows: u32,
        route_experts: &[u16],
        route_tokens: &[u32],
        route_slots: &[u8],
        expert_offsets: &[u32; 257],
    ) -> Result<NativeFc1Case, KernelError> {
        let assignments = u32::try_from(route_experts.len()).map_err(|_| KernelError::Overflow)?;
        validate_native_library(assignments)?;
        let input = NativeBuffer::upload(words_as_bytes(input_bf16), self.stream.0)?;
        let route_weights = vec![1.0_f32; assignments as usize];
        let route_expert = NativeBuffer::upload(words_as_bytes(route_experts), self.stream.0)?;
        let route_token = NativeBuffer::upload(dwords_as_bytes(route_tokens), self.stream.0)?;
        let route_slot = NativeBuffer::upload(route_slots, self.stream.0)?;
        let route_weight = NativeBuffer::upload(floats_as_bytes(&route_weights), self.stream.0)?;
        let offsets = NativeBuffer::upload(dwords_as_bytes(expert_offsets), self.stream.0)?;
        let compacted = NativeBuffer::allocate(u64::from(assignments) * u64::from(HIDDEN) * 2)?;
        let grouped_sfa = grouped_sfa_plan(expert_offsets)?;
        compacted.upload_at(
            qwords_as_bytes(&grouped_sfa.expert_byte_offsets),
            0,
            self.stream.0,
        )?;
        let padded_assignments = u64::from(assignments.next_multiple_of(128));
        let activation_values = NativeBuffer::allocate(padded_assignments * u64::from(HIDDEN) / 2)?;
        let activation_scales = NativeBuffer::allocate(grouped_sfa_capacity_bytes(assignments)?)?;
        let activation_globals = NativeBuffer::allocate(u64::from(assignments) * 4)?;
        let gate_up =
            NativeBuffer::allocate(u64::from(assignments) * u64::from(crate::LOCAL_GATE_UP) * 4)?;
        let output =
            NativeBuffer::allocate(u64::from(assignments) * u64::from(LOCAL_INTERMEDIATE) * 2)?;
        let mut descriptor = Fc1Descriptor::new(LaunchGeometry {
            rows,
            assignments,
            path: if rows <= 128 {
                KernelPath::DecodePersistent
            } else {
                KernelPath::PrefillGrouped
            },
        });
        descriptor.input_bf16 = input.pointer;
        descriptor.expert_value_base = self.weight_values.pointer;
        descriptor.expert_scale_base = self.weight_scales.pointer;
        descriptor.expert_global_scales = self.weight_globals.pointer;
        descriptor.route_experts_u16 = route_expert.pointer;
        descriptor.route_tokens_u32 = route_token.pointer;
        descriptor.route_slots_u8 = route_slot.pointer;
        descriptor.route_weights_f32 = route_weight.pointer;
        descriptor.expert_offsets_u32 = offsets.pointer;
        descriptor.compacted_input_bf16 = compacted.pointer;
        descriptor.activation_values = activation_values.pointer;
        descriptor.activation_scales = activation_scales.pointer;
        descriptor.activation_global_scales = activation_globals.pointer;
        descriptor.gate_up_accum_f32 = gate_up.pointer;
        descriptor.output_bf16 = output.pointer;
        descriptor.workspace_bytes = grouped_workspace_bytes(assignments)?;
        descriptor.sequence = 1;
        validate_descriptor(&descriptor)?;
        // Ensure all H2D inputs are complete before returning a replayable case
        // or beginning stream capture.
        check(unsafe { glmaxx_stream_synchronize(self.stream.0) })?;
        Ok(NativeFc1Case {
            _input: input,
            _route_expert: route_expert,
            _route_token: route_token,
            _route_slot: route_slot,
            _route_weight: route_weight,
            _offsets: offsets,
            _compacted: compacted,
            _activation_values: activation_values,
            _activation_scales: activation_scales,
            _activation_globals: activation_globals,
            _gate_up: gate_up,
            output,
            descriptor,
        })
    }
}

pub struct NativeFc2Fixture {
    stream: NativeStream,
    weight_values: NativeBuffer,
    weight_scales: NativeBuffer,
    weight_globals: NativeBuffer,
    initialized_experts: [bool; 256],
}

impl NativeFc2Fixture {
    pub fn replicated(weights: &PackedNvfp4, experts: &[u16]) -> Result<Self, KernelError> {
        validate_fc2_weights(weights)?;
        if experts.is_empty() {
            return Err(KernelError::Shape);
        }
        let mut initialized_experts = [false; 256];
        for &expert in experts {
            *initialized_experts
                .get_mut(usize::from(expert))
                .ok_or(KernelError::Shape)? = true;
        }
        let first_expert = initialized_experts
            .iter()
            .position(|&initialized| initialized)
            .ok_or(KernelError::Shape)?;
        let allocated_experts = initialized_experts
            .iter()
            .rposition(|&initialized| initialized)
            .and_then(|index| index.checked_add(1))
            .ok_or(KernelError::Shape)?;
        let value_stride =
            u64::try_from(weights.values.len()).map_err(|_| KernelError::Overflow)?;
        let scale_stride =
            u64::try_from(weights.scales.len()).map_err(|_| KernelError::Overflow)?;
        let value_bytes = value_stride
            .checked_mul(allocated_experts as u64)
            .ok_or(KernelError::Overflow)?;
        let scale_bytes = scale_stride
            .checked_mul(allocated_experts as u64)
            .ok_or(KernelError::Overflow)?;
        let stream = NativeStream::create()?;
        let weight_values = NativeBuffer::allocate(value_bytes)?;
        let weight_scales = NativeBuffer::allocate(scale_bytes)?;
        let first_value_offset = value_stride
            .checked_mul(first_expert as u64)
            .ok_or(KernelError::Overflow)?;
        let first_scale_offset = scale_stride
            .checked_mul(first_expert as u64)
            .ok_or(KernelError::Overflow)?;
        weight_values.upload_at(&weights.values, first_value_offset, stream.0)?;
        weight_scales.upload_at(&weights.scales, first_scale_offset, stream.0)?;
        for (expert, &initialized) in initialized_experts.iter().enumerate() {
            if initialized && expert != first_expert {
                weight_values.copy_within(
                    first_value_offset,
                    value_stride
                        .checked_mul(expert as u64)
                        .ok_or(KernelError::Overflow)?,
                    value_stride,
                    stream.0,
                )?;
                weight_scales.copy_within(
                    first_scale_offset,
                    scale_stride
                        .checked_mul(expert as u64)
                        .ok_or(KernelError::Overflow)?,
                    scale_stride,
                    stream.0,
                )?;
            }
        }
        let mut globals = vec![0.0_f32; allocated_experts];
        for (expert, &initialized) in initialized_experts.iter().enumerate() {
            if initialized {
                globals[expert] = weights.metadata.global_scale;
            }
        }
        let weight_globals = NativeBuffer::upload(floats_as_bytes(&globals), stream.0)?;
        check(unsafe { glmaxx_stream_synchronize(stream.0) })?;
        Ok(Self {
            stream,
            weight_values,
            weight_scales,
            weight_globals,
            initialized_experts,
        })
    }

    pub fn run(
        &self,
        input_bf16: &[u16],
        rows: u32,
        route_experts: &[u16],
        route_tokens: &[u32],
        route_slots: &[u8],
        route_weights: &[f32],
    ) -> Result<Vec<f32>, KernelError> {
        let execution = self.prepare_case(
            input_bf16,
            rows,
            route_experts,
            route_tokens,
            route_slots,
            route_weights,
        )?;
        launch_native_fc2(&execution.descriptor, self.stream.0)?;
        execution.download(self.stream.0)
    }

    pub fn run_dense_control(
        &self,
        input_bf16: &[u16],
        rows: u32,
        route_experts: &[u16],
        route_tokens: &[u32],
        route_slots: &[u8],
        route_weights: &[f32],
    ) -> Result<Vec<f32>, KernelError> {
        let &expert = route_experts.first().ok_or(KernelError::Shape)?;
        if route_experts.iter().any(|&candidate| candidate != expert) {
            return Err(KernelError::Shape);
        }
        let execution = self.prepare_case(
            input_bf16,
            rows,
            route_experts,
            route_tokens,
            route_slots,
            route_weights,
        )?;
        let mut async_error = 0_i32;
        // SAFETY: the prepared descriptor and all allocations remain live
        // until `download` synchronizes; every assignment uses `expert`.
        let status = unsafe {
            glmaxx_nvfp4_fc2_dense_control_launch(
                std::ptr::from_ref(&execution.descriptor),
                u32::from(expert),
                self.stream.0 as *mut c_void,
                std::ptr::from_mut(&mut async_error),
            )
        };
        check(status)?;
        if async_error != 0 {
            return Err(KernelError::Async(async_error));
        }
        execution.download(self.stream.0)
    }

    pub fn run_grouped_control(
        &self,
        input_bf16: &[u16],
        rows: u32,
        route_experts: &[u16],
        route_tokens: &[u32],
        route_slots: &[u8],
        route_weights: &[f32],
    ) -> Result<Vec<f32>, KernelError> {
        let mut active_experts = Vec::new();
        for &expert in route_experts {
            if active_experts.last().copied() != Some(expert) {
                active_experts.push(expert);
            }
        }
        let execution = self.prepare_case(
            input_bf16,
            rows,
            route_experts,
            route_tokens,
            route_slots,
            route_weights,
        )?;
        let active_expert_count =
            u32::try_from(active_experts.len()).map_err(|_| KernelError::Overflow)?;
        let mut async_error = 0_i32;
        // SAFETY: `prepare_case` proved exact expert-major ranges and the
        // active-expert slice and device allocations remain live through sync.
        let status = unsafe {
            glmaxx_nvfp4_fc2_grouped_control_launch(
                std::ptr::from_ref(&execution.descriptor),
                active_experts.as_ptr(),
                active_expert_count,
                self.stream.0 as *mut c_void,
                std::ptr::from_mut(&mut async_error),
            )
        };
        check(status)?;
        if async_error != 0 {
            return Err(KernelError::Async(async_error));
        }
        execution.download(self.stream.0)
    }

    fn prepare_case(
        &self,
        input_bf16: &[u16],
        rows: u32,
        route_experts: &[u16],
        route_tokens: &[u32],
        route_slots: &[u8],
        route_weights: &[f32],
    ) -> Result<NativeFc2Case, KernelError> {
        let assignments = u32::try_from(route_experts.len()).map_err(|_| KernelError::Overflow)?;
        if rows == 0
            || rows > 65_536
            || assignments == 0
            || assignments > 65_535
            || assignments > rows.checked_mul(8).ok_or(KernelError::Overflow)?
            || input_bf16.len()
                != (assignments as usize)
                    .checked_mul(LOCAL_INTERMEDIATE as usize)
                    .ok_or(KernelError::Overflow)?
            || route_tokens.len() != route_experts.len()
            || route_slots.len() != route_experts.len()
            || route_weights.len() != route_experts.len()
        {
            return Err(KernelError::Shape);
        }
        let mut slot_masks = vec![0_u8; rows as usize];
        let mut expert_masks = vec![[0_u64; 4]; rows as usize];
        let mut offsets = [0_u32; 257];
        let mut previous = None;
        for (((&expert, &token), &slot), &weight) in route_experts
            .iter()
            .zip(route_tokens)
            .zip(route_slots)
            .zip(route_weights)
        {
            let token_index = usize::try_from(token).map_err(|_| KernelError::Shape)?;
            let expert_index = usize::from(expert);
            let order = (expert, token, slot);
            if token >= rows
                || slot >= 8
                || !weight.is_finite()
                || weight < 0.0
                || !self
                    .initialized_experts
                    .get(expert_index)
                    .copied()
                    .unwrap_or(false)
                || previous.is_some_and(|prior| prior >= order)
            {
                return Err(KernelError::Shape);
            }
            let slot_bit = 1_u8 << slot;
            let expert_word = expert_index / 64;
            let expert_bit = 1_u64 << (expert_index % 64);
            if slot_masks[token_index] & slot_bit != 0
                || expert_masks[token_index][expert_word] & expert_bit != 0
            {
                return Err(KernelError::Shape);
            }
            slot_masks[token_index] |= slot_bit;
            expert_masks[token_index][expert_word] |= expert_bit;
            offsets[expert_index + 1] = offsets[expert_index + 1]
                .checked_add(1)
                .ok_or(KernelError::Overflow)?;
            previous = Some(order);
        }
        for expert in 1..offsets.len() {
            offsets[expert] = offsets[expert]
                .checked_add(offsets[expert - 1])
                .ok_or(KernelError::Overflow)?;
        }
        validate_native_fc2_library(rows, assignments)?;
        let input = NativeBuffer::upload(words_as_bytes(input_bf16), self.stream.0)?;
        let route_expert = NativeBuffer::upload(words_as_bytes(route_experts), self.stream.0)?;
        let route_token = NativeBuffer::upload(dwords_as_bytes(route_tokens), self.stream.0)?;
        let route_slot = NativeBuffer::upload(route_slots, self.stream.0)?;
        let route_weight = NativeBuffer::upload(floats_as_bytes(route_weights), self.stream.0)?;
        let offsets = NativeBuffer::upload(dwords_as_bytes(&offsets), self.stream.0)?;
        let padded_assignments = u64::from(assignments.next_multiple_of(128));
        let activation_values =
            NativeBuffer::allocate(padded_assignments * u64::from(LOCAL_INTERMEDIATE) / 2)?;
        let activation_scales =
            NativeBuffer::allocate(fc2_grouped_sfa_capacity_bytes(assignments)?)?;
        let activation_globals = NativeBuffer::allocate(u64::from(assignments) * 4)?;
        let assignment_down =
            NativeBuffer::allocate(u64::from(assignments) * u64::from(HIDDEN) * (4 + 2))?;
        let output = NativeBuffer::allocate(u64::from(rows) * u64::from(HIDDEN) * 4)?;
        let slot_assignment =
            NativeBuffer::allocate(u64::from(rows) * u64::from(crate::TOP_K) * 4)?;
        let validation_error = NativeBuffer::allocate(4)?;
        let mut descriptor = Fc2Descriptor::new(LaunchGeometry {
            rows,
            assignments,
            path: if rows <= 128 {
                KernelPath::DecodePersistent
            } else {
                KernelPath::PrefillGrouped
            },
        });
        descriptor.input_bf16 = input.pointer;
        descriptor.expert_value_base = self.weight_values.pointer;
        descriptor.expert_scale_base = self.weight_scales.pointer;
        descriptor.expert_global_scales = self.weight_globals.pointer;
        descriptor.route_experts_u16 = route_expert.pointer;
        descriptor.route_tokens_u32 = route_token.pointer;
        descriptor.route_slots_u8 = route_slot.pointer;
        descriptor.route_weights_f32 = route_weight.pointer;
        descriptor.expert_offsets_u32 = offsets.pointer;
        descriptor.activation_values = activation_values.pointer;
        descriptor.activation_scales = activation_scales.pointer;
        descriptor.activation_global_scales = activation_globals.pointer;
        descriptor.assignment_down_f32 = assignment_down.pointer;
        descriptor.token_output_f32 = output.pointer;
        descriptor.slot_assignment_u32 = slot_assignment.pointer;
        descriptor.validation_error_u32 = validation_error.pointer;
        descriptor.workspace_bytes = fc2_grouped_workspace_bytes(rows, assignments)?;
        descriptor.sequence = 1;
        crate::validate_fc2_descriptor(&descriptor)?;
        check(unsafe { glmaxx_stream_synchronize(self.stream.0) })?;
        Ok(NativeFc2Case {
            _input: input,
            _route_expert: route_expert,
            _route_token: route_token,
            _route_slot: route_slot,
            _route_weight: route_weight,
            _offsets: offsets,
            _activation_values: activation_values,
            _activation_scales: activation_scales,
            _activation_globals: activation_globals,
            _assignment_down: assignment_down,
            output,
            _slot_assignment: slot_assignment,
            validation_error,
            descriptor,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Exl3Replay {
    pub output_f16: Vec<u16>,
    pub repeat_count: u32,
    pub bitwise_deterministic: bool,
}

/// Owns one direct source-order EXL3 projection on the active CUDA device.
///
/// The source trellis and rotations are uploaded without transformation.
/// Per-launch scratch contains only rotated activations and projected FP16
/// rows, never a reconstructed persistent weight matrix.
pub struct NativeExl3Fixture {
    stream: NativeStream,
    trellis: NativeBuffer,
    suh: NativeBuffer,
    svh: NativeBuffer,
    projection: Exl3KernelProjection,
    logical_k: u32,
    logical_n: u32,
}

impl NativeExl3Fixture {
    pub fn from_source(tensor: &Exl3Trellis) -> Result<Self, KernelError> {
        tensor.validate().map_err(|_| KernelError::Shape)?;
        let projection = match tensor.metadata.projection {
            Exl3Projection::Gate => Exl3KernelProjection::Gate,
            Exl3Projection::Up => Exl3KernelProjection::Up,
            Exl3Projection::Down => Exl3KernelProjection::Down,
        };
        let logical_k = tensor.metadata.logical_k;
        let logical_n = tensor.metadata.logical_n;
        let trellis_bytes = u64::try_from(std::mem::size_of_val(tensor.trellis.as_slice()))
            .map_err(|_| KernelError::Overflow)?;
        if trellis_bytes
            != exl3_trellis_bytes(logical_k, logical_n, u32::from(tensor.metadata.bits))?
        {
            return Err(KernelError::Shape);
        }
        let stream = NativeStream::create()?;
        let trellis = NativeBuffer::upload(words_as_bytes(&tensor.trellis), stream.0)?;
        let suh = NativeBuffer::upload(words_as_bytes(&tensor.suh), stream.0)?;
        let svh = NativeBuffer::upload(words_as_bytes(&tensor.svh), stream.0)?;
        check(unsafe { glmaxx_stream_synchronize(stream.0) })?;
        Ok(Self {
            stream,
            trellis,
            suh,
            svh,
            projection,
            logical_k,
            logical_n,
        })
    }

    pub fn run(&self, input_f16: &[u16], rows: u32) -> Result<Vec<u16>, KernelError> {
        let execution = self.prepare_case(input_f16, rows)?;
        launch_native_exl3(&execution.descriptor, self.stream.0)?;
        execution.download(self.stream.0)
    }

    pub fn run_repeated(
        &self,
        input_f16: &[u16],
        rows: u32,
        repeat_count: u32,
    ) -> Result<Exl3Replay, KernelError> {
        if repeat_count == 0 || repeat_count > 100 {
            return Err(KernelError::Shape);
        }
        let execution = self.prepare_case(input_f16, rows)?;
        let mut first_output = None;
        let mut bitwise_deterministic = true;
        for _ in 0..repeat_count {
            launch_native_exl3(&execution.descriptor, self.stream.0)?;
            let output = execution.download(self.stream.0)?;
            if let Some(first) = &first_output {
                bitwise_deterministic &= first == &output;
            } else {
                first_output = Some(output);
            }
        }
        Ok(Exl3Replay {
            output_f16: first_output.ok_or(KernelError::Shape)?,
            repeat_count,
            bitwise_deterministic,
        })
    }

    fn prepare_case(&self, input_f16: &[u16], rows: u32) -> Result<NativeExl3Case, KernelError> {
        let expected_input = usize::try_from(rows)
            .map_err(|_| KernelError::Overflow)?
            .checked_mul(usize::try_from(self.logical_k).map_err(|_| KernelError::Overflow)?)
            .ok_or(KernelError::Overflow)?;
        if input_f16.len() != expected_input {
            return Err(KernelError::Shape);
        }
        validate_native_exl3_library(rows, self.logical_k, self.logical_n)?;
        let input = NativeBuffer::upload(words_as_bytes(input_f16), self.stream.0)?;
        let rotated_input =
            NativeBuffer::allocate(u64::from(rows) * u64::from(self.logical_k) * 2)?;
        let projected = NativeBuffer::allocate(u64::from(rows) * u64::from(self.logical_n) * 2)?;
        let output = NativeBuffer::allocate(u64::from(rows) * u64::from(self.logical_n) * 2)?;
        let validation_error = NativeBuffer::allocate(4)?;
        let mut descriptor = Exl3Descriptor::new(rows, self.projection);
        if descriptor.logical_k != self.logical_k || descriptor.logical_n != self.logical_n {
            return Err(KernelError::Shape);
        }
        descriptor.input_f16 = input.pointer;
        descriptor.trellis_u16 = self.trellis.pointer;
        descriptor.suh_f16 = self.suh.pointer;
        descriptor.svh_f16 = self.svh.pointer;
        descriptor.rotated_input_f16 = rotated_input.pointer;
        descriptor.projected_f16 = projected.pointer;
        descriptor.output_f16 = output.pointer;
        descriptor.validation_error_u32 = validation_error.pointer;
        descriptor.workspace_bytes = exl3_workspace_bytes(rows, self.logical_k, self.logical_n)?;
        descriptor.sequence = 1;
        validate_exl3_descriptor(&descriptor)?;
        check(unsafe { glmaxx_stream_synchronize(self.stream.0) })?;
        Ok(NativeExl3Case {
            _input: input,
            _rotated_input: rotated_input,
            _projected: projected,
            output,
            validation_error,
            descriptor,
        })
    }
}

pub fn run_single_expert(
    input_bf16: &[u16],
    rows: u32,
    weights: &PackedNvfp4,
) -> Result<Vec<u16>, KernelError> {
    validate_weights(weights)?;
    if rows == 0
        || rows > 65_536
        || input_bf16.len()
            != (rows as usize)
                .checked_mul(HIDDEN as usize)
                .ok_or(KernelError::Overflow)?
    {
        return Err(KernelError::Shape);
    }
    let route_experts = vec![0_u16; rows as usize];
    let route_tokens: Vec<u32> = (0..rows).collect();
    let route_slots = vec![0_u8; rows as usize];
    NativeFc1Fixture::replicated(weights, &[0])?.run(
        input_bf16,
        rows,
        &route_experts,
        &route_tokens,
        &route_slots,
    )
}

fn validate_weights(weights: &PackedNvfp4) -> Result<(), KernelError> {
    if weights.metadata.logical_n != 1024
        || weights.metadata.logical_k != HIDDEN
        || weights.metadata.padded_n != 1024
        || weights.metadata.padded_k != HIDDEN
        || weights.metadata.codec != Codec::OneDimensional
        || weights.values.len() != 1024 * HIDDEN as usize / 2
        || weights.scales.len() != 1024 * HIDDEN as usize / 16
        || weights.validate().is_err()
    {
        Err(KernelError::Shape)
    } else {
        Ok(())
    }
}

fn validate_fc2_weights(weights: &PackedNvfp4) -> Result<(), KernelError> {
    if weights.metadata.logical_n != HIDDEN
        || weights.metadata.logical_k != LOCAL_INTERMEDIATE
        || weights.metadata.padded_n != HIDDEN
        || weights.metadata.padded_k != LOCAL_INTERMEDIATE
        || weights.metadata.codec != Codec::OneDimensional
        || weights.values.len() != HIDDEN as usize * LOCAL_INTERMEDIATE as usize / 2
        || weights.scales.len() != HIDDEN as usize * LOCAL_INTERMEDIATE as usize / 16
        || weights.validate().is_err()
    {
        Err(KernelError::Shape)
    } else {
        Ok(())
    }
}

fn words_as_bytes(words: &[u16]) -> &[u8] {
    // SAFETY: u16 has no invalid bit patterns and the byte length is exact.
    unsafe { std::slice::from_raw_parts(words.as_ptr().cast(), std::mem::size_of_val(words)) }
}

fn dwords_as_bytes(words: &[u32]) -> &[u8] {
    // SAFETY: u32 has no invalid bit patterns and the byte length is exact.
    unsafe { std::slice::from_raw_parts(words.as_ptr().cast(), std::mem::size_of_val(words)) }
}

fn floats_as_bytes(values: &[f32]) -> &[u8] {
    // SAFETY: f32 has no invalid bit patterns and the byte length is exact.
    unsafe { std::slice::from_raw_parts(values.as_ptr().cast(), std::mem::size_of_val(values)) }
}

fn qwords_as_bytes(words: &[u64]) -> &[u8] {
    // SAFETY: u64 has no invalid bit patterns and the byte length is exact.
    unsafe { std::slice::from_raw_parts(words.as_ptr().cast(), std::mem::size_of_val(words)) }
}

fn launch_native_fc1(descriptor: &Fc1Descriptor, stream: u64) -> Result<(), KernelError> {
    let mut async_error = 0_i32;
    // SAFETY: the caller keeps the descriptor allocations and stream live.
    check(unsafe {
        glmaxx_nvfp4_routed_fc1_launch(
            std::ptr::from_ref(descriptor),
            stream as *mut c_void,
            std::ptr::from_mut(&mut async_error),
        )
    })?;
    if async_error == 0 {
        Ok(())
    } else {
        Err(KernelError::Async(async_error))
    }
}

fn launch_native_fc2(descriptor: &Fc2Descriptor, stream: u64) -> Result<(), KernelError> {
    let mut async_error = 0_i32;
    // SAFETY: the caller keeps the descriptor allocations and stream live.
    check(unsafe {
        glmaxx_nvfp4_routed_fc2_launch(
            std::ptr::from_ref(descriptor),
            stream as *mut c_void,
            std::ptr::from_mut(&mut async_error),
        )
    })?;
    if async_error == 0 {
        Ok(())
    } else {
        Err(KernelError::Async(async_error))
    }
}

fn launch_native_exl3(descriptor: &Exl3Descriptor, stream: u64) -> Result<(), KernelError> {
    let mut async_error = 0_i32;
    // SAFETY: the caller keeps the descriptor allocations and stream live.
    check(unsafe {
        glmaxx_exl3_projection_launch(
            std::ptr::from_ref(descriptor),
            stream as *mut c_void,
            std::ptr::from_mut(&mut async_error),
        )
    })?;
    if async_error == 0 {
        Ok(())
    } else {
        Err(KernelError::Async(async_error))
    }
}

fn launch_native_grouped_control(
    descriptor: &Fc1Descriptor,
    active_experts: &[u16],
    stream: u64,
    inclusive: bool,
) -> Result<(), KernelError> {
    let active_expert_count =
        u32::try_from(active_experts.len()).map_err(|_| KernelError::Overflow)?;
    let mut async_error = 0_i32;
    // SAFETY: the caller keeps the descriptor allocations, active-expert
    // slice, and stream live through synchronization.
    let status = unsafe {
        if inclusive {
            glmaxx_nvfp4_grouped_control_launch(
                std::ptr::from_ref(descriptor),
                active_experts.as_ptr(),
                active_expert_count,
                stream as *mut c_void,
                std::ptr::from_mut(&mut async_error),
            )
        } else {
            glmaxx_nvfp4_grouped_core_swiglu_launch(
                std::ptr::from_ref(descriptor),
                active_experts.as_ptr(),
                active_expert_count,
                stream as *mut c_void,
                std::ptr::from_mut(&mut async_error),
            )
        }
    };
    check(status)?;
    if async_error == 0 {
        Ok(())
    } else {
        Err(KernelError::Async(async_error))
    }
}

fn prepare_native_grouped_control(
    descriptor: &Fc1Descriptor,
    active_experts: &[u16],
    stream: u64,
) -> Result<(), KernelError> {
    let active_expert_count =
        u32::try_from(active_experts.len()).map_err(|_| KernelError::Overflow)?;
    // SAFETY: the caller keeps the descriptor allocations, active-expert
    // slice, and stream live until the enqueued copy and metadata initializer
    // complete.
    check(unsafe {
        glmaxx_nvfp4_grouped_prepare_launch(
            std::ptr::from_ref(descriptor),
            active_experts.as_ptr(),
            active_expert_count,
            stream as *mut c_void,
        )
    })
}

fn launch_native_grouped_prepared(
    descriptor: &Fc1Descriptor,
    active_expert_count: u32,
    stream: u64,
    inclusive: bool,
) -> Result<(), KernelError> {
    let mut async_error = 0_i32;
    // SAFETY: `prepare_native_grouped_control` completed on this stream for
    // the same descriptor and group count; all allocations remain live.
    let status = unsafe {
        if inclusive {
            glmaxx_nvfp4_grouped_prepared_control_launch(
                std::ptr::from_ref(descriptor),
                active_expert_count,
                stream as *mut c_void,
                std::ptr::from_mut(&mut async_error),
            )
        } else {
            glmaxx_nvfp4_grouped_prepared_core_swiglu_launch(
                std::ptr::from_ref(descriptor),
                active_expert_count,
                stream as *mut c_void,
                std::ptr::from_mut(&mut async_error),
            )
        }
    };
    check(status)?;
    if async_error == 0 {
        Ok(())
    } else {
        Err(KernelError::Async(async_error))
    }
}

fn time_cuda_launches(
    stream: u64,
    iterations: u32,
    mut launch: impl FnMut() -> Result<(), KernelError>,
) -> Result<f32, KernelError> {
    let start = NativeEvent::create()?;
    let end = NativeEvent::create()?;
    start.record(stream)?;
    for _ in 0..iterations {
        launch()?;
    }
    end.record(stream)?;
    end.synchronize()?;
    Ok(start.elapsed_ms(&end)? * 1_000.0 / iterations as f32)
}

fn check(status: i32) -> Result<(), KernelError> {
    if status == 0 {
        Ok(())
    } else {
        Err(KernelError::Driver(status))
    }
}

fn validate_native_library(assignments: u32) -> Result<(), KernelError> {
    // SAFETY: both functions return immutable process-lifetime ABI metadata.
    let native_abi = unsafe { glmaxx_kernel_abi() };
    if native_abi.is_null()
        || unsafe { CStr::from_ptr(native_abi) }.to_bytes() != KERNEL_ABI.as_bytes()
        || unsafe { glmaxx_nvfp4_routed_fc1_workspace_bytes(assignments) }
            != workspace_bytes(assignments)?
        || unsafe { glmaxx_nvfp4_grouped_workspace_bytes(assignments) }
            != grouped_workspace_bytes(assignments)?
    {
        return Err(KernelError::Abi);
    }
    Ok(())
}

fn validate_native_fc2_library(rows: u32, assignments: u32) -> Result<(), KernelError> {
    validate_native_library(assignments)?;
    // SAFETY: this pure ABI helper neither initializes CUDA nor touches a
    // device; it mirrors Rust's checked workspace arithmetic.
    if unsafe { glmaxx_nvfp4_routed_fc2_workspace_bytes(rows, assignments) }
        != fc2_workspace_bytes(rows, assignments)?
        || unsafe { glmaxx_nvfp4_grouped_fc2_workspace_bytes(rows, assignments) }
            != fc2_grouped_workspace_bytes(rows, assignments)?
    {
        return Err(KernelError::Abi);
    }
    Ok(())
}

fn validate_native_exl3_library(
    rows: u32,
    logical_k: u32,
    logical_n: u32,
) -> Result<(), KernelError> {
    // SAFETY: both functions return immutable process-lifetime ABI metadata
    // or perform pure checked integer arithmetic without creating a context.
    let native_abi = unsafe { glmaxx_exl3_kernel_abi() };
    if native_abi.is_null()
        || unsafe { CStr::from_ptr(native_abi) }.to_bytes() != EXL3_KERNEL_ABI.as_bytes()
        || unsafe { glmaxx_exl3_projection_workspace_bytes(rows, logical_k, logical_n) }
            != exl3_workspace_bytes(rows, logical_k, logical_n)?
    {
        return Err(KernelError::Abi);
    }
    Ok(())
}

/// Verifies the loaded native library's ABI identifier and workspace formula
/// without creating a CUDA context or touching a device.
pub fn validate_native_abi(assignments: u32) -> Result<(), KernelError> {
    validate_native_library(assignments)
}

/// Verifies both routed FC1 and FC2 workspace formulas for a concrete step
/// shape without creating a CUDA context or touching a device.
pub fn validate_native_moe_abi(rows: u32, assignments: u32) -> Result<(), KernelError> {
    validate_native_fc2_library(rows, assignments)
}

/// Verifies the direct EXL3 source-projection ABI and workspace arithmetic
/// without creating a CUDA context or touching a device.
pub fn validate_native_exl3_abi(
    rows: u32,
    logical_k: u32,
    logical_n: u32,
) -> Result<(), KernelError> {
    validate_native_exl3_library(rows, logical_k, logical_n)
}
