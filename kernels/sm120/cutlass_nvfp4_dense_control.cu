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
#include <cutlass/gemm/group_array_problem_shape.hpp>
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

using ElementC = void;
using ElementD = cutlass::bfloat16_t;
using LayoutCTag = cutlass::layout::RowMajor;
using LayoutDTag = cutlass::layout::RowMajor;
constexpr int kAlignmentC = 1;
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

using GroupedProblemShape =
    cutlass::gemm::GroupProblemShape<Shape<int, int, int>>;
using GroupedCollectiveEpilogue =
    typename cutlass::epilogue::collective::CollectiveBuilder<
        ArchTag, OperatorClass, ThreadBlockShape, ClusterShape,
        cutlass::epilogue::collective::EpilogueTileAuto, ElementAccumulator,
        ElementAccumulator, ElementC, LayoutCTag*, kAlignmentC, ElementD,
        LayoutDTag*, kAlignmentD,
        cutlass::epilogue::collective::EpilogueScheduleAuto>::CollectiveOp;
using GroupedCollectiveMainloop =
    typename cutlass::gemm::collective::CollectiveBuilder<
        ArchTag, OperatorClass, ElementA, LayoutATag*, kAlignmentA, ElementB,
        LayoutBTag*, kAlignmentB, ElementAccumulator, ThreadBlockShape,
        ClusterShape,
        cutlass::gemm::collective::StageCountAutoCarveout<static_cast<int>(
            sizeof(typename GroupedCollectiveEpilogue::SharedStorage))>,
        cutlass::gemm::collective::KernelScheduleAuto>::CollectiveOp;
using GroupedGemmKernel = cutlass::gemm::kernel::GemmUniversal<
    GroupedProblemShape, GroupedCollectiveMainloop,
    GroupedCollectiveEpilogue>;
using GroupedGemm =
    cutlass::gemm::device::GemmUniversalAdapter<GroupedGemmKernel>;
using GroupedStrideA = typename GroupedGemmKernel::InternalStrideA;
using GroupedStrideB = typename GroupedGemmKernel::InternalStrideB;
using GroupedStrideD = typename GroupedGemmKernel::InternalStrideD;
using GroupedLayoutSFA =
    typename GroupedCollectiveMainloop::InternalLayoutSFA;
using GroupedLayoutSFB =
    typename GroupedCollectiveMainloop::InternalLayoutSFB;
using GroupedElementA = typename GroupedGemm::ElementA;
using GroupedElementB = typename GroupedGemm::ElementB;
using GroupedElementSF = typename GroupedCollectiveMainloop::ElementSF;
using GroupedElementD =
    typename GroupedGemm::EpilogueOutputOp::ElementOutput;
using UnderlyingProblemShape =
    typename GroupedProblemShape::UnderlyingProblemShape;

int32_t cutlass_error(cutlass::Status status) {
  return -1000 - static_cast<int32_t>(status);
}

__global__ void scale_and_swiglu_grouped(
    const glmaxx_fc1_descriptor descriptor) {
  const uint64_t total =
      uint64_t{descriptor.assignments} * kLocalIntermediate;
  const auto* gate_up =
      reinterpret_cast<const __nv_bfloat16*>(descriptor.gate_up_accum_f32);
  const auto* route_experts =
      reinterpret_cast<const uint16_t*>(descriptor.route_experts_u16);
  const auto* activation_globals =
      reinterpret_cast<const float*>(descriptor.activation_global_scales);
  const auto* weight_globals =
      reinterpret_cast<const float*>(descriptor.expert_global_scales);
  auto* output = reinterpret_cast<__nv_bfloat16*>(descriptor.output_bf16);

  for (uint64_t linear = uint64_t{blockIdx.x} * blockDim.x + threadIdx.x;
       linear < total; linear += uint64_t{gridDim.x} * blockDim.x) {
    const uint32_t assignment =
        static_cast<uint32_t>(linear / kLocalIntermediate);
    const uint32_t column =
        static_cast<uint32_t>(linear % kLocalIntermediate);
    const uint32_t expert = route_experts[assignment];
    const float scale =
        activation_globals[assignment] * weight_globals[expert];
    const uint64_t row = uint64_t{assignment} * kLocalGateUp;
    const float gate = __bfloat162float(gate_up[row + column]) * scale;
    const float up =
        __bfloat162float(gate_up[row + kLocalIntermediate + column]) * scale;
    const float silu = gate / (1.0f + expf(-gate));
    output[linear] = __float2bfloat16_rn(silu * up);
  }
}

struct GroupedScratch {
  uint16_t* active_experts;
  UnderlyingProblemShape* problem_shapes;
  const GroupedElementA** ptr_a;
  const GroupedElementB** ptr_b;
  const GroupedElementSF** ptr_sfa;
  const GroupedElementSF** ptr_sfb;
  GroupedElementD** ptr_d;
  GroupedStrideA* stride_a;
  GroupedStrideB* stride_b;
  GroupedStrideD* stride_d;
  GroupedLayoutSFA* layout_sfa;
  GroupedLayoutSFB* layout_sfb;
  uint64_t workspace;
  uint64_t metadata_bytes;
};

