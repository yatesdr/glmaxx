#ifndef GLMAXX_SM120_RANK_EXECUTOR_NATIVE_ABI_V1_H_
#define GLMAXX_SM120_RANK_EXECUTOR_NATIVE_ABI_V1_H_

/*
 * Normative design header for the production rank-executor ABI.
 *
 * This header deliberately contains no CUDA or NCCL types. All native
 * resources are owner-thread-affine opaque handles. It is a review candidate,
 * not evidence that the functions have been implemented.
 */

#include <stddef.h>
#include <stdint.h>

#if defined(__cplusplus)
#define GLMAXX_EXECUTOR_ALIGN16 alignas(16)
#define GLMAXX_EXECUTOR_NOEXCEPT noexcept
extern "C" {
#elif defined(__clang__) || defined(__GNUC__)
#define GLMAXX_EXECUTOR_ALIGN16 __attribute__((aligned(16)))
#define GLMAXX_EXECUTOR_NOEXCEPT
#else
#error "GLMAXX executor ABI v1 requires C++17 alignas or a Clang/GNU C11 compiler"
#endif

enum {
  GLMAXX_EXECUTOR_ABI_VERSION = 1,
  GLMAXX_EXECUTOR_WORLD_SIZE = 4,
  GLMAXX_EXECUTOR_UNIQUE_ID_BYTES = 128,
  GLMAXX_EXECUTOR_FLAGS_V1_NONE = 0,
  GLMAXX_EXECUTOR_COMPUTE_CAPABILITY_SM120 = 120,
};

enum glmaxx_executor_status_v1 {
  GLMAXX_EXECUTOR_OK = 0,
  GLMAXX_EXECUTOR_NOT_READY = 1,
  GLMAXX_EXECUTOR_INVALID_ARGUMENT = -1,
  GLMAXX_EXECUTOR_ABI_MISMATCH = -2,
  GLMAXX_EXECUTOR_UNSUPPORTED = -3,
  GLMAXX_EXECUTOR_WRONG_THREAD = -4,
  GLMAXX_EXECUTOR_STALE_GENERATION = -5,
  GLMAXX_EXECUTOR_OUT_OF_MEMORY = -6,
  GLMAXX_EXECUTOR_CUDA_ERROR = -7,
  GLMAXX_EXECUTOR_NCCL_ERROR = -8,
  GLMAXX_EXECUTOR_ASYNC_ERROR = -9,
  GLMAXX_EXECUTOR_POISONED = -10,
  GLMAXX_EXECUTOR_INTERNAL_ERROR = -11,
};

enum glmaxx_executor_subsystem_v1 {
  GLMAXX_SUBSYSTEM_NONE = 0,
  GLMAXX_SUBSYSTEM_CONTEXT = 1,
  GLMAXX_SUBSYSTEM_PEER = 2,
  GLMAXX_SUBSYSTEM_MODULE = 3,
  GLMAXX_SUBSYSTEM_ARENA = 4,
  GLMAXX_SUBSYSTEM_STREAM_EVENT = 5,
  GLMAXX_SUBSYSTEM_COLLECTIVE = 6,
  GLMAXX_SUBSYSTEM_GRAPH = 7,
  GLMAXX_SUBSYSTEM_PROGRAM = 8,
  GLMAXX_SUBSYSTEM_VALIDATION = 9,
};

enum glmaxx_executor_arena_kind_v1 {
  GLMAXX_ARENA_DEVICE = 1,
  GLMAXX_ARENA_HOST_PINNED = 2,
};

enum glmaxx_executor_arena_role_v1 {
  GLMAXX_ARENA_ROLE_DEVICE_WEIGHTS = 1,
  GLMAXX_ARENA_ROLE_DEVICE_CODEC_METADATA = 2,
  GLMAXX_ARENA_ROLE_DEVICE_TARGET_KV = 3,
  GLMAXX_ARENA_ROLE_DEVICE_TARGET_INDEXER = 4,
  GLMAXX_ARENA_ROLE_DEVICE_DRAFT_SIDECAR = 5,
  GLMAXX_ARENA_ROLE_DEVICE_PAGE_TABLE = 6,
  GLMAXX_ARENA_ROLE_DEVICE_GRAPH_ARGUMENT = 7,
  GLMAXX_ARENA_ROLE_DEVICE_GRAPH_SCRATCH = 8,
  GLMAXX_ARENA_ROLE_DEVICE_COLLECTIVE = 9,
  GLMAXX_ARENA_ROLE_DEVICE_TIER_TRANSFER = 10,
  GLMAXX_ARENA_ROLE_DEVICE_COMPLETION_STATUS = 11,
  GLMAXX_ARENA_ROLE_DEVICE_DIAGNOSTIC_STATUS = 12,
  GLMAXX_ARENA_ROLE_HOST_CHECKPOINT_STAGING = 13,
  GLMAXX_ARENA_ROLE_HOST_ARGUMENT_MIRROR = 14,
  GLMAXX_ARENA_ROLE_HOST_COMPLETION_MIRROR = 15,
  GLMAXX_ARENA_ROLE_HOST_TIER_IN = 16,
  GLMAXX_ARENA_ROLE_HOST_TIER_OUT = 17,
  GLMAXX_ARENA_ROLE_HOST_DIAGNOSTIC_STATUS = 18,
};

enum glmaxx_executor_copy_kind_v1 {
  GLMAXX_COPY_H2D = 1,
  GLMAXX_COPY_D2H = 2,
  GLMAXX_COPY_D2D = 3,
};

enum glmaxx_executor_collective_family_v1 {
  GLMAXX_COLLECTIVE_NCCL_ALL_REDUCE = 1,
  GLMAXX_COLLECTIVE_NCCL_ALL_GATHER = 2,
  GLMAXX_COLLECTIVE_DIRECT_ONE_SHOT = 3,
  GLMAXX_COLLECTIVE_RING = 4,
  GLMAXX_COLLECTIVE_TREE = 5,
  GLMAXX_COLLECTIVE_TWO_PAIR = 6,
  GLMAXX_COLLECTIVE_PACKED_RECORD_GATHER = 7,
  GLMAXX_COLLECTIVE_SAMPLING_GATHER_BROADCAST = 8,
  GLMAXX_COLLECTIVE_PARTIAL_LSE = 9,
};

enum glmaxx_executor_graph_kind_v1 {
  GLMAXX_GRAPH_PREFILL = 1,
  GLMAXX_GRAPH_DECODE = 2,
  GLMAXX_GRAPH_VERIFY = 3,
  GLMAXX_GRAPH_MIXED = 4,
  GLMAXX_GRAPH_VALIDATION = 5,
  GLMAXX_GRAPH_COLLECTIVE_KAT = 6,
};

enum glmaxx_executor_node_kind_v1 {
  GLMAXX_NODE_DEVICE_VALIDATE = 1,
  GLMAXX_NODE_TARGET_PROGRAM = 2,
  GLMAXX_NODE_MTP_PROGRAM = 3,
  GLMAXX_NODE_COLLECTIVE = 4,
  GLMAXX_NODE_STATUS_FINALIZE = 5,
};

enum glmaxx_executor_kernel_family_v1 {
  GLMAXX_KERNEL_TARGET_PROGRAM = 1,
  GLMAXX_KERNEL_MTP_PROGRAM = 2,
  GLMAXX_KERNEL_DEVICE_VALIDATION = 3,
};

typedef uint64_t glmaxx_executor_handle_v1;
typedef uint64_t glmaxx_executor_device_address_v1;

struct GLMAXX_EXECUTOR_ALIGN16 glmaxx_executor_error_v1 {
  uint32_t abi_version;
  uint32_t struct_bytes;
  int32_t status;
  uint32_t subsystem;
  int32_t native_code;
  uint32_t reserved0;
  uint64_t operation;
  uint64_t sequence;
  uint64_t detail0;
  uint64_t detail1;
  uint64_t reserved1;
};

struct GLMAXX_EXECUTOR_ALIGN16 glmaxx_executor_context_config_v1 {
  uint32_t abi_version;
  uint32_t struct_bytes;
  uint32_t flags;
  uint32_t rank;
  uint32_t visible_device_count;
  uint32_t device_ordinal;
  uint32_t required_compute_capability;
  uint32_t reserved0;
  uint8_t expected_device_uuid[16];
  uint8_t expected_load_plan_sha256[32];
  uint64_t generation;
  uint64_t reserved1;
};

struct GLMAXX_EXECUTOR_ALIGN16 glmaxx_executor_device_caps_v1 {
  uint32_t abi_version;
  uint32_t struct_bytes;
  uint32_t rank;
  uint32_t device_ordinal;
  uint32_t compute_capability;
  uint32_t multiprocessor_count;
  uint32_t flags;
  uint32_t reserved0;
  uint64_t total_memory_bytes;
  uint64_t free_memory_bytes;
  uint8_t device_uuid[16];
  uint8_t pci_bus_id[16];
  uint32_t driver_version;
  uint32_t runtime_version;
  uint32_t nccl_version;
  uint32_t reserved1;
  uint8_t identity_sha256[32];
};

struct GLMAXX_EXECUTOR_ALIGN16 glmaxx_executor_peer_desc_v1 {
  uint32_t abi_version;
  uint32_t struct_bytes;
  uint32_t flags;
  uint32_t peer_rank;
  glmaxx_executor_handle_v1 context;
  glmaxx_executor_handle_v1 peer_context;
  uint32_t enable;
  uint32_t require_atomics;
  uint64_t generation;
  uint8_t topology_sha256[32];
};

struct GLMAXX_EXECUTOR_ALIGN16 glmaxx_executor_peer_caps_v1 {
  uint32_t abi_version;
  uint32_t struct_bytes;
  uint32_t owner_rank;
  uint32_t peer_rank;
  uint32_t can_access;
  uint32_t native_atomics;
  uint32_t enabled;
  uint32_t reserved0;
  uint8_t ordered_pair_sha256[32];
};

struct GLMAXX_EXECUTOR_ALIGN16 glmaxx_executor_module_image_v1 {
  uint32_t abi_version;
  uint32_t struct_bytes;
  uint32_t flags;
  uint32_t reserved0;
  uint64_t image_host_address;
  uint64_t image_bytes;
  uint8_t module_sha256[32];
  uint8_t expected_capability_sha256[32];
  uint64_t generation;
  uint64_t reserved1[3];
};

struct GLMAXX_EXECUTOR_ALIGN16 glmaxx_executor_module_capability_v1 {
  uint32_t abi_version;
  uint32_t struct_bytes;
  uint32_t kernel_family;
  uint32_t flags;
  uint32_t descriptor_version;
  uint32_t descriptor_bytes;
  uint32_t maximum_rows;
  uint32_t maximum_bucket;
  uint64_t required_dynamic_shared_bytes;
  uint64_t required_static_shared_bytes;
  uint64_t codec_mask;
  uint64_t tensor_role_mask;
  uint8_t module_sha256[32];
  uint8_t family_capability_sha256[32];
  uint64_t reserved[8];
};

struct GLMAXX_EXECUTOR_ALIGN16 glmaxx_executor_arena_desc_v1 {
  uint32_t abi_version;
  uint32_t struct_bytes;
  uint32_t arena_id;
  uint32_t arena_kind;
  uint64_t bytes;
  uint64_t generation;
  uint32_t alignment;
  uint32_t role;
  uint8_t resource_sha256[32];
  uint64_t reserved0;
};

struct GLMAXX_EXECUTOR_ALIGN16 glmaxx_executor_arena_binding_v1 {
  uint32_t abi_version;
  uint32_t struct_bytes;
  uint32_t arena_id;
  uint32_t arena_kind;
  glmaxx_executor_handle_v1 arena;
  uint64_t base_address;
  uint64_t bytes;
  uint32_t alignment;
  uint32_t reserved0;
};

struct GLMAXX_EXECUTOR_ALIGN16 glmaxx_executor_span_v1 {
  glmaxx_executor_handle_v1 arena;
  uint64_t offset;
  uint64_t bytes;
  uint64_t generation;
};

struct GLMAXX_EXECUTOR_ALIGN16 glmaxx_executor_copy_desc_v1 {
  uint32_t abi_version;
  uint32_t struct_bytes;
  uint32_t copy_kind;
  uint32_t flags;
  struct glmaxx_executor_span_v1 source;
  struct glmaxx_executor_span_v1 destination;
  glmaxx_executor_handle_v1 stream;
  glmaxx_executor_handle_v1 completion_event;
  uint64_t sequence;
  uint64_t reserved0;
};

struct GLMAXX_EXECUTOR_ALIGN16 glmaxx_executor_communicator_desc_v1 {
  uint32_t abi_version;
  uint32_t struct_bytes;
  uint32_t flags;
  uint32_t reserved0;
  glmaxx_executor_handle_v1 context;
  uint64_t generation;
  uint32_t rank;
  uint32_t world_size;
  uint32_t route_count;
  uint32_t reserved1;
  uint8_t topology_sha256[32];
  uint8_t route_table_sha256[32];
  uint8_t unique_id[GLMAXX_EXECUTOR_UNIQUE_ID_BYTES];
  uint64_t reserved2[2];
};

struct GLMAXX_EXECUTOR_ALIGN16 glmaxx_executor_route_desc_v1 {
  uint32_t abi_version;
  uint32_t struct_bytes;
  uint32_t family;
  uint32_t flags;
  glmaxx_executor_handle_v1 communicator;
  uint64_t route_id;
  uint32_t participant_mask;
  uint32_t reserved0;
  uint64_t maximum_logical_bytes;
  uint64_t maximum_wire_bytes;
  struct glmaxx_executor_span_v1 send;
  struct glmaxx_executor_span_v1 receive;
  struct glmaxx_executor_span_v1 scratch;
  uint8_t route_sha256[32];
  uint8_t topology_sha256[32];
  uint64_t generation;
  uint64_t reserved1[3];
};

struct GLMAXX_EXECUTOR_ALIGN16 glmaxx_executor_graph_desc_v1 {
  uint32_t abi_version;
  uint32_t struct_bytes;
  uint32_t graph_kind;
  uint32_t flags;
  glmaxx_executor_handle_v1 context;
  glmaxx_executor_handle_v1 stream;
  uint64_t graph_id;
  uint64_t generation;
  uint32_t row_bucket;
  uint32_t sequence_bucket;
  uint32_t token_bucket;
  uint32_t mtp_depth;
  uint32_t first_collective_ordinal;
  uint32_t last_collective_ordinal;
  uint32_t reserved0;
  uint32_t reserved1;
  struct glmaxx_executor_span_v1 argument_slab;
  struct glmaxx_executor_span_v1 scratch;
  uint8_t graph_profile_sha256[32];
  uint64_t reserved2[2];
};

struct GLMAXX_EXECUTOR_ALIGN16 glmaxx_executor_graph_node_v1 {
  uint32_t abi_version;
  uint32_t struct_bytes;
  uint32_t node_kind;
  uint32_t flags;
  glmaxx_executor_handle_v1 graph_builder;
  uint64_t node_ordinal;
  uint32_t launch_rows;
  uint32_t launch_bucket;
  uint32_t first_collective_ordinal;
  uint32_t last_collective_ordinal;
  uint8_t program_sha256[32];
  uint8_t collective_schedule_sha256[32];
  struct glmaxx_executor_span_v1 descriptor;
  struct glmaxx_executor_span_v1 status;
  /* Module handle for TARGET/MTP, route handle for COLLECTIVE, zero for status. */
  glmaxx_executor_handle_v1 native_object;
  uint64_t reserved0;
};

struct GLMAXX_EXECUTOR_ALIGN16 glmaxx_executor_validation_desc_v1 {
  uint32_t abi_version;
  uint32_t struct_bytes;
  uint32_t flags;
  uint32_t reserved0;
  glmaxx_executor_handle_v1 context;
  glmaxx_executor_handle_v1 stream;
  struct glmaxx_executor_span_v1 argument_slab;
  struct glmaxx_executor_span_v1 status;
  struct glmaxx_executor_span_v1 arena_table;
  uint64_t expected_graph_id;
  uint64_t expected_generation;
  uint32_t first_collective_ordinal;
  uint32_t last_collective_ordinal;
  uint32_t expected_rank;
  uint32_t reserved1;
  uint8_t program_sha256[32];
};

struct GLMAXX_EXECUTOR_ALIGN16 glmaxx_executor_launch_desc_v1 {
  uint32_t abi_version;
  uint32_t struct_bytes;
  uint32_t flags;
  uint32_t reserved0;
  glmaxx_executor_handle_v1 graph_exec;
  glmaxx_executor_handle_v1 stream;
  struct glmaxx_executor_span_v1 argument_slab;
  glmaxx_executor_handle_v1 completion_event;
  uint64_t step_id;
  uint64_t sequence;
  uint64_t reserved1;
};

struct GLMAXX_EXECUTOR_ALIGN16 glmaxx_executor_device_status_v1 {
  uint32_t abi_version;
  uint32_t struct_bytes;
  uint32_t rank;
  uint32_t flags;
  uint64_t generation;
  uint64_t step_id;
  uint32_t last_entered_collective_ordinal;
  uint32_t last_completed_collective_ordinal;
  uint32_t validation_latch;
  int32_t asynchronous_status;
  int32_t cuda_status;
  int32_t collective_status;
  int32_t kernel_status;
  uint32_t reserved0;
  uint8_t checksum_sha256[32];
  uint64_t reserved1[4];
};

/* Device/context and peer capability. */
int32_t glmaxx_executor_context_create_v1(
    const struct glmaxx_executor_context_config_v1* config,
    glmaxx_executor_handle_v1* context,
    struct glmaxx_executor_device_caps_v1* caps,
    struct glmaxx_executor_error_v1* error) GLMAXX_EXECUTOR_NOEXCEPT;
int32_t glmaxx_executor_context_memory_info_v1(
    glmaxx_executor_handle_v1 context,
    uint64_t* free_bytes,
    uint64_t* total_bytes,
    struct glmaxx_executor_error_v1* error) GLMAXX_EXECUTOR_NOEXCEPT;
int32_t glmaxx_executor_context_synchronize_v1(
    glmaxx_executor_handle_v1 context,
    struct glmaxx_executor_error_v1* error) GLMAXX_EXECUTOR_NOEXCEPT;
int32_t glmaxx_executor_peer_query_v1(
    glmaxx_executor_handle_v1 context,
    uint32_t peer_rank,
    struct glmaxx_executor_peer_caps_v1* caps,
    struct glmaxx_executor_error_v1* error) GLMAXX_EXECUTOR_NOEXCEPT;
int32_t glmaxx_executor_peer_apply_v1(
    const struct glmaxx_executor_peer_desc_v1* descriptor,
    struct glmaxx_executor_error_v1* error) GLMAXX_EXECUTOR_NOEXCEPT;
int32_t glmaxx_executor_context_destroy_v1(
    glmaxx_executor_handle_v1 context,
    struct glmaxx_executor_error_v1* error) GLMAXX_EXECUTOR_NOEXCEPT;

/* Module load and capability query. */
int32_t glmaxx_executor_module_load_v1(
    glmaxx_executor_handle_v1 context,
    const struct glmaxx_executor_module_image_v1* image,
    glmaxx_executor_handle_v1* module,
    struct glmaxx_executor_error_v1* error) GLMAXX_EXECUTOR_NOEXCEPT;
int32_t glmaxx_executor_module_capability_count_v1(
    glmaxx_executor_handle_v1 module,
    uint32_t* count,
    struct glmaxx_executor_error_v1* error) GLMAXX_EXECUTOR_NOEXCEPT;
int32_t glmaxx_executor_module_capability_query_v1(
    glmaxx_executor_handle_v1 module,
    uint32_t index,
    struct glmaxx_executor_module_capability_v1* capability,
    struct glmaxx_executor_error_v1* error) GLMAXX_EXECUTOR_NOEXCEPT;
int32_t glmaxx_executor_module_unload_v1(
    glmaxx_executor_handle_v1 module,
    struct glmaxx_executor_error_v1* error) GLMAXX_EXECUTOR_NOEXCEPT;

/* Deterministic device or pinned-host arenas and checked copies. */
int32_t glmaxx_executor_arena_create_v1(
    glmaxx_executor_handle_v1 context,
    const struct glmaxx_executor_arena_desc_v1* descriptor,
    struct glmaxx_executor_arena_binding_v1* binding,
    struct glmaxx_executor_error_v1* error) GLMAXX_EXECUTOR_NOEXCEPT;
int32_t glmaxx_executor_arena_memset_zero_v1(
    glmaxx_executor_handle_v1 context,
    const struct glmaxx_executor_span_v1* destination,
    glmaxx_executor_handle_v1 stream,
    struct glmaxx_executor_error_v1* error) GLMAXX_EXECUTOR_NOEXCEPT;
int32_t glmaxx_executor_copy_async_v1(
    glmaxx_executor_handle_v1 context,
    const struct glmaxx_executor_copy_desc_v1* descriptor,
    struct glmaxx_executor_error_v1* error) GLMAXX_EXECUTOR_NOEXCEPT;
int32_t glmaxx_executor_arena_destroy_v1(
    glmaxx_executor_handle_v1 arena,
    struct glmaxx_executor_error_v1* error) GLMAXX_EXECUTOR_NOEXCEPT;

/* Nonblocking stream and event lifecycle. */
int32_t glmaxx_executor_stream_create_v1(
    glmaxx_executor_handle_v1 context,
    uint32_t flags,
    glmaxx_executor_handle_v1* stream,
    struct glmaxx_executor_error_v1* error) GLMAXX_EXECUTOR_NOEXCEPT;
int32_t glmaxx_executor_stream_destroy_v1(
    glmaxx_executor_handle_v1 stream,
    struct glmaxx_executor_error_v1* error) GLMAXX_EXECUTOR_NOEXCEPT;
int32_t glmaxx_executor_event_create_v1(
    glmaxx_executor_handle_v1 context,
    uint32_t flags,
    glmaxx_executor_handle_v1* event,
    struct glmaxx_executor_error_v1* error) GLMAXX_EXECUTOR_NOEXCEPT;
int32_t glmaxx_executor_event_record_v1(
    glmaxx_executor_handle_v1 event,
    glmaxx_executor_handle_v1 stream,
    struct glmaxx_executor_error_v1* error) GLMAXX_EXECUTOR_NOEXCEPT;
int32_t glmaxx_executor_stream_wait_event_v1(
    glmaxx_executor_handle_v1 stream,
    glmaxx_executor_handle_v1 event,
    struct glmaxx_executor_error_v1* error) GLMAXX_EXECUTOR_NOEXCEPT;
int32_t glmaxx_executor_event_query_v1(
    glmaxx_executor_handle_v1 event,
    struct glmaxx_executor_error_v1* error) GLMAXX_EXECUTOR_NOEXCEPT;
int32_t glmaxx_executor_event_destroy_v1(
    glmaxx_executor_handle_v1 event,
    struct glmaxx_executor_error_v1* error) GLMAXX_EXECUTOR_NOEXCEPT;

/* Collective communicator and immutable route lifecycle. */
int32_t glmaxx_executor_collective_unique_id_v1(
    uint8_t unique_id[GLMAXX_EXECUTOR_UNIQUE_ID_BYTES],
    struct glmaxx_executor_error_v1* error) GLMAXX_EXECUTOR_NOEXCEPT;
int32_t glmaxx_executor_communicator_create_v1(
    const struct glmaxx_executor_communicator_desc_v1* descriptor,
    glmaxx_executor_handle_v1* communicator,
    struct glmaxx_executor_error_v1* error) GLMAXX_EXECUTOR_NOEXCEPT;
int32_t glmaxx_executor_route_create_v1(
    const struct glmaxx_executor_route_desc_v1* descriptor,
    glmaxx_executor_handle_v1* route,
    struct glmaxx_executor_error_v1* error) GLMAXX_EXECUTOR_NOEXCEPT;
int32_t glmaxx_executor_route_destroy_v1(
    glmaxx_executor_handle_v1 route,
    struct glmaxx_executor_error_v1* error) GLMAXX_EXECUTOR_NOEXCEPT;
int32_t glmaxx_executor_communicator_abort_v1(
    glmaxx_executor_handle_v1 communicator,
    struct glmaxx_executor_error_v1* error) GLMAXX_EXECUTOR_NOEXCEPT;
int32_t glmaxx_executor_communicator_destroy_v1(
    glmaxx_executor_handle_v1 communicator,
    struct glmaxx_executor_error_v1* error) GLMAXX_EXECUTOR_NOEXCEPT;

/* Cooperative graph construction, capture, instantiate, launch, destroy. */
int32_t glmaxx_executor_graph_builder_create_v1(
    const struct glmaxx_executor_graph_desc_v1* descriptor,
    glmaxx_executor_handle_v1* graph_builder,
    struct glmaxx_executor_error_v1* error) GLMAXX_EXECUTOR_NOEXCEPT;
int32_t glmaxx_executor_graph_node_add_v1(
    const struct glmaxx_executor_graph_node_v1* node,
    struct glmaxx_executor_error_v1* error) GLMAXX_EXECUTOR_NOEXCEPT;
int32_t glmaxx_executor_graph_instantiate_v1(
    glmaxx_executor_handle_v1 graph_builder,
    glmaxx_executor_handle_v1* graph_exec,
    struct glmaxx_executor_error_v1* error) GLMAXX_EXECUTOR_NOEXCEPT;
int32_t glmaxx_executor_graph_builder_destroy_v1(
    glmaxx_executor_handle_v1 graph_builder,
    struct glmaxx_executor_error_v1* error) GLMAXX_EXECUTOR_NOEXCEPT;
int32_t glmaxx_executor_graph_launch_v1(
    const struct glmaxx_executor_launch_desc_v1* descriptor,
    struct glmaxx_executor_error_v1* error) GLMAXX_EXECUTOR_NOEXCEPT;
int32_t glmaxx_executor_graph_destroy_v1(
    glmaxx_executor_handle_v1 graph_exec,
    struct glmaxx_executor_error_v1* error) GLMAXX_EXECUTOR_NOEXCEPT;

/* Device-validation construction and asynchronous status query. */
int32_t glmaxx_executor_validation_node_add_v1(
    glmaxx_executor_handle_v1 graph_builder,
    glmaxx_executor_handle_v1 validation_module,
    const struct glmaxx_executor_validation_desc_v1* descriptor,
    struct glmaxx_executor_error_v1* error) GLMAXX_EXECUTOR_NOEXCEPT;
int32_t glmaxx_executor_device_status_query_v1(
    glmaxx_executor_handle_v1 context,
    const struct glmaxx_executor_span_v1* status_span,
    struct glmaxx_executor_device_status_v1* status,
    struct glmaxx_executor_error_v1* error) GLMAXX_EXECUTOR_NOEXCEPT;

#if defined(__cplusplus)
}
#define GLMAXX_EXECUTOR_STATIC_ASSERT(condition, message) \
  static_assert((condition), message)
