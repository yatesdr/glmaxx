use std::{collections::BTreeSet, fmt, marker::PhantomData, rc::Rc};

use glm_cuda::{KernelError, RankLoadBackend};
use sha2::{Digest, Sha256};

use crate::{
    LoadPlanError, QuarantinedArenaWriter, RANK_SET_SIZE, READER_CHUNK_BYTES,
    WeightArenaExecutionPermit,
};

const DEVICE_ALIGNMENT: u64 = 256;
const PINNED_ALIGNMENT: u64 = 64;
const READBACK_EVIDENCE_DOMAIN: &[u8] = b"glmaxx.cuda-arena-readback.v1\0";
const READBACK_MISMATCH: u32 = 0x4c4f_4144;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CudaArenaVerificationEvidence {
    rank: u8,
    plan_sha256: [u8; 32],
    owner_allocation_generation: u64,
    weight_bytes: u64,
    metadata_bytes: u64,
    readback_chunk_bytes: u32,
    readback_chunks: u64,
    expected_weight_sha256: [u8; 32],
    observed_weight_sha256: [u8; 32],
    expected_metadata_sha256: [u8; 32],
    observed_metadata_sha256: [u8; 32],
}

impl CudaArenaVerificationEvidence {
    #[must_use]
    pub const fn rank(self) -> u8 {
        self.rank
    }

    #[must_use]
    pub const fn plan_sha256(self) -> [u8; 32] {
        self.plan_sha256
    }

    #[must_use]
    pub const fn owner_allocation_generation(self) -> u64 {
        self.owner_allocation_generation
    }

    #[must_use]
    pub const fn weight_bytes(self) -> u64 {
        self.weight_bytes
    }

    #[must_use]
    pub const fn metadata_bytes(self) -> u64 {
        self.metadata_bytes
    }

    #[must_use]
    pub const fn readback_chunk_bytes(self) -> u32 {
        self.readback_chunk_bytes
    }

    #[must_use]
    pub const fn readback_chunks(self) -> u64 {
        self.readback_chunks
    }

    #[must_use]
    pub const fn expected_weight_sha256(self) -> [u8; 32] {
        self.expected_weight_sha256
    }

    #[must_use]
    pub const fn observed_weight_sha256(self) -> [u8; 32] {
        self.observed_weight_sha256
    }

    #[must_use]
    pub const fn expected_metadata_sha256(self) -> [u8; 32] {
        self.expected_metadata_sha256
    }

    #[must_use]
    pub const fn observed_metadata_sha256(self) -> [u8; 32] {
        self.observed_metadata_sha256
    }

    #[must_use]
    pub fn evidence_sha256(self) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(READBACK_EVIDENCE_DOMAIN);
        hasher.update([self.rank]);
        hasher.update([0; 7]);
        hasher.update(self.plan_sha256);
        hasher.update(self.owner_allocation_generation.to_le_bytes());
        hasher.update(self.weight_bytes.to_le_bytes());
        hasher.update(self.metadata_bytes.to_le_bytes());
        hasher.update(self.readback_chunk_bytes.to_le_bytes());
        hasher.update([0; 4]);
        hasher.update(self.readback_chunks.to_le_bytes());
        hasher.update(self.expected_weight_sha256);
        hasher.update(self.observed_weight_sha256);
        hasher.update(self.expected_metadata_sha256);
        hasher.update(self.observed_metadata_sha256);
        hasher.finalize().into()
    }
}

struct PinnedSlot {
    host_pointer: Option<u64>,
    event: Option<u64>,
    in_flight: bool,
}

struct CudaArenaResources<B: RankLoadBackend> {
    backend: B,
    stream: Option<u64>,
    weight_pointer: Option<u64>,
    metadata_pointer: Option<u64>,
    slots: Vec<PinnedSlot>,
    released: bool,
    _thread_affine: PhantomData<Rc<()>>,
}

impl<B: RankLoadBackend> CudaArenaResources<B> {
    fn new(backend: B) -> Self {
        Self {
            backend,
            stream: None,
            weight_pointer: None,
            metadata_pointer: None,
            slots: Vec::new(),
            released: false,
            _thread_affine: PhantomData,
        }
    }

    fn cleanup(&mut self) -> Result<(), KernelError> {
        if self.released {
            return Ok(());
        }
        if let Some(stream) = self.stream
            && let Err(error) = self.backend.synchronize_stream(stream)
        {
            // CUDA did not prove that borrowed pinned memory is no longer
            // referenced by DMA. Leak every native resource and require
            // process termination instead of risking use-after-free.
            self.stream = None;
            self.weight_pointer = None;
            self.metadata_pointer = None;
            self.slots.clear();
            self.released = true;
            return Err(error);
        }

        let mut first_error = None;
        for mut slot in std::mem::take(&mut self.slots) {
            if let Some(event) = slot.event.take() {
                retain_first(&mut first_error, self.backend.destroy_event(event));
            }
            if let Some(pointer) = slot.host_pointer.take() {
                retain_first(&mut first_error, self.backend.free_pinned(pointer));
            }
        }
        if let Some(pointer) = self.weight_pointer.take() {
            retain_first(&mut first_error, self.backend.free_device(pointer));
        }
        if let Some(pointer) = self.metadata_pointer.take() {
            retain_first(&mut first_error, self.backend.free_device(pointer));
        }
        if let Some(stream) = self.stream.take() {
            retain_first(&mut first_error, self.backend.destroy_stream(stream));
        }
        self.released = true;
        first_error.map_or(Ok(()), Err)
    }
}

