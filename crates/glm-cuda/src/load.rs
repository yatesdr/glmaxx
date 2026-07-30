use crate::KernelError;

/// Thread-affine CUDA operations needed by the checkpoint staging path.
///
/// Implementations must keep every handle bound to the current rank's CUDA
/// context. The engine owns the allocation/ring state machine and never calls
/// this trait from a different thread.
pub trait RankLoadBackend: 'static {
    fn allocate_device(&mut self, bytes: u64) -> Result<u64, KernelError>;
    fn free_device(&mut self, pointer: u64) -> Result<(), KernelError>;

    fn allocate_pinned(&mut self, bytes: u64) -> Result<u64, KernelError>;
    fn free_pinned(&mut self, pointer: u64) -> Result<(), KernelError>;
    fn copy_to_pinned(&mut self, destination: u64, bytes: &[u8]) -> Result<(), KernelError>;
    fn copy_from_pinned(&mut self, source: u64, bytes: &mut [u8]) -> Result<(), KernelError>;

    fn create_stream(&mut self) -> Result<u64, KernelError>;
    fn synchronize_stream(&mut self, stream: u64) -> Result<(), KernelError>;
    fn destroy_stream(&mut self, stream: u64) -> Result<(), KernelError>;

    fn create_event(&mut self) -> Result<u64, KernelError>;
    fn record_event(&mut self, event: u64, stream: u64) -> Result<(), KernelError>;
    fn synchronize_event(&mut self, event: u64) -> Result<(), KernelError>;
    fn destroy_event(&mut self, event: u64) -> Result<(), KernelError>;

    fn memset_zero(&mut self, destination: u64, bytes: u64, stream: u64)
    -> Result<(), KernelError>;
    fn copy_h2d(
        &mut self,
        destination: u64,
        source: u64,
        bytes: u64,
        stream: u64,
    ) -> Result<(), KernelError>;
    fn copy_d2h(
        &mut self,
        destination: u64,
        source: u64,
        bytes: u64,
        stream: u64,
    ) -> Result<(), KernelError>;
}
