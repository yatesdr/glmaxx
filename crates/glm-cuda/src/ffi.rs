use std::ffi::{CStr, c_char, c_void};

use glm_format::{Codec, KERNEL_ABI, PackedNvfp4};

use crate::{
    CudaDriver, Fc1Descriptor, HIDDEN, KernelError, KernelPath, LOCAL_INTERMEDIATE, LaunchGeometry,
    validate_descriptor, workspace_bytes,
};

unsafe extern "C" {
    fn glmaxx_nvfp4_routed_fc1_launch(
        descriptor: *const Fc1Descriptor,
        stream: *mut c_void,
        error_code: *mut i32,
    ) -> i32;
    fn glmaxx_nvfp4_routed_fc1_workspace_bytes(assignments: u32) -> u64;
    fn glmaxx_kernel_abi() -> *const c_char;
    fn glmaxx_device_alloc(bytes: u64, pointer: *mut u64) -> i32;
    fn glmaxx_device_free(pointer: u64) -> i32;
    fn glmaxx_stream_create(stream: *mut u64) -> i32;
    fn glmaxx_stream_destroy(stream: u64) -> i32;
    fn glmaxx_stream_query(stream: u64, complete: *mut i32) -> i32;
    fn glmaxx_stream_synchronize(stream: u64) -> i32;
    fn glmaxx_memcpy_h2d(destination: u64, source: *const c_void, bytes: u64, stream: u64) -> i32;
    fn glmaxx_memcpy_d2h(destination: *mut c_void, source: u64, bytes: u64, stream: u64) -> i32;
}

pub struct NativeKernelDriver;

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
}

impl NativeBuffer {
    fn allocate(bytes: u64) -> Result<Self, KernelError> {
        let mut pointer = 0_u64;
        // SAFETY: `pointer` is a valid out-parameter.
        check(unsafe { glmaxx_device_alloc(bytes, std::ptr::from_mut(&mut pointer)) })?;
        Ok(Self { pointer })
    }

    fn upload(bytes: &[u8], stream: u64) -> Result<Self, KernelError> {
        let buffer = Self::allocate(bytes.len() as u64)?;
        // SAFETY: both buffers are valid for `bytes.len()` and stream is owned.
        check(unsafe {
            glmaxx_memcpy_h2d(
                buffer.pointer,
                bytes.as_ptr().cast(),
                bytes.len() as u64,
                stream,
            )
        })?;
        Ok(buffer)
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

pub fn run_single_expert(
    input_bf16: &[u16],
    rows: u32,
    weights: &PackedNvfp4,
) -> Result<Vec<u16>, KernelError> {
    if rows == 0
        || input_bf16.len() != rows as usize * HIDDEN as usize
        || weights.metadata.logical_n != 1024
        || weights.metadata.logical_k != HIDDEN
        || weights.metadata.padded_n != 1024
        || weights.metadata.padded_k != HIDDEN
        || weights.metadata.codec != Codec::OneDimensional
        || weights.values.len() != 1024 * HIDDEN as usize / 2
        || weights.scales.len() != 1024 * HIDDEN as usize / 16
    {
        return Err(KernelError::Shape);
    }
    validate_native_library(rows)?;
    let stream = NativeStream::create()?;
    let input_bytes = words_as_bytes(input_bf16);
    let input = NativeBuffer::upload(input_bytes, stream.0)?;
    let weight_values = NativeBuffer::upload(&weights.values, stream.0)?;
    let weight_scales = NativeBuffer::upload(&weights.scales, stream.0)?;
    let weight_global =
        NativeBuffer::upload(&weights.metadata.global_scale.to_le_bytes(), stream.0)?;
    let route_experts = vec![0_u16; rows as usize];
    let route_tokens: Vec<u32> = (0..rows).collect();
    let route_slots = vec![0_u8; rows as usize];
    let route_weights = vec![1.0_f32; rows as usize];
    let route_expert = NativeBuffer::upload(words_as_bytes(&route_experts), stream.0)?;
    let route_token = NativeBuffer::upload(dwords_as_bytes(&route_tokens), stream.0)?;
    let route_slot = NativeBuffer::upload(&route_slots, stream.0)?;
    let route_weight = NativeBuffer::upload(floats_as_bytes(&route_weights), stream.0)?;
    let offsets = NativeBuffer::allocate(257 * 4)?;
    let compacted = NativeBuffer::allocate(u64::from(rows) * u64::from(HIDDEN) * 2)?;
    let padded_assignments = u64::from(rows.next_multiple_of(128));
    let activation_values = NativeBuffer::allocate(padded_assignments * u64::from(HIDDEN) / 2)?;
    let activation_scales = NativeBuffer::allocate(padded_assignments * u64::from(HIDDEN) / 16)?;
    let activation_globals = NativeBuffer::allocate(u64::from(rows) * 4)?;
    let gate_up = NativeBuffer::allocate(u64::from(rows) * u64::from(crate::LOCAL_GATE_UP) * 4)?;
    let output = NativeBuffer::allocate(u64::from(rows) * u64::from(LOCAL_INTERMEDIATE) * 2)?;
    let mut descriptor = Fc1Descriptor::new(LaunchGeometry {
        rows,
        assignments: rows,
        path: if rows <= 128 {
            KernelPath::DecodePersistent
        } else {
            KernelPath::PrefillGrouped
        },
    });
    descriptor.input_bf16 = input.pointer;
    descriptor.expert_value_base = weight_values.pointer;
    descriptor.expert_scale_base = weight_scales.pointer;
    descriptor.expert_global_scales = weight_global.pointer;
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
    descriptor.workspace_bytes = workspace_bytes(rows)?;
    descriptor.sequence = 1;
    validate_descriptor(&descriptor)?;
    let mut async_error = 0_i32;
    // SAFETY: all device pointers, descriptor bytes, and stream ownership were
    // established above and remain live through synchronization.
    check(unsafe {
        glmaxx_nvfp4_routed_fc1_launch(
            std::ptr::from_ref(&descriptor),
            stream.0 as *mut c_void,
            std::ptr::from_mut(&mut async_error),
        )
    })?;
    if async_error != 0 {
        return Err(KernelError::Async(async_error));
    }
    let mut host_output = vec![0_u16; rows as usize * LOCAL_INTERMEDIATE as usize];
    // SAFETY: output and destination are both valid for 1024 bytes.
    check(unsafe {
        glmaxx_memcpy_d2h(
            host_output.as_mut_ptr().cast(),
            output.pointer,
            u64::from(rows) * u64::from(LOCAL_INTERMEDIATE) * 2,
            stream.0,
        )
    })?;
    // SAFETY: stream is valid and all enqueued work must finish before drops.
    check(unsafe { glmaxx_stream_synchronize(stream.0) })?;
    Ok(host_output)
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
    {
        return Err(KernelError::Abi);
    }
    Ok(())
}
