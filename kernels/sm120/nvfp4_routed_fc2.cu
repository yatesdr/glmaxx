// Correctness-oriented SM120 implementation of routed FC2.
//
// The kernel consumes the same tensor-core-ready NVFP4 value/SFA/SFB layout
// as FC1. The retained CUDA-core dot product is intentionally slow and serves
// as the direct-layout control for the native block-scaled MMA successor.
// Projection results are materialized assignment-major in FP32. A separate
// slot-ordered reduction applies route weights after projection and produces
// deterministic token-major rank partials without atomic floating-point
// accumulation.

#include "glmaxx_kernel.h"

#include <cuda_bf16.h>
#include <cuda_runtime_api.h>

#include <cutlass/float8.h>
#include <cutlass/float_subbyte.h>

#include <cstdint>

namespace {

constexpr uint32_t kHidden = 6144;
constexpr uint32_t kLocalIntermediate = 512;
constexpr uint32_t kExperts = 256;
constexpr uint32_t kTopK = 8;
constexpr uint32_t kThreads = 256;
constexpr uint64_t kWeightValueBytes =
    uint64_t{kHidden} * kLocalIntermediate / 2;
constexpr uint64_t kWeightScaleBytes =
    uint64_t{kHidden} * kLocalIntermediate / 16;

enum : uint32_t {
  kRouteTokenOutOfRange = 1u << 0,
  kRouteSlotOutOfRange = 1u << 1,
  kDuplicateTokenSlot = 1u << 2,
  kExpertOutOfRange = 1u << 3,
  kRouteWeightNotFinite = 1u << 4,
};

__device__ __forceinline__ uint8_t encode_e2m1(float value) {
  return cutlass::float_e2m1_t(value).raw() & 0x0f;
}

__device__ __forceinline__ float decode_e2m1(uint8_t code) {
  return static_cast<float>(cutlass::float_e2m1_t::bitcast(code & 0x0f));
}

__device__ __forceinline__ uint8_t encode_e4m3(float value) {
  return cutlass::float_e4m3_t(value).raw();
}

__device__ __forceinline__ float decode_e4m3(uint8_t code) {
  return static_cast<float>(cutlass::float_e4m3_t::bitcast(code));
}

__host__ __device__ __forceinline__ uint32_t round_up_128(
    uint32_t value) {
  return (value + 127u) & ~127u;
}

__host__ __device__ __forceinline__ uint64_t scale_offset(
    uint32_t row, uint32_t group, uint32_t logical_k) {
  const uint32_t row_block = row / 128;
  const uint32_t row0 = row % 32;
  const uint32_t row1 = (row % 128) / 32;
  const uint32_t k_block = group / 4;
  const uint32_t group_in = group % 4;
  const uint32_t k_blocks = logical_k / 64;
  return uint64_t{512} * (uint64_t{row_block} * k_blocks + k_block) +
         16 * row0 + 4 * row1 + group_in;
}

__device__ float block_max(float value, float* scratch) {
  scratch[threadIdx.x] = value;
  __syncthreads();
  for (uint32_t stride = blockDim.x / 2; stride != 0; stride >>= 1) {
    if (threadIdx.x < stride) {
      scratch[threadIdx.x] =
          fmaxf(scratch[threadIdx.x], scratch[threadIdx.x + stride]);
    }
    __syncthreads();
  }
  return scratch[0];
}

__global__ void quantize_fc2_rows(
    const glmaxx_fc2_descriptor descriptor) {
  const uint32_t assignment = blockIdx.x;
  if (assignment >= descriptor.assignments) {
    return;
  }
  const auto* input =
      reinterpret_cast<const __nv_bfloat16*>(descriptor.input_bf16);
  auto* values = reinterpret_cast<uint8_t*>(descriptor.activation_values);
  auto* scales = reinterpret_cast<uint8_t*>(descriptor.activation_scales);
  auto* globals =
      reinterpret_cast<float*>(descriptor.activation_global_scales);
  const __nv_bfloat16* row =
      input + uint64_t{assignment} * kLocalIntermediate;

  float local_amax = 0.0f;
  for (uint32_t k = threadIdx.x; k < kLocalIntermediate;
       k += blockDim.x) {
    local_amax = fmaxf(local_amax, fabsf(__bfloat162float(row[k])));
  }
  __shared__ float reduction[kThreads];
  const float amax = block_max(local_amax, reduction);
  const float global_scale =
      amax == 0.0f ? 1.0f : amax / (448.0f * 6.0f);
  if (threadIdx.x == 0) {
    globals[assignment] = global_scale;
  }
  __syncthreads();

  constexpr uint32_t kGroups = kLocalIntermediate / 16;
  for (uint32_t group = threadIdx.x; group < kGroups;
       group += blockDim.x) {
    float group_amax = 0.0f;
    const uint32_t start = group * 16;
    #pragma unroll
    for (uint32_t lane = 0; lane < 16; ++lane) {
      group_amax =
          fmaxf(group_amax, fabsf(__bfloat162float(row[start + lane])));
    }
    const uint8_t scale_code =
        group_amax == 0.0f
            ? 0
            : encode_e4m3((group_amax / 6.0f) / global_scale);
    scales[scale_offset(assignment, group, kLocalIntermediate)] =
        scale_code;
    const float decoded_scale = decode_e4m3(scale_code) * global_scale;
    uint8_t* packed_row =
        values + uint64_t{assignment} * kLocalIntermediate / 2;
    #pragma unroll
    for (uint32_t pair = 0; pair < 8; ++pair) {
      const float lo = __bfloat162float(row[start + pair * 2]);
      const float hi = __bfloat162float(row[start + pair * 2 + 1]);
      const uint8_t lo_code =
          scale_code == 0 ? 0 : encode_e2m1(lo / decoded_scale);
      const uint8_t hi_code =
          scale_code == 0 ? 0 : encode_e2m1(hi / decoded_scale);
      packed_row[start / 2 + pair] = lo_code | (hi_code << 4);
    }
  }
}

__global__ void build_slot_assignment(
    const glmaxx_fc2_descriptor descriptor) {
  const uint32_t assignment = blockIdx.x * blockDim.x + threadIdx.x;
  if (assignment >= descriptor.assignments) {
    return;
  }
  const auto* route_experts =
      reinterpret_cast<const uint16_t*>(descriptor.route_experts_u16);
  const auto* route_tokens =
      reinterpret_cast<const uint32_t*>(descriptor.route_tokens_u32);
  const auto* route_slots =
      reinterpret_cast<const uint8_t*>(descriptor.route_slots_u8);
  const auto* route_weights =
      reinterpret_cast<const float*>(descriptor.route_weights_f32);
  auto* slot_assignment =
      reinterpret_cast<uint32_t*>(descriptor.slot_assignment_u32);
  auto* validation_error =
      reinterpret_cast<uint32_t*>(descriptor.validation_error_u32);

  const uint32_t token = route_tokens[assignment];
  const uint32_t slot = route_slots[assignment];
  uint32_t error = 0;
  if (token >= descriptor.rows) {
    error |= kRouteTokenOutOfRange;
  }
  if (slot >= kTopK) {
    error |= kRouteSlotOutOfRange;
  }
  if (route_experts[assignment] >= kExperts) {
    error |= kExpertOutOfRange;
  }
  if (!isfinite(route_weights[assignment])) {
    error |= kRouteWeightNotFinite;
  }
  if (error != 0) {
    atomicOr(validation_error, error);
    return;
  }
  const uint64_t index = uint64_t{token} * kTopK + slot;
  if (atomicCAS(slot_assignment + index, UINT32_MAX, assignment) !=
      UINT32_MAX) {
    atomicOr(validation_error, kDuplicateTokenSlot);
  }
}

__global__ void direct_fc2(const glmaxx_fc2_descriptor descriptor) {
  const uint64_t block_linear =
      uint64_t{blockIdx.y} * gridDim.x + blockIdx.x;
  const uint64_t block_stride = uint64_t{gridDim.x} * gridDim.y;
  const uint64_t total_work =
      uint64_t{descriptor.assignments} * kHidden;
  const auto* route_experts =
      reinterpret_cast<const uint16_t*>(descriptor.route_experts_u16);
  const auto* activation_values =
      reinterpret_cast<const uint8_t*>(descriptor.activation_values);
  const auto* activation_scales =
      reinterpret_cast<const uint8_t*>(descriptor.activation_scales);
  const auto* activation_globals =
      reinterpret_cast<const float*>(descriptor.activation_global_scales);
  const auto* weight_values =
      reinterpret_cast<const uint8_t*>(descriptor.expert_value_base);
  const auto* weight_scales =
      reinterpret_cast<const uint8_t*>(descriptor.expert_scale_base);
  const auto* weight_globals =
      reinterpret_cast<const float*>(descriptor.expert_global_scales);
  auto* assignment_down =
      reinterpret_cast<float*>(descriptor.assignment_down_f32);

  __shared__ float reduction[kThreads];
  for (uint64_t work = block_linear; work < total_work;
       work += block_stride) {
    const uint32_t assignment =
        static_cast<uint32_t>(work / kHidden);
    const uint32_t column = static_cast<uint32_t>(work % kHidden);
    const uint32_t expert = route_experts[assignment];
    float accumulator = 0.0f;
    if (expert < kExperts) {
      const uint8_t* expert_values =
          weight_values + uint64_t{expert} * kWeightValueBytes;
      const uint8_t* expert_scales =
          weight_scales + uint64_t{expert} * kWeightScaleBytes;
      const float combined_global =
          activation_globals[assignment] * weight_globals[expert];
      for (uint32_t k = threadIdx.x; k < kLocalIntermediate;
           k += blockDim.x) {
        const uint64_t activation_linear =
            uint64_t{assignment} * kLocalIntermediate + k;
        const uint8_t activation_byte =
            activation_values[activation_linear / 2];
        const uint8_t activation_code =
            (activation_linear & 1) == 0
                ? activation_byte & 0x0f
                : activation_byte >> 4;
        const uint8_t activation_scale =
            activation_scales[scale_offset(
                assignment, k / 16, kLocalIntermediate)];

        const uint64_t weight_linear =
            uint64_t{column} * kLocalIntermediate + k;
        const uint8_t weight_byte = expert_values[weight_linear / 2];
        const uint8_t weight_code =
            (weight_linear & 1) == 0
                ? weight_byte & 0x0f
                : weight_byte >> 4;
        const uint8_t weight_scale =
            expert_scales[scale_offset(
                column, k / 16, kLocalIntermediate)];
        const float product_scale =
            decode_e4m3(activation_scale) *
            decode_e4m3(weight_scale) * combined_global;
        accumulator =
            fmaf(decode_e2m1(activation_code) *
                     decode_e2m1(weight_code),
                 product_scale, accumulator);
      }
    }
    reduction[threadIdx.x] = accumulator;
    __syncthreads();
    for (uint32_t stride = blockDim.x / 2; stride != 0; stride >>= 1) {
      if (threadIdx.x < stride) {
        reduction[threadIdx.x] += reduction[threadIdx.x + stride];
      }
      __syncthreads();
    }
    if (threadIdx.x == 0) {
      assignment_down[work] = reduction[0];
    }
    __syncthreads();
  }
}

__global__ void reduce_fc2_slots(
    const glmaxx_fc2_descriptor descriptor) {
  const uint64_t total = uint64_t{descriptor.rows} * kHidden;
  const auto* route_weights =
      reinterpret_cast<const float*>(descriptor.route_weights_f32);
  const auto* slot_assignment =
      reinterpret_cast<const uint32_t*>(descriptor.slot_assignment_u32);
  const auto* assignment_down =
      reinterpret_cast<const float*>(descriptor.assignment_down_f32);
  auto* output = reinterpret_cast<float*>(descriptor.token_output_f32);

  for (uint64_t linear = uint64_t{blockIdx.x} * blockDim.x + threadIdx.x;
       linear < total; linear += uint64_t{gridDim.x} * blockDim.x) {
    const uint32_t token = static_cast<uint32_t>(linear / kHidden);
    const uint32_t column = static_cast<uint32_t>(linear % kHidden);
    float accumulator = 0.0f;
    #pragma unroll
    for (uint32_t slot = 0; slot < kTopK; ++slot) {
      const uint32_t assignment =
          slot_assignment[uint64_t{token} * kTopK + slot];
      if (assignment != UINT32_MAX &&
          assignment < descriptor.assignments) {
        accumulator =
            fmaf(route_weights[assignment],
                 assignment_down[uint64_t{assignment} * kHidden + column],
                 accumulator);
      }
    }
    output[linear] = accumulator;
  }
}

bool valid_host_descriptor(const glmaxx_fc2_descriptor& descriptor) {
  if (descriptor.abi_version != GLMAXX_FC2_ABI_VERSION ||
      descriptor.struct_bytes != sizeof(glmaxx_fc2_descriptor) ||
      descriptor.flags != 0 || descriptor.hidden != kHidden ||
      descriptor.local_intermediate != kLocalIntermediate ||
      descriptor.experts != kExperts || descriptor.top_k != kTopK ||
      descriptor.reserved0 != 0 || descriptor.reserved1 != 0 ||
      descriptor.rows == 0 || descriptor.assignments == 0 ||
      descriptor.assignments > 65535 ||
      descriptor.assignments > descriptor.rows * kTopK ||
      (descriptor.path != GLMAXX_FC1_DECODE_PERSISTENT &&
       descriptor.path != GLMAXX_FC1_PREFILL_GROUPED)) {
    return false;
  }
  if ((descriptor.path == GLMAXX_FC1_DECODE_PERSISTENT &&
       descriptor.rows > 128) ||
      (descriptor.path == GLMAXX_FC1_PREFILL_GROUPED &&
       descriptor.rows > 65536)) {
    return false;
  }
  for (uint64_t reserved : descriptor.reserved) {
    if (reserved != 0) {
      return false;
    }
  }
  return descriptor.input_bf16 != 0 &&
         descriptor.expert_value_base != 0 &&
         descriptor.expert_scale_base != 0 &&
         descriptor.expert_global_scales != 0 &&
         descriptor.route_experts_u16 != 0 &&
         descriptor.route_tokens_u32 != 0 &&
         descriptor.route_slots_u8 != 0 &&
         descriptor.route_weights_f32 != 0 &&
         descriptor.expert_offsets_u32 != 0 &&
         descriptor.activation_values != 0 &&
         descriptor.activation_scales != 0 &&
         descriptor.activation_global_scales != 0 &&
         descriptor.assignment_down_f32 != 0 &&
         descriptor.token_output_f32 != 0 &&
         descriptor.slot_assignment_u32 != 0 &&
         descriptor.validation_error_u32 != 0 &&
         descriptor.expert_value_base % 256 == 0 &&
         descriptor.expert_scale_base % 256 == 0 &&
         descriptor.activation_values % 16 == 0 &&
         descriptor.activation_scales % 16 == 0 &&
         descriptor.assignment_down_f32 % 4 == 0 &&
         descriptor.token_output_f32 % 4 == 0 &&
         descriptor.slot_assignment_u32 % 4 == 0 &&
         descriptor.validation_error_u32 % 4 == 0;
}

cudaError_t sm120_properties(cudaDeviceProp* properties) {
  int device = -1;
  cudaError_t status = cudaGetDevice(&device);
  if (status == cudaSuccess) {
    status = cudaGetDeviceProperties(properties, device);
  }
  if (status == cudaSuccess &&
      (properties->major != 12 || properties->minor != 0)) {
    return cudaErrorInvalidDevice;
  }
  return status;
}

cudaError_t enqueue_prepare(const glmaxx_fc2_descriptor& descriptor,
                            cudaStream_t stream) {
  cudaError_t status = cudaMemsetAsync(
      reinterpret_cast<void*>(descriptor.slot_assignment_u32), 0xff,
      uint64_t{descriptor.rows} * kTopK * sizeof(uint32_t), stream);
  if (status == cudaSuccess) {
    status = cudaMemsetAsync(
        reinterpret_cast<void*>(descriptor.validation_error_u32), 0,
        sizeof(uint32_t), stream);
  }
  if (status == cudaSuccess) {
    const uint32_t blocks =
        (descriptor.assignments + kThreads - 1) / kThreads;
    build_slot_assignment<<<blocks, kThreads, 0, stream>>>(descriptor);
    status = cudaPeekAtLastError();
  }
  return status;
}

cudaError_t enqueue_quantize(const glmaxx_fc2_descriptor& descriptor,
                             cudaStream_t stream) {
  quantize_fc2_rows<<<descriptor.assignments, kThreads, 0, stream>>>(
      descriptor);
  return cudaPeekAtLastError();
}

cudaError_t enqueue_core(const glmaxx_fc2_descriptor& descriptor,
                         const cudaDeviceProp& properties,
                         cudaStream_t stream) {
  const uint64_t total_blocks =
      uint64_t{descriptor.assignments} * kHidden;
  const uint32_t ctas_per_sm =
      descriptor.path == GLMAXX_FC1_DECODE_PERSISTENT ? 2 : 8;
  const uint64_t target_blocks =
      uint64_t{properties.multiProcessorCount} * ctas_per_sm;
  const uint32_t blocks = static_cast<uint32_t>(
      total_blocks < target_blocks ? total_blocks : target_blocks);
  direct_fc2<<<blocks, kThreads, 0, stream>>>(descriptor);
  return cudaPeekAtLastError();
}

cudaError_t enqueue_reduce(const glmaxx_fc2_descriptor& descriptor,
                           const cudaDeviceProp& properties,
                           cudaStream_t stream) {
  const uint64_t total = uint64_t{descriptor.rows} * kHidden;
  const uint64_t target_blocks =
      uint64_t{properties.multiProcessorCount} * 8;
  const uint64_t required_blocks = (total + kThreads - 1) / kThreads;
  const uint32_t blocks = static_cast<uint32_t>(
      required_blocks < target_blocks ? required_blocks : target_blocks);
  reduce_fc2_slots<<<blocks, kThreads, 0, stream>>>(descriptor);
  return cudaPeekAtLastError();
}

cudaError_t enqueue_fc2(const glmaxx_fc2_descriptor& descriptor,
                        const cudaDeviceProp& properties,
                        cudaStream_t stream) {
  cudaError_t status = enqueue_prepare(descriptor, stream);
  if (status == cudaSuccess) {
    status = enqueue_quantize(descriptor, stream);
  }
  if (status == cudaSuccess) {
    status = enqueue_core(descriptor, properties, stream);
  }
  if (status == cudaSuccess) {
    status = enqueue_reduce(descriptor, properties, stream);
  }
  return status;
}

int32_t validate_launch(const glmaxx_fc2_descriptor* descriptor,
                        void* cuda_stream, cudaDeviceProp* properties) {
  if (descriptor == nullptr || cuda_stream == nullptr) {
    return -1;
  }
  if (!valid_host_descriptor(*descriptor) ||
      descriptor->workspace_bytes <
          glmaxx_nvfp4_routed_fc2_workspace_bytes(
              descriptor->rows, descriptor->assignments)) {
    return -2;
  }
  const cudaError_t status = sm120_properties(properties);
  if (status == cudaErrorInvalidDevice) {
    return -120;
  }
  return static_cast<int32_t>(status);
}

}  // namespace