template <typename T>
T* take_scratch(uint64_t base, uint64_t* offset, uint32_t count) {
  const uint64_t alignment = alignof(T);
  *offset = (*offset + alignment - 1) / alignment * alignment;
  T* result = reinterpret_cast<T*>(base + *offset);
  *offset += uint64_t{count} * sizeof(T);
  return result;
}

GroupedScratch grouped_scratch(const glmaxx_fc1_descriptor& descriptor,
                               uint32_t groups) {
  const uint64_t base = descriptor.compacted_input_bf16;
  uint64_t offset = uint64_t{kExperts + 1} * sizeof(uint64_t);
  GroupedScratch scratch{};
  scratch.active_experts =
      take_scratch<uint16_t>(base, &offset, groups);
  scratch.problem_shapes =
      take_scratch<UnderlyingProblemShape>(base, &offset, groups);
  scratch.ptr_a =
      take_scratch<const GroupedElementA*>(base, &offset, groups);
  scratch.ptr_b =
      take_scratch<const GroupedElementB*>(base, &offset, groups);
  scratch.ptr_sfa =
      take_scratch<const GroupedElementSF*>(base, &offset, groups);
  scratch.ptr_sfb =
      take_scratch<const GroupedElementSF*>(base, &offset, groups);
  scratch.ptr_d = take_scratch<GroupedElementD*>(base, &offset, groups);
  scratch.stride_a =
      take_scratch<GroupedStrideA>(base, &offset, groups);
  scratch.stride_b =
      take_scratch<GroupedStrideB>(base, &offset, groups);
  scratch.stride_d =
      take_scratch<GroupedStrideD>(base, &offset, groups);
  scratch.layout_sfa =
      take_scratch<GroupedLayoutSFA>(base, &offset, groups);
  scratch.layout_sfb =
      take_scratch<GroupedLayoutSFB>(base, &offset, groups);
  offset = (offset + 255) / 256 * 256;
  scratch.workspace = base + offset;
  scratch.metadata_bytes = offset;
  return scratch;
}

__global__ void initialize_grouped_scratch(
    glmaxx_fc1_descriptor descriptor, uint32_t groups,
    const uint16_t* active_experts, UnderlyingProblemShape* problem_shapes,
    const GroupedElementA** ptr_a, const GroupedElementB** ptr_b,
    const GroupedElementSF** ptr_sfa, const GroupedElementSF** ptr_sfb,
    GroupedElementD** ptr_d, GroupedStrideA* stride_a,
    GroupedStrideB* stride_b, GroupedStrideD* stride_d,
    GroupedLayoutSFA* layout_sfa, GroupedLayoutSFB* layout_sfb) {
  const uint32_t group = blockIdx.x * blockDim.x + threadIdx.x;
  if (group >= groups) {
    return;
  }
  const uint32_t expert = active_experts[group];
  const auto* expert_offsets =
      reinterpret_cast<const uint32_t*>(descriptor.expert_offsets_u32);
  const auto* expert_sfa_offsets =
      reinterpret_cast<const uint64_t*>(descriptor.compacted_input_bf16);
  const uint32_t begin = expert_offsets[expert];
  const uint32_t end = expert_offsets[expert + 1];
  const int m = static_cast<int>(end - begin);
  const auto problem = make_shape(m, kLocalGateUp, kHidden);
  problem_shapes[group] = problem;
  ptr_a[group] = reinterpret_cast<const GroupedElementA*>(
      descriptor.activation_values + uint64_t{begin} * kHidden / 2);
  ptr_b[group] = reinterpret_cast<const GroupedElementB*>(
      descriptor.expert_value_base + uint64_t{expert} * kWeightValueBytes);
  ptr_sfa[group] = reinterpret_cast<const GroupedElementSF*>(
      descriptor.activation_scales + expert_sfa_offsets[expert]);
  ptr_sfb[group] = reinterpret_cast<const GroupedElementSF*>(
      descriptor.expert_scale_base + uint64_t{expert} * kWeightScaleBytes);
  ptr_d[group] = reinterpret_cast<GroupedElementD*>(
      descriptor.gate_up_accum_f32 +
      uint64_t{begin} * kLocalGateUp * sizeof(__nv_bfloat16));
  stride_a[group] =
      cutlass::make_cute_packed_stride(GroupedStrideA{},
                                      make_shape(m, kHidden, 1));
  stride_b[group] =
      cutlass::make_cute_packed_stride(GroupedStrideB{},
                                      make_shape(kLocalGateUp, kHidden, 1));
  stride_d[group] =
      cutlass::make_cute_packed_stride(GroupedStrideD{},
                                      make_shape(m, kLocalGateUp, 1));
  using BlockScaledConfig =
      typename GroupedCollectiveMainloop::Sm1xxBlkScaledConfig;
  const auto batched_problem =
      make_shape(m, kLocalGateUp, kHidden, 1);
  layout_sfa[group] =
      BlockScaledConfig::tile_atom_to_shape_SFA(batched_problem);
  layout_sfb[group] =
      BlockScaledConfig::tile_atom_to_shape_SFB(batched_problem);
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
      {{}, nullptr, stride_c, d, stride_d}};

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