#define GLMAXX_EXECUTOR_ALIGNOF(type) alignof(type)
#else
#define GLMAXX_EXECUTOR_STATIC_ASSERT(condition, message) \
  _Static_assert((condition), message)
#define GLMAXX_EXECUTOR_ALIGNOF(type) _Alignof(type)
#endif

GLMAXX_EXECUTOR_STATIC_ASSERT(sizeof(struct glmaxx_executor_error_v1) == 64,
                              "error size");
GLMAXX_EXECUTOR_STATIC_ASSERT(sizeof(struct glmaxx_executor_context_config_v1) == 96,
                              "context config size");
GLMAXX_EXECUTOR_STATIC_ASSERT(sizeof(struct glmaxx_executor_device_caps_v1) == 128,
                              "device caps size");
GLMAXX_EXECUTOR_STATIC_ASSERT(sizeof(struct glmaxx_executor_peer_desc_v1) == 80,
                              "peer descriptor size");
GLMAXX_EXECUTOR_STATIC_ASSERT(sizeof(struct glmaxx_executor_peer_caps_v1) == 64,
                              "peer caps size");
GLMAXX_EXECUTOR_STATIC_ASSERT(sizeof(struct glmaxx_executor_module_image_v1) == 128,
                              "module image size");
GLMAXX_EXECUTOR_STATIC_ASSERT(sizeof(struct glmaxx_executor_module_capability_v1) == 192,
                              "module capability size");