impl<B: RankLoadBackend> Drop for CudaArenaResources<B> {
    fn drop(&mut self) {
        if self.cleanup().is_err() {
            // An implicit teardown has no caller that can elevate the CUDA
            // error to the process-wide fatal route. Continuing could either
            // hide a native resource failure or free memory still referenced
            // by DMA, so this path is deliberately unrecoverable.
            std::process::abort();
        }
    }
}

fn retain_first(first: &mut Option<KernelError>, result: Result<(), KernelError>) {
    if let Err(error) = result {
        first.get_or_insert(error);
    }
}

/// Rank-thread-owned checkpoint arenas and fixed pinned staging ring.
///
/// The object exposes no device pointer. It can become executor-visible only
/// by consuming the matching global-adoption execution permit.
pub struct CudaQuarantinedArena<B: RankLoadBackend> {
    rank: u8,
    plan_sha256: [u8; 32],
    owner_allocation_generation: u64,
    weight_bytes: u64,
    metadata_bytes: u64,
    slot_bytes: u64,
    next_slot: usize,
    sealed: bool,
    content_verified: bool,
    poisoned: Option<KernelError>,
    weight_cursor: u64,
    metadata_cursor: u64,
    weight_hasher: Sha256,
    metadata_hasher: Sha256,
    expected_weight_sha256: Option<[u8; 32]>,
    expected_metadata_sha256: Option<[u8; 32]>,
    resources: Option<CudaArenaResources<B>>,
}

impl<B: RankLoadBackend> CudaQuarantinedArena<B> {
    #[allow(clippy::too_many_arguments)]
    pub fn allocate(
        backend: B,
        rank: u8,
        plan_sha256: [u8; 32],
        owner_allocation_generation: u64,
        weight_bytes: u64,
        metadata_bytes: u64,
        slot_bytes: u32,
        slot_count: u16,
    ) -> Result<Self, KernelError> {
        if usize::from(rank) >= RANK_SET_SIZE
            || plan_sha256 == [0; 32]
            || owner_allocation_generation == 0
            || weight_bytes == 0
            || metadata_bytes == 0
            || slot_bytes < READER_CHUNK_BYTES
            || slot_count < 2
        {
            return Err(KernelError::Shape);
        }
        u64::from(slot_bytes)
            .checked_mul(u64::from(slot_count))
            .ok_or(KernelError::Overflow)?;

        let mut resources = CudaArenaResources::new(backend);
        let stream = resources.backend.create_stream()?;
        require_handle(stream, 1)?;
        resources.stream = Some(stream);

        let weight_pointer = resources.backend.allocate_device(weight_bytes)?;
        resources.weight_pointer = Some(weight_pointer);
        require_handle(weight_pointer, DEVICE_ALIGNMENT)?;
        let metadata_pointer = resources.backend.allocate_device(metadata_bytes)?;
        resources.metadata_pointer = Some(metadata_pointer);
        require_handle(metadata_pointer, DEVICE_ALIGNMENT)?;

        for _ in 0..slot_count {
            let host_pointer = resources.backend.allocate_pinned(u64::from(slot_bytes))?;
            let mut slot = PinnedSlot {
                host_pointer: Some(host_pointer),
                event: None,
                in_flight: false,
            };
            resources.slots.push(slot);
            require_handle(host_pointer, PINNED_ALIGNMENT)?;
            let event = resources.backend.create_event()?;
            require_handle(event, 1)?;
            slot = resources.slots.pop().expect("slot just pushed");
            slot.event = Some(event);
            resources.slots.push(slot);
        }
        require_unique(resources.slots.iter().filter_map(|slot| slot.host_pointer))?;
        require_unique(resources.slots.iter().filter_map(|slot| slot.event))?;

        resources
            .backend
            .memset_zero(weight_pointer, weight_bytes, stream)?;
        resources
            .backend
            .memset_zero(metadata_pointer, metadata_bytes, stream)?;

        Ok(Self {
            rank,
            plan_sha256,
            owner_allocation_generation,
            weight_bytes,
            metadata_bytes,
            slot_bytes: u64::from(slot_bytes),
            next_slot: 0,
            sealed: false,
            content_verified: false,
            poisoned: None,
            weight_cursor: 0,
            metadata_cursor: 0,
            weight_hasher: Sha256::new(),
            metadata_hasher: Sha256::new(),
            expected_weight_sha256: None,
            expected_metadata_sha256: None,
            resources: Some(resources),
        })
    }