extern "C" uint64_t glmaxx_nvfp4_routed_fc2_workspace_bytes(
    uint32_t rows, uint32_t assignments) {
  if (rows == 0 || assignments == 0 ||
      assignments > uint64_t{rows} * kTopK) {
    return 0;
  }
  const uint64_t padded = round_up_128(assignments);
  return padded * kLocalIntermediate / 2 +
         padded * kLocalIntermediate / 16 +
         uint64_t{assignments} * sizeof(float) +
         uint64_t{assignments} * kHidden * sizeof(float) +
         uint64_t{rows} * kHidden * sizeof(float) +
         uint64_t{rows} * kTopK * sizeof(uint32_t) + sizeof(uint32_t);
}

extern "C" int32_t glmaxx_nvfp4_routed_fc2_launch(
    const glmaxx_fc2_descriptor* descriptor, void* cuda_stream,
    int32_t* asynchronous_error) {
  if (asynchronous_error == nullptr) {
    return -1;
  }
  *asynchronous_error = 0;
  cudaDeviceProp properties{};
  const int32_t validation =
      validate_launch(descriptor, cuda_stream, &properties);
  if (validation != 0) {
    return validation;
  }
  return static_cast<int32_t>(enqueue_fc2(
      *descriptor, properties, reinterpret_cast<cudaStream_t>(cuda_stream)));
}