GLMAXX_EXECUTOR_STATIC_ASSERT(sizeof(struct glmaxx_executor_arena_desc_v1) == 80,
                              "arena descriptor size");
GLMAXX_EXECUTOR_STATIC_ASSERT(sizeof(struct glmaxx_executor_arena_binding_v1) == 48,
                              "arena binding size");
GLMAXX_EXECUTOR_STATIC_ASSERT(sizeof(struct glmaxx_executor_span_v1) == 32,
                              "span size");
GLMAXX_EXECUTOR_STATIC_ASSERT(sizeof(struct glmaxx_executor_copy_desc_v1) == 112,
                              "copy descriptor size");
GLMAXX_EXECUTOR_STATIC_ASSERT(sizeof(struct glmaxx_executor_communicator_desc_v1) == 256,
                              "communicator descriptor size");
GLMAXX_EXECUTOR_STATIC_ASSERT(sizeof(struct glmaxx_executor_route_desc_v1) == 256,
                              "route descriptor size");
GLMAXX_EXECUTOR_STATIC_ASSERT(sizeof(struct glmaxx_executor_graph_desc_v1) == 192,
                              "graph descriptor size");
GLMAXX_EXECUTOR_STATIC_ASSERT(sizeof(struct glmaxx_executor_graph_node_v1) == 192,
                              "graph node size");