    pub fn adopt(
        mut self,
        permit: WeightArenaExecutionPermit,
    ) -> Result<CudaWeightArena<B>, LoadPlanError> {
        if !self.sealed
            || !self.content_verified
            || self.poisoned.is_some()
            || permit.rank() != self.rank
            || permit.plan_sha256() != self.plan_sha256
            || permit.owner_allocation_generation() != self.owner_allocation_generation
        {
            return Err(LoadPlanError::Adoption);
        }
        let resources = self.resources.take().ok_or(LoadPlanError::Adoption)?;
        Ok(CudaWeightArena {
            permit,
            weight_bytes: self.weight_bytes,
            metadata_bytes: self.metadata_bytes,
            resources,
        })
    }

    pub fn abort_and_release(mut self) -> Result<(), KernelError> {
        let result = self
            .resources
            .as_mut()
            .map(CudaArenaResources::cleanup)
            .unwrap_or(Ok(()));
        self.resources.take();
        result
    }

    pub fn verify_device_contents(&mut self) -> Result<CudaArenaVerificationEvidence, KernelError> {
        if !self.sealed || self.content_verified || self.poisoned.is_some() {
            return Err(self.poisoned.unwrap_or(KernelError::Async(-1)));
        }
        let expected_weight_sha256 = self.expected_weight_sha256.ok_or(KernelError::Async(-1))?;
        let expected_metadata_sha256 = self
            .expected_metadata_sha256
            .ok_or(KernelError::Async(-1))?;
        let resources = self.resources.as_mut().ok_or(KernelError::Async(-1))?;
        let weight_pointer = resources.weight_pointer.ok_or(KernelError::Null)?;
        let metadata_pointer = resources.metadata_pointer.ok_or(KernelError::Null)?;
        let readback = (|| {
            let (observed_weight_sha256, weight_chunks) = readback_sha256(
                resources,
                weight_pointer,
                self.weight_bytes,
                READER_CHUNK_BYTES,
            )?;
            let (observed_metadata_sha256, metadata_chunks) = readback_sha256(
                resources,
                metadata_pointer,
                self.metadata_bytes,
                READER_CHUNK_BYTES,
            )?;
            Ok::<_, KernelError>((
                observed_weight_sha256,
                weight_chunks,
                observed_metadata_sha256,
                metadata_chunks,
            ))
        })();
        let (observed_weight_sha256, weight_chunks, observed_metadata_sha256, metadata_chunks) =
            match readback {
                Ok(result) => result,
                Err(error) => return self.poison(error),
            };
        if observed_weight_sha256 != expected_weight_sha256
            || observed_metadata_sha256 != expected_metadata_sha256
        {
            return self.poison(KernelError::DeviceValidation(READBACK_MISMATCH));
        }
        let evidence = CudaArenaVerificationEvidence {
            rank: self.rank,
            plan_sha256: self.plan_sha256,
            owner_allocation_generation: self.owner_allocation_generation,
            weight_bytes: self.weight_bytes,
            metadata_bytes: self.metadata_bytes,
            readback_chunk_bytes: READER_CHUNK_BYTES,
            readback_chunks: weight_chunks
                .checked_add(metadata_chunks)
                .ok_or(KernelError::Overflow)?,
            expected_weight_sha256,
            observed_weight_sha256,
            expected_metadata_sha256,
            observed_metadata_sha256,
        };
        self.content_verified = true;
        Ok(evidence)
    }

