// First correctness-oriented SM120 implementation of the frozen routed-FC1
// ABI. It consumes the production value/SFB layout directly and fuses the
// gate/up dot products with the SwiGLU store. The authorized M2 pass will
// retain this implementation as the direct-layout control while replacing
// its CUDA-core dot product with the pinned CUTLASS block-scaled MMA path.

#include "glmaxx_kernel.h"

#include <cuda_bf16.h>
#include <cuda_runtime_api.h>

#include <cutlass/float8.h>
#include <cutlass/float_subbyte.h>

#include <cstdint>

namespace {

constexpr uint32_t kHidden = 6144;
constexpr uint32_t kLocalGateUp = 1024;
constexpr uint32_t kLocalIntermediate = 512;
constexpr uint32_t kExperts = 256;
constexpr uint32_t kTopK = 8;
constexpr uint64_t kWeightValueBytes = uint64_t{kLocalGateUp} * kHidden / 2;
constexpr uint64_t kWeightScaleBytes = uint64_t{kLocalGateUp} * kHidden / 16;
constexpr uint32_t kGroupsK = kHidden / 16;

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

__host__ __device__ __forceinline__ uint32_t round_up_128(uint32_t value) {
  return (value + 127u) & ~127u;
}

__host__ __device__ __forceinline__ uint64_t scale_offset(
    uint32_t row, uint32_t group, uint32_t padded_rows) {
  const uint32_t row_block = row / 128;
  const uint32_t row0 = row % 32;
  const uint32_t row1 = (row % 128) / 32;
  const uint32_t k_block = group / 4;
  const uint32_t group_in = group % 4;
  constexpr uint32_t kKBlocks = kHidden / 64;
  (void)padded_rows;
  return uint64_t{512} * (uint64_t{row_block} * kKBlocks + k_block) +
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

__global__ void quantize_compacted_rows(
    const glmaxx_fc1_descriptor descriptor) {
  const uint32_t assignment = blockIdx.x;
  if (assignment >= descriptor.assignments) {
    return;
  }
  const auto* input =
      reinterpret_cast<const __nv_bfloat16*>(descriptor.input_bf16);
  const auto* route_tokens =
      reinterpret_cast<const uint32_t*>(descriptor.route_tokens_u32);
  auto* values = reinterpret_cast<uint8_t*>(descriptor.activation_values);
  auto* scales = reinterpret_cast<uint8_t*>(descriptor.activation_scales);
  auto* globals =
      reinterpret_cast<float*>(descriptor.activation_global_scales);
  const uint32_t token = route_tokens[assignment];
  const __nv_bfloat16* row = input + uint64_t{token} * kHidden;

  float local_amax = 0.0f;
  for (uint32_t k = threadIdx.x; k < kHidden; k += blockDim.x) {
    local_amax = fmaxf(local_amax, fabsf(__bfloat162float(row[k])));
  }
  __shared__ float reduction[256];
  const float amax = block_max(local_amax, reduction);
  const float global_scale = amax == 0.0f ? 1.0f : amax / (448.0f * 6.0f);
  if (threadIdx.x == 0) {
    globals[assignment] = global_scale;
  }
  __syncthreads();

  const uint32_t padded_rows = round_up_128(descriptor.assignments);
  for (uint32_t group = threadIdx.x; group < kGroupsK;
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
    scales[scale_offset(assignment, group, padded_rows)] = scale_code;
    const float decoded_scale = decode_e4m3(scale_code) * global_scale;
    uint8_t* packed_row =
        values + uint64_t{assignment} * kHidden / 2;
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

__global__ void direct_fc1_swiglu(const glmaxx_fc1_descriptor descriptor) {
  const uint64_t block_linear =
      uint64_t{blockIdx.y} * gridDim.x + blockIdx.x;
  const uint64_t block_stride = uint64_t{gridDim.x} * gridDim.y;
  const uint64_t total_work =
      uint64_t{descriptor.assignments} * kLocalIntermediate;
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
  auto* output = reinterpret_cast<__nv_bfloat16*>(descriptor.output_bf16);

  const uint32_t padded_assignments = round_up_128(descriptor.assignments);
  __shared__ float gate_reduction[256];
  __shared__ float up_reduction[256];

  // Decode launches a small fixed CTA pool and strides over output work.
  // Prefill launches one CTA per (assignment,column). Both consume identical
  // descriptors and bytes, so the schedule cannot alter arithmetic.
  for (uint64_t work = block_linear; work < total_work;
       work += block_stride) {
    const uint32_t assignment =
        static_cast<uint32_t>(work / kLocalIntermediate);
    const uint32_t column =
        static_cast<uint32_t>(work % kLocalIntermediate);
    const uint32_t expert = route_experts[assignment];
    const uint8_t* expert_values =
        weight_values + uint64_t{expert} * kWeightValueBytes;
    const uint8_t* expert_scales =
        weight_scales + uint64_t{expert} * kWeightScaleBytes;
    const float weight_global = weight_globals[expert];
    const float activation_global = activation_globals[assignment];

    float gate = 0.0f;
    float up = 0.0f;
    for (uint32_t k = threadIdx.x; k < kHidden; k += blockDim.x) {
      const uint64_t a_linear = uint64_t{assignment} * kHidden + k;
      const uint8_t a_byte = activation_values[a_linear / 2];
      const uint8_t a_code =
          (a_linear & 1) == 0 ? a_byte & 0x0f : a_byte >> 4;
      const uint8_t a_scale =
          activation_scales[scale_offset(assignment, k / 16,
                                         padded_assignments)];
      const float a = decode_e2m1(a_code) * decode_e4m3(a_scale) *
                      activation_global;

      const uint64_t gate_linear = uint64_t{column} * kHidden + k;
      const uint64_t up_linear =
          uint64_t{kLocalIntermediate + column} * kHidden + k;
      const uint8_t gate_byte = expert_values[gate_linear / 2];
      const uint8_t up_byte = expert_values[up_linear / 2];
      const uint8_t gate_code =
          (gate_linear & 1) == 0 ? gate_byte & 0x0f : gate_byte >> 4;
      const uint8_t up_code =
          (up_linear & 1) == 0 ? up_byte & 0x0f : up_byte >> 4;
      const float gate_w =
          decode_e2m1(gate_code) *
          decode_e4m3(expert_scales[scale_offset(column, k / 16,
                                                 kLocalGateUp)]) *
          weight_global;
      const float up_w =
          decode_e2m1(up_code) *
          decode_e4m3(expert_scales[scale_offset(
              kLocalIntermediate + column, k / 16, kLocalGateUp)]) *
          weight_global;
      gate = fmaf(a, gate_w, gate);
      up = fmaf(a, up_w, up);
    }

    gate_reduction[threadIdx.x] = gate;
    up_reduction[threadIdx.x] = up;
    __syncthreads();
    for (uint32_t stride = blockDim.x / 2; stride != 0; stride >>= 1) {
      if (threadIdx.x < stride) {
        gate_reduction[threadIdx.x] += gate_reduction[threadIdx.x + stride];
        up_reduction[threadIdx.x] += up_reduction[threadIdx.x + stride];
      }
      __syncthreads();
    }
    if (threadIdx.x == 0) {
      const float gate_value = gate_reduction[0];
      const float silu = gate_value / (1.0f + expf(-gate_value));
      output[uint64_t{assignment} * kLocalIntermediate + column] =
          __float2bfloat16_rn(silu * up_reduction[0]);
    }
    __syncthreads();
  }
}

bool valid_host_descriptor(const glmaxx_fc1_descriptor& descriptor) {
  if (descriptor.abi_version != GLMAXX_FC1_ABI_VERSION ||
      descriptor.struct_bytes != sizeof(glmaxx_fc1_descriptor) ||
      descriptor.flags != 0 || descriptor.hidden != kHidden ||
      descriptor.local_gate_up != kLocalGateUp ||
      descriptor.local_intermediate != kLocalIntermediate ||
      descriptor.experts != kExperts || descriptor.top_k != kTopK ||
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
  return descriptor.input_bf16 != 0 && descriptor.expert_value_base != 0 &&
         descriptor.expert_scale_base != 0 &&
         descriptor.expert_global_scales != 0 &&
         descriptor.route_experts_u16 != 0 &&
         descriptor.route_tokens_u32 != 0 &&
         descriptor.activation_values != 0 &&
         descriptor.activation_scales != 0 &&
         descriptor.activation_global_scales != 0 &&
         descriptor.output_bf16 != 0;
}

}  // namespace

extern "C" uint64_t glmaxx_nvfp4_routed_fc1_workspace_bytes(
    uint32_t assignments) {
  const uint64_t padded = round_up_128(assignments);
  return uint64_t{assignments} * (kHidden * 2 + 4 + kLocalGateUp * 4) +
         padded * kHidden / 2 + padded * kHidden / 16 +
         uint64_t{kExperts + 1} * 4;
}

extern "C" const char* glmaxx_kernel_abi(void) {
  return "glmaxx.sm120.nvfp4.routed_fc1.v1";
}

extern "C" int32_t glmaxx_device_alloc(uint64_t bytes, uint64_t* pointer) {
  if (bytes == 0 || pointer == nullptr) {
    return -1;
  }
  void* allocation = nullptr;
  const cudaError_t status = cudaMalloc(&allocation, bytes);
  if (status == cudaSuccess) {
    *pointer = reinterpret_cast<uint64_t>(allocation);
  }
  return static_cast<int32_t>(status);
}

extern "C" int32_t glmaxx_device_free(uint64_t pointer) {
  if (pointer == 0) {
    return -1;
  }
  return static_cast<int32_t>(cudaFree(reinterpret_cast<void*>(pointer)));
}

extern "C" int32_t glmaxx_stream_create(uint64_t* stream) {
  if (stream == nullptr) {
    return -1;
  }
  cudaStream_t native_stream = nullptr;
  const cudaError_t status =
      cudaStreamCreateWithFlags(&native_stream, cudaStreamNonBlocking);
  if (status == cudaSuccess) {
    *stream = reinterpret_cast<uint64_t>(native_stream);
  }
  return static_cast<int32_t>(status);
}

extern "C" int32_t glmaxx_stream_destroy(uint64_t stream) {
  return static_cast<int32_t>(
      cudaStreamDestroy(reinterpret_cast<cudaStream_t>(stream)));
}

extern "C" int32_t glmaxx_stream_query(uint64_t stream, int32_t* complete) {
  if (stream == 0 || complete == nullptr) {
    return -1;
  }
  const cudaError_t status =
      cudaStreamQuery(reinterpret_cast<cudaStream_t>(stream));
  if (status == cudaSuccess) {
    *complete = 1;
    return 0;
  }
  if (status == cudaErrorNotReady) {
    *complete = 0;
    return 0;
  }
  *complete = 0;
  return static_cast<int32_t>(status);
}

extern "C" int32_t glmaxx_stream_synchronize(uint64_t stream) {
  return static_cast<int32_t>(
      cudaStreamSynchronize(reinterpret_cast<cudaStream_t>(stream)));
}

extern "C" int32_t glmaxx_memcpy_h2d(uint64_t destination,
                                      const void* source, uint64_t bytes,
                                      uint64_t stream) {
  return static_cast<int32_t>(cudaMemcpyAsync(
      reinterpret_cast<void*>(destination), source, bytes,
      cudaMemcpyHostToDevice, reinterpret_cast<cudaStream_t>(stream)));
}

extern "C" int32_t glmaxx_memcpy_d2h(void* destination, uint64_t source,
                                      uint64_t bytes, uint64_t stream) {
  return static_cast<int32_t>(cudaMemcpyAsync(
      destination, reinterpret_cast<const void*>(source), bytes,
      cudaMemcpyDeviceToHost, reinterpret_cast<cudaStream_t>(stream)));
}

extern "C" int32_t glmaxx_nvfp4_routed_fc1_launch(
    const glmaxx_fc1_descriptor* descriptor, void* cuda_stream,
    int32_t* asynchronous_error) {
  if (descriptor == nullptr || cuda_stream == nullptr ||
      asynchronous_error == nullptr) {
    return -1;
  }
  *asynchronous_error = 0;
  if (!valid_host_descriptor(*descriptor) ||
      descriptor->workspace_bytes <
          glmaxx_nvfp4_routed_fc1_workspace_bytes(descriptor->assignments)) {
    return -2;
  }
  int device = -1;
  cudaDeviceProp properties{};
  cudaError_t status = cudaGetDevice(&device);
  if (status != cudaSuccess ||
      (status = cudaGetDeviceProperties(&properties, device)) != cudaSuccess) {
    return static_cast<int32_t>(status);
  }
  if (properties.major != 12 || properties.minor != 0) {
    return -120;
  }
  const cudaStream_t stream = reinterpret_cast<cudaStream_t>(cuda_stream);
  quantize_compacted_rows<<<descriptor->assignments, 256, 0, stream>>>(
      *descriptor);
  status = cudaPeekAtLastError();
  if (status != cudaSuccess) {
    return static_cast<int32_t>(status);
  }
  if (descriptor->path == GLMAXX_FC1_DECODE_PERSISTENT) {
    const uint64_t total_blocks =
        uint64_t{descriptor->assignments} * kLocalIntermediate;
    const uint32_t target_blocks =
        static_cast<uint32_t>(properties.multiProcessorCount) * 2;
    const uint32_t persistent_blocks =
        total_blocks < target_blocks ? static_cast<uint32_t>(total_blocks)
                                    : target_blocks;
    direct_fc1_swiglu<<<persistent_blocks, 256, 0, stream>>>(*descriptor);
  } else {
    direct_fc1_swiglu<<<dim3{kLocalIntermediate, descriptor->assignments}, 256,
                         0, stream>>>(*descriptor);
  }
  status = cudaPeekAtLastError();
  if (status != cudaSuccess) {
    return static_cast<int32_t>(status);
  }
  return 0;
}