GLMAXX_EXECUTOR_STATIC_ASSERT(sizeof(struct glmaxx_executor_validation_desc_v1) == 192,
                              "validation descriptor size");
GLMAXX_EXECUTOR_STATIC_ASSERT(sizeof(struct glmaxx_executor_launch_desc_v1) == 96,
                              "launch descriptor size");
GLMAXX_EXECUTOR_STATIC_ASSERT(sizeof(struct glmaxx_executor_device_status_v1) == 128,
                              "device status size");

GLMAXX_EXECUTOR_STATIC_ASSERT(GLMAXX_EXECUTOR_ALIGNOF(struct glmaxx_executor_error_v1) == 16,
                              "error alignment");
GLMAXX_EXECUTOR_STATIC_ASSERT(GLMAXX_EXECUTOR_ALIGNOF(struct glmaxx_executor_context_config_v1) == 16,
                              "context config alignment");
GLMAXX_EXECUTOR_STATIC_ASSERT(GLMAXX_EXECUTOR_ALIGNOF(struct glmaxx_executor_device_caps_v1) == 16,
                              "device caps alignment");
GLMAXX_EXECUTOR_STATIC_ASSERT(GLMAXX_EXECUTOR_ALIGNOF(struct glmaxx_executor_peer_desc_v1) == 16,
                              "peer descriptor alignment");