    fn write_plane(
        &mut self,
        metadata: bool,
        offset: u64,
        bytes: &[u8],
    ) -> Result<(), KernelError> {
        if self.sealed || self.poisoned.is_some() {
            return Err(KernelError::Async(-1));
        }
        if bytes.is_empty() {
            return Ok(());
        }
        let byte_count = u64::try_from(bytes.len()).map_err(|_| KernelError::Overflow)?;
        if byte_count > self.slot_bytes {
            return self.poison(KernelError::Shape);
        }
        let capacity = if metadata {
            self.metadata_bytes
        } else {
            self.weight_bytes
        };
        let cursor = if metadata {
            self.metadata_cursor
        } else {
            self.weight_cursor
        };
        if offset < cursor {
            return self.poison(KernelError::Shape);
        }
        let Some(end) = offset.checked_add(byte_count) else {
            return self.poison(KernelError::Overflow);
        };
        if end > capacity {
            return self.poison(KernelError::Shape);
        }

        let resources = self.resources.as_mut().ok_or(KernelError::Async(-1))?;
        let stream = resources.stream.ok_or(KernelError::Async(-1))?;
        let slot_index = self.next_slot;
        let (host_pointer, event, in_flight) = {
            let slot = resources.slots.get(slot_index).ok_or(KernelError::Shape)?;
            (
                slot.host_pointer.ok_or(KernelError::Null)?,
                slot.event.ok_or(KernelError::Null)?,
                slot.in_flight,
            )
        };
        if in_flight {
            if let Err(error) = resources.backend.synchronize_event(event) {
                return self.poison(error);
            }
            resources.slots[slot_index].in_flight = false;
        }
        if let Err(error) = resources.backend.copy_to_pinned(host_pointer, bytes) {
            return self.poison(error);
        }
        let base = if metadata {
            resources.metadata_pointer
        } else {
            resources.weight_pointer
        }
        .ok_or(KernelError::Null)?;
        let Some(destination) = base.checked_add(offset) else {
            return self.poison(KernelError::Overflow);
        };
        if let Err(error) =
            resources
                .backend
                .copy_h2d(destination, host_pointer, byte_count, stream)
        {
            return self.poison(error);
        }
        resources.slots[slot_index].in_flight = true;
        if let Err(error) = resources.backend.record_event(event, stream) {
            return self.poison(error);
        }
        let hasher = if metadata {
            &mut self.metadata_hasher
        } else {
            &mut self.weight_hasher
        };
        hash_zeros(hasher, offset - cursor);
        hasher.update(bytes);
        if metadata {
            self.metadata_cursor = end;
        } else {
            self.weight_cursor = end;
        }
        self.next_slot = (slot_index + 1) % resources.slots.len();
        Ok(())
    }

    fn poison<T>(&mut self, error: KernelError) -> Result<T, KernelError> {
        self.poisoned.get_or_insert(error);
        Err(error)
    }
}

impl<B: RankLoadBackend> QuarantinedArenaWriter for CudaQuarantinedArena<B> {
    type Error = KernelError;

    fn weight_capacity(&self) -> u64 {
        self.weight_bytes
    }

    fn metadata_capacity(&self) -> u64 {
        self.metadata_bytes
    }

    fn write_weight(&mut self, offset: u64, bytes: &[u8]) -> Result<(), Self::Error> {
        self.write_plane(false, offset, bytes)
    }

    fn write_metadata(&mut self, offset: u64, bytes: &[u8]) -> Result<(), Self::Error> {
        self.write_plane(true, offset, bytes)
    }

    fn drain_and_seal(&mut self) -> Result<(), Self::Error> {
        if self.sealed || self.poisoned.is_some() {
            return Err(self.poisoned.unwrap_or(KernelError::Async(-1)));
        }
        let resources = self.resources.as_mut().ok_or(KernelError::Async(-1))?;
        for slot in &mut resources.slots {
            if slot.in_flight {
                let event = slot.event.ok_or(KernelError::Null)?;
                if let Err(error) = resources.backend.synchronize_event(event) {
                    return self.poison(error);
                }
                slot.in_flight = false;
            }
        }
        let stream = resources.stream.ok_or(KernelError::Null)?;
        if let Err(error) = resources.backend.synchronize_stream(stream) {
            return self.poison(error);
        }
        hash_zeros(
            &mut self.weight_hasher,
            self.weight_bytes - self.weight_cursor,
        );
        hash_zeros(
            &mut self.metadata_hasher,
            self.metadata_bytes - self.metadata_cursor,
        );
        self.weight_cursor = self.weight_bytes;
        self.metadata_cursor = self.metadata_bytes;
        self.expected_weight_sha256 = Some(self.weight_hasher.clone().finalize().into());
        self.expected_metadata_sha256 = Some(self.metadata_hasher.clone().finalize().into());
        self.sealed = true;
        Ok(())
    }
}

/// Globally adopted immutable device weight arenas.
///
/// This is the first type in the load path that exposes device pointers.
pub struct CudaWeightArena<B: RankLoadBackend> {
    permit: WeightArenaExecutionPermit,
    weight_bytes: u64,
    metadata_bytes: u64,
    resources: CudaArenaResources<B>,
}

impl<B: RankLoadBackend> CudaWeightArena<B> {
    #[must_use]
    pub fn rank(&self) -> u8 {
        self.permit.rank()
    }

    #[must_use]
    pub fn weight_pointer(&self) -> u64 {
        self.resources
            .weight_pointer
            .expect("adopted arena retains weight allocation")
    }

    #[must_use]
    pub fn metadata_pointer(&self) -> u64 {
        self.resources
            .metadata_pointer
            .expect("adopted arena retains metadata allocation")
    }

    #[must_use]
    pub const fn weight_bytes(&self) -> u64 {
        self.weight_bytes
    }

    #[must_use]
    pub const fn metadata_bytes(&self) -> u64 {
        self.metadata_bytes
    }

    pub fn shutdown(mut self) -> Result<(), KernelError> {
        self.resources.cleanup()
    }
}

