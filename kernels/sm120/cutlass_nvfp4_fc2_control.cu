// GLMAXX-owned SM120 NVFP4 tensor-core FC2 development control.
//
// This control proves direct consumption of the frozen FC2 value/SFA/SFB
// planes by native block-scaled MMA. It materializes the unscaled projection
// in BF16 in the upper half of the assignment accumulator allocation, expands
// it to FP32 with the two global scales, and invokes the deterministic
// slot-ordered route-weight reduction. The production grouped operator must
// fuse the scaling and weighted scatter into its epilogue.

#include "glmaxx_kernel.h"

#include <cuda_bf16.h>
#include <cuda_runtime_api.h>

#include <cute/tensor.hpp>
#include <cutlass/bfloat16.h>
#include <cutlass/cutlass.h>
#include <cutlass/epilogue/collective/collective_builder.hpp>
#include <cutlass/gemm/collective/collective_builder.hpp>
#include <cutlass/gemm/device/gemm_universal_adapter.h>
#include <cutlass/gemm/kernel/gemm_universal.hpp>
#include <cutlass/util/packed_stride.hpp>

#include <cstddef>
#include <cstdint>

namespace glmaxx::fc2_control {

using namespace cute;

constexpr int kHidden = 6144;
constexpr int kLocalIntermediate = 512;
constexpr int kExperts = 256;
constexpr uint64_t kWeightValueBytes =
    uint64_t{kHidden} * kLocalIntermediate / 2;
constexpr uint64_t kWeightScaleBytes =
    uint64_t{kHidden} * kLocalIntermediate / 16;

using ElementA = cutlass::nv_float4_t<cutlass::float_e2m1_t>;
using LayoutATag = cutlass::layout::RowMajor;
constexpr int kAlignmentA = 32;

using ElementB = cutlass::nv_float4_t<cutlass::float_e2m1_t>;
using LayoutBTag = cutlass::layout::ColumnMajor;
constexpr int kAlignmentB = 32;

using ElementC = void;
using ElementD = cutlass::bfloat16_t;
using LayoutCTag = cutlass::layout::RowMajor;
using LayoutDTag = cutlass::layout::RowMajor;
constexpr int kAlignmentC = 1;
constexpr int kAlignmentD =
    128 / cutlass::sizeof_bits<ElementD>::value;

using ElementAccumulator = float;
using ArchTag = cutlass::arch::Sm120;
using OperatorClass = cutlass::arch::OpClassBlockScaledTensorOp;
using ThreadBlockShape = Shape<_128, _128, _128>;
using ClusterShape = Shape<_1, _1, _1>;

using CollectiveEpilogue =
    typename cutlass::epilogue::collective::CollectiveBuilder<
        ArchTag, OperatorClass, ThreadBlockShape, ClusterShape,
        cutlass::epilogue::collective::EpilogueTileAuto,
        ElementAccumulator, ElementAccumulator, ElementC, LayoutCTag,
        kAlignmentC, ElementD, LayoutDTag, kAlignmentD,
        cutlass::epilogue::collective::EpilogueScheduleAuto>::CollectiveOp;

using CollectiveMainloop =
    typename cutlass::gemm::collective::CollectiveBuilder<
        ArchTag, OperatorClass, ElementA, LayoutATag, kAlignmentA,
        ElementB, LayoutBTag, kAlignmentB, ElementAccumulator,
        ThreadBlockShape, ClusterShape,
        cutlass::gemm::collective::StageCountAutoCarveout<
            static_cast<int>(
                sizeof(typename CollectiveEpilogue::SharedStorage))>,
        cutlass::gemm::collective::KernelScheduleAuto>::CollectiveOp;

using GemmKernel = cutlass::gemm::kernel::GemmUniversal<
    Shape<int, int, int, int>, CollectiveMainloop,
    CollectiveEpilogue, void>;
using Gemm = cutlass::gemm::device::GemmUniversalAdapter<GemmKernel>;
using StrideA = typename GemmKernel::StrideA;
using StrideB = typename GemmKernel::StrideB;
using StrideC = typename GemmKernel::StrideC;
using StrideD = typename GemmKernel::StrideD;

int32_t cutlass_error(cutlass::Status status) {
  return -1100 - static_cast<int32_t>(status);
}

__global__ void expand_scaled_projection(
    const glmaxx_fc2_descriptor descriptor, uint32_t expert,
    const __nv_bfloat16* materialized) {
  const uint64_t total =
      uint64_t{descriptor.assignments} * kHidden;
  const auto* activation_globals =
      reinterpret_cast<const float*>(descriptor.activation_global_scales);
  const auto* weight_globals =
      reinterpret_cast<const float*>(descriptor.expert_global_scales);
  auto* output =
      reinterpret_cast<float*>(descriptor.assignment_down_f32);
  const float weight_global = weight_globals[expert];

  for (uint64_t linear = uint64_t{blockIdx.x} * blockDim.x + threadIdx.x;
       linear < total; linear += uint64_t{gridDim.x} * blockDim.x) {
    const uint32_t assignment =
        static_cast<uint32_t>(linear / kHidden);
    output[linear] =
        __bfloat162float(materialized[linear]) *
        activation_globals[assignment] * weight_global;
  }
}

int32_t enqueue_dense_control(const glmaxx_fc2_descriptor& descriptor,
                              uint32_t expert, cudaStream_t stream) {
  const int m = static_cast<int>(descriptor.assignments);
  const int n = kHidden;
  const int k = kLocalIntermediate;
  const auto problem_shape = make_shape(m, n, k, 1);
  const auto stride_a =
      cutlass::make_cute_packed_stride(
          StrideA{}, make_shape(m, k, 1));
  const auto stride_b =
      cutlass::make_cute_packed_stride(
          StrideB{}, make_shape(n, k, 1));
  const auto stride_c =
      cutlass::make_cute_packed_stride(
          StrideC{}, make_shape(m, n, 1));
  const auto stride_d =
      cutlass::make_cute_packed_stride(
          StrideD{}, make_shape(m, n, 1));

  using BlockScaledConfig =
      typename CollectiveMainloop::Sm1xxBlkScaledConfig;
  const auto layout_sfa =
      BlockScaledConfig::tile_atom_to_shape_SFA(problem_shape);
  const auto layout_sfb =
      BlockScaledConfig::tile_atom_to_shape_SFB(problem_shape);

  auto* a = reinterpret_cast<ElementA::DataType*>(
      descriptor.activation_values);
  auto* b = reinterpret_cast<ElementB::DataType*>(
      descriptor.expert_value_base +
      uint64_t{expert} * kWeightValueBytes);
  auto* sfa = reinterpret_cast<ElementA::ScaleFactorType*>(
      descriptor.activation_scales);
  auto* sfb = reinterpret_cast<ElementB::ScaleFactorType*>(
      descriptor.expert_scale_base +
      uint64_t{expert} * kWeightScaleBytes);
  const uint64_t projection_elements =
      uint64_t{descriptor.assignments} * kHidden;
  auto* materialized = reinterpret_cast<ElementD*>(
      descriptor.assignment_down_f32 +
      projection_elements * sizeof(uint16_t));

  typename Gemm::Arguments arguments{
      cutlass::gemm::GemmUniversalMode::kGemm,
      problem_shape,
      {a, stride_a, b, stride_b, sfa, layout_sfa, sfb, layout_sfb},
      {{}, nullptr, stride_c, materialized, stride_d}};

  cutlass::Status status = Gemm::can_implement(arguments);
  if (status != cutlass::Status::kSuccess) {
    return cutlass_error(status);
  }
  const size_t cutlass_workspace_bytes =
      Gemm::get_workspace_size(arguments);
  const uint64_t available_workspace =
      uint64_t{descriptor.rows} * kHidden * sizeof(float);
  if (cutlass_workspace_bytes > available_workspace) {
    return -3;
  }
  void* cutlass_workspace =
      reinterpret_cast<void*>(descriptor.token_output_f32);
  Gemm gemm;
  status = gemm.initialize(arguments, cutlass_workspace, stream);
  if (status != cutlass::Status::kSuccess) {
    return cutlass_error(status);
  }
  status = gemm.run(stream);
  if (status != cutlass::Status::kSuccess) {
    return cutlass_error(status);
  }

  constexpr uint32_t kThreads = 256;
  const uint64_t required_blocks =
      (projection_elements + kThreads - 1) / kThreads;
  uint32_t blocks = static_cast<uint32_t>(
      required_blocks < 4096 ? required_blocks : 4096);
  expand_scaled_projection<<<blocks, kThreads, 0, stream>>>(
      descriptor, expert,
      reinterpret_cast<const __nv_bfloat16*>(materialized));
  return static_cast<int32_t>(cudaPeekAtLastError());
}

}  // namespace glmaxx::fc2_control

extern "C" int32_t glmaxx_nvfp4_fc2_dense_control_launch(
    const glmaxx_fc2_descriptor* descriptor, uint32_t expert,
    void* cuda_stream, int32_t* asynchronous_error) {
  if (descriptor == nullptr || cuda_stream == nullptr ||
      asynchronous_error == nullptr ||
      expert >= glmaxx::fc2_control::kExperts) {
    return -1;
  }
  *asynchronous_error = 0;
  const int32_t quantize_status =
      glmaxx_nvfp4_fc2_quantize_launch(descriptor, cuda_stream);
  if (quantize_status != 0) {
    return quantize_status;
  }
  const int32_t core_status =
      glmaxx::fc2_control::enqueue_dense_control(
          *descriptor, expert,
          reinterpret_cast<cudaStream_t>(cuda_stream));
  if (core_status != 0) {
    return core_status;
  }
  return glmaxx_nvfp4_fc2_reduce_launch(descriptor, cuda_stream);
}
