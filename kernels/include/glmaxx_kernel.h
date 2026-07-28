#ifndef GLMAXX_KERNEL_H_
#define GLMAXX_KERNEL_H_

#include <stddef.h>
#include <stdint.h>

#if defined(__cplusplus)
extern "C" {
#endif

enum {
  GLMAXX_FC1_ABI_VERSION = 1,
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

int32_t glmaxx_nvfp4_routed_fc1_launch(
    const struct glmaxx_fc1_descriptor* descriptor,
    void* cuda_stream,
    int32_t* asynchronous_error);

uint64_t glmaxx_nvfp4_routed_fc1_workspace_bytes(uint32_t assignments);
const char* glmaxx_kernel_abi(void);
int32_t glmaxx_device_alloc(uint64_t bytes, uint64_t* pointer);
int32_t glmaxx_device_free(uint64_t pointer);
int32_t glmaxx_stream_create(uint64_t* stream);
int32_t glmaxx_stream_destroy(uint64_t stream);
int32_t glmaxx_stream_query(uint64_t stream, int32_t* complete);
int32_t glmaxx_stream_synchronize(uint64_t stream);
int32_t glmaxx_memcpy_h2d(uint64_t destination, const void* source,
                          uint64_t bytes, uint64_t stream);
int32_t glmaxx_memcpy_d2h(void* destination, uint64_t source,
                          uint64_t bytes, uint64_t stream);

#if defined(__cplusplus)
}

static_assert(sizeof(glmaxx_fc1_descriptor) == 224,
              "Rust/C descriptor size mismatch");
static_assert(alignof(glmaxx_fc1_descriptor) == 16,
              "Rust/C descriptor alignment mismatch");
#endif

#endif  // GLMAXX_KERNEL_H_