fn require_handle(handle: u64, alignment: u64) -> Result<(), KernelError> {
    if handle == 0 || !handle.is_multiple_of(alignment) {
        Err(KernelError::Alignment)
    } else {
        Ok(())
    }
}

fn require_unique(handles: impl Iterator<Item = u64>) -> Result<(), KernelError> {
    let mut observed = BTreeSet::new();
    for handle in handles {
        if !observed.insert(handle) {
            return Err(KernelError::Null);
        }
    }
    Ok(())
}

fn hash_zeros(hasher: &mut Sha256, mut bytes: u64) {
    const ZEROES: [u8; 8192] = [0; 8192];
    while bytes > 0 {
        let chunk = usize::try_from(bytes.min(ZEROES.len() as u64))
            .expect("bounded zero-hash chunk fits usize");
        hasher.update(&ZEROES[..chunk]);
        bytes -= chunk as u64;
    }
}

fn readback_sha256<B: RankLoadBackend>(
    resources: &mut CudaArenaResources<B>,
    base: u64,
    bytes: u64,
    chunk_bytes: u32,
) -> Result<([u8; 32], u64), KernelError> {
    if bytes == 0 || chunk_bytes == 0 {
        return Err(KernelError::Shape);
    }
    let stream = resources.stream.ok_or(KernelError::Null)?;
    let slot = resources.slots.first_mut().ok_or(KernelError::Shape)?;
    if slot.in_flight {
        return Err(KernelError::Async(-1));
    }
    let host_pointer = slot.host_pointer.ok_or(KernelError::Null)?;
    let event = slot.event.ok_or(KernelError::Null)?;
    let buffer_len = usize::try_from(chunk_bytes).map_err(|_| KernelError::Overflow)?;
    let mut buffer = vec![0_u8; buffer_len];
    let mut hasher = Sha256::new();
    let mut offset = 0_u64;
    let mut chunks = 0_u64;
    while offset < bytes {
        let count = (bytes - offset).min(u64::from(chunk_bytes));
        let count_usize = usize::try_from(count).map_err(|_| KernelError::Overflow)?;
        let source = base.checked_add(offset).ok_or(KernelError::Overflow)?;
        resources
            .backend
            .copy_d2h(host_pointer, source, count, stream)?;
        slot.in_flight = true;
        resources.backend.record_event(event, stream)?;
        resources.backend.synchronize_event(event)?;
        slot.in_flight = false;
        resources
            .backend
            .copy_from_pinned(host_pointer, &mut buffer[..count_usize])?;
        hasher.update(&buffer[..count_usize]);
        offset = offset.checked_add(count).ok_or(KernelError::Overflow)?;
        chunks = chunks.checked_add(1).ok_or(KernelError::Overflow)?;
    }
    Ok((hasher.finalize().into(), chunks))
}

