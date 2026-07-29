// Direct source-order EXL3/Trellis projection correctness control.
//
// This kernel deliberately favors a transparent arithmetic match over speed.
// It consumes the pinned source-order trellis and FP16 rotations directly,
// reconstructs each weight only when it is accumulated, and never creates a
// persistent dense weight matrix. The retained control is the reference for
// later fragment-local SM120 optimization.

#include "glmaxx_kernel.h"

#include <cuda_fp16.h>
#include <cuda_runtime_api.h>

#include <cmath>
#include <cstddef>
#include <cstdint>

namespace {

constexpr uint32_t kBits = 3;
constexpr uint32_t kH128 = 128;
constexpr uint32_t kTile = 16;
constexpr uint32_t kTrellisHalvesPerTile = 16 * kBits;
constexpr uint32_t kTrellisWordsPerTile = 8 * kBits;
constexpr uint32_t kMcgMultiplier = 0xCBAC1FEDu;
constexpr uint32_t kLop3Mask = 0x8FFF8FFFu;
constexpr uint32_t kLop3Xor = 0x3B603B60u;
constexpr uint32_t kMaximumRows = 3072;

__device__ __forceinline__ float half_bits_to_float(uint16_t bits) {
  return __half2float(__ushort_as_half(bits));
}

__device__ __forceinline__ uint16_t decode_weight_bits(
    const uint16_t* trellis, uint32_t logical_n, uint32_t row,
    uint32_t column) {
  const uint32_t local_row = row & 15u;
  const uint32_t local_column = column & 15u;
  const uint32_t row_quadrant = (local_row & 7u) >> 1;
  const uint32_t row_selector =
      (local_row >= 8u ? 2u : 0u) + (local_row & 1u);
  const uint32_t column_group = (local_column >> 1) & 3u;
  const uint32_t parity = local_column & 1u;
  const uint32_t lane =
      column_group * 8u + parity * 4u + row_quadrant;
  const uint32_t weight =
      (local_column >= 8u ? 4u : 0u) + row_selector;
  const uint32_t end_bit = (lane * 8u + weight + 257u) * kBits;
  const uint32_t start_bit = end_bit - 16u;
  const uint32_t first_word = start_bit / 32u;
  const uint32_t last_word = (end_bit - 1u) / 32u;
  const uint32_t shift = (last_word + 1u) * 32u - end_bit;

  const uint64_t tile_index =
      uint64_t{row / kTile} * (logical_n / kTile) +
      column / kTile;
  const uint64_t tile_base =
      tile_index * kTrellisHalvesPerTile;
  const uint32_t first_index =
      (first_word % kTrellisWordsPerTile) * 2u;
  const uint32_t last_index =
      (last_word % kTrellisWordsPerTile) * 2u;
  const uint32_t first =
      uint32_t{trellis[tile_base + first_index]} |
      (uint32_t{trellis[tile_base + first_index + 1u]} << 16);
  const uint32_t last =
      uint32_t{trellis[tile_base + last_index]} |
      (uint32_t{trellis[tile_base + last_index + 1u]} << 16);
  const uint64_t merged = (uint64_t{first} << 32) | last;
  const uint16_t window =
      static_cast<uint16_t>((merged >> shift) & 0xffffu);
  const uint32_t multiplied =
      uint32_t{window} * kMcgMultiplier;
  const uint32_t packed =
      (multiplied & kLop3Mask) ^ kLop3Xor;
  const __half low =
      __ushort_as_half(static_cast<uint16_t>(packed));
  const __half high =
      __ushort_as_half(static_cast<uint16_t>(packed >> 16));
  return __half_as_ushort(__float2half_rn(
      __fadd_rn(__half2float(low), __half2float(high))));
}

__global__ void rotate_input_f16(
    glmaxx_exl3_descriptor descriptor) {
  __shared__ float source[kH128];
  const uint32_t row = blockIdx.x;
  const uint32_t block = blockIdx.y;
  const uint32_t output_offset = threadIdx.x;
  if (row >= descriptor.rows || output_offset >= kH128) {
    return;
  }
  const uint32_t index = block * kH128 + output_offset;
  const auto* input =
      reinterpret_cast<const uint16_t*>(descriptor.input_f16);
  const auto* suh =
      reinterpret_cast<const uint16_t*>(descriptor.suh_f16);
  auto* rotated =
      reinterpret_cast<uint16_t*>(descriptor.rotated_input_f16);
  auto* validation =
      reinterpret_cast<uint32_t*>(descriptor.validation_error_u32);
  const float scaled =
      half_bits_to_float(input[uint64_t{row} * descriptor.logical_k +
                               index]) *
      half_bits_to_float(suh[index]);
  const __half rounded = __float2half_rn(scaled);
  source[output_offset] = __half2float(rounded);
  __syncthreads();

  float sum = 0.0f;
  for (uint32_t column = 0; column < kH128; ++column) {
    const float signed_value =
        (__popc(output_offset & column) & 1u) == 0u
            ? source[column]
            : -source[column];
    sum = __fadd_rn(sum, signed_value);
  }
  const float transformed =
      __fmul_rn(sum, 0.08838834764831845f);
  if (!isfinite(transformed)) {
    atomicOr(validation, 1u);
  }
  rotated[uint64_t{row} * descriptor.logical_k + index] =
      __half_as_ushort(__float2half_rn(transformed));
}

__global__ void project_native_f16(
    glmaxx_exl3_descriptor descriptor) {
  const uint64_t total =
      uint64_t{descriptor.rows} * descriptor.logical_n;
  const auto* rotated =
      reinterpret_cast<const uint16_t*>(
          descriptor.rotated_input_f16);
  const auto* trellis =
      reinterpret_cast<const uint16_t*>(descriptor.trellis_u16);
  auto* projected =
      reinterpret_cast<uint16_t*>(descriptor.projected_f16);
  auto* validation =
      reinterpret_cast<uint32_t*>(descriptor.validation_error_u32);

  for (uint64_t linear =
           uint64_t{blockIdx.x} * blockDim.x + threadIdx.x;
       linear < total;
       linear += uint64_t{gridDim.x} * blockDim.x) {
    const uint32_t row =
        static_cast<uint32_t>(linear / descriptor.logical_n);
    const uint32_t column =
        static_cast<uint32_t>(linear % descriptor.logical_n);
    float accumulator = 0.0f;
    for (uint32_t inner = 0; inner < descriptor.logical_k;
         ++inner) {
      const float activation = half_bits_to_float(
          rotated[uint64_t{row} * descriptor.logical_k + inner]);
      const float weight = half_bits_to_float(
          decode_weight_bits(trellis, descriptor.logical_n, inner,
                             column));
      accumulator =
          __fadd_rn(accumulator, __fmul_rn(activation, weight));
    }
    if (!isfinite(accumulator)) {
      atomicOr(validation, 2u);
    }
    projected[linear] =
        __half_as_ushort(__float2half_rn(accumulator));
  }
}

__global__ void rotate_output_f16(
    glmaxx_exl3_descriptor descriptor) {
  __shared__ float source[kH128];
  const uint32_t row = blockIdx.x;
  const uint32_t block = blockIdx.y;
  const uint32_t output_offset = threadIdx.x;
  if (row >= descriptor.rows || output_offset >= kH128) {
    return;
  }
  const uint32_t index = block * kH128 + output_offset;
  const auto* projected =
      reinterpret_cast<const uint16_t*>(descriptor.projected_f16);
  const auto* svh =
      reinterpret_cast<const uint16_t*>(descriptor.svh_f16);
  auto* output =
      reinterpret_cast<uint16_t*>(descriptor.output_f16);
  auto* validation =
      reinterpret_cast<uint32_t*>(descriptor.validation_error_u32);
  source[output_offset] = half_bits_to_float(
      projected[uint64_t{row} * descriptor.logical_n + index]);
  __syncthreads();

  float sum = 0.0f;
  for (uint32_t column = 0; column < kH128; ++column) {
    const float signed_value =
        (__popc(output_offset & column) & 1u) == 0u
            ? source[column]
            : -source[column];
    sum = __fadd_rn(sum, signed_value);
  }
  const float transformed =
      __fmul_rn(sum, 0.08838834764831845f);
  const float scaled =
      __fmul_rn(transformed, half_bits_to_float(svh[index]));
  if (!isfinite(scaled)) {
    atomicOr(validation, 4u);
  }
  output[uint64_t{row} * descriptor.logical_n + index] =
      __half_as_ushort(__float2half_rn(scaled));
}

uint64_t workspace_bytes(uint32_t rows, uint32_t logical_k,
                         uint32_t logical_n) {
  if (rows == 0 || rows > kMaximumRows ||
      !((logical_k == 6144u && logical_n == 512u) ||
        (logical_k == 512u && logical_n == 6144u))) {
    return 0;
  }
  return uint64_t{rows} * (logical_k + logical_n) *
         sizeof(uint16_t);
}

bool descriptor_valid(const glmaxx_exl3_descriptor& descriptor) {
  const bool gate_or_up =
      descriptor.projection == GLMAXX_EXL3_GATE ||
      descriptor.projection == GLMAXX_EXL3_UP;
  const bool down =
      descriptor.projection == GLMAXX_EXL3_DOWN;
  const bool shape =
      (gate_or_up && descriptor.logical_k == 6144u &&
       descriptor.logical_n == 512u) ||
      (down && descriptor.logical_k == 512u &&
       descriptor.logical_n == 6144u);
  const uint64_t required =
      workspace_bytes(descriptor.rows, descriptor.logical_k,
                      descriptor.logical_n);
  return descriptor.abi_version == GLMAXX_EXL3_ABI_VERSION &&
         descriptor.struct_bytes ==
             sizeof(glmaxx_exl3_descriptor) &&
         descriptor.flags == 0u && descriptor.bits == kBits &&
         shape && required != 0u &&
         descriptor.workspace_bytes >= required &&
         descriptor.input_f16 != 0u &&
         descriptor.trellis_u16 != 0u &&
         descriptor.suh_f16 != 0u &&
         descriptor.svh_f16 != 0u &&
         descriptor.rotated_input_f16 != 0u &&
         descriptor.projected_f16 != 0u &&
         descriptor.output_f16 != 0u &&
         descriptor.validation_error_u32 != 0u &&
         descriptor.input_f16 % 2u == 0u &&
         descriptor.trellis_u16 % 4u == 0u &&
         descriptor.suh_f16 % 2u == 0u &&
         descriptor.svh_f16 % 2u == 0u &&
         descriptor.rotated_input_f16 % 2u == 0u &&
         descriptor.projected_f16 % 2u == 0u &&
         descriptor.output_f16 % 2u == 0u &&
         descriptor.validation_error_u32 % 4u == 0u &&
         descriptor.reserved[0] == 0u &&
         descriptor.reserved[1] == 0u &&
         descriptor.reserved[2] == 0u &&
         descriptor.reserved[3] == 0u;
}

}  // namespace

