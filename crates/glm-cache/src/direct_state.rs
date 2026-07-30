use std::sync::{Arc, RwLock};

use crate::{DirectExtentBuffer, DirectTierCapability};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct DirectBufferId {
    pub slot: u32,
    pub generation: u64,
}

impl DirectBufferId {
    pub const INVALID_GENERATION: u64 = 0;

    #[must_use]
    pub const fn is_valid(self) -> bool {
        self.generation != Self::INVALID_GENERATION
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectBufferState {
    Free,
    FillingFromHbm,
    HashingForWrite,
    WriteQueued,
    WriteInflight,
    ReadQueued,
    ReadInflight,
    HashingForRead,
    HostReady,
    CopyingToHbm,
    Failed,
    Quarantined,
    Retired,
}

impl DirectBufferState {
    #[must_use]
    pub const fn is_active(self) -> bool {
        !matches!(
            self,
            Self::Free | Self::Failed | Self::Quarantined | Self::Retired
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectBufferUse {
    CpuWrite,
    CpuRead,
}

#[derive(Debug)]
struct DirectBufferSlot {
    generation: u64,
    state: DirectBufferState,
    bytes: Arc<RwLock<DirectExtentBuffer>>,
}

#[derive(Clone, Debug)]
pub(crate) struct DirectHashBuffer {
    id: DirectBufferId,
    bytes: Arc<RwLock<DirectExtentBuffer>>,
}

impl DirectHashBuffer {
    pub(crate) const fn id(&self) -> DirectBufferId {
        self.id
    }

    pub(crate) fn verify<R>(
        &self,
        verifier: impl FnOnce(&[u8]) -> R,
    ) -> Result<R, DirectBufferStateError> {
        let bytes = self
            .bytes
            .read()
            .map_err(|_| DirectBufferStateError::Poisoned)?;
        Ok(verifier(bytes.as_slice()))
    }

    pub(crate) fn address(&self) -> Result<usize, DirectBufferStateError> {
        self.verify(|bytes| bytes.as_ptr().addr())
    }
}

#[derive(Debug)]
pub struct DirectBufferPool {
    slots: Vec<DirectBufferSlot>,
    next_search: usize,
}

impl DirectBufferPool {
    pub fn new(capacity: u32) -> Result<Self, DirectBufferStateError> {
        if capacity == 0 {
            return Err(DirectBufferStateError::Capacity);
        }
        let capacity = usize::try_from(capacity).map_err(|_| DirectBufferStateError::Capacity)?;
        let mut slots = Vec::new();
        slots
            .try_reserve_exact(capacity)
            .map_err(|_| DirectBufferStateError::Allocation)?;
        for _ in 0..capacity {
            slots.push(DirectBufferSlot {
                generation: DirectBufferId::INVALID_GENERATION,
                state: DirectBufferState::Free,
                bytes: Arc::new(RwLock::new(
                    DirectExtentBuffer::new(DirectTierCapability::Mtp)
                        .map_err(|_| DirectBufferStateError::Allocation)?,
                )),
            });
        }
        Ok(Self {
            slots,
            next_search: 0,
        })
    }

    pub fn reserve(
        &mut self,
        usage: DirectBufferUse,
    ) -> Result<DirectBufferId, DirectBufferStateError> {
        let capacity = self.slots.len();
        for distance in 0..capacity {
            let slot_index = (self.next_search + distance) % capacity;
            let slot = &mut self.slots[slot_index];
            if slot.state != DirectBufferState::Free {
                continue;
            }
            let Some(generation) = slot.generation.checked_add(1) else {
                slot.state = DirectBufferState::Retired;
                continue;
            };
            if generation == DirectBufferId::INVALID_GENERATION {
                slot.state = DirectBufferState::Retired;
                continue;
            }
            let mut bytes = match slot.bytes.write() {
                Ok(bytes) => bytes,
                Err(_) => {
                    slot.state = DirectBufferState::Quarantined;
                    return Err(DirectBufferStateError::Poisoned);
                }
            };
            bytes.as_mut_slice().fill(0);
            drop(bytes);
            slot.generation = generation;
            slot.state = match usage {
                DirectBufferUse::CpuWrite => DirectBufferState::HashingForWrite,
                DirectBufferUse::CpuRead => DirectBufferState::ReadQueued,
            };
            self.next_search = (slot_index + 1) % capacity;
            return Ok(DirectBufferId {
                slot: u32::try_from(slot_index).map_err(|_| DirectBufferStateError::Capacity)?,
                generation,
            });
        }
        Err(DirectBufferStateError::Capacity)
    }

    pub fn state(&self, id: DirectBufferId) -> Result<DirectBufferState, DirectBufferStateError> {
        Ok(self.slot(id)?.state)
    }

    pub fn with_bytes<R>(
        &self,
        id: DirectBufferId,
        reader: impl FnOnce(&[u8]) -> R,
    ) -> Result<R, DirectBufferStateError> {
        let slot = self.slot(id)?;
        if !slot.state.is_active() {
            return Err(DirectBufferStateError::State {
                current: slot.state,
                requested: slot.state,
            });
        }
        let bytes = slot
            .bytes
            .read()
            .map_err(|_| DirectBufferStateError::Poisoned)?;
        Ok(reader(bytes.as_slice()))
    }

    pub fn with_bytes_mut<R>(
        &mut self,
        id: DirectBufferId,
        writer: impl FnOnce(&mut [u8]) -> R,
    ) -> Result<R, DirectBufferStateError> {
        let slot = self.slot_mut(id)?;
        if !slot.state.is_active() {
            return Err(DirectBufferStateError::State {
                current: slot.state,
                requested: slot.state,
            });
        }
        let mut bytes = slot
            .bytes
            .write()
            .map_err(|_| DirectBufferStateError::Poisoned)?;
        Ok(writer(bytes.as_mut_slice()))
    }

    pub(crate) fn hash_buffer(
        &self,
        id: DirectBufferId,
    ) -> Result<DirectHashBuffer, DirectBufferStateError> {
        let slot = self.slot(id)?;
        if slot.state != DirectBufferState::HashingForRead {
            return Err(DirectBufferStateError::State {
                current: slot.state,
                requested: DirectBufferState::HashingForRead,
            });
        }
        Ok(DirectHashBuffer {
            id,
            bytes: Arc::clone(&slot.bytes),
        })
    }

    pub fn transition(
        &mut self,
        id: DirectBufferId,
        next: DirectBufferState,
    ) -> Result<(), DirectBufferStateError> {
        let slot = self.slot_mut(id)?;
        let current = slot.state;
        if !valid_transition(current, next) {
            return Err(DirectBufferStateError::State {
                current,
                requested: next,
            });
        }
        if next == DirectBufferState::Free {
            slot.bytes
                .write()
                .map_err(|_| DirectBufferStateError::Poisoned)?
                .as_mut_slice()
                .fill(0);
        }
        slot.state = next;
        Ok(())
    }

    pub fn fail(&mut self, id: DirectBufferId) -> Result<(), DirectBufferStateError> {
        self.transition(id, DirectBufferState::Failed)
    }

    pub fn quarantine(&mut self, id: DirectBufferId) -> Result<(), DirectBufferStateError> {
        self.transition(id, DirectBufferState::Quarantined)
    }

    pub fn release_abandoned_read(
        &mut self,
        id: DirectBufferId,
    ) -> Result<(), DirectBufferStateError> {
        let slot = self.slot_mut(id)?;
        if slot.state != DirectBufferState::ReadInflight {
            return Err(DirectBufferStateError::State {
                current: slot.state,
                requested: DirectBufferState::Free,
            });
        }
        slot.bytes
            .write()
            .map_err(|_| DirectBufferStateError::Poisoned)?
            .as_mut_slice()
            .fill(0);
        slot.state = DirectBufferState::Free;
        Ok(())
    }

    #[must_use]
    pub fn free_slots(&self) -> usize {
        self.slots
            .iter()
            .filter(|slot| slot.state == DirectBufferState::Free)
            .count()
    }

    #[must_use]
    pub fn active_slots(&self) -> usize {
        self.slots
            .iter()
            .filter(|slot| slot.state.is_active())
            .count()
    }

    #[must_use]
    pub fn quarantined_slots(&self) -> usize {
        self.slots
            .iter()
            .filter(|slot| slot.state == DirectBufferState::Quarantined)
            .count()
    }

    #[must_use]
    pub fn retired_slots(&self) -> usize {
        self.slots
            .iter()
            .filter(|slot| slot.state == DirectBufferState::Retired)
            .count()
    }

    fn slot(&self, id: DirectBufferId) -> Result<&DirectBufferSlot, DirectBufferStateError> {
        validate_id(id)?;
        let slot = self
            .slots
            .get(id.slot as usize)
            .ok_or(DirectBufferStateError::UnknownSlot)?;
        if slot.generation != id.generation {
            return Err(DirectBufferStateError::StaleGeneration);
        }
        Ok(slot)
    }

    fn slot_mut(
        &mut self,
        id: DirectBufferId,
    ) -> Result<&mut DirectBufferSlot, DirectBufferStateError> {
        validate_id(id)?;
        let slot = self
            .slots
            .get_mut(id.slot as usize)
            .ok_or(DirectBufferStateError::UnknownSlot)?;
        if slot.generation != id.generation {
            return Err(DirectBufferStateError::StaleGeneration);
        }
        Ok(slot)
    }

    #[cfg(test)]
    fn set_generation_for_test(
        &mut self,
        slot: usize,
        generation: u64,
    ) -> Result<(), DirectBufferStateError> {
        let slot = self
            .slots
            .get_mut(slot)
            .ok_or(DirectBufferStateError::UnknownSlot)?;
        if slot.state != DirectBufferState::Free {
            return Err(DirectBufferStateError::State {
                current: slot.state,
                requested: DirectBufferState::Free,
            });
        }
        slot.generation = generation;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectOperationKind {
    Read,
    Write,
    Fsync,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectCompletionKind {
    Original,
    AsyncCancel,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct DirectCompletionToken(u64);

impl DirectCompletionToken {
    const CANCEL_BIT: u64 = 1;
    const SLOT_MASK: u64 = 0x7fff_ffff;

    fn new(slot: u32, generation: u32, kind: DirectCompletionKind) -> Self {
        let role = match kind {
            DirectCompletionKind::Original => 0,
            DirectCompletionKind::AsyncCancel => Self::CANCEL_BIT,
        };
        Self((u64::from(generation) << 32) | (u64::from(slot) << 1) | role)
    }

    pub fn from_user_data(user_data: u64) -> Result<Self, DirectDescriptorError> {
        let token = Self(user_data);
        if token.generation() == 0 {
            return Err(DirectDescriptorError::Identity);
        }
        Ok(token)
    }

    #[must_use]
    pub const fn user_data(self) -> u64 {
        self.0
    }

    #[must_use]
    pub const fn kind(self) -> DirectCompletionKind {
        if self.0 & Self::CANCEL_BIT == 0 {
            DirectCompletionKind::Original
        } else {
            DirectCompletionKind::AsyncCancel
        }
    }

    const fn slot(self) -> u32 {
        ((self.0 >> 1) & Self::SLOT_MASK) as u32
    }

    const fn generation(self) -> u32 {
        (self.0 >> 32) as u32
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectDescriptorBinding {
    pub buffer: DirectBufferId,
    pub operation_generation: u64,
    pub operation: DirectOperationKind,
}

#[derive(Clone, Copy, Debug)]
struct DirectDescriptorEntry {
    binding: DirectDescriptorBinding,
    original_pending: bool,
    cancel_pending: bool,
}

#[derive(Clone, Copy, Debug)]
struct DirectDescriptorSlot {
    generation: u32,
    retired: bool,
    entry: Option<DirectDescriptorEntry>,
}

#[derive(Debug)]
pub struct DirectDescriptorTable {
    slots: Vec<DirectDescriptorSlot>,
    next_search: usize,
}

impl DirectDescriptorTable {
    pub fn new(capacity: u32) -> Result<Self, DirectDescriptorError> {
        if capacity == 0 || capacity > 0x8000_0000 {
            return Err(DirectDescriptorError::Capacity);
        }
        let capacity = usize::try_from(capacity).map_err(|_| DirectDescriptorError::Capacity)?;
        let mut slots = Vec::new();
        slots
            .try_reserve_exact(capacity)
            .map_err(|_| DirectDescriptorError::Allocation)?;
        slots.resize(
            capacity,
            DirectDescriptorSlot {
                generation: 0,
                retired: false,
                entry: None,
            },
        );
        Ok(Self {
            slots,
            next_search: 0,
        })
    }

    pub fn allocate(
        &mut self,
        binding: DirectDescriptorBinding,
    ) -> Result<DirectCompletionToken, DirectDescriptorError> {
        if !binding.buffer.is_valid() || binding.operation_generation == 0 {
            return Err(DirectDescriptorError::Identity);
        }
        let capacity = self.slots.len();
        for distance in 0..capacity {
            let slot_index = (self.next_search + distance) % capacity;
            let slot = &mut self.slots[slot_index];
            if slot.retired || slot.entry.is_some() {
                continue;
            }
            let Some(generation) = slot.generation.checked_add(1) else {
                slot.retired = true;
                continue;
            };
            if generation == 0 {
                slot.retired = true;
                continue;
            }
            slot.generation = generation;
            slot.entry = Some(DirectDescriptorEntry {
                binding,
                original_pending: true,
                cancel_pending: false,
            });
            self.next_search = (slot_index + 1) % capacity;
            return Ok(DirectCompletionToken::new(
                u32::try_from(slot_index).map_err(|_| DirectDescriptorError::Capacity)?,
                generation,
                DirectCompletionKind::Original,
            ));
        }
        Err(DirectDescriptorError::Capacity)
    }

    pub fn issue_cancel(
        &mut self,
        original: DirectCompletionToken,
    ) -> Result<DirectCompletionToken, DirectDescriptorError> {
        if original.kind() != DirectCompletionKind::Original {
            return Err(DirectDescriptorError::Role);
        }
        let slot = self.slot_mut(original)?;
        let entry = slot.entry.as_mut().ok_or(DirectDescriptorError::Missing)?;
        if !entry.original_pending || entry.cancel_pending {
            return Err(DirectDescriptorError::NoPendingCompletion);
        }
        entry.cancel_pending = true;
        Ok(DirectCompletionToken::new(
            original.slot(),
            original.generation(),
            DirectCompletionKind::AsyncCancel,
        ))
    }

    pub fn resolve(
        &self,
        token: DirectCompletionToken,
    ) -> Result<DirectDescriptorBinding, DirectDescriptorError> {
        let slot = self.slot(token)?;
        let entry = slot.entry.ok_or(DirectDescriptorError::Missing)?;
        let pending = match token.kind() {
            DirectCompletionKind::Original => entry.original_pending,
            DirectCompletionKind::AsyncCancel => entry.cancel_pending,
        };
        if !pending {
            return Err(DirectDescriptorError::NoPendingCompletion);
        }
        Ok(entry.binding)
    }

    pub fn complete(
        &mut self,
        token: DirectCompletionToken,
    ) -> Result<DirectDescriptorBinding, DirectDescriptorError> {
        let slot = self.slot_mut(token)?;
        let entry = slot.entry.as_mut().ok_or(DirectDescriptorError::Missing)?;
        let pending = match token.kind() {
            DirectCompletionKind::Original => &mut entry.original_pending,
            DirectCompletionKind::AsyncCancel => &mut entry.cancel_pending,
        };
        if !*pending {
            return Err(DirectDescriptorError::NoPendingCompletion);
        }
        *pending = false;
        let binding = entry.binding;
        if !entry.original_pending && !entry.cancel_pending {
            slot.entry = None;
        }
        Ok(binding)
    }

    #[must_use]
    pub fn outstanding_descriptors(&self) -> usize {
        self.slots
            .iter()
            .filter(|slot| slot.entry.is_some())
            .count()
    }

    #[must_use]
    pub fn retired_descriptors(&self) -> usize {
        self.slots.iter().filter(|slot| slot.retired).count()
    }

    fn slot(
        &self,
        token: DirectCompletionToken,
    ) -> Result<&DirectDescriptorSlot, DirectDescriptorError> {
        if token.generation() == 0 {
            return Err(DirectDescriptorError::Identity);
        }
        let slot = self
            .slots
            .get(token.slot() as usize)
            .ok_or(DirectDescriptorError::UnknownSlot)?;
        if slot.generation != token.generation() {
            return Err(DirectDescriptorError::StaleGeneration);
        }
        Ok(slot)
    }

    fn slot_mut(
        &mut self,
        token: DirectCompletionToken,
    ) -> Result<&mut DirectDescriptorSlot, DirectDescriptorError> {
        if token.generation() == 0 {
            return Err(DirectDescriptorError::Identity);
        }
        let slot = self
            .slots
            .get_mut(token.slot() as usize)
            .ok_or(DirectDescriptorError::UnknownSlot)?;
        if slot.generation != token.generation() {
            return Err(DirectDescriptorError::StaleGeneration);
        }
        Ok(slot)
    }

    #[cfg(test)]
    fn set_generation_for_test(
        &mut self,
        slot: usize,
        generation: u32,
    ) -> Result<(), DirectDescriptorError> {
        let slot = self
            .slots
            .get_mut(slot)
            .ok_or(DirectDescriptorError::UnknownSlot)?;
        if slot.entry.is_some() {
            return Err(DirectDescriptorError::Capacity);
        }
        slot.generation = generation;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectDescriptorError {
    Capacity,
    Allocation,
    Identity,
    UnknownSlot,
    StaleGeneration,
    Role,
    Missing,
    NoPendingCompletion,
}

impl std::fmt::Display for DirectDescriptorError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for DirectDescriptorError {}

fn validate_id(id: DirectBufferId) -> Result<(), DirectBufferStateError> {
    if !id.is_valid() {
        return Err(DirectBufferStateError::InvalidGeneration);
    }
    Ok(())
}

fn valid_transition(current: DirectBufferState, next: DirectBufferState) -> bool {
    if current.is_active() && next == DirectBufferState::Failed {
        return true;
    }
    matches!(
        (current, next),
        (
            DirectBufferState::FillingFromHbm,
            DirectBufferState::HashingForWrite
        ) | (
            DirectBufferState::HashingForWrite,
            DirectBufferState::WriteQueued
        ) | (
            DirectBufferState::WriteQueued,
            DirectBufferState::WriteInflight
        ) | (DirectBufferState::WriteQueued, DirectBufferState::Free)
            | (DirectBufferState::WriteInflight, DirectBufferState::Free)
            | (
                DirectBufferState::ReadQueued,
                DirectBufferState::ReadInflight
            )
            | (DirectBufferState::ReadQueued, DirectBufferState::Free)
            | (
                DirectBufferState::ReadInflight,
                DirectBufferState::HashingForRead
            )
            | (
                DirectBufferState::HashingForRead,
                DirectBufferState::HostReady
            )
            | (
                DirectBufferState::HostReady,
                DirectBufferState::CopyingToHbm
            )
            | (DirectBufferState::HostReady, DirectBufferState::Free)
            | (DirectBufferState::CopyingToHbm, DirectBufferState::Free)
            | (DirectBufferState::Failed, DirectBufferState::Quarantined)
    )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectBufferStateError {
    Capacity,
    Allocation,
    Poisoned,
    InvalidGeneration,
    UnknownSlot,
    StaleGeneration,
    State {
        current: DirectBufferState,
        requested: DirectBufferState,
    },
}

impl std::fmt::Display for DirectBufferStateError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for DirectBufferStateError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn read_to_free(pool: &mut DirectBufferPool, id: DirectBufferId) {
        pool.transition(id, DirectBufferState::ReadInflight)
            .unwrap();
        pool.transition(id, DirectBufferState::HashingForRead)
            .unwrap();
        pool.transition(id, DirectBufferState::HostReady).unwrap();
        pool.transition(id, DirectBufferState::Free).unwrap();
    }

    #[test]
    fn buffers_are_fixed_aligned_zeroed_and_generation_bound() {
        let mut pool = DirectBufferPool::new(2).unwrap();
        let first = pool.reserve(DirectBufferUse::CpuRead).unwrap();
        assert_eq!(pool.state(first).unwrap(), DirectBufferState::ReadQueued);
        assert!(first.is_valid());
        assert_eq!(pool.with_bytes(first, <[u8]>::len).unwrap(), 2_052_096);
        assert!(
            pool.with_bytes(first, |bytes| bytes.as_ptr().addr())
                .unwrap()
                .is_multiple_of(4_096)
        );
        assert!(
            pool.with_bytes(first, |bytes| bytes.iter().all(|&byte| byte == 0))
                .unwrap()
        );
        pool.with_bytes_mut(first, |bytes| bytes[0] = 7).unwrap();
        read_to_free(&mut pool, first);
        assert!(matches!(
            pool.with_bytes(first, |_| ()),
            Err(DirectBufferStateError::State {
                current: DirectBufferState::Free,
                ..
            })
        ));

        let second_slot = pool.reserve(DirectBufferUse::CpuWrite).unwrap();
        pool.transition(second_slot, DirectBufferState::WriteQueued)
            .unwrap();
        pool.transition(second_slot, DirectBufferState::Free)
            .unwrap();
        let reused = pool.reserve(DirectBufferUse::CpuRead).unwrap();
        assert_eq!(reused.slot, first.slot);
        assert_eq!(reused.generation, first.generation + 1);
        assert!(
            pool.with_bytes(reused, |bytes| bytes.iter().all(|&byte| byte == 0))
                .unwrap()
        );
        assert_eq!(
            pool.transition(first, DirectBufferState::ReadInflight),
            Err(DirectBufferStateError::StaleGeneration)
        );
    }

    #[test]
    fn hash_handle_reads_the_same_stable_aligned_allocation() {
        let mut pool = DirectBufferPool::new(1).unwrap();
        let id = pool.reserve(DirectBufferUse::CpuRead).unwrap();
        pool.transition(id, DirectBufferState::ReadInflight)
            .unwrap();
        pool.with_bytes_mut(id, |bytes| bytes[17] = 9).unwrap();
        pool.transition(id, DirectBufferState::HashingForRead)
            .unwrap();
        let authority_address = pool.with_bytes(id, |bytes| bytes.as_ptr().addr()).unwrap();
        let handle = pool.hash_buffer(id).unwrap();
        let worker_observation = handle
            .verify(|bytes| (bytes.as_ptr().addr(), bytes[17]))
            .unwrap();
        assert_eq!(handle.id(), id);
        assert_eq!(worker_observation, (authority_address, 9));
        assert!(authority_address.is_multiple_of(4_096));
    }

    #[test]
    fn generation_overflow_permanently_retires_a_slot() {
        let mut pool = DirectBufferPool::new(2).unwrap();
        pool.set_generation_for_test(0, u64::MAX).unwrap();
        let live = pool.reserve(DirectBufferUse::CpuRead).unwrap();
        assert_eq!(live.slot, 1);
        assert_eq!(pool.retired_slots(), 1);
        assert_eq!(
            pool.reserve(DirectBufferUse::CpuRead),
            Err(DirectBufferStateError::Capacity)
        );
        read_to_free(&mut pool, live);
        let reused = pool.reserve(DirectBufferUse::CpuRead).unwrap();
        assert_eq!(reused.slot, 1);
        assert_eq!(pool.retired_slots(), 1);
    }

    #[test]
    fn failures_quarantine_without_reuse() {
        let mut pool = DirectBufferPool::new(1).unwrap();
        let id = pool.reserve(DirectBufferUse::CpuRead).unwrap();
        pool.transition(id, DirectBufferState::ReadInflight)
            .unwrap();
        pool.fail(id).unwrap();
        assert_eq!(pool.state(id).unwrap(), DirectBufferState::Failed);
        pool.quarantine(id).unwrap();
        assert_eq!(pool.state(id).unwrap(), DirectBufferState::Quarantined);
        assert_eq!(pool.quarantined_slots(), 1);
        assert_eq!(pool.active_slots(), 0);
        assert_eq!(
            pool.reserve(DirectBufferUse::CpuRead),
            Err(DirectBufferStateError::Capacity)
        );
        assert_eq!(
            pool.transition(id, DirectBufferState::Free),
            Err(DirectBufferStateError::State {
                current: DirectBufferState::Quarantined,
                requested: DirectBufferState::Free,
            })
        );
    }

    #[test]
    fn transition_table_is_exact_for_every_state_pair() {
        let states = [
            DirectBufferState::Free,
            DirectBufferState::FillingFromHbm,
            DirectBufferState::HashingForWrite,
            DirectBufferState::WriteQueued,
            DirectBufferState::WriteInflight,
            DirectBufferState::ReadQueued,
            DirectBufferState::ReadInflight,
            DirectBufferState::HashingForRead,
            DirectBufferState::HostReady,
            DirectBufferState::CopyingToHbm,
            DirectBufferState::Failed,
            DirectBufferState::Quarantined,
            DirectBufferState::Retired,
        ];
        for &current in &states {
            for &next in &states {
                let expected = current.is_active() && next == DirectBufferState::Failed
                    || matches!(
                        (current, next),
                        (
                            DirectBufferState::FillingFromHbm,
                            DirectBufferState::HashingForWrite
                        ) | (
                            DirectBufferState::HashingForWrite,
                            DirectBufferState::WriteQueued
                        ) | (
                            DirectBufferState::WriteQueued,
                            DirectBufferState::WriteInflight
                        ) | (DirectBufferState::WriteQueued, DirectBufferState::Free)
                            | (DirectBufferState::WriteInflight, DirectBufferState::Free)
                            | (
                                DirectBufferState::ReadQueued,
                                DirectBufferState::ReadInflight
                            )
                            | (DirectBufferState::ReadQueued, DirectBufferState::Free)
                            | (
                                DirectBufferState::ReadInflight,
                                DirectBufferState::HashingForRead
                            )
                            | (
                                DirectBufferState::HashingForRead,
                                DirectBufferState::HostReady
                            )
                            | (
                                DirectBufferState::HostReady,
                                DirectBufferState::CopyingToHbm
                            )
                            | (DirectBufferState::HostReady, DirectBufferState::Free)
                            | (DirectBufferState::CopyingToHbm, DirectBufferState::Free)
                            | (DirectBufferState::Failed, DirectBufferState::Quarantined)
                    );
                assert_eq!(valid_transition(current, next), expected);
            }
        }
    }

    fn binding(generation: u64) -> DirectDescriptorBinding {
        DirectDescriptorBinding {
            buffer: DirectBufferId {
                slot: 3,
                generation: 7,
            },
            operation_generation: generation,
            operation: DirectOperationKind::Read,
        }
    }

    #[test]
    fn descriptor_resolves_full_generations_in_both_completion_orders() {
        for original_first in [false, true] {
            let mut table = DirectDescriptorTable::new(1).unwrap();
            let original = table.allocate(binding(11)).unwrap();
            let cancel = table.issue_cancel(original).unwrap();
            assert_eq!(original.kind(), DirectCompletionKind::Original);
            assert_eq!(cancel.kind(), DirectCompletionKind::AsyncCancel);
            assert_ne!(original.user_data(), cancel.user_data());
            assert_eq!(
                DirectCompletionToken::from_user_data(original.user_data()).unwrap(),
                original
            );
            assert_eq!(
                DirectCompletionToken::from_user_data(cancel.user_data()).unwrap(),
                cancel
            );
            assert_eq!(table.resolve(original).unwrap(), binding(11));
            assert_eq!(table.resolve(cancel).unwrap(), binding(11));
            if original_first {
                assert_eq!(table.complete(original).unwrap(), binding(11));
                assert_eq!(table.outstanding_descriptors(), 1);
                assert_eq!(table.complete(cancel).unwrap(), binding(11));
            } else {
                assert_eq!(table.complete(cancel).unwrap(), binding(11));
                assert_eq!(table.outstanding_descriptors(), 1);
                assert_eq!(table.complete(original).unwrap(), binding(11));
            }
            assert_eq!(table.outstanding_descriptors(), 0);
            assert_eq!(table.resolve(original), Err(DirectDescriptorError::Missing));
        }
        assert_eq!(
            DirectCompletionToken::from_user_data(0),
            Err(DirectDescriptorError::Identity)
        );
    }

    #[test]
    fn descriptor_reuse_rejects_late_cqe_and_overflow_retires() {
        let mut table = DirectDescriptorTable::new(2).unwrap();
        let old = table.allocate(binding(1)).unwrap();
        table.complete(old).unwrap();
        let other = table.allocate(binding(2)).unwrap();
        table.complete(other).unwrap();
        let reused = table.allocate(binding(3)).unwrap();
        assert_ne!(old.user_data(), reused.user_data());
        assert_eq!(
            table.resolve(old),
            Err(DirectDescriptorError::StaleGeneration)
        );
        table.complete(reused).unwrap();

        table.set_generation_for_test(1, u32::MAX).unwrap();
        table.set_generation_for_test(0, u32::MAX).unwrap();
        assert_eq!(
            table.allocate(binding(4)),
            Err(DirectDescriptorError::Capacity)
        );
        assert_eq!(table.retired_descriptors(), 2);
    }
}
