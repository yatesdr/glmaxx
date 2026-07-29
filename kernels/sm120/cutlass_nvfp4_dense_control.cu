// GLMAXX-owned SM120 NVFP4 tensor-core development control.
//
// This deliberately materializes the 1,024-column gate/up result in BF16
// before applying row/expert global scales and SwiGLU. It exists to prove that
// the frozen GLMAXX value/SFA/SFB bytes feed native SM120 block-scaled MMA.
// The production operator must replace this materialized boundary with paired
// FP32 gate/up fragments and a fused BF16 SwiGLU store.

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

namespace glmaxx::dense_control {

using namespace cute;

constexpr int kHidden = 6144;
constexpr int kLocalGateUp = 1024;
constexpr int kLocalIntermediate = 512;
constexpr int kExperts = 256;
constexpr uint64_t kWeightValueBytes =
    uint64_t{kLocalGateUp} * kHidden / 2;
constexpr uint64_t kWeightScaleBytes =
    uint64_t{kLocalGateUp} * kHidden / 16;

using ElementA = cutlass::nv_float4_t<cutlass::float_e2m1_t>;
using LayoutATag = cutlass::layout::RowMajor;
constexpr int kAlignmentA = 32;

using ElementB = cutlass::nv_float4_t<cutlass::float_e2m1_t>;
using LayoutBTag = cutlass::layout::ColumnMajor;
constexpr int kAlignmentB = 32;

using ElementC = cutlass::bfloat16_t;
using ElementD = cutlass::bfloat16_t;
using LayoutCTag = cutlass::layout::RowMajor;
using LayoutDTag = cutlass::layout::RowMajor;
constexpr int kAlignmentC = 128 / cutlass::sizeof_bits<ElementC>::value;
constexpr int kAlignmentD = 128 / cutlass::sizeof_bits<ElementD>::value;

using ElementAccumulator = float;
using ArchTag = cutlass::arch::Sm120;
using OperatorClass = cutlass::arch::OpClassBlockScaledTensorOp;
using ThreadBlockShape = Shape<_128, _128, _128>;
using ClusterShape = Shape<_1, _1, _1>;

using CollectiveEpilogue =
    typename cutlass::epilogue::collective::CollectiveBuilder<
        ArchTag, OperatorClass, ThreadBlockShape, ClusterShape,
        cutlass::epilogue::collective::EpilogueTileAuto, ElementAccumulator,
        ElementAccumulator, ElementC, LayoutCTag, kAlignmentC, ElementD,
        LayoutDTag, kAlignmentD,
        cutlass::epilogue::collective::EpilogueScheduleAuto>::CollectiveOp;

using CollectiveMainloop =
    typename cutlass::gemm::collective::CollectiveBuilder<
        ArchTag, OperatorClass, ElementA, LayoutATag, kAlignmentA, ElementB,
        LayoutBTag, kAlignmentB, ElementAccumulator, ThreadBlockShape,
        ClusterShape,
        cutlass::gemm::collective::StageCountAutoCarveout<
            static_cast<int>(sizeof(typename CollectiveEpilogue::SharedStorage))>,
        cutlass::gemm::collective::KernelScheduleAuto>::CollectiveOp;

using GemmKernel = cutlass::gemm::kernel::GemmUniversal<
    Shape<int, int, int, int>, CollectiveMainloop, CollectiveEpilogue, void>;
using Gemm = cutlass::gemm::device::GemmUniversalAdapter<GemmKernel>;
using StrideA = typename GemmKernel::StrideA;
using StrideB = typename GemmKernel::StrideB;
using StrideC = typename GemmKernel::StrideC;
using StrideD = typename GemmKernel::StrideD;

int32_t cutlass_error(cutlass::Status status) {
  return -1000 - static_cast<int32_t>(status);
}

__global__ void scale_and_swiglu(const glmaxx_fc1_descriptor descriptor,
                                 uint32_t expert) {
  const uint64_t total =
      uint64_t{descriptor.assignments} * kLocalIntermediate;
  const auto* gate_up =
      reinterpret_cast<const __nv_bfloat16*>(descriptor.gate_up_accum_f32);
  const auto* activation_globals =
      reinterpret_cast<const float*>(descriptor.activation_global_scales);
  const auto* weight_globals =
      reinterpret_cast<const float*>(descriptor.expert_global_scales);
  auto* output = reinterpret_cast<__nv_bfloat16*>(descriptor.output_bf16);
  const float weight_global = weight_globals[expert];

  for (uint64_t linear = uint64_t{blockIdx.x} * blockDim.x + threadIdx.x;
       linear < total; linear += uint64_t{gridDim.x} * blockDim.x) {
    const uint32_t assignment =
        static_cast<uint32_t>(linear / kLocalIntermediate);
    const uint32_t column =
        static_cast<uint32_t>(linear % kLocalIntermediate);
    const float scale = activation_globals[assignment] * weight_global;
    const uint64_t row = uint64_t{assignment} * kLocalGateUp;
    const float gate = __bfloat162float(gate_up[row + column]) * scale;
    const float up =
        __bfloat162float(gate_up[row + kLocalIntermediate + column]) * scale;
    const float silu = gate / (1.0f + expf(-gate));
    output[linear] = __float2bfloat16_rn(silu * up);
  }
}

int32_t enqueue_dense_control(const glmaxx_fc1_descriptor& descriptor,
                              uint32_t expert, cudaStream_t stream) {
  const int m = static_cast<int>(descriptor.assignments);
  const int n = kLocalGateUp;
  const int k = kHidden;
  const auto problem_shape = make_shape(m, n, k, 1);
  const auto stride_a =
      cutlass::make_cute_packed_stride(StrideA{}, make_shape(m, k, 1));
  const auto stride_b =
      cutlass::make_cute_packed_stride(StrideB{}, make_shape(n, k, 1));
  const auto stride_c =
      cutlass::make_cute_packed_stride(StrideC{}, make_shape(m, n, 1));
  const auto stride_d =
      cutlass::make_cute_packed_stride(StrideD{}, make_shape(m, n, 1));

  using BlockScaledConfig =
      typename CollectiveMainloop::Sm1xxBlkScaledConfig;
  const auto layout_sfa =
      BlockScaledConfig::tile_atom_to_shape_SFA(problem_shape);
  const auto layout_sfb =
      BlockScaledConfig::tile_atom_to_shape_SFB(problem_shape);

  auto* a = reinterpret_cast<ElementA::DataType*>(
      descriptor.activation_values);
  auto* b = reinterpret_cast<ElementB::DataType*>(
      descriptor.expert_value_base + uint64_t{expert} * kWeightValueBytes);
  auto* sfa = reinterpret_cast<ElementA::ScaleFactorType*>(
      descriptor.activation_scales);
  auto* sfb = reinterpret_cast<ElementB::ScaleFactorType*>(
      descriptor.expert_scale_base + uint64_t{expert} * kWeightScaleBytes);
  auto* d =
      reinterpret_cast<ElementD*>(descriptor.gate_up_accum_f32);

  typename Gemm::Arguments arguments{
      cutlass::gemm::GemmUniversalMode::kGemm,
      problem_shape,
      {a, stride_a, b, stride_b, sfa, layout_sfa, sfb, layout_sfb},
      {{1.0f, 0.0f}, d, stride_c, d, stride_d}};

  const cutlass::Status implementable = Gemm::can_implement(arguments);
  if (implementable != cutlass::Status::kSuccess) {
    return cutlass_error(implementable);
  }
  const size_t cutlass_workspace_bytes = Gemm::get_workspace_size(arguments);
  const uint64_t available_workspace =
      uint64_t{descriptor.assignments} * kHidden * sizeof(uint16_t);
  if (cutlass_workspace_bytes > available_workspace) {
    return -3;
  }
  void* cutlass_workspace =
      reinterpret_cast<void*>(descriptor.compacted_input_bf16);
  Gemm gemm;
  cutlass::Status status =
      gemm.initialize(arguments, cutlass_workspace, stream);
  if (status != cutlass::Status::kSuccess) {
    return cutlass_error(status);
  }
  status = gemm.run(stream);
  if (status != cutlass::Status::kSuccess) {
    return cutlass_error(status);
  }

  constexpr uint32_t kThreads = 256;
  const uint64_t total =
      uint64_t{descriptor.assignments} * kLocalIntermediate;
  uint32_t blocks = static_cast<uint32_t>((total + kThreads - 1) / kThreads);
  if (blocks > 4096) {
    blocks = 4096;
  }
  scale_and_swiglu<<<blocks, kThreads, 0, stream>>>(descriptor, expert);
  return static_cast<int32_t>(cudaPeekAtLastError());
}

}  // namespace glmaxx::dense_control

extern "C" int32_t glmaxx_nvfp4_dense_control_launch(
    const glmaxx_fc1_descriptor* descriptor, uint32_t expert,
    void* cuda_stream, int32_t* asynchronous_error) {
  if (descriptor == nullptr || cuda_stream == nullptr ||
      asynchronous_error == nullptr ||
      expert >= glmaxx::dense_control::kExperts) {
    return -1;
  }
  *asynchronous_error = 0;
  const int32_t quantize_status =
      glmaxx_nvfp4_quantize_launch(descriptor, cuda_stream);
  if (quantize_status != 0) {
    return quantize_status;
  }
  return glmaxx::dense_control::enqueue_dense_control(
      *descriptor, expert, reinterpret_cast<cudaStream_t>(cuda_stream));
}
