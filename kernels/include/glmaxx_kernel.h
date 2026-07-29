#ifndef GLMAXX_KERNEL_H_
#define GLMAXX_KERNEL_H_

#include <stddef.h>
#include <stdint.h>

#if defined(__cplusplus)
extern "C" {
#endif

enum {
  GLMAXX_FC1_ABI_VERSION = 1,
  GLMAXX_FC2_ABI_VERSION = 1,
  GLMAXX_FC1_DECODE_PERSISTENT = 1,
  GLMAXX_FC1_PREFILL_GROUPED = 2,
};

#if defined(__cplusplus)
struct alignas(16) glmaxx_fc1_descriptor {
#else
struct glmaxx_fc1_descriptor {
#endif
  uint32_t abi_version;
  uint32_t struct_bytes;
  uint32_t flags;
  uint32_t path;
  uint32_t rows;
  uint32_t assignments;
  uint32_t hidden;
  uint32_t local_gate_up;
  uint32_t local_intermediate;
  uint32_t experts;
  uint32_t top_k;
  uint32_t reserved0;
  uint64_t input_bf16;
  uint64_t expert_value_base;
  uint64_t expert_scale_base;
  uint64_t expert_global_scales;
  uint64_t route_experts_u16;
  uint64_t route_tokens_u32;
  uint64_t route_slots_u8;
  uint64_t route_weights_f32;
  uint64_t expert_offsets_u32;
  uint64_t compacted_input_bf16;
  uint64_t activation_values;
  uint64_t activation_scales;
  uint64_t activation_global_scales;
  uint64_t gate_up_accum_f32;
  uint64_t output_bf16;
  uint64_t workspace_bytes;
  uint64_t sequence;
  uint64_t reserved[4];
};

#if defined(__cplusplus)
struct alignas(16) glmaxx_fc2_descriptor {
#else
struct glmaxx_fc2_descriptor {
#endif
  uint32_t abi_version;
  uint32_t struct_bytes;
  uint32_t flags;
  uint32_t path;
  uint32_t rows;
  uint32_t assignments;
  uint32_t hidden;
  uint32_t local_intermediate;
  uint32_t experts;
  uint32_t top_k;
  uint32_t reserved0;
  uint32_t reserved1;
  uint64_t input_bf16;
  uint64_t expert_value_base;
  uint64_t expert_scale_base;
  uint64_t expert_global_scales;
  uint64_t route_experts_u16;
  uint64_t route_tokens_u32;
  uint64_t route_slots_u8;
  uint64_t route_weights_f32;
  uint64_t expert_offsets_u32;
  uint64_t activation_values;
  uint64_t activation_scales;
  uint64_t activation_global_scales;
  uint64_t assignment_down_f32;
  uint64_t token_output_f32;
  uint64_t slot_assignment_u32;
  uint64_t validation_error_u32;
  uint64_t workspace_bytes;
  uint64_t sequence;
  uint64_t reserved[4];
};

int32_t glmaxx_nvfp4_routed_fc1_launch(
    const struct glmaxx_fc1_descriptor* descriptor,
    void* cuda_stream,
    int32_t* asynchronous_error);
int32_t glmaxx_nvfp4_routed_fc1_graph_instantiate(
    const struct glmaxx_fc1_descriptor* descriptor,
    void* cuda_stream,
    uint64_t* graph_exec);
int32_t glmaxx_nvfp4_quantize_launch(
    const struct glmaxx_fc1_descriptor* descriptor,
    void* cuda_stream);
int32_t glmaxx_nvfp4_grouped_quantize_launch(
    const struct glmaxx_fc1_descriptor* descriptor,
    void* cuda_stream);
int32_t glmaxx_nvfp4_core_swiglu_launch(
    const struct glmaxx_fc1_descriptor* descriptor,
    void* cuda_stream);
int32_t glmaxx_nvfp4_dense_control_launch(
    const struct glmaxx_fc1_descriptor* descriptor,
    uint32_t expert,
    void* cuda_stream,
    int32_t* asynchronous_error);
int32_t glmaxx_nvfp4_grouped_control_launch(
    const struct glmaxx_fc1_descriptor* descriptor,
    const uint16_t* active_experts,
    uint32_t active_expert_count,
    void* cuda_stream,
    int32_t* asynchronous_error);
int32_t glmaxx_nvfp4_grouped_core_swiglu_launch(
    const struct glmaxx_fc1_descriptor* descriptor,
    const uint16_t* active_experts,
    uint32_t active_expert_count,
    void* cuda_stream,
    int32_t* asynchronous_error);
int32_t glmaxx_nvfp4_grouped_prepare_launch(
    const struct glmaxx_fc1_descriptor* descriptor,
    const uint16_t* active_experts,
    uint32_t active_expert_count,
    void* cuda_stream);
int32_t glmaxx_nvfp4_grouped_prepared_control_launch(
    const struct glmaxx_fc1_descriptor* descriptor,
    uint32_t active_expert_count,
    void* cuda_stream,
    int32_t* asynchronous_error);
int32_t glmaxx_nvfp4_grouped_prepared_core_swiglu_launch(
    const struct glmaxx_fc1_descriptor* descriptor,
    uint32_t active_expert_count,
    void* cuda_stream,
    int32_t* asynchronous_error);
int32_t glmaxx_nvfp4_routed_fc2_launch(
    const struct glmaxx_fc2_descriptor* descriptor,
    void* cuda_stream,
    int32_t* asynchronous_error);
int32_t glmaxx_nvfp4_fc2_quantize_launch(
    const struct glmaxx_fc2_descriptor* descriptor,
    void* cuda_stream);
int32_t glmaxx_nvfp4_fc2_core_launch(
    const struct glmaxx_fc2_descriptor* descriptor,
    void* cuda_stream);
int32_t glmaxx_nvfp4_fc2_reduce_launch(
    const struct glmaxx_fc2_descriptor* descriptor,
    void* cuda_stream);
int32_t glmaxx_graph_exec_launch(uint64_t graph_exec, uint64_t stream);
int32_t glmaxx_graph_exec_destroy(uint64_t graph_exec);
int32_t glmaxx_event_create(uint64_t* event);
int32_t glmaxx_event_record(uint64_t event, uint64_t stream);
int32_t glmaxx_event_synchronize(uint64_t event);
int32_t glmaxx_event_elapsed_ms(uint64_t start, uint64_t end,
                                float* milliseconds);
int32_t glmaxx_event_destroy(uint64_t event);

uint64_t glmaxx_nvfp4_routed_fc1_workspace_bytes(uint32_t assignments);
uint64_t glmaxx_nvfp4_grouped_workspace_bytes(uint32_t assignments);
uint64_t glmaxx_nvfp4_routed_fc2_workspace_bytes(uint32_t rows,
                                                 uint32_t assignments);
const char* glmaxx_kernel_abi(void);
int32_t glmaxx_device_alloc(uint64_t bytes, uint64_t* pointer);
int32_t glmaxx_device_free(uint64_t pointer);
int32_t glmaxx_stream_create(uint64_t* stream);
int32_t glmaxx_stream_destroy(uint64_t stream);
int32_t glmaxx_stream_query(uint64_t stream, int32_t* complete);
int32_t glmaxx_stream_synchronize(uint64_t stream);
int32_t glmaxx_memcpy_h2d(uint64_t destination, const void* source,
                          uint64_t bytes, uint64_t stream);
int32_t glmaxx_memcpy_d2d(uint64_t destination, uint64_t source,
                          uint64_t bytes, uint64_t stream);
int32_t glmaxx_memcpy_d2h(void* destination, uint64_t source,
                          uint64_t bytes, uint64_t stream);

#if defined(__cplusplus)
}

static_assert(sizeof(glmaxx_fc1_descriptor) == 224,
              "Rust/C descriptor size mismatch");
static_assert(alignof(glmaxx_fc1_descriptor) == 16,
              "Rust/C descriptor alignment mismatch");
static_assert(sizeof(glmaxx_fc2_descriptor) == 224,
              "Rust/C FC2 descriptor size mismatch");
static_assert(alignof(glmaxx_fc2_descriptor) == 16,
              "Rust/C FC2 descriptor alignment mismatch");
#endif

#endif  // GLMAXX_KERNEL_H_