GLMAXX_EXECUTOR_STATIC_ASSERT(GLMAXX_EXECUTOR_ALIGNOF(struct glmaxx_executor_peer_caps_v1) == 16,
                              "peer caps alignment");
GLMAXX_EXECUTOR_STATIC_ASSERT(GLMAXX_EXECUTOR_ALIGNOF(struct glmaxx_executor_module_image_v1) == 16,
                              "module image alignment");
GLMAXX_EXECUTOR_STATIC_ASSERT(GLMAXX_EXECUTOR_ALIGNOF(struct glmaxx_executor_module_capability_v1) == 16,
                              "module capability alignment");
GLMAXX_EXECUTOR_STATIC_ASSERT(GLMAXX_EXECUTOR_ALIGNOF(struct glmaxx_executor_arena_desc_v1) == 16,
                              "arena descriptor alignment");
GLMAXX_EXECUTOR_STATIC_ASSERT(GLMAXX_EXECUTOR_ALIGNOF(struct glmaxx_executor_arena_binding_v1) == 16,
                              "arena binding alignment");
GLMAXX_EXECUTOR_STATIC_ASSERT(GLMAXX_EXECUTOR_ALIGNOF(struct glmaxx_executor_span_v1) == 16,
                              "span alignment");
GLMAXX_EXECUTOR_STATIC_ASSERT(GLMAXX_EXECUTOR_ALIGNOF(struct glmaxx_executor_copy_desc_v1) == 16,
                              "copy descriptor alignment");
