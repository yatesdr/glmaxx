//! Safe Rust boundary for the isolated SM120 CUDA kernel library.

mod abi;
mod ownership;

pub use abi::{
    ABI_VERSION, Fc1Descriptor, HIDDEN, KernelError, KernelPath, LOCAL_GATE_UP, LOCAL_INTERMEDIATE,
    LaunchGeometry, validate_descriptor, workspace_bytes,
};
pub use ownership::{CudaDriver, DeviceBuffer, LaunchTicket};

// Parse and type-check the native boundary on CPU-only development hosts when
// the feature is requested. Only Linux exposes it as a runnable public API.
#[cfg(feature = "cuda-ffi")]
mod ffi;

#[cfg(feature = "cuda-ffi")]
pub use ffi::{NativeFc1Fixture, NativeKernelDriver, run_single_expert};