impl<B: RankLoadBackend> fmt::Debug for CudaQuarantinedArena<B> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CudaQuarantinedArena")
            .field("rank", &self.rank)
            .field("weight_bytes", &self.weight_bytes)
            .field("metadata_bytes", &self.metadata_bytes)
            .field("slot_bytes", &self.slot_bytes)
            .field(
                "slot_count",
                &self.resources.as_ref().map(|r| r.slots.len()),
            )
            .field("sealed", &self.sealed)
            .field("poisoned", &self.poisoned)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::*;

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum Operation {
        DeviceAllocate(u64, u64),
        DeviceFree(u64),
        PinnedAllocate(u64, u64),
        PinnedFree(u64),
        HostCopy(u64, u64),
        StreamCreate(u64),
        StreamSynchronize(u64),
        StreamDestroy(u64),
        EventCreate(u64),
        EventRecord(u64),
        EventSynchronize(u64),
        EventDestroy(u64),
        Memset(u64, u64),
        H2d(u64, u64, u64),
        D2h(u64, u64, u64),
    }

    struct FakeState {
        next_device: u64,
        next_pinned: u64,
        next_stream: u64,
        next_event: u64,
        operations: Vec<Operation>,
        device_allocations: std::collections::BTreeMap<u64, Vec<u8>>,
        pinned_allocations: std::collections::BTreeMap<u64, Vec<u8>>,
        fail_stream_synchronize: bool,
        fail_event_record: bool,
    }

    impl Default for FakeState {
        fn default() -> Self {
            Self {
                next_device: 0x10_0000,
                next_pinned: 0x1000,
                next_stream: 0x10,
                next_event: 0x20,
                operations: Vec::new(),
                device_allocations: std::collections::BTreeMap::new(),
                pinned_allocations: std::collections::BTreeMap::new(),
                fail_stream_synchronize: false,
                fail_event_record: false,
            }
        }
    }

    #[derive(Clone)]
    struct FakeBackend {
        state: Arc<Mutex<FakeState>>,
    }

    impl FakeBackend {
        fn new() -> (Self, Arc<Mutex<FakeState>>) {
            let state = Arc::new(Mutex::new(FakeState::default()));
            (
                Self {
                    state: state.clone(),
                },
                state,
            )
        }
    }

    impl RankLoadBackend for FakeBackend {
        fn allocate_device(&mut self, bytes: u64) -> Result<u64, KernelError> {
            let mut state = self.state.lock().unwrap();
            let pointer = state.next_device;
            state.next_device += 0x10_0000;
            let length = usize::try_from(bytes).map_err(|_| KernelError::Overflow)?;
            state.device_allocations.insert(pointer, vec![0xa5; length]);
            state
                .operations
                .push(Operation::DeviceAllocate(pointer, bytes));
            Ok(pointer)
        }

        fn free_device(&mut self, pointer: u64) -> Result<(), KernelError> {
            let mut state = self.state.lock().unwrap();
            if state.device_allocations.remove(&pointer).is_none() {
                return Err(KernelError::Null);
            }
            state.operations.push(Operation::DeviceFree(pointer));
            Ok(())
        }

        fn allocate_pinned(&mut self, bytes: u64) -> Result<u64, KernelError> {
            let mut state = self.state.lock().unwrap();
            let pointer = state.next_pinned;
            state.next_pinned += 0x10_0000;
            let length = usize::try_from(bytes).map_err(|_| KernelError::Overflow)?;
            state.pinned_allocations.insert(pointer, vec![0; length]);
            state
                .operations
                .push(Operation::PinnedAllocate(pointer, bytes));
            Ok(pointer)
        }

        fn free_pinned(&mut self, pointer: u64) -> Result<(), KernelError> {
            let mut state = self.state.lock().unwrap();
            if state.pinned_allocations.remove(&pointer).is_none() {
                return Err(KernelError::Null);
            }
            state.operations.push(Operation::PinnedFree(pointer));
            Ok(())
        }

        fn copy_to_pinned(&mut self, destination: u64, bytes: &[u8]) -> Result<(), KernelError> {
            let mut state = self.state.lock().unwrap();
            let allocation = state
                .pinned_allocations
                .get_mut(&destination)
                .ok_or(KernelError::Null)?;
            let target = allocation
                .get_mut(..bytes.len())
                .ok_or(KernelError::Shape)?;
            target.copy_from_slice(bytes);
            state.operations.push(Operation::HostCopy(
                destination,
                u64::try_from(bytes.len()).map_err(|_| KernelError::Overflow)?,
            ));
            Ok(())
        }

        fn copy_from_pinned(&mut self, source: u64, bytes: &mut [u8]) -> Result<(), KernelError> {
            let state = self.state.lock().unwrap();
            let allocation = state
                .pinned_allocations
                .get(&source)
                .ok_or(KernelError::Null)?;
            let source = allocation.get(..bytes.len()).ok_or(KernelError::Shape)?;
            bytes.copy_from_slice(source);
            Ok(())
        }

        fn create_stream(&mut self) -> Result<u64, KernelError> {
            let mut state = self.state.lock().unwrap();
            let stream = state.next_stream;
            state.next_stream += 1;
            state.operations.push(Operation::StreamCreate(stream));
            Ok(stream)
        }

        fn synchronize_stream(&mut self, stream: u64) -> Result<(), KernelError> {
            let mut state = self.state.lock().unwrap();
            state.operations.push(Operation::StreamSynchronize(stream));
            if state.fail_stream_synchronize {
                Err(KernelError::Async(71))
            } else {
                Ok(())
            }
        }

        fn destroy_stream(&mut self, stream: u64) -> Result<(), KernelError> {
            self.state
                .lock()
                .unwrap()
                .operations
                .push(Operation::StreamDestroy(stream));
            Ok(())
        }

        fn create_event(&mut self) -> Result<u64, KernelError> {
            let mut state = self.state.lock().unwrap();
            let event = state.next_event;
            state.next_event += 1;
            state.operations.push(Operation::EventCreate(event));
            Ok(event)
        }

        fn record_event(&mut self, event: u64, _stream: u64) -> Result<(), KernelError> {
            let mut state = self.state.lock().unwrap();
            state.operations.push(Operation::EventRecord(event));
            if state.fail_event_record {
                Err(KernelError::Driver(72))
            } else {
                Ok(())
            }
        }

        fn synchronize_event(&mut self, event: u64) -> Result<(), KernelError> {
            self.state
                .lock()
                .unwrap()
                .operations
                .push(Operation::EventSynchronize(event));
            Ok(())
        }

        fn destroy_event(&mut self, event: u64) -> Result<(), KernelError> {
            self.state
                .lock()
                .unwrap()
                .operations
                .push(Operation::EventDestroy(event));
            Ok(())
        }

        fn memset_zero(
            &mut self,
            destination: u64,
            bytes: u64,
            _stream: u64,
        ) -> Result<(), KernelError> {
            let mut state = self.state.lock().unwrap();
            let (base, offset) = locate_allocation(&state.device_allocations, destination, bytes)?;
            let start = usize::try_from(offset).map_err(|_| KernelError::Overflow)?;
            let end = start
                .checked_add(usize::try_from(bytes).map_err(|_| KernelError::Overflow)?)
                .ok_or(KernelError::Overflow)?;
            state
                .device_allocations
                .get_mut(&base)
                .ok_or(KernelError::Null)?[start..end]
                .fill(0);
            state.operations.push(Operation::Memset(destination, bytes));
            Ok(())
        }

        fn copy_h2d(
            &mut self,
            destination: u64,
            source: u64,
            bytes: u64,
            _stream: u64,
        ) -> Result<(), KernelError> {
            let mut state = self.state.lock().unwrap();
            let (base, offset) = locate_allocation(&state.device_allocations, destination, bytes)?;
            let count = usize::try_from(bytes).map_err(|_| KernelError::Overflow)?;
            let source_bytes = state
                .pinned_allocations
                .get(&source)
                .and_then(|allocation| allocation.get(..count))
                .ok_or(KernelError::Shape)?
                .to_vec();
            let start = usize::try_from(offset).map_err(|_| KernelError::Overflow)?;
            let end = start.checked_add(count).ok_or(KernelError::Overflow)?;
            state
                .device_allocations
                .get_mut(&base)
                .ok_or(KernelError::Null)?[start..end]
                .copy_from_slice(&source_bytes);
            state
                .operations
                .push(Operation::H2d(destination, source, bytes));
            Ok(())
        }

        fn copy_d2h(
            &mut self,
            destination: u64,
            source: u64,
            bytes: u64,
            _stream: u64,
        ) -> Result<(), KernelError> {
            let mut state = self.state.lock().unwrap();
            let (base, offset) = locate_allocation(&state.device_allocations, source, bytes)?;
            let count = usize::try_from(bytes).map_err(|_| KernelError::Overflow)?;
            let start = usize::try_from(offset).map_err(|_| KernelError::Overflow)?;
            let end = start.checked_add(count).ok_or(KernelError::Overflow)?;
            let source_bytes = state
                .device_allocations
                .get(&base)
                .ok_or(KernelError::Null)?[start..end]
                .to_vec();
            state
                .pinned_allocations
                .get_mut(&destination)
                .and_then(|allocation| allocation.get_mut(..count))
                .ok_or(KernelError::Shape)?
                .copy_from_slice(&source_bytes);
            state
                .operations
                .push(Operation::D2h(destination, source, bytes));
            Ok(())
        }
    }

    fn locate_allocation(
        allocations: &std::collections::BTreeMap<u64, Vec<u8>>,
        pointer: u64,
        bytes: u64,
    ) -> Result<(u64, u64), KernelError> {
        allocations
            .iter()
            .find_map(|(&base, allocation)| {
                let offset = pointer.checked_sub(base)?;
                let end = offset.checked_add(bytes)?;
                (end <= allocation.len() as u64).then_some((base, offset))
            })
            .ok_or(KernelError::Shape)
    }

    fn arena() -> (CudaQuarantinedArena<FakeBackend>, Arc<Mutex<FakeState>>) {
        let (backend, state) = FakeBackend::new();
        let arena = CudaQuarantinedArena::allocate(
            backend,
            2,
            [9; 32],
            17,
            1024,
            256,
            READER_CHUNK_BYTES,
            2,
        )
        .unwrap();
        (arena, state)
    }

    #[test]
    fn fixed_ring_waits_before_reuse_and_only_adopted_type_exposes_pointers() {
        let (mut arena, state) = arena();
        arena.write_weight(0, &[1; 16]).unwrap();
        arena.write_metadata(0, &[2; 8]).unwrap();
        arena.write_weight(16, &[3; 4]).unwrap();
        let operations = state.lock().unwrap().operations.clone();
        let first_event_sync = operations
            .iter()
            .position(|operation| *operation == Operation::EventSynchronize(0x20))
            .unwrap();
        let third_host_copy = operations
            .iter()
            .rposition(|operation| matches!(operation, Operation::HostCopy(_, 4)))
            .unwrap();
        assert!(first_event_sync < third_host_copy);

        arena.drain_and_seal().unwrap();
        let evidence = arena.verify_device_contents().unwrap();
        assert_eq!(
            evidence.expected_weight_sha256,
            evidence.observed_weight_sha256
        );
        assert_eq!(
            evidence.expected_metadata_sha256,
            evidence.observed_metadata_sha256
        );
        assert_eq!(evidence.readback_chunks, 2);
        assert_eq!(
            evidence.evidence_sha256(),
            [
                0xe8, 0x63, 0x57, 0x80, 0x68, 0xc1, 0xfd, 0x64, 0xbf, 0xde, 0x6a, 0xb1, 0x16, 0x82,
                0xe4, 0xd4, 0x77, 0x0f, 0xd1, 0xd1, 0xf5, 0xae, 0xbf, 0xf7, 0x12, 0xfb, 0xe2, 0x32,
                0xe6, 0x6d, 0xec, 0x87,
            ]
        );
        assert_eq!(arena.write_weight(20, &[]), Err(KernelError::Async(-1)));
        let permit = WeightArenaExecutionPermit::test_only(2, [9; 32], 17);
        let adopted = arena.adopt(permit).unwrap();
        assert_eq!(adopted.rank(), 2);
        assert_eq!(adopted.weight_pointer(), 0x10_0000);
        assert_eq!(adopted.metadata_pointer(), 0x20_0000);
        assert_eq!(adopted.weight_bytes(), 1024);
        assert_eq!(adopted.metadata_bytes(), 256);
        adopted.shutdown().unwrap();

        let operations = &state.lock().unwrap().operations;
        let cleanup_sync = operations
            .iter()
            .rposition(|operation| matches!(operation, Operation::StreamSynchronize(_)))
            .unwrap();
        let first_free = operations
            .iter()
            .position(|operation| {
                matches!(
                    operation,
                    Operation::PinnedFree(_) | Operation::DeviceFree(_)
                )
            })
            .unwrap();
        assert!(cleanup_sync < first_free);
        assert_eq!(
            operations
                .iter()
                .filter(|operation| matches!(operation, Operation::PinnedFree(_)))
                .count(),
            2
        );
        assert_eq!(
            operations
                .iter()
                .filter(|operation| matches!(operation, Operation::DeviceFree(_)))
                .count(),
            2
        );
    }

    #[test]
    fn asynchronous_failure_poisons_sealing_but_safe_abort_synchronizes_then_frees() {
        let (mut arena, state) = arena();
        state.lock().unwrap().fail_event_record = true;
        assert_eq!(arena.write_weight(0, &[1; 4]), Err(KernelError::Driver(72)));
        assert_eq!(arena.drain_and_seal(), Err(KernelError::Driver(72)));
        arena.abort_and_release().unwrap();

        let operations = &state.lock().unwrap().operations;
        let cleanup_sync = operations
            .iter()
            .rposition(|operation| matches!(operation, Operation::StreamSynchronize(_)))
            .unwrap();
        let first_free = operations
            .iter()
            .position(|operation| {
                matches!(
                    operation,
                    Operation::PinnedFree(_) | Operation::DeviceFree(_)
                )
            })
            .unwrap();
        assert!(cleanup_sync < first_free);
    }

    #[test]
    fn failed_cleanup_synchronization_leaks_dma_resources_instead_of_freeing_them() {
        let (mut arena, state) = arena();
        arena.write_weight(0, &[1; 4]).unwrap();
        state.lock().unwrap().fail_stream_synchronize = true;
        assert_eq!(arena.abort_and_release(), Err(KernelError::Async(71)));
        let operations = &state.lock().unwrap().operations;
        assert!(
            operations
                .iter()
                .any(|operation| matches!(operation, Operation::StreamSynchronize(_)))
        );
        assert!(operations.iter().all(|operation| !matches!(
            operation,
            Operation::PinnedFree(_)
                | Operation::DeviceFree(_)
                | Operation::EventDestroy(_)
                | Operation::StreamDestroy(_)
        )));
    }

    #[test]
    fn allocation_zero_fills_both_arenas_and_permit_identity_is_exact() {
        let (mut arena, state) = arena();
        assert_eq!(
            state
                .lock()
                .unwrap()
                .operations
                .iter()
                .filter(|operation| matches!(operation, Operation::Memset(_, _)))
                .count(),
            2
        );
        arena.drain_and_seal().unwrap();
        arena.verify_device_contents().unwrap();
        let wrong = WeightArenaExecutionPermit::test_only(1, [9; 32], 17);
        assert!(matches!(arena.adopt(wrong), Err(LoadPlanError::Adoption)));

        let operations = &state.lock().unwrap().operations;
        assert!(
            operations
                .iter()
                .any(|operation| matches!(operation, Operation::DeviceFree(_)))
        );
    }

    #[test]
    fn device_corruption_is_detected_before_adoption() {
        let (mut arena, state) = arena();
        arena.write_weight(8, &[7; 16]).unwrap();
        arena.write_metadata(4, &[3; 8]).unwrap();
        arena.drain_and_seal().unwrap();
        state
            .lock()
            .unwrap()
            .device_allocations
            .get_mut(&0x10_0000)
            .unwrap()[511] ^= 0xff;
        assert_eq!(
            arena.verify_device_contents(),
            Err(KernelError::DeviceValidation(READBACK_MISMATCH))
        );
        let permit = WeightArenaExecutionPermit::test_only(2, [9; 32], 17);
        assert!(matches!(arena.adopt(permit), Err(LoadPlanError::Adoption)));
    }
}