int32_t enqueue_grouped_control(const glmaxx_fc1_descriptor& descriptor,
                                const uint16_t* active_experts,
                                uint32_t groups, cudaStream_t stream) {
  const GroupedScratch scratch = grouped_scratch(descriptor, groups);
  const uint64_t scratch_bytes =
      uint64_t{descriptor.assignments} * kHidden * sizeof(uint16_t);
  if (scratch.metadata_bytes >= scratch_bytes) {
    return -3;
  }
  cudaError_t cuda_status = cudaMemcpyAsync(
      scratch.active_experts, active_experts,
      uint64_t{groups} * sizeof(uint16_t), cudaMemcpyHostToDevice, stream);
  if (cuda_status != cudaSuccess) {
    return static_cast<int32_t>(cuda_status);
  }
  initialize_grouped_scratch<<<1, 256, 0, stream>>>(
      descriptor, groups, scratch.active_experts, scratch.problem_shapes,
      scratch.ptr_a, scratch.ptr_b, scratch.ptr_sfa, scratch.ptr_sfb,
      scratch.ptr_d, scratch.stride_a, scratch.stride_b, scratch.stride_d,
      scratch.layout_sfa, scratch.layout_sfb);
  cuda_status = cudaPeekAtLastError();
  if (cuda_status != cudaSuccess) {
    return static_cast<int32_t>(cuda_status);
  }

  cutlass::KernelHardwareInfo hardware{};
  cuda_status = cudaGetDevice(&hardware.device_id);
  if (cuda_status != cudaSuccess) {
    return static_cast<int32_t>(cuda_status);
  }
  hardware.sm_count =
      cutlass::KernelHardwareInfo::query_device_multiprocessor_count(
          hardware.device_id);
  typename GroupedGemmKernel::TileSchedulerArguments scheduler{};
  typename GroupedGemm::Arguments arguments{
      cutlass::gemm::GemmUniversalMode::kGrouped,
      {static_cast<int>(groups), scratch.problem_shapes, nullptr},
      {scratch.ptr_a, scratch.stride_a, scratch.ptr_b, scratch.stride_b,
       scratch.ptr_sfa, scratch.layout_sfa, scratch.ptr_sfb,
       scratch.layout_sfb},
      {{}, nullptr, nullptr, scratch.ptr_d, scratch.stride_d},
      hardware,
      scheduler};

  const size_t cutlass_workspace_bytes =
      GroupedGemm::get_workspace_size(arguments);
  if (scratch.metadata_bytes + cutlass_workspace_bytes > scratch_bytes) {
    return -3;
  }
  GroupedGemm gemm;
  cutlass::Status status = gemm.can_implement(arguments);
  if (status != cutlass::Status::kSuccess) {
    return cutlass_error(status);
  }
  status = gemm.initialize(
      arguments, reinterpret_cast<void*>(scratch.workspace), stream);
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
  scale_and_swiglu_grouped<<<blocks, kThreads, 0, stream>>>(descriptor);
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

extern "C" int32_t glmaxx_nvfp4_grouped_control_launch(
    const glmaxx_fc1_descriptor* descriptor,
    const uint16_t* active_experts, uint32_t active_expert_count,
    void* cuda_stream, int32_t* asynchronous_error) {
  if (descriptor == nullptr || active_experts == nullptr ||
      cuda_stream == nullptr || asynchronous_error == nullptr ||
      active_expert_count == 0 ||
      active_expert_count > glmaxx::dense_control::kExperts ||
      active_expert_count > descriptor->assignments) {
    return -1;
  }
  for (uint32_t index = 0; index < active_expert_count; ++index) {
    if (active_experts[index] >= glmaxx::dense_control::kExperts ||
        (index != 0 &&
         active_experts[index - 1] >= active_experts[index])) {
      return -2;
    }
  }
  *asynchronous_error = 0;
  const int32_t quantize_status =
      glmaxx_nvfp4_grouped_quantize_launch(descriptor, cuda_stream);
  if (quantize_status != 0) {
    return quantize_status;
  }
  return glmaxx::dense_control::enqueue_grouped_control(
      *descriptor, active_experts, active_expert_count,
      reinterpret_cast<cudaStream_t>(cuda_stream));
}
