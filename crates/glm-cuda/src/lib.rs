//! Safe Rust boundary for the isolated SM120 CUDA kernel library.

mod abi;
mod ownership;

pub use abi::{
    ABI_VERSION, Fc1Descriptor, Fc2Descriptor, GroupedSfaPlan, HIDDEN, KernelError, KernelPath,
    LOCAL_GATE_UP, LOCAL_INTERMEDIATE, LaunchGeometry, SFA_BYTES_PER_PADDED_ROW,
    fc2_workspace_bytes, grouped_sfa_capacity_bytes, grouped_sfa_plan, grouped_workspace_bytes,
    validate_descriptor, validate_fc2_descriptor, workspace_bytes,
};
pub use ownership::{CudaDriver, DeviceBuffer, Fc2LaunchTicket, LaunchTicket};

// Parse and type-check the native boundary on CPU-only development hosts when
// the feature is requested. Only Linux exposes it as a runnable public API.
#[cfg(feature = "cuda-ffi")]
mod ffi;

#[cfg(feature = "cuda-ffi")]
pub use ffi::{
    Fc1BenchmarkConfig, Fc1Timing, GraphReplay, GroupedFc1Timing, NativeFc1Fixture,
    NativeFc2Fixture, NativeKernelDriver, run_single_expert, validate_native_abi,
    validate_native_moe_abi,
};
