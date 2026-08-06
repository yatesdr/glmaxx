//! Safe Rust boundary for the isolated SM120 CUDA kernel library.

mod abi;
mod load;
mod ownership;

pub use abi::{
    ABI_VERSION, EXL3_ABI_VERSION, EXL3_BITS, EXL3_KERNEL_ABI, EXL3_MAX_ROWS, Exl3Descriptor,
    Exl3KernelProjection, Fc1Descriptor, Fc2Descriptor, GroupedSfaPlan, HIDDEN, KernelError,
    KernelPath, LOCAL_GATE_UP, LOCAL_INTERMEDIATE, LaunchGeometry, SFA_BYTES_PER_PADDED_ROW, TOP_K,
    exl3_trellis_bytes, exl3_workspace_bytes, fc2_grouped_sfa_capacity_bytes,
    fc2_grouped_workspace_bytes, fc2_workspace_bytes, grouped_sfa_capacity_bytes, grouped_sfa_plan,
    grouped_workspace_bytes, validate_descriptor, validate_exl3_descriptor,
    validate_fc2_descriptor, workspace_bytes,
};
pub use load::RankLoadBackend;
pub use ownership::{CudaDriver, DeviceBuffer, Exl3LaunchTicket, Fc2LaunchTicket, LaunchTicket};

// Parse and type-check the native boundary on CPU-only development hosts when
// the feature is requested. Only Linux exposes it as a runnable public API.
#[cfg(feature = "cuda-ffi")]
mod ffi;

#[cfg(feature = "cuda-ffi")]
pub use ffi::{
    Exl3Replay, Exl3Timing, Fc1BenchmarkConfig, Fc1ProfilePhase, Fc1Timing, Fc2ProfilePhase,
    Fc2Timing, GraphReplay, GroupedFc1Timing, NativeDeviceIdentity, NativeExecutionBuffer,
    NativeExl3Fixture, NativeFc1Fixture, NativeFc2Fixture, NativeKernelDriver, NativeRankContext,
    NativeRankLoadBackend, NativeResidentExl3Projection, NativeResidentExl3Workspace,
    native_checkpoint_codec_capability_sha256, run_single_expert, validate_native_abi,
    validate_native_exl3_abi, validate_native_moe_abi,
};