extern "C" int32_t glmaxx_nvfp4_fc2_quantize_launch(
    const glmaxx_fc2_descriptor* descriptor, void* cuda_stream) {
  cudaDeviceProp properties{};
  const int32_t validation =
      validate_launch(descriptor, cuda_stream, &properties);
  if (validation != 0) {
    return validation;
  }
  return static_cast<int32_t>(enqueue_quantize(
      *descriptor, reinterpret_cast<cudaStream_t>(cuda_stream)));
}

extern "C" int32_t glmaxx_nvfp4_fc2_core_launch(
    const glmaxx_fc2_descriptor* descriptor, void* cuda_stream) {
  cudaDeviceProp properties{};
  const int32_t validation =
      validate_launch(descriptor, cuda_stream, &properties);
  if (validation != 0) {
    return validation;
  }
  return static_cast<int32_t>(enqueue_core(
      *descriptor, properties, reinterpret_cast<cudaStream_t>(cuda_stream)));
}

extern "C" int32_t glmaxx_nvfp4_fc2_reduce_launch(
    const glmaxx_fc2_descriptor* descriptor, void* cuda_stream) {
  cudaDeviceProp properties{};
  const int32_t validation =
      validate_launch(descriptor, cuda_stream, &properties);
  if (validation != 0) {
    return validation;
  }
  cudaError_t status =
      enqueue_prepare(*descriptor, reinterpret_cast<cudaStream_t>(cuda_stream));
  if (status == cudaSuccess) {
    status = enqueue_reduce(
        *descriptor, properties,
        reinterpret_cast<cudaStream_t>(cuda_stream));
  }
  return static_cast<int32_t>(status);
}
