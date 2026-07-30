use std::collections::BTreeMap;

use crate::{
    DirectBufferId, DirectBufferPool, DirectBufferState, DirectBufferStateError, DirectBufferUse,
    DirectCompletionToken, DirectDescriptorBinding, DirectDescriptorError, DirectDescriptorTable,
    DirectExtentRecord, DirectOperationKind, DirectTierCapability, decode_direct_extent,
};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct DirectRestoreTicketId(pub u64);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectRestoreRequest {
    pub request_id: u64,
    pub tenant_id: u64,
    pub required_capability: DirectTierCapability,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectRestoreConfig {
    pub maximum_tickets: u32,
    pub maximum_waiters_per_ticket: u32,
    pub maximum_hash_jobs: u32,
    pub maximum_physical_bytes: u64,
    pub maximum_logical_bytes_per_tenant: u64,
    pub buffer_slots: u32,
    pub descriptor_capacity: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectRestoreState {
    Planned,
    BufferReserved,
    ReadSubmitted,
    DataReady,
    HashVerified,
    Abandoned,
    Failed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectRestoreAdmission {
    Created(DirectRestoreTicketId),
    Joined(DirectRestoreTicketId),
}

impl DirectRestoreAdmission {
    #[must_use]
    pub const fn ticket(self) -> DirectRestoreTicketId {
        match self {
            Self::Created(ticket) | Self::Joined(ticket) => ticket,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectCancellation {
    WaiterRemoved,
    ReleasedBeforeSubmission,
    AbandonedWithoutAsyncCancel,
    AsyncCancelSubmitted,
    WaitingForHashAcknowledgement,
    ReleasedAfterVerification,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectCatalogBinding {
    Exact,
    ReplanRequired,
    SubmittedRecordPinned,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectReadCompletion {
    Exact,
    Cancelled,
    Failed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DirectHashState {
    None,
    Reserved,
    Queued,
    Running,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectHashJob {
    ticket: DirectRestoreTicketId,
    buffer: DirectBufferId,
}

impl DirectHashJob {
    #[must_use]
    pub const fn ticket(self) -> DirectRestoreTicketId {
        self.ticket
    }

    #[must_use]
    pub const fn buffer(self) -> DirectBufferId {
        self.buffer
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectHashResult {
    job: DirectHashJob,
    verified: bool,
}

impl DirectHashResult {
    #[must_use]
    pub const fn job(self) -> DirectHashJob {
        self.job
    }

    #[must_use]
    pub const fn verified(self) -> bool {
        self.verified
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectCqKind {
    Original,
    AsyncCancel,
    Fsync,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectCqTracker {
    descriptor_capacity: u32,
    cq_entries: u32,
    original: u32,
    cancel: u32,
    fsync: u32,
    nodrop_present: bool,
}

impl DirectCqTracker {
    pub fn new(
        descriptor_capacity: u32,
        cq_entries: u32,
        nodrop_present: bool,
    ) -> Result<Self, DirectRestoreError> {
        let required_cq = descriptor_capacity
            .checked_mul(2)
            .ok_or(DirectRestoreError::Overflow)?;
        if descriptor_capacity == 0 || cq_entries != required_cq {
            return Err(DirectRestoreError::CqConfig);
        }
        Ok(Self {
            descriptor_capacity,
            cq_entries,
            original: 0,
            cancel: 0,
            fsync: 0,
            nodrop_present,
        })
    }

    pub fn try_submit(&mut self, kind: DirectCqKind) -> Result<(), DirectRestoreError> {
        let (next_original, next_cancel, next_fsync) = match kind {
            DirectCqKind::Original => (
                self.original
                    .checked_add(1)
                    .ok_or(DirectRestoreError::Overflow)?,
                self.cancel,
                self.fsync,
            ),
            DirectCqKind::AsyncCancel => (
                self.original,
                self.cancel
                    .checked_add(1)
                    .ok_or(DirectRestoreError::Overflow)?,
                self.fsync,
            ),
            DirectCqKind::Fsync => (
                self.original,
                self.cancel,
                self.fsync
                    .checked_add(1)
                    .ok_or(DirectRestoreError::Overflow)?,
            ),
        };
        let next_total = next_original
            .checked_add(next_cancel)
            .and_then(|sum| sum.checked_add(next_fsync))
            .ok_or(DirectRestoreError::Overflow)?;
        if next_original > self.descriptor_capacity
            || next_fsync > self.descriptor_capacity
            || next_total > self.cq_entries
        {
            return Err(DirectRestoreError::CqWait);
        }
        self.original = next_original;
        self.cancel = next_cancel;
        self.fsync = next_fsync;
        Ok(())
    }

    pub fn complete(&mut self, kind: DirectCqKind) -> Result<(), DirectRestoreError> {
        self.validate_completion(kind)?;
        let counter = match kind {
            DirectCqKind::Original => &mut self.original,
            DirectCqKind::AsyncCancel => &mut self.cancel,
            DirectCqKind::Fsync => &mut self.fsync,
        };
        *counter -= 1;
        Ok(())
    }

    pub fn validate_completion(&self, kind: DirectCqKind) -> Result<(), DirectRestoreError> {
        let counter = match kind {
            DirectCqKind::Original => self.original,
            DirectCqKind::AsyncCancel => self.cancel,
            DirectCqKind::Fsync => self.fsync,
        };
        if counter == 0 {
            return Err(DirectRestoreError::CqUnderflow);
        }
        Ok(())
    }

    #[must_use]
    pub const fn outstanding(self) -> u32 {
        self.original + self.cancel + self.fsync
    }

    #[must_use]
    pub const fn cq_entries(self) -> u32 {
        self.cq_entries
    }

    #[must_use]
    pub const fn nodrop_present(self) -> bool {
        self.nodrop_present
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DirectRestoreWaiter {
    tenant_id: u64,
    required_capability: DirectTierCapability,
}

#[derive(Clone, Debug)]
struct DirectRestoreTicket {
    record: DirectExtentRecord,
    catalog_epoch: u64,
    catalog_record_sha256: [u8; 32],
    state: DirectRestoreState,
    buffer: Option<DirectBufferId>,
    waiters: BTreeMap<u64, DirectRestoreWaiter>,
    original_pending: bool,
    cancel_pending: bool,
    original_token: Option<DirectCompletionToken>,
    cancel_token: Option<DirectCompletionToken>,
    hash_state: DirectHashState,
}

#[derive(Debug)]
pub struct DirectRestoreTable {
    config: DirectRestoreConfig,
    next_ticket: u64,
    tickets: BTreeMap<DirectRestoreTicketId, DirectRestoreTicket>,
    request_to_ticket: BTreeMap<u64, DirectRestoreTicketId>,
    logical_by_tenant: BTreeMap<u64, u64>,
    physical_bytes: u64,
    active_hash_jobs: u32,
    buffers: DirectBufferPool,
    descriptors: DirectDescriptorTable,
    cq: DirectCqTracker,
}

impl DirectRestoreTable {
    pub fn new(
        config: DirectRestoreConfig,
        nodrop_present: bool,
    ) -> Result<Self, DirectRestoreError> {
        if config.maximum_tickets == 0
            || config.maximum_waiters_per_ticket == 0
            || config.maximum_hash_jobs == 0
            || config.maximum_physical_bytes == 0
            || config.maximum_logical_bytes_per_tenant == 0
            || config.buffer_slots == 0
            || config.descriptor_capacity == 0
            || config.maximum_hash_jobs > config.buffer_slots
        {
            return Err(DirectRestoreError::Config);
        }
        let cq_entries = config
            .descriptor_capacity
            .checked_mul(2)
            .ok_or(DirectRestoreError::Overflow)?;
        Ok(Self {
            config,
            next_ticket: 1,
            tickets: BTreeMap::new(),
            request_to_ticket: BTreeMap::new(),
            logical_by_tenant: BTreeMap::new(),
            physical_bytes: 0,
            active_hash_jobs: 0,
            buffers: DirectBufferPool::new(config.buffer_slots)?,
            descriptors: DirectDescriptorTable::new(config.descriptor_capacity)?,
            cq: DirectCqTracker::new(config.descriptor_capacity, cq_entries, nodrop_present)?,
        })
    }

    pub fn plan(
        &mut self,
        request: DirectRestoreRequest,
        record: DirectExtentRecord,
        catalog_epoch: u64,
        catalog_record_sha256: [u8; 32],
    ) -> Result<DirectRestoreAdmission, DirectRestoreError> {
        validate_restore_request(request)?;
        record.validate().map_err(|_| DirectRestoreError::Record)?;
        if catalog_epoch == 0 || catalog_record_sha256 == [0; 32] {
            return Err(DirectRestoreError::Catalog);
        }
        if !capability_satisfies(record.capability, request.required_capability) {
            return Err(DirectRestoreError::Capability);
        }
        if self.request_to_ticket.contains_key(&request.request_id) {
            return Err(DirectRestoreError::DuplicateRequest);
        }
        let logical_charge = request.required_capability.logical_bytes();
        self.validate_tenant_charge(request.tenant_id, logical_charge)?;

        if let Some(ticket_id) = self.compatible_ticket(
            &record,
            catalog_epoch,
            catalog_record_sha256,
            request.required_capability,
        ) {
            let ticket = self
                .tickets
                .get(&ticket_id)
                .ok_or(DirectRestoreError::MissingTicket)?;
            if ticket.waiters.len() >= self.config.maximum_waiters_per_ticket as usize {
                return Err(DirectRestoreError::WaiterCapacity);
            }
            self.charge_tenant(request.tenant_id, logical_charge)?;
            let ticket = self
                .tickets
                .get_mut(&ticket_id)
                .ok_or(DirectRestoreError::MissingTicket)?;
            ticket.waiters.insert(
                request.request_id,
                DirectRestoreWaiter {
                    tenant_id: request.tenant_id,
                    required_capability: request.required_capability,
                },
            );
            self.request_to_ticket.insert(request.request_id, ticket_id);
            return Ok(DirectRestoreAdmission::Joined(ticket_id));
        }

        if self.tickets.len() >= self.config.maximum_tickets as usize {
            return Err(DirectRestoreError::TicketCapacity);
        }
        let next_physical = self
            .physical_bytes
            .checked_add(record.physical_length)
            .ok_or(DirectRestoreError::Overflow)?;
        if next_physical > self.config.maximum_physical_bytes {
            return Err(DirectRestoreError::PhysicalCapacity);
        }
        let ticket_id = DirectRestoreTicketId(self.next_ticket);
        self.next_ticket = self
            .next_ticket
            .checked_add(1)
            .ok_or(DirectRestoreError::Overflow)?;
        self.charge_tenant(request.tenant_id, logical_charge)?;
        let mut waiters = BTreeMap::new();
        waiters.insert(
            request.request_id,
            DirectRestoreWaiter {
                tenant_id: request.tenant_id,
                required_capability: request.required_capability,
            },
        );
        self.tickets.insert(
            ticket_id,
            DirectRestoreTicket {
                record,
                catalog_epoch,
                catalog_record_sha256,
                state: DirectRestoreState::Planned,
                buffer: None,
                waiters,
                original_pending: false,
                cancel_pending: false,
                original_token: None,
                cancel_token: None,
                hash_state: DirectHashState::None,
            },
        );
        self.request_to_ticket.insert(request.request_id, ticket_id);
        self.physical_bytes = next_physical;
        Ok(DirectRestoreAdmission::Created(ticket_id))
    }

    pub fn reserve_buffer(
        &mut self,
        ticket_id: DirectRestoreTicketId,
    ) -> Result<DirectBufferId, DirectRestoreError> {
        let ticket = self.ticket(ticket_id)?;
        if ticket.state != DirectRestoreState::Planned || ticket.buffer.is_some() {
            return Err(DirectRestoreError::State);
        }
        let buffer = self.buffers.reserve(DirectBufferUse::CpuRead)?;
        let ticket = self.ticket_mut(ticket_id)?;
        ticket.buffer = Some(buffer);
        ticket.state = DirectRestoreState::BufferReserved;
        Ok(buffer)
    }

    pub fn submit_read(
        &mut self,
        ticket_id: DirectRestoreTicketId,
    ) -> Result<(), DirectRestoreError> {
        let ticket = self.ticket(ticket_id)?;
        if ticket.state != DirectRestoreState::BufferReserved
            || ticket.original_pending
            || ticket.cancel_pending
            || ticket.hash_state != DirectHashState::None
        {
            return Err(DirectRestoreError::State);
        }
        let next_hash_jobs = self
            .active_hash_jobs
            .checked_add(1)
            .ok_or(DirectRestoreError::Overflow)?;
        if next_hash_jobs > self.config.maximum_hash_jobs {
            return Err(DirectRestoreError::HashWait);
        }
        let buffer = ticket.buffer.ok_or(DirectRestoreError::State)?;
        let binding = DirectDescriptorBinding {
            buffer,
            operation_generation: ticket_id.0,
            operation: DirectOperationKind::Read,
        };
        let original_token = self.descriptors.allocate(binding)?;
        if let Err(error) = self.cq.try_submit(DirectCqKind::Original) {
            self.descriptors.complete(original_token)?;
            return Err(error);
        }
        if let Err(error) = self
            .buffers
            .transition(buffer, DirectBufferState::ReadInflight)
        {
            self.cq.complete(DirectCqKind::Original)?;
            self.descriptors.complete(original_token)?;
            return Err(error.into());
        }
        let ticket = self.ticket_mut(ticket_id)?;
        ticket.original_pending = true;
        ticket.original_token = Some(original_token);
        ticket.hash_state = DirectHashState::Reserved;
        ticket.state = DirectRestoreState::ReadSubmitted;
        self.active_hash_jobs = next_hash_jobs;
        Ok(())
    }

    pub fn read_destination_mut(
        &mut self,
        ticket_id: DirectRestoreTicketId,
    ) -> Result<&mut [u8], DirectRestoreError> {
        let ticket = self.ticket(ticket_id)?;
        if ticket.state != DirectRestoreState::ReadSubmitted
            || !ticket.original_pending
            || ticket.cancel_pending
            || ticket.hash_state != DirectHashState::Reserved
        {
            return Err(DirectRestoreError::State);
        }
        let buffer = ticket.buffer.ok_or(DirectRestoreError::State)?;
        let physical_length = usize::try_from(ticket.record.physical_length)
            .map_err(|_| DirectRestoreError::Overflow)?;
        self.buffers
            .bytes_mut(buffer)?
            .get_mut(..physical_length)
            .ok_or(DirectRestoreError::Record)
    }

    pub fn cancel_waiter(
        &mut self,
        request_id: u64,
        async_cancel_supported: bool,
    ) -> Result<DirectCancellation, DirectRestoreError> {
        let ticket_id = *self
            .request_to_ticket
            .get(&request_id)
            .ok_or(DirectRestoreError::MissingRequest)?;
        let waiter = *self
            .ticket(ticket_id)?
            .waiters
            .get(&request_id)
            .ok_or(DirectRestoreError::MissingRequest)?;
        self.uncharge_tenant(waiter.tenant_id, waiter.required_capability.logical_bytes())?;
        self.request_to_ticket.remove(&request_id);
        let ticket = self.ticket_mut(ticket_id)?;
        ticket.waiters.remove(&request_id);
        if !ticket.waiters.is_empty() {
            return Ok(DirectCancellation::WaiterRemoved);
        }

        let state = ticket.state;
        let buffer = ticket.buffer;
        match state {
            DirectRestoreState::Planned => {
                self.remove_ticket(ticket_id)?;
                Ok(DirectCancellation::ReleasedBeforeSubmission)
            }
            DirectRestoreState::BufferReserved => {
                let buffer = buffer.ok_or(DirectRestoreError::State)?;
                self.buffers.transition(buffer, DirectBufferState::Free)?;
                self.remove_ticket(ticket_id)?;
                Ok(DirectCancellation::ReleasedBeforeSubmission)
            }
            DirectRestoreState::ReadSubmitted => {
                self.ticket_mut(ticket_id)?.state = DirectRestoreState::Abandoned;
                if async_cancel_supported {
                    match self.cq.try_submit(DirectCqKind::AsyncCancel) {
                        Ok(()) => {
                            let original_token = self
                                .ticket(ticket_id)?
                                .original_token
                                .ok_or(DirectRestoreError::State)?;
                            match self.descriptors.issue_cancel(original_token) {
                                Ok(cancel_token) => {
                                    let ticket = self.ticket_mut(ticket_id)?;
                                    ticket.cancel_pending = true;
                                    ticket.cancel_token = Some(cancel_token);
                                    Ok(DirectCancellation::AsyncCancelSubmitted)
                                }
                                Err(error) => {
                                    self.cq.complete(DirectCqKind::AsyncCancel)?;
                                    Err(error.into())
                                }
                            }
                        }
                        Err(DirectRestoreError::CqWait) => {
                            Ok(DirectCancellation::AbandonedWithoutAsyncCancel)
                        }
                        Err(error) => Err(error),
                    }
                } else {
                    Ok(DirectCancellation::AbandonedWithoutAsyncCancel)
                }
            }
            DirectRestoreState::DataReady => {
                self.ticket_mut(ticket_id)?.state = DirectRestoreState::Abandoned;
                Ok(DirectCancellation::WaitingForHashAcknowledgement)
            }
            DirectRestoreState::HashVerified => {
                let buffer = buffer.ok_or(DirectRestoreError::State)?;
                self.buffers.transition(buffer, DirectBufferState::Free)?;
                self.remove_ticket(ticket_id)?;
                Ok(DirectCancellation::ReleasedAfterVerification)
            }
            DirectRestoreState::Abandoned | DirectRestoreState::Failed => {
                Err(DirectRestoreError::State)
            }
        }
    }

    pub fn complete_original(
        &mut self,
        ticket_id: DirectRestoreTicketId,
        completion: DirectReadCompletion,
    ) -> Result<(), DirectRestoreError> {
        let ticket = self.ticket(ticket_id)?;
        if !ticket.original_pending
            || !matches!(
                ticket.state,
                DirectRestoreState::ReadSubmitted | DirectRestoreState::Abandoned
            )
        {
            return Err(DirectRestoreError::DuplicateCompletion);
        }
        let state = ticket.state;
        let buffer = ticket.buffer.ok_or(DirectRestoreError::State)?;
        let original_token = ticket.original_token.ok_or(DirectRestoreError::State)?;
        let expected_binding = DirectDescriptorBinding {
            buffer,
            operation_generation: ticket_id.0,
            operation: DirectOperationKind::Read,
        };
        if self.descriptors.resolve(original_token)? != expected_binding {
            return Err(DirectRestoreError::DescriptorBinding);
        }
        self.cq.validate_completion(DirectCqKind::Original)?;
        self.descriptors.complete(original_token)?;
        self.cq.complete(DirectCqKind::Original)?;
        let ticket = self.ticket_mut(ticket_id)?;
        ticket.original_pending = false;
        ticket.original_token = None;

        match (state, completion) {
            (DirectRestoreState::ReadSubmitted, DirectReadCompletion::Exact) => {
                self.buffers
                    .transition(buffer, DirectBufferState::HashingForRead)?;
                let ticket = self.ticket_mut(ticket_id)?;
                if ticket.hash_state != DirectHashState::Reserved {
                    return Err(DirectRestoreError::HashBinding);
                }
                ticket.hash_state = DirectHashState::Queued;
                ticket.state = DirectRestoreState::DataReady;
                Ok(())
            }
            (DirectRestoreState::Abandoned, DirectReadCompletion::Exact)
            | (DirectRestoreState::Abandoned, DirectReadCompletion::Cancelled) => {
                self.finish_abandoned_if_reaped(ticket_id)
            }
            (_, DirectReadCompletion::Failed)
            | (DirectRestoreState::ReadSubmitted, DirectReadCompletion::Cancelled) => {
                self.fail_ticket(ticket_id)
            }
            _ => Err(DirectRestoreError::State),
        }
    }

    pub fn complete_cancel(
        &mut self,
        ticket_id: DirectRestoreTicketId,
    ) -> Result<(), DirectRestoreError> {
        let ticket = self.ticket(ticket_id)?;
        if !ticket.cancel_pending
            || !matches!(
                ticket.state,
                DirectRestoreState::Abandoned | DirectRestoreState::Failed
            )
        {
            return Err(DirectRestoreError::DuplicateCompletion);
        }
        let cancel_token = ticket.cancel_token.ok_or(DirectRestoreError::State)?;
        let buffer = ticket.buffer.ok_or(DirectRestoreError::State)?;
        let expected_binding = DirectDescriptorBinding {
            buffer,
            operation_generation: ticket_id.0,
            operation: DirectOperationKind::Read,
        };
        if self.descriptors.resolve(cancel_token)? != expected_binding {
            return Err(DirectRestoreError::DescriptorBinding);
        }
        self.cq.validate_completion(DirectCqKind::AsyncCancel)?;
        self.descriptors.complete(cancel_token)?;
        self.cq.complete(DirectCqKind::AsyncCancel)?;
        let ticket = self.ticket_mut(ticket_id)?;
        ticket.cancel_pending = false;
        ticket.cancel_token = None;
        if self.ticket(ticket_id)?.original_pending {
            return Ok(());
        }
        if self.ticket(ticket_id)?.state == DirectRestoreState::Abandoned {
            self.finish_abandoned_if_reaped(ticket_id)
        } else {
            self.remove_ticket(ticket_id)
        }
    }

    pub fn next_hash_job(&mut self) -> Result<Option<DirectHashJob>, DirectRestoreError> {
        let Some((&ticket_id, ticket)) = self
            .tickets
            .iter()
            .find(|(_, ticket)| ticket.hash_state == DirectHashState::Queued)
        else {
            return Ok(None);
        };
        if !matches!(
            ticket.state,
            DirectRestoreState::DataReady | DirectRestoreState::Abandoned
        ) || ticket.original_pending
            || ticket.cancel_pending
        {
            return Err(DirectRestoreError::HashBinding);
        }
        let job = DirectHashJob {
            ticket: ticket_id,
            buffer: ticket.buffer.ok_or(DirectRestoreError::HashBinding)?,
        };
        self.ticket_mut(ticket_id)?.hash_state = DirectHashState::Running;
        Ok(Some(job))
    }

    pub fn run_hash_job(&self, job: DirectHashJob) -> Result<DirectHashResult, DirectRestoreError> {
        let ticket = self.ticket(job.ticket)?;
        if ticket.hash_state != DirectHashState::Running
            || ticket.buffer != Some(job.buffer)
            || !matches!(
                ticket.state,
                DirectRestoreState::DataReady | DirectRestoreState::Abandoned
            )
            || ticket.original_pending
            || ticket.cancel_pending
        {
            return Err(DirectRestoreError::HashBinding);
        }
        let physical_length = usize::try_from(ticket.record.physical_length)
            .map_err(|_| DirectRestoreError::Overflow)?;
        let bytes = self
            .buffers
            .bytes(job.buffer)?
            .get(..physical_length)
            .ok_or(DirectRestoreError::Record)?;
        let verified = decode_direct_extent(&ticket.record, bytes).is_ok();
        Ok(DirectHashResult { job, verified })
    }

    pub fn complete_hash(&mut self, result: DirectHashResult) -> Result<(), DirectRestoreError> {
        let ticket_id = result.job.ticket;
        let ticket = self.ticket(ticket_id)?;
        if !matches!(
            ticket.state,
            DirectRestoreState::DataReady | DirectRestoreState::Abandoned
        ) || ticket.original_pending
            || ticket.cancel_pending
            || ticket.hash_state != DirectHashState::Running
            || ticket.buffer != Some(result.job.buffer)
        {
            return Err(DirectRestoreError::HashBinding);
        }
        let state = ticket.state;
        let buffer = ticket.buffer.ok_or(DirectRestoreError::State)?;
        if !result.verified {
            self.fail_ticket(ticket_id)?;
            return Err(DirectRestoreError::Integrity);
        }
        self.buffers
            .transition(buffer, DirectBufferState::HostReady)?;
        self.release_hash_job(ticket_id)?;
        if state == DirectRestoreState::Abandoned {
            self.buffers.transition(buffer, DirectBufferState::Free)?;
            self.remove_ticket(ticket_id)
        } else {
            self.ticket_mut(ticket_id)?.state = DirectRestoreState::HashVerified;
            Ok(())
        }
    }

    pub fn finish_cpu_delivery(
        &mut self,
        ticket_id: DirectRestoreTicketId,
    ) -> Result<Vec<u64>, DirectRestoreError> {
        let ticket = self.ticket(ticket_id)?;
        if ticket.state != DirectRestoreState::HashVerified
            || ticket.original_pending
            || ticket.cancel_pending
            || ticket.hash_state != DirectHashState::None
        {
            return Err(DirectRestoreError::State);
        }
        let request_ids = ticket.waiters.keys().copied().collect::<Vec<_>>();
        let buffer = ticket.buffer.ok_or(DirectRestoreError::State)?;
        self.buffers.transition(buffer, DirectBufferState::Free)?;
        self.remove_ticket(ticket_id)?;
        Ok(request_ids)
    }

    pub fn catalog_binding(
        &self,
        ticket_id: DirectRestoreTicketId,
        catalog_epoch: u64,
        catalog_record_sha256: [u8; 32],
    ) -> Result<DirectCatalogBinding, DirectRestoreError> {
        let ticket = self.ticket(ticket_id)?;
        if ticket.catalog_epoch == catalog_epoch
            && ticket.catalog_record_sha256 == catalog_record_sha256
        {
            return Ok(DirectCatalogBinding::Exact);
        }
        Ok(match ticket.state {
            DirectRestoreState::Planned | DirectRestoreState::BufferReserved => {
                DirectCatalogBinding::ReplanRequired
            }
            DirectRestoreState::ReadSubmitted
            | DirectRestoreState::DataReady
            | DirectRestoreState::HashVerified
            | DirectRestoreState::Abandoned
            | DirectRestoreState::Failed => DirectCatalogBinding::SubmittedRecordPinned,
        })
    }

    pub fn state(
        &self,
        ticket_id: DirectRestoreTicketId,
    ) -> Result<DirectRestoreState, DirectRestoreError> {
        Ok(self.ticket(ticket_id)?.state)
    }

    pub fn waiter_order(
        &self,
        ticket_id: DirectRestoreTicketId,
    ) -> Result<Vec<u64>, DirectRestoreError> {
        Ok(self.ticket(ticket_id)?.waiters.keys().copied().collect())
    }

    #[must_use]
    pub const fn physical_bytes(&self) -> u64 {
        self.physical_bytes
    }

    #[must_use]
    pub fn tenant_logical_bytes(&self, tenant_id: u64) -> u64 {
        self.logical_by_tenant.get(&tenant_id).copied().unwrap_or(0)
    }

    #[must_use]
    pub fn ticket_count(&self) -> usize {
        self.tickets.len()
    }

    #[must_use]
    pub fn waiter_count(&self) -> usize {
        self.request_to_ticket.len()
    }

    #[must_use]
    pub fn active_buffers(&self) -> usize {
        self.buffers.active_slots()
    }

    #[must_use]
    pub fn quarantined_buffers(&self) -> usize {
        self.buffers.quarantined_slots()
    }

    #[must_use]
    pub const fn outstanding_cqes(&self) -> u32 {
        self.cq.outstanding()
    }

    #[must_use]
    pub fn outstanding_descriptors(&self) -> usize {
        self.descriptors.outstanding_descriptors()
    }

    #[must_use]
    pub const fn active_hash_jobs(&self) -> u32 {
        self.active_hash_jobs
    }

    #[must_use]
    pub fn queued_hash_jobs(&self) -> usize {
        self.tickets
            .values()
            .filter(|ticket| ticket.hash_state == DirectHashState::Queued)
            .count()
    }

    #[must_use]
    pub fn running_hash_jobs(&self) -> usize {
        self.tickets
            .values()
            .filter(|ticket| ticket.hash_state == DirectHashState::Running)
            .count()
    }

    pub fn validate_invariants(&self) -> Result<(), DirectRestoreError> {
        let maximum_waiters = self
            .tickets
            .len()
            .checked_mul(self.config.maximum_waiters_per_ticket as usize)
            .ok_or(DirectRestoreError::Overflow)?;
        let expected_physical = self.tickets.values().try_fold(0_u64, |total, ticket| {
            total
                .checked_add(ticket.record.physical_length)
                .ok_or(DirectRestoreError::Overflow)
        })?;
        if expected_physical != self.physical_bytes
            || self.physical_bytes > self.config.maximum_physical_bytes
            || self.tickets.len() > self.config.maximum_tickets as usize
            || self.request_to_ticket.len() > maximum_waiters
        {
            return Err(DirectRestoreError::Accounting);
        }

        let mut expected_logical = BTreeMap::<u64, u64>::new();
        let mut expected_descriptors = 0_usize;
        let mut expected_cqes = 0_u32;
        let mut expected_active_buffers = 0_usize;
        let mut expected_hash_jobs = 0_u32;
        for (&ticket_id, ticket) in &self.tickets {
            ticket
                .record
                .validate()
                .map_err(|_| DirectRestoreError::Record)?;
            if ticket.catalog_epoch == 0
                || ticket.catalog_record_sha256 == [0; 32]
                || ticket.waiters.len() > self.config.maximum_waiters_per_ticket as usize
                || (ticket.waiters.is_empty()
                    && !matches!(
                        ticket.state,
                        DirectRestoreState::Abandoned | DirectRestoreState::Failed
                    ))
            {
                return Err(DirectRestoreError::State);
            }
            for (&request_id, waiter) in &ticket.waiters {
                if request_id == 0
                    || waiter.tenant_id == 0
                    || !capability_satisfies(ticket.record.capability, waiter.required_capability)
                    || self.request_to_ticket.get(&request_id) != Some(&ticket_id)
                {
                    return Err(DirectRestoreError::Accounting);
                }
                let charge = expected_logical.entry(waiter.tenant_id).or_default();
                *charge = charge
                    .checked_add(waiter.required_capability.logical_bytes())
                    .ok_or(DirectRestoreError::Overflow)?;
            }

            if ticket.original_pending != ticket.original_token.is_some()
                || ticket.cancel_pending != ticket.cancel_token.is_some()
            {
                return Err(DirectRestoreError::DescriptorBinding);
            }
            let expected_binding = ticket.buffer.map(|buffer| DirectDescriptorBinding {
                buffer,
                operation_generation: ticket_id.0,
                operation: DirectOperationKind::Read,
            });
            for token in [ticket.original_token, ticket.cancel_token]
                .into_iter()
                .flatten()
            {
                if self.descriptors.resolve(token)?
                    != expected_binding.ok_or(DirectRestoreError::DescriptorBinding)?
                {
                    return Err(DirectRestoreError::DescriptorBinding);
                }
                expected_cqes = expected_cqes
                    .checked_add(1)
                    .ok_or(DirectRestoreError::Overflow)?;
            }
            if ticket.original_pending || ticket.cancel_pending {
                expected_descriptors += 1;
            }
            if ticket.hash_state != DirectHashState::None {
                expected_hash_jobs = expected_hash_jobs
                    .checked_add(1)
                    .ok_or(DirectRestoreError::Overflow)?;
            }

            let buffer_state = ticket
                .buffer
                .map(|buffer| self.buffers.state(buffer))
                .transpose()?;
            let state_matches = match ticket.state {
                DirectRestoreState::Planned => {
                    buffer_state.is_none()
                        && !ticket.original_pending
                        && !ticket.cancel_pending
                        && ticket.hash_state == DirectHashState::None
                }
                DirectRestoreState::BufferReserved => {
                    buffer_state == Some(DirectBufferState::ReadQueued)
                        && !ticket.original_pending
                        && !ticket.cancel_pending
                        && ticket.hash_state == DirectHashState::None
                }
                DirectRestoreState::ReadSubmitted => {
                    ticket.original_pending
                        && !ticket.cancel_pending
                        && ticket.hash_state == DirectHashState::Reserved
                        && buffer_state == Some(DirectBufferState::ReadInflight)
                }
                DirectRestoreState::DataReady => {
                    !ticket.original_pending
                        && !ticket.cancel_pending
                        && matches!(
                            ticket.hash_state,
                            DirectHashState::Queued | DirectHashState::Running
                        )
                        && buffer_state == Some(DirectBufferState::HashingForRead)
                }
                DirectRestoreState::HashVerified => {
                    !ticket.original_pending
                        && !ticket.cancel_pending
                        && ticket.hash_state == DirectHashState::None
                        && buffer_state == Some(DirectBufferState::HostReady)
                }
                DirectRestoreState::Abandoned => {
                    ticket.waiters.is_empty()
                        && matches!(
                            (buffer_state, ticket.hash_state),
                            (
                                Some(DirectBufferState::ReadInflight),
                                DirectHashState::Reserved
                            ) | (
                                Some(DirectBufferState::HashingForRead),
                                DirectHashState::Queued | DirectHashState::Running
                            )
                        )
                }
                DirectRestoreState::Failed => {
                    ticket.waiters.is_empty()
                        && !ticket.original_pending
                        && ticket.hash_state == DirectHashState::None
                        && buffer_state == Some(DirectBufferState::Quarantined)
                }
            };
            if !state_matches {
                return Err(DirectRestoreError::State);
            }
            if buffer_state.is_some_and(DirectBufferState::is_active) {
                expected_active_buffers += 1;
            }
        }
        let waiter_total = self
            .tickets
            .values()
            .map(|ticket| ticket.waiters.len())
            .sum::<usize>();
        if self.request_to_ticket.len() != waiter_total
            || expected_logical != self.logical_by_tenant
            || expected_logical
                .values()
                .any(|&bytes| bytes > self.config.maximum_logical_bytes_per_tenant)
            || expected_descriptors != self.descriptors.outstanding_descriptors()
            || expected_cqes != self.cq.outstanding()
            || expected_active_buffers != self.buffers.active_slots()
            || expected_hash_jobs != self.active_hash_jobs
            || self.active_hash_jobs > self.config.maximum_hash_jobs
        {
            return Err(DirectRestoreError::Accounting);
        }
        Ok(())
    }

    fn compatible_ticket(
        &self,
        record: &DirectExtentRecord,
        catalog_epoch: u64,
        catalog_record_sha256: [u8; 32],
        required_capability: DirectTierCapability,
    ) -> Option<DirectRestoreTicketId> {
        self.tickets.iter().find_map(|(&ticket_id, ticket)| {
            (ticket.record == *record
                && ticket.catalog_epoch == catalog_epoch
                && ticket.catalog_record_sha256 == catalog_record_sha256
                && capability_satisfies(ticket.record.capability, required_capability)
                && !matches!(
                    ticket.state,
                    DirectRestoreState::Abandoned | DirectRestoreState::Failed
                ))
            .then_some(ticket_id)
        })
    }

    fn validate_tenant_charge(&self, tenant_id: u64, bytes: u64) -> Result<(), DirectRestoreError> {
        let next = self
            .tenant_logical_bytes(tenant_id)
            .checked_add(bytes)
            .ok_or(DirectRestoreError::Overflow)?;
        if next > self.config.maximum_logical_bytes_per_tenant {
            return Err(DirectRestoreError::TenantCapacity);
        }
        Ok(())
    }

    fn charge_tenant(&mut self, tenant_id: u64, bytes: u64) -> Result<(), DirectRestoreError> {
        self.validate_tenant_charge(tenant_id, bytes)?;
        let entry = self.logical_by_tenant.entry(tenant_id).or_default();
        *entry = entry
            .checked_add(bytes)
            .ok_or(DirectRestoreError::Overflow)?;
        Ok(())
    }

    fn uncharge_tenant(&mut self, tenant_id: u64, bytes: u64) -> Result<(), DirectRestoreError> {
        let entry = self
            .logical_by_tenant
            .get_mut(&tenant_id)
            .ok_or(DirectRestoreError::Accounting)?;
        *entry = entry
            .checked_sub(bytes)
            .ok_or(DirectRestoreError::Accounting)?;
        if *entry == 0 {
            self.logical_by_tenant.remove(&tenant_id);
        }
        Ok(())
    }

    fn clear_waiters(
        &mut self,
        ticket_id: DirectRestoreTicketId,
    ) -> Result<(), DirectRestoreError> {
        let waiters = self
            .ticket(ticket_id)?
            .waiters
            .iter()
            .map(|(&request_id, waiter)| (request_id, *waiter))
            .collect::<Vec<_>>();
        for (request_id, waiter) in waiters {
            self.uncharge_tenant(waiter.tenant_id, waiter.required_capability.logical_bytes())?;
            self.request_to_ticket.remove(&request_id);
        }
        self.ticket_mut(ticket_id)?.waiters.clear();
        Ok(())
    }

    fn fail_ticket(&mut self, ticket_id: DirectRestoreTicketId) -> Result<(), DirectRestoreError> {
        let ticket = self.ticket(ticket_id)?;
        let buffer = ticket.buffer.ok_or(DirectRestoreError::State)?;
        let current = self.buffers.state(buffer)?;
        if current != DirectBufferState::Quarantined {
            self.buffers.fail(buffer)?;
            self.buffers.quarantine(buffer)?;
        }
        self.release_hash_job(ticket_id)?;
        self.clear_waiters(ticket_id)?;
        self.ticket_mut(ticket_id)?.state = DirectRestoreState::Failed;
        let ticket = self.ticket(ticket_id)?;
        if !ticket.original_pending && !ticket.cancel_pending {
            self.remove_ticket(ticket_id)?;
        }
        Ok(())
    }

    fn finish_abandoned_if_reaped(
        &mut self,
        ticket_id: DirectRestoreTicketId,
    ) -> Result<(), DirectRestoreError> {
        let ticket = self.ticket(ticket_id)?;
        if ticket.state != DirectRestoreState::Abandoned {
            return Err(DirectRestoreError::State);
        }
        if ticket.original_pending || ticket.cancel_pending {
            return Ok(());
        }
        let buffer = ticket.buffer.ok_or(DirectRestoreError::State)?;
        self.release_hash_job(ticket_id)?;
        self.buffers.release_abandoned_read(buffer)?;
        self.remove_ticket(ticket_id)
    }

    fn release_hash_job(
        &mut self,
        ticket_id: DirectRestoreTicketId,
    ) -> Result<(), DirectRestoreError> {
        let hash_state = self.ticket(ticket_id)?.hash_state;
        if hash_state == DirectHashState::None {
            return Ok(());
        }
        self.active_hash_jobs = self
            .active_hash_jobs
            .checked_sub(1)
            .ok_or(DirectRestoreError::Accounting)?;
        self.ticket_mut(ticket_id)?.hash_state = DirectHashState::None;
        Ok(())
    }

    fn remove_ticket(
        &mut self,
        ticket_id: DirectRestoreTicketId,
    ) -> Result<(), DirectRestoreError> {
        let ticket = self
            .tickets
            .remove(&ticket_id)
            .ok_or(DirectRestoreError::MissingTicket)?;
        if ticket.original_pending || ticket.cancel_pending {
            self.tickets.insert(ticket_id, ticket);
            return Err(DirectRestoreError::CompletionOutstanding);
        }
        if ticket.original_token.is_some() || ticket.cancel_token.is_some() {
            self.tickets.insert(ticket_id, ticket);
            return Err(DirectRestoreError::DescriptorBinding);
        }
        if ticket.hash_state != DirectHashState::None {
            self.tickets.insert(ticket_id, ticket);
            return Err(DirectRestoreError::HashBinding);
        }
        for (request_id, waiter) in ticket.waiters {
            self.request_to_ticket.remove(&request_id);
            self.uncharge_tenant(waiter.tenant_id, waiter.required_capability.logical_bytes())?;
        }
        self.physical_bytes = self
            .physical_bytes
            .checked_sub(ticket.record.physical_length)
            .ok_or(DirectRestoreError::Accounting)?;
        Ok(())
    }

    fn ticket(
        &self,
        ticket_id: DirectRestoreTicketId,
    ) -> Result<&DirectRestoreTicket, DirectRestoreError> {
        self.tickets
            .get(&ticket_id)
            .ok_or(DirectRestoreError::MissingTicket)
    }

    fn ticket_mut(
        &mut self,
        ticket_id: DirectRestoreTicketId,
    ) -> Result<&mut DirectRestoreTicket, DirectRestoreError> {
        self.tickets
            .get_mut(&ticket_id)
            .ok_or(DirectRestoreError::MissingTicket)
    }
}

fn validate_restore_request(request: DirectRestoreRequest) -> Result<(), DirectRestoreError> {
    if request.request_id == 0 || request.tenant_id == 0 {
        return Err(DirectRestoreError::Request);
    }
    Ok(())
}

const fn capability_satisfies(
    available: DirectTierCapability,
    required: DirectTierCapability,
) -> bool {
    matches!(
        (available, required),
        (DirectTierCapability::Mtp, _)
            | (DirectTierCapability::Target, DirectTierCapability::Target)
    )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectRestoreError {
    Config,
    Request,
    Record,
    Catalog,
    Capability,
    DuplicateRequest,
    MissingRequest,
    MissingTicket,
    TicketCapacity,
    WaiterCapacity,
    PhysicalCapacity,
    TenantCapacity,
    State,
    DuplicateCompletion,
    CompletionOutstanding,
    Integrity,
    CqConfig,
    CqWait,
    CqUnderflow,
    HashWait,
    HashBinding,
    Accounting,
    Overflow,
    Buffer(DirectBufferStateError),
    Descriptor(DirectDescriptorError),
    DescriptorBinding,
}

impl From<DirectBufferStateError> for DirectRestoreError {
    fn from(value: DirectBufferStateError) -> Self {
        Self::Buffer(value)
    }
}

impl From<DirectDescriptorError> for DirectRestoreError {
    fn from(value: DirectDescriptorError) -> Self {
        Self::Descriptor(value)
    }
}

impl std::fmt::Display for DirectRestoreError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for DirectRestoreError {}

#[cfg(test)]
mod tests {
    use crate::{
        DIRECT_TIER_FORMAT_VERSION, DRAFT_SIDECAR_EXTENT_LENGTH, DRAFT_SIDECAR_EXTENT_OFFSET,
        MTP_PHYSICAL_BYTES, TARGET_INDEXER_EXTENT_LENGTH, TARGET_INDEXER_EXTENT_OFFSET,
        TARGET_KV_EXTENT_LENGTH, TARGET_KV_EXTENT_OFFSET, TARGET_ONLY_PHYSICAL_BYTES, TierPiece,
    };
    use sha2::{Digest, Sha256};

    use super::*;

    fn zero_sha256(length: u64) -> [u8; 32] {
        let zeros = [0_u8; 4_096];
        let mut remaining = length;
        let mut hasher = Sha256::new();
        while remaining != 0 {
            let bytes = usize::try_from(remaining.min(zeros.len() as u64)).unwrap();
            hasher.update(&zeros[..bytes]);
            remaining -= bytes as u64;
        }
        hasher.finalize().into()
    }

    fn record(capability: DirectTierCapability, key: u8) -> DirectExtentRecord {
        let mut pieces = vec![
            crate::DirectPieceRecord {
                piece: TierPiece::TargetKv,
                extent_offset: TARGET_KV_EXTENT_OFFSET,
                logical_length: TARGET_KV_EXTENT_LENGTH,
                sha256: zero_sha256(TARGET_KV_EXTENT_LENGTH),
            },
            crate::DirectPieceRecord {
                piece: TierPiece::TargetIndexer,
                extent_offset: TARGET_INDEXER_EXTENT_OFFSET,
                logical_length: TARGET_INDEXER_EXTENT_LENGTH,
                sha256: zero_sha256(TARGET_INDEXER_EXTENT_LENGTH),
            },
        ];
        if capability == DirectTierCapability::Mtp {
            pieces.push(crate::DirectPieceRecord {
                piece: TierPiece::DraftSidecar,
                extent_offset: DRAFT_SIDECAR_EXTENT_OFFSET,
                logical_length: DRAFT_SIDECAR_EXTENT_LENGTH,
                sha256: zero_sha256(DRAFT_SIDECAR_EXTENT_LENGTH),
            });
        }
        DirectExtentRecord {
            format_version: DIRECT_TIER_FORMAT_VERSION,
            namespace: [0x11; 32],
            page_key: [key; 32],
            durable_revision: 7,
            capability,
            segment_id: 3,
            physical_offset: 4_096 * u64::from(key),
            physical_length: match capability {
                DirectTierCapability::Target => TARGET_ONLY_PHYSICAL_BYTES,
                DirectTierCapability::Mtp => MTP_PHYSICAL_BYTES,
            },
            physical_sha256: zero_sha256(capability.physical_bytes()),
            pieces,
        }
    }

    fn config() -> DirectRestoreConfig {
        DirectRestoreConfig {
            maximum_tickets: 8,
            maximum_waiters_per_ticket: 4,
            maximum_hash_jobs: 4,
            maximum_physical_bytes: MTP_PHYSICAL_BYTES * 8,
            maximum_logical_bytes_per_tenant: crate::MTP_LOGICAL_BYTES * 4,
            buffer_slots: 4,
            descriptor_capacity: 4,
        }
    }

    fn request(
        request_id: u64,
        tenant_id: u64,
        required_capability: DirectTierCapability,
    ) -> DirectRestoreRequest {
        DirectRestoreRequest {
            request_id,
            tenant_id,
            required_capability,
        }
    }

    fn plan(
        table: &mut DirectRestoreTable,
        request: DirectRestoreRequest,
        record: DirectExtentRecord,
    ) -> DirectRestoreTicketId {
        table.plan(request, record, 5, [0x51; 32]).unwrap().ticket()
    }

    fn submit(table: &mut DirectRestoreTable, ticket: DirectRestoreTicketId) {
        table.reserve_buffer(ticket).unwrap();
        table.submit_read(ticket).unwrap();
        table.validate_invariants().unwrap();
    }

    fn verify_next_hash(table: &mut DirectRestoreTable, expected_ticket: DirectRestoreTicketId) {
        let job = table.next_hash_job().unwrap().unwrap();
        assert_eq!(job.ticket(), expected_ticket);
        let result = table.run_hash_job(job).unwrap();
        assert!(result.verified());
        table.complete_hash(result).unwrap();
    }

    fn assert_valid(table: &DirectRestoreTable) {
        table.validate_invariants().unwrap();
    }

    #[test]
    fn same_and_cross_tenant_waiters_share_one_physical_ticket() {
        let mut table = DirectRestoreTable::new(config(), false).unwrap();
        let source = record(DirectTierCapability::Mtp, 1);
        let ticket = plan(
            &mut table,
            request(30, 1, DirectTierCapability::Target),
            source.clone(),
        );
        assert_eq!(
            table
                .plan(
                    request(10, 1, DirectTierCapability::Mtp),
                    source.clone(),
                    5,
                    [0x51; 32]
                )
                .unwrap(),
            DirectRestoreAdmission::Joined(ticket)
        );
        assert_eq!(
            table
                .plan(
                    request(20, 2, DirectTierCapability::Target),
                    source,
                    5,
                    [0x51; 32]
                )
                .unwrap(),
            DirectRestoreAdmission::Joined(ticket)
        );
        assert_eq!(table.ticket_count(), 1);
        assert_eq!(table.physical_bytes(), MTP_PHYSICAL_BYTES);
        assert_eq!(
            table.tenant_logical_bytes(1),
            crate::TARGET_ONLY_LOGICAL_BYTES + crate::MTP_LOGICAL_BYTES
        );
        assert_eq!(
            table.tenant_logical_bytes(2),
            crate::TARGET_ONLY_LOGICAL_BYTES
        );
        assert_eq!(table.waiter_order(ticket).unwrap(), vec![10, 20, 30]);
        assert_valid(&table);

        submit(&mut table, ticket);
        table
            .complete_original(ticket, DirectReadCompletion::Exact)
            .unwrap();
        assert_valid(&table);
        verify_next_hash(&mut table, ticket);
        assert_valid(&table);
        assert_eq!(table.finish_cpu_delivery(ticket).unwrap(), vec![10, 20, 30]);
        assert_eq!(table.ticket_count(), 0);
        assert_eq!(table.waiter_count(), 0);
        assert_eq!(table.physical_bytes(), 0);
        assert_eq!(table.tenant_logical_bytes(1), 0);
        assert_eq!(table.tenant_logical_bytes(2), 0);
        assert_eq!(table.active_buffers(), 0);
        assert_eq!(table.outstanding_cqes(), 0);
        assert_eq!(table.outstanding_descriptors(), 0);
    }

    #[test]
    fn capability_lattice_never_uses_target_for_mtp() {
        let mut table = DirectRestoreTable::new(config(), false).unwrap();
        let target = record(DirectTierCapability::Target, 2);
        assert_eq!(
            table.plan(
                request(1, 1, DirectTierCapability::Mtp),
                target.clone(),
                5,
                [0x51; 32]
            ),
            Err(DirectRestoreError::Capability)
        );
        let target_ticket = plan(
            &mut table,
            request(2, 1, DirectTierCapability::Target),
            target,
        );
        let mtp_ticket = plan(
            &mut table,
            request(3, 1, DirectTierCapability::Mtp),
            record(DirectTierCapability::Mtp, 2),
        );
        assert_ne!(target_ticket, mtp_ticket);
        assert_eq!(table.ticket_count(), 2);
    }

    #[test]
    fn cancellation_before_submit_and_with_remaining_waiter_releases_exact_charges() {
        let mut table = DirectRestoreTable::new(config(), false).unwrap();
        let source = record(DirectTierCapability::Target, 3);
        let _ticket = plan(
            &mut table,
            request(1, 1, DirectTierCapability::Target),
            source.clone(),
        );
        plan(
            &mut table,
            request(2, 2, DirectTierCapability::Target),
            source,
        );
        assert_eq!(
            table.cancel_waiter(1, true).unwrap(),
            DirectCancellation::WaiterRemoved
        );
        assert_eq!(table.physical_bytes(), TARGET_ONLY_PHYSICAL_BYTES);
        assert_eq!(table.tenant_logical_bytes(1), 0);
        assert_eq!(
            table.cancel_waiter(2, true).unwrap(),
            DirectCancellation::ReleasedBeforeSubmission
        );
        assert_eq!(table.ticket_count(), 0);
        assert_eq!(table.physical_bytes(), 0);

        let ticket = plan(
            &mut table,
            request(3, 1, DirectTierCapability::Target),
            record(DirectTierCapability::Target, 4),
        );
        table.reserve_buffer(ticket).unwrap();
        assert_eq!(
            table.cancel_waiter(3, true).unwrap(),
            DirectCancellation::ReleasedBeforeSubmission
        );
        assert_eq!(table.active_buffers(), 0);
    }

    #[test]
    fn original_then_cancel_and_cancel_then_original_never_reuse_early() {
        for original_first in [false, true] {
            let mut table = DirectRestoreTable::new(config(), false).unwrap();
            let ticket = plan(
                &mut table,
                request(1, 1, DirectTierCapability::Target),
                record(DirectTierCapability::Target, 5),
            );
            submit(&mut table, ticket);
            assert_eq!(
                table.cancel_waiter(1, true).unwrap(),
                DirectCancellation::AsyncCancelSubmitted
            );
            assert_eq!(table.active_buffers(), 1);
            assert_eq!(table.outstanding_cqes(), 2);
            assert_eq!(table.outstanding_descriptors(), 1);
            assert_valid(&table);
            if original_first {
                table
                    .complete_original(ticket, DirectReadCompletion::Cancelled)
                    .unwrap();
                assert_eq!(table.active_buffers(), 1);
                assert_eq!(table.physical_bytes(), TARGET_ONLY_PHYSICAL_BYTES);
                assert_valid(&table);
                table.complete_cancel(ticket).unwrap();
            } else {
                table.complete_cancel(ticket).unwrap();
                assert_eq!(table.active_buffers(), 1);
                assert_eq!(table.physical_bytes(), TARGET_ONLY_PHYSICAL_BYTES);
                assert_valid(&table);
                table
                    .complete_original(ticket, DirectReadCompletion::Cancelled)
                    .unwrap();
            }
            assert_eq!(table.ticket_count(), 0);
            assert_eq!(table.active_buffers(), 0);
            assert_eq!(table.physical_bytes(), 0);
            assert_eq!(table.outstanding_cqes(), 0);
            assert_eq!(table.outstanding_descriptors(), 0);
        }
    }

    #[test]
    fn logical_abandonment_without_async_cancel_waits_for_original() {
        let mut table = DirectRestoreTable::new(config(), false).unwrap();
        let ticket = plan(
            &mut table,
            request(1, 1, DirectTierCapability::Target),
            record(DirectTierCapability::Target, 11),
        );
        submit(&mut table, ticket);
        assert_eq!(
            table.cancel_waiter(1, false).unwrap(),
            DirectCancellation::AbandonedWithoutAsyncCancel
        );
        assert_eq!(table.ticket_count(), 1);
        assert_eq!(table.active_buffers(), 1);
        assert_eq!(table.outstanding_cqes(), 1);
        assert_eq!(table.outstanding_descriptors(), 1);
        assert_valid(&table);
        table
            .complete_original(ticket, DirectReadCompletion::Exact)
            .unwrap();
        assert_eq!(table.ticket_count(), 0);
        assert_eq!(table.active_buffers(), 0);
        assert_eq!(table.physical_bytes(), 0);
        assert_eq!(table.outstanding_cqes(), 0);
        assert_eq!(table.outstanding_descriptors(), 0);
    }

    #[test]
    fn cancellation_after_cqe_waits_for_hash_acknowledgement() {
        let mut table = DirectRestoreTable::new(config(), false).unwrap();
        let ticket = plan(
            &mut table,
            request(1, 1, DirectTierCapability::Target),
            record(DirectTierCapability::Target, 6),
        );
        submit(&mut table, ticket);
        table
            .complete_original(ticket, DirectReadCompletion::Exact)
            .unwrap();
        assert_eq!(
            table.cancel_waiter(1, true).unwrap(),
            DirectCancellation::WaitingForHashAcknowledgement
        );
        assert_eq!(table.active_buffers(), 1);
        verify_next_hash(&mut table, ticket);
        assert_eq!(table.ticket_count(), 0);
        assert_eq!(table.active_buffers(), 0);
        assert_eq!(table.physical_bytes(), 0);
    }

    #[test]
    fn cancellation_after_hash_verification_releases_host_ready_buffer() {
        let mut table = DirectRestoreTable::new(config(), false).unwrap();
        let ticket = plan(
            &mut table,
            request(1, 1, DirectTierCapability::Target),
            record(DirectTierCapability::Target, 12),
        );
        submit(&mut table, ticket);
        table
            .complete_original(ticket, DirectReadCompletion::Exact)
            .unwrap();
        verify_next_hash(&mut table, ticket);
        assert_eq!(
            table.cancel_waiter(1, true).unwrap(),
            DirectCancellation::ReleasedAfterVerification
        );
        assert_eq!(table.ticket_count(), 0);
        assert_eq!(table.active_buffers(), 0);
        assert_eq!(table.waiter_count(), 0);
        assert_eq!(table.physical_bytes(), 0);
    }

    #[test]
    fn failed_read_quarantines_but_reaps_both_completion_orders() {
        for original_first in [false, true] {
            let mut table = DirectRestoreTable::new(config(), false).unwrap();
            let ticket = plan(
                &mut table,
                request(1, 1, DirectTierCapability::Target),
                record(DirectTierCapability::Target, 13),
            );
            submit(&mut table, ticket);
            assert_eq!(
                table.cancel_waiter(1, true).unwrap(),
                DirectCancellation::AsyncCancelSubmitted
            );
            if original_first {
                table
                    .complete_original(ticket, DirectReadCompletion::Failed)
                    .unwrap();
                assert_eq!(table.state(ticket).unwrap(), DirectRestoreState::Failed);
                assert_eq!(table.ticket_count(), 1);
                assert_valid(&table);
                table.complete_cancel(ticket).unwrap();
            } else {
                table.complete_cancel(ticket).unwrap();
                assert_valid(&table);
                table
                    .complete_original(ticket, DirectReadCompletion::Failed)
                    .unwrap();
            }
            assert_eq!(table.ticket_count(), 0);
            assert_eq!(table.waiter_count(), 0);
            assert_eq!(table.quarantined_buffers(), 1);
            assert_eq!(table.physical_bytes(), 0);
            assert_eq!(table.outstanding_cqes(), 0);
            assert_eq!(table.outstanding_descriptors(), 0);
        }
    }

    #[test]
    fn duplicate_original_and_cancel_completions_fail_closed() {
        let mut table = DirectRestoreTable::new(config(), false).unwrap();
        let ticket = plan(
            &mut table,
            request(1, 1, DirectTierCapability::Target),
            record(DirectTierCapability::Target, 14),
        );
        submit(&mut table, ticket);
        assert_eq!(
            table.cancel_waiter(1, true).unwrap(),
            DirectCancellation::AsyncCancelSubmitted
        );
        table.complete_cancel(ticket).unwrap();
        assert_eq!(
            table.complete_cancel(ticket),
            Err(DirectRestoreError::DuplicateCompletion)
        );
        table
            .complete_original(ticket, DirectReadCompletion::Cancelled)
            .unwrap();
        assert_eq!(
            table.complete_original(ticket, DirectReadCompletion::Cancelled),
            Err(DirectRestoreError::MissingTicket)
        );
    }

    #[test]
    fn integrity_failure_quarantines_and_releases_all_accounting() {
        let mut table = DirectRestoreTable::new(config(), false).unwrap();
        let ticket = plan(
            &mut table,
            request(1, 1, DirectTierCapability::Mtp),
            record(DirectTierCapability::Mtp, 7),
        );
        submit(&mut table, ticket);
        table.read_destination_mut(ticket).unwrap()[0] = 1;
        table
            .complete_original(ticket, DirectReadCompletion::Exact)
            .unwrap();
        let job = table.next_hash_job().unwrap().unwrap();
        let result = table.run_hash_job(job).unwrap();
        assert!(!result.verified());
        assert_eq!(
            table.complete_hash(result),
            Err(DirectRestoreError::Integrity)
        );
        assert_eq!(table.quarantined_buffers(), 1);
        assert_eq!(table.active_buffers(), 0);
        assert_eq!(table.ticket_count(), 0);
        assert_eq!(table.waiter_count(), 0);
        assert_eq!(table.physical_bytes(), 0);
        assert_eq!(table.tenant_logical_bytes(1), 0);
    }

    #[test]
    fn checksum_capacity_waits_before_read_submission_and_preserves_reservation() {
        let mut limited = config();
        limited.maximum_hash_jobs = 1;
        let mut table = DirectRestoreTable::new(limited, false).unwrap();
        let first = plan(
            &mut table,
            request(1, 1, DirectTierCapability::Target),
            record(DirectTierCapability::Target, 23),
        );
        let second = plan(
            &mut table,
            request(2, 2, DirectTierCapability::Target),
            record(DirectTierCapability::Target, 24),
        );
        table.reserve_buffer(first).unwrap();
        table.reserve_buffer(second).unwrap();
        table.submit_read(first).unwrap();
        assert_eq!(table.active_hash_jobs(), 1);
        assert_eq!(table.submit_read(second), Err(DirectRestoreError::HashWait));
        assert_eq!(
            table.state(second).unwrap(),
            DirectRestoreState::BufferReserved
        );
        assert_eq!(table.outstanding_descriptors(), 1);
        assert_eq!(table.outstanding_cqes(), 1);
        assert_valid(&table);

        table
            .complete_original(first, DirectReadCompletion::Exact)
            .unwrap();
        assert_eq!(table.queued_hash_jobs(), 1);
        let job = table.next_hash_job().unwrap().unwrap();
        assert_eq!(job.ticket(), first);
        assert_eq!(table.queued_hash_jobs(), 0);
        assert_eq!(table.running_hash_jobs(), 1);
        assert_eq!(table.submit_read(second), Err(DirectRestoreError::HashWait));
        let result = table.run_hash_job(job).unwrap();
        table.complete_hash(result).unwrap();
        table.finish_cpu_delivery(first).unwrap();
        assert_eq!(table.active_hash_jobs(), 0);

        table.submit_read(second).unwrap();
        table
            .complete_original(second, DirectReadCompletion::Exact)
            .unwrap();
        verify_next_hash(&mut table, second);
        table.finish_cpu_delivery(second).unwrap();
        assert_eq!(table.ticket_count(), 0);
        assert_valid(&table);
    }

    #[test]
    fn checksum_jobs_are_bound_to_the_exact_ticket_and_buffer_generation() {
        let mut table = DirectRestoreTable::new(config(), false).unwrap();
        let ticket = plan(
            &mut table,
            request(1, 1, DirectTierCapability::Target),
            record(DirectTierCapability::Target, 25),
        );
        submit(&mut table, ticket);
        table
            .complete_original(ticket, DirectReadCompletion::Exact)
            .unwrap();
        let job = table.next_hash_job().unwrap().unwrap();
        let wrong_generation = DirectHashJob {
            ticket,
            buffer: DirectBufferId {
                slot: job.buffer().slot,
                generation: job.buffer().generation + 1,
            },
        };
        assert_eq!(
            table.run_hash_job(wrong_generation),
            Err(DirectRestoreError::HashBinding)
        );
        let result = table.run_hash_job(job).unwrap();
        assert!(result.verified());
        table.complete_hash(result).unwrap();
        assert_eq!(
            table.complete_hash(result),
            Err(DirectRestoreError::HashBinding)
        );
        table.finish_cpu_delivery(ticket).unwrap();
        assert_eq!(
            table.run_hash_job(job),
            Err(DirectRestoreError::MissingTicket)
        );
        assert_eq!(table.active_hash_jobs(), 0);
        assert_valid(&table);
    }

    #[test]
    fn catalog_change_replans_before_submit_and_pins_after_submit() {
        let mut table = DirectRestoreTable::new(config(), false).unwrap();
        let ticket = plan(
            &mut table,
            request(1, 1, DirectTierCapability::Target),
            record(DirectTierCapability::Target, 8),
        );
        assert_eq!(
            table.catalog_binding(ticket, 6, [0x52; 32]).unwrap(),
            DirectCatalogBinding::ReplanRequired
        );
        table.reserve_buffer(ticket).unwrap();
        assert_eq!(
            table.catalog_binding(ticket, 6, [0x52; 32]).unwrap(),
            DirectCatalogBinding::ReplanRequired
        );
        table.submit_read(ticket).unwrap();
        assert_eq!(
            table.catalog_binding(ticket, 6, [0x52; 32]).unwrap(),
            DirectCatalogBinding::SubmittedRecordPinned
        );
        assert_eq!(
            table.catalog_binding(ticket, 5, [0x51; 32]).unwrap(),
            DirectCatalogBinding::Exact
        );
    }

    #[test]
    fn admission_limits_are_atomic() {
        let mut limited = config();
        limited.maximum_tickets = 1;
        limited.maximum_waiters_per_ticket = 1;
        limited.maximum_physical_bytes = TARGET_ONLY_PHYSICAL_BYTES;
        limited.maximum_logical_bytes_per_tenant = crate::TARGET_ONLY_LOGICAL_BYTES;
        let mut table = DirectRestoreTable::new(limited, false).unwrap();
        let first = record(DirectTierCapability::Target, 9);
        plan(
            &mut table,
            request(1, 1, DirectTierCapability::Target),
            first.clone(),
        );
        assert_eq!(
            table.plan(
                request(2, 2, DirectTierCapability::Target),
                first,
                5,
                [0x51; 32]
            ),
            Err(DirectRestoreError::WaiterCapacity)
        );
        assert_eq!(
            table.plan(
                request(3, 1, DirectTierCapability::Target),
                record(DirectTierCapability::Target, 10),
                5,
                [0x51; 32]
            ),
            Err(DirectRestoreError::TenantCapacity)
        );
        assert_eq!(table.ticket_count(), 1);
        assert_eq!(table.waiter_count(), 1);
        assert_eq!(table.physical_bytes(), TARGET_ONLY_PHYSICAL_BYTES);
        assert_eq!(
            table.tenant_logical_bytes(1),
            crate::TARGET_ONLY_LOGICAL_BYTES
        );
        assert_eq!(table.tenant_logical_bytes(2), 0);
        assert_valid(&table);

        let mut ticket_limited = config();
        ticket_limited.maximum_tickets = 1;
        let mut table = DirectRestoreTable::new(ticket_limited, false).unwrap();
        plan(
            &mut table,
            request(1, 1, DirectTierCapability::Target),
            record(DirectTierCapability::Target, 15),
        );
        assert_eq!(
            table.plan(
                request(2, 2, DirectTierCapability::Target),
                record(DirectTierCapability::Target, 16),
                5,
                [0x51; 32]
            ),
            Err(DirectRestoreError::TicketCapacity)
        );
        assert_valid(&table);

        let mut physical_limited = config();
        physical_limited.maximum_physical_bytes = TARGET_ONLY_PHYSICAL_BYTES;
        let mut table = DirectRestoreTable::new(physical_limited, false).unwrap();
        plan(
            &mut table,
            request(1, 1, DirectTierCapability::Target),
            record(DirectTierCapability::Target, 17),
        );
        assert_eq!(
            table.plan(
                request(2, 2, DirectTierCapability::Target),
                record(DirectTierCapability::Target, 18),
                5,
                [0x51; 32]
            ),
            Err(DirectRestoreError::PhysicalCapacity)
        );
        assert_valid(&table);
    }

    #[test]
    fn cq_arithmetic_is_identical_with_or_without_nodrop() {
        for nodrop in [false, true] {
            let mut cq = DirectCqTracker::new(2, 4, nodrop).unwrap();
            cq.try_submit(DirectCqKind::Original).unwrap();
            cq.try_submit(DirectCqKind::Original).unwrap();
            cq.try_submit(DirectCqKind::AsyncCancel).unwrap();
            cq.try_submit(DirectCqKind::Fsync).unwrap();
            assert_eq!(cq.outstanding(), cq.cq_entries());
            assert_eq!(
                cq.try_submit(DirectCqKind::AsyncCancel),
                Err(DirectRestoreError::CqWait)
            );
            assert_eq!(cq.nodrop_present(), nodrop);
            for kind in [
                DirectCqKind::AsyncCancel,
                DirectCqKind::Original,
                DirectCqKind::Original,
                DirectCqKind::Fsync,
            ] {
                cq.complete(kind).unwrap();
            }
            assert_eq!(cq.outstanding(), 0);
            assert_eq!(
                cq.complete(DirectCqKind::Original),
                Err(DirectRestoreError::CqUnderflow)
            );
        }
        assert_eq!(
            DirectCqTracker::new(2, 3, true),
            Err(DirectRestoreError::CqConfig)
        );
    }

    #[test]
    fn buffer_and_descriptor_waits_preserve_every_reservation() {
        let mut buffer_limited = config();
        buffer_limited.buffer_slots = 1;
        buffer_limited.maximum_hash_jobs = 1;
        let mut table = DirectRestoreTable::new(buffer_limited, false).unwrap();
        let first = plan(
            &mut table,
            request(1, 1, DirectTierCapability::Target),
            record(DirectTierCapability::Target, 19),
        );
        let second = plan(
            &mut table,
            request(2, 2, DirectTierCapability::Target),
            record(DirectTierCapability::Target, 20),
        );
        table.reserve_buffer(first).unwrap();
        assert_eq!(
            table.reserve_buffer(second),
            Err(DirectRestoreError::Buffer(DirectBufferStateError::Capacity))
        );
        assert_eq!(table.state(second).unwrap(), DirectRestoreState::Planned);
        assert_eq!(table.physical_bytes(), TARGET_ONLY_PHYSICAL_BYTES * 2);
        assert_valid(&table);
        table.cancel_waiter(1, true).unwrap();
        table.reserve_buffer(second).unwrap();
        table.cancel_waiter(2, true).unwrap();
        assert_eq!(table.ticket_count(), 0);

        let mut descriptor_limited = config();
        descriptor_limited.descriptor_capacity = 1;
        let mut table = DirectRestoreTable::new(descriptor_limited, false).unwrap();
        let first = plan(
            &mut table,
            request(1, 1, DirectTierCapability::Target),
            record(DirectTierCapability::Target, 21),
        );
        let second = plan(
            &mut table,
            request(2, 2, DirectTierCapability::Target),
            record(DirectTierCapability::Target, 22),
        );
        table.reserve_buffer(first).unwrap();
        table.reserve_buffer(second).unwrap();
        table.submit_read(first).unwrap();
        assert_eq!(
            table.submit_read(second),
            Err(DirectRestoreError::Descriptor(
                DirectDescriptorError::Capacity
            ))
        );
        assert_eq!(
            table.state(second).unwrap(),
            DirectRestoreState::BufferReserved
        );
        assert_eq!(table.outstanding_descriptors(), 1);
        assert_eq!(table.outstanding_cqes(), 1);
        assert_valid(&table);
        table
            .complete_original(first, DirectReadCompletion::Exact)
            .unwrap();
        verify_next_hash(&mut table, first);
        table.finish_cpu_delivery(first).unwrap();
        table.submit_read(second).unwrap();
        table
            .complete_original(second, DirectReadCompletion::Exact)
            .unwrap();
        verify_next_hash(&mut table, second);
        table.finish_cpu_delivery(second).unwrap();
        assert_eq!(table.ticket_count(), 0);
        assert_eq!(table.outstanding_descriptors(), 0);
        assert_eq!(table.outstanding_cqes(), 0);
    }
}