extern "C" int32_t glmaxx_exl3_projection_launch(
    const glmaxx_exl3_descriptor* descriptor, void* cuda_stream,
    int32_t* asynchronous_error) {
  if (descriptor == nullptr || cuda_stream == nullptr ||
      asynchronous_error == nullptr || !descriptor_valid(*descriptor)) {
    return -1;
  }
  *asynchronous_error = 0;
  const cudaStream_t stream =
      reinterpret_cast<cudaStream_t>(cuda_stream);
  cudaError_t status = cudaMemsetAsync(
      reinterpret_cast<void*>(descriptor->validation_error_u32), 0,
      sizeof(uint32_t), stream);
  if (status != cudaSuccess) {
    return static_cast<int32_t>(status);
  }

  const dim3 input_grid(descriptor->rows,
                        descriptor->logical_k / kH128);
  rotate_input_f16<<<input_grid, kH128, 0, stream>>>(
      *descriptor);
  status = cudaPeekAtLastError();
  if (status != cudaSuccess) {
    return static_cast<int32_t>(status);
  }

  constexpr uint32_t kProjectionThreads = 256;
  const uint64_t projection_elements =
      uint64_t{descriptor->rows} * descriptor->logical_n;
  uint64_t projection_blocks =
      (projection_elements + kProjectionThreads - 1u) /
      kProjectionThreads;
  if (projection_blocks > 4096u) {
    projection_blocks = 4096u;
  }
  project_native_f16<<<static_cast<uint32_t>(projection_blocks),
                       kProjectionThreads, 0, stream>>>(*descriptor);
  status = cudaPeekAtLastError();
  if (status != cudaSuccess) {
    return static_cast<int32_t>(status);
  }

  const dim3 output_grid(descriptor->rows,
                         descriptor->logical_n / kH128);
  rotate_output_f16<<<output_grid, kH128, 0, stream>>>(
      *descriptor);
  return static_cast<int32_t>(cudaPeekAtLastError());
}

extern "C" uint64_t glmaxx_exl3_projection_workspace_bytes(
    uint32_t rows, uint32_t logical_k, uint32_t logical_n) {
  return workspace_bytes(rows, logical_k, logical_n);
}

extern "C" const char* glmaxx_exl3_kernel_abi(void) {
  return "glmaxx.sm120.exl3.source_projection.v1";
}
