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
    fn glmaxx_memcpy_d2d(destination: u64, source: u64, bytes: u64, stream: u64) -> i32;
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
        self.run_validated(
            input_bf16,
            rows,
            route_experts,
            route_tokens,
            route_slots,
            &counts,
        )
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
        let padded_assignments = u64::from(assignments.next_multiple_of(128));
        let activation_values = NativeBuffer::allocate(padded_assignments * u64::from(HIDDEN) / 2)?;
        let activation_scales =
            NativeBuffer::allocate(padded_assignments * u64::from(HIDDEN) / 16)?;
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
        descriptor.workspace_bytes = workspace_bytes(assignments)?;
        descriptor.sequence = 1;
        validate_descriptor(&descriptor)?;
        let mut async_error = 0_i32;
        // SAFETY: all device allocations remain alive through synchronization.
        check(unsafe {
            glmaxx_nvfp4_routed_fc1_launch(
                std::ptr::from_ref(&descriptor),
                self.stream.0 as *mut c_void,
                std::ptr::from_mut(&mut async_error),
            )
        })?;
        if async_error != 0 {
            return Err(KernelError::Async(async_error));
        }
        let mut host_output = vec![0_u16; assignments as usize * LOCAL_INTERMEDIATE as usize];
        // SAFETY: the source and destination cover exactly the output allocation.
        check(unsafe {
            glmaxx_memcpy_d2h(
                host_output.as_mut_ptr().cast(),
                output.pointer,
                u64::from(assignments) * u64::from(LOCAL_INTERMEDIATE) * 2,
                self.stream.0,
            )
        })?;
        check(unsafe { glmaxx_stream_synchronize(self.stream.0) })?;
        Ok(host_output)
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