GLMAXX_EXECUTOR_STATIC_ASSERT(GLMAXX_EXECUTOR_ALIGNOF(struct glmaxx_executor_communicator_desc_v1) == 16,
                              "communicator descriptor alignment");
GLMAXX_EXECUTOR_STATIC_ASSERT(GLMAXX_EXECUTOR_ALIGNOF(struct glmaxx_executor_route_desc_v1) == 16,
                              "route descriptor alignment");
GLMAXX_EXECUTOR_STATIC_ASSERT(GLMAXX_EXECUTOR_ALIGNOF(struct glmaxx_executor_graph_desc_v1) == 16,
                              "graph descriptor alignment");
GLMAXX_EXECUTOR_STATIC_ASSERT(GLMAXX_EXECUTOR_ALIGNOF(struct glmaxx_executor_graph_node_v1) == 16,
                              "graph node alignment");
GLMAXX_EXECUTOR_STATIC_ASSERT(GLMAXX_EXECUTOR_ALIGNOF(struct glmaxx_executor_validation_desc_v1) == 16,
                              "validation descriptor alignment");
GLMAXX_EXECUTOR_STATIC_ASSERT(GLMAXX_EXECUTOR_ALIGNOF(struct glmaxx_executor_launch_desc_v1) == 16,
                              "launch descriptor alignment");
GLMAXX_EXECUTOR_STATIC_ASSERT(GLMAXX_EXECUTOR_ALIGNOF(struct glmaxx_executor_device_status_v1) == 16,
                              "device status alignment");

#undef GLMAXX_EXECUTOR_ALIGN16
#undef GLMAXX_EXECUTOR_ALIGNOF
#undef GLMAXX_EXECUTOR_NOEXCEPT
#undef GLMAXX_EXECUTOR_STATIC_ASSERT

#endif  /* GLMAXX_SM120_RANK_EXECUTOR_NATIVE_ABI_V1_H_ */
