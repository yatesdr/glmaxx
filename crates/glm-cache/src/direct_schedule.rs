use std::{
    cmp::Reverse,
    collections::{BinaryHeap, HashSet},
    hash::Hash,
};

pub const MAX_DIRECT_IO_QUEUED_COMMANDS: u32 = 65_536;
/// One W0 lease keeps room for its original completion and one possible
/// cancellation completion. Correctness never depends on `IORING_FEAT_NODROP`.
pub const DIRECT_PUBLICATION_CQ_RESERVATION: u32 = 2;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DirectIoClass {
    ResumeRead,
    AdmissionRead,
    PublicationWrite,
    CleanerWrite,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DirectIoOrderKey {
    pub service_epoch: u64,
    pub owner_id: u64,
    pub page_ordinal: u32,
    pub operation_ordinal: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectIoCommand {
    pub command_id: u64,
    pub class: DirectIoClass,
    pub order: DirectIoOrderKey,
    pub bytes: u64,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct QueuedCommand {
    order: DirectIoOrderKey,
    command_id: u64,
    bytes: u64,
}

impl From<DirectIoCommand> for QueuedCommand {
    fn from(command: DirectIoCommand) -> Self {
        Self {
            order: command.order,
            command_id: command.command_id,
            bytes: command.bytes,
        }
    }
}

impl QueuedCommand {
    fn into_command(self, class: DirectIoClass) -> DirectIoCommand {
        DirectIoCommand {
            command_id: self.command_id,
            class,
            order: self.order,
            bytes: self.bytes,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectIoSchedulerConfig {
    pub maximum_queued_commands: u32,
    pub total_buffers: u32,
    pub read_reserved_buffers: u32,
    pub total_descriptors: u32,
    pub read_reserved_descriptors: u32,
    pub total_cq_entries: u32,
    pub read_reserved_cq_entries: u32,
    pub read_low_watermark_bytes: u64,
    pub read_high_watermark_bytes: u64,
    pub publication_low_watermark_commands: u32,
    pub maximum_read_command_bytes: u64,
    pub maximum_resume_read_bytes_before_admission_read: u64,
    pub maximum_read_bytes_before_publication_admission: u64,
    pub maximum_read_bytes_before_publication_service: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectIoResources {
    pub free_buffers: u32,
    pub free_descriptors: u32,
    pub free_cq_entries: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectIoDecision {
    /// A publication candidate acquired one shared buffer, one descriptor,
    /// and two CQ slots. Those resources remain owned by the returned command
    /// until its terminal durability path releases them.
    PublicationAdmitted(DirectIoCommand),
    /// Already-admitted work selected for its next operation.
    Service(DirectIoCommand),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectIoSchedulerStats {
    pub resume_reads: u32,
    pub admission_reads: u32,
    pub publication_candidates: u32,
    pub admitted_publications: u32,
    pub cleaner_writes: u32,
    pub queued_read_bytes: u64,
    pub resume_read_bytes_since_admission_read_waited: u64,
    pub read_bytes_since_publication_candidate_waited: u64,
    pub read_bytes_since_admitted_publication_advanced: u64,
    pub publication_admitted_since_service: bool,
}

#[derive(Debug)]
pub struct DirectIoScheduler {
    config: DirectIoSchedulerConfig,
    resume_reads: BinaryHeap<Reverse<QueuedCommand>>,
    admission_reads: BinaryHeap<Reverse<QueuedCommand>>,
    publication_candidates: BinaryHeap<Reverse<QueuedCommand>>,
    admitted_publications: BinaryHeap<Reverse<QueuedCommand>>,
    cleaner_writes: BinaryHeap<Reverse<QueuedCommand>>,
    command_ids: HashSet<u64>,
    order_keys: HashSet<(DirectIoClass, DirectIoOrderKey)>,
    queued_commands: u32,
    queued_read_bytes: u64,
    resume_read_bytes_since_admission_read_waited: u64,
    read_bytes_since_publication_candidate_waited: u64,
    read_bytes_since_admitted_publication_advanced: u64,
    publication_admitted_since_service: bool,
}

impl DirectIoScheduler {
    pub fn new(config: DirectIoSchedulerConfig) -> Result<Self, DirectIoScheduleError> {
        validate_config(config)?;
        let capacity = usize::try_from(config.maximum_queued_commands)
            .map_err(|_| DirectIoScheduleError::Overflow)?;
        Ok(Self {
            config,
            resume_reads: preallocated_heap(capacity)?,
            admission_reads: preallocated_heap(capacity)?,
            publication_candidates: preallocated_heap(capacity)?,
            admitted_publications: preallocated_heap(capacity)?,
            cleaner_writes: preallocated_heap(capacity)?,
            command_ids: preallocated_set(capacity)?,
            order_keys: preallocated_set(capacity)?,
            queued_commands: 0,
            queued_read_bytes: 0,
            resume_read_bytes_since_admission_read_waited: 0,
            read_bytes_since_publication_candidate_waited: 0,
            read_bytes_since_admitted_publication_advanced: 0,
            publication_admitted_since_service: false,
        })
    }

    /// Queues work that already owns any required operation resources.
    ///
    /// New publication work must enter through `offer_publication`; callers
    /// cannot forge an accepted W0 lease.
    pub fn enqueue_ready(&mut self, command: DirectIoCommand) -> Result<(), DirectIoScheduleError> {
        if command.class == DirectIoClass::PublicationWrite {
            return Err(DirectIoScheduleError::PublicationRequiresAdmission);
        }
        self.insert(command, false)
    }

    /// Offers capacity-, catalog-, tenant-, and endurance-eligible W0 work.
    ///
    /// Eligibility is settled before this boundary. The scheduler owns only
    /// read-pressure fairness and shared I/O-resource admission.
    pub fn offer_publication(
        &mut self,
        command: DirectIoCommand,
    ) -> Result<(), DirectIoScheduleError> {
        if command.class != DirectIoClass::PublicationWrite {
            return Err(DirectIoScheduleError::Class);
        }
        self.insert(command, true)
    }

    pub fn next(
        &mut self,
        resources: &mut DirectIoResources,
    ) -> Result<Option<DirectIoDecision>, DirectIoScheduleError> {
        self.validate_resources(*resources)?;

        let admission_read_due = self.admission_read_due();
        let next_read_bytes = self.next_read_bytes(admission_read_due);
        let reads_waiting = next_read_bytes.is_some();
        let publication_service_due = !self.admitted_publications.is_empty()
            && (!reads_waiting
                || self.read_bytes_since_admitted_publication_advanced
                    >= self.config.maximum_read_bytes_before_publication_service
                || next_read_bytes.is_some_and(|bytes| {
                    would_exceed(
                        self.read_bytes_since_admitted_publication_advanced,
                        bytes,
                        self.config.maximum_read_bytes_before_publication_service,
                    )
                }));
        if publication_service_due {
            let command = self.pop_service(DirectIoClass::PublicationWrite)?;
            self.read_bytes_since_admitted_publication_advanced = 0;
            self.publication_admitted_since_service = false;
            return Ok(Some(DirectIoDecision::Service(command)));
        }

        let publication_admission_due = !self.publication_admitted_since_service
            && !self.publication_candidates.is_empty()
            && (self.queued_read_bytes <= self.config.read_high_watermark_bytes
                || self.read_bytes_since_publication_candidate_waited
                    >= self.config.maximum_read_bytes_before_publication_admission
                || next_read_bytes.is_some_and(|bytes| {
                    would_exceed(
                        self.read_bytes_since_publication_candidate_waited,
                        bytes,
                        self.config.maximum_read_bytes_before_publication_admission,
                    )
                }));
        if publication_admission_due && self.reserve_publication_resources(resources) {
            let queued = self
                .publication_candidates
                .pop()
                .ok_or(DirectIoScheduleError::Invariant)?
                .0;
            self.admitted_publications.push(Reverse(queued));
            self.read_bytes_since_publication_candidate_waited = 0;
            if self.admitted_publications.len() == 1 {
                self.read_bytes_since_admitted_publication_advanced = 0;
            }
            self.publication_admitted_since_service = true;
            return Ok(Some(DirectIoDecision::PublicationAdmitted(
                queued.into_command(DirectIoClass::PublicationWrite),
            )));
        }

        if admission_read_due {
            let command = self.pop_service(DirectIoClass::AdmissionRead)?;
            self.publication_admitted_since_service = false;
            self.record_read_service(DirectIoClass::AdmissionRead, command.bytes);
            return Ok(Some(DirectIoDecision::Service(command)));
        }
        if !self.resume_reads.is_empty() {
            let command = self.pop_service(DirectIoClass::ResumeRead)?;
            self.publication_admitted_since_service = false;
            self.record_read_service(DirectIoClass::ResumeRead, command.bytes);
            return Ok(Some(DirectIoDecision::Service(command)));
        }
        if !self.admission_reads.is_empty() {
            let command = self.pop_service(DirectIoClass::AdmissionRead)?;
            self.publication_admitted_since_service = false;
            self.record_read_service(DirectIoClass::AdmissionRead, command.bytes);
            return Ok(Some(DirectIoDecision::Service(command)));
        }
        if !self.admitted_publications.is_empty() {
            let command = self.pop_service(DirectIoClass::PublicationWrite)?;
            self.read_bytes_since_admitted_publication_advanced = 0;
            self.publication_admitted_since_service = false;
            return Ok(Some(DirectIoDecision::Service(command)));
        }

        let publication_pressure = self
            .publication_candidates
            .len()
            .checked_add(self.admitted_publications.len())
            .ok_or(DirectIoScheduleError::Overflow)?;
        let publication_pressure =
            u32::try_from(publication_pressure).map_err(|_| DirectIoScheduleError::Overflow)?;
        if !self.cleaner_writes.is_empty()
            && self.queued_read_bytes <= self.config.read_low_watermark_bytes
            && publication_pressure <= self.config.publication_low_watermark_commands
        {
            let command = self.pop_service(DirectIoClass::CleanerWrite)?;
            self.publication_admitted_since_service = false;
            return Ok(Some(DirectIoDecision::Service(command)));
        }

        Ok(None)
    }

    #[must_use]
    pub fn stats(&self) -> DirectIoSchedulerStats {
        DirectIoSchedulerStats {
            resume_reads: u32::try_from(self.resume_reads.len()).unwrap_or(u32::MAX),
            admission_reads: u32::try_from(self.admission_reads.len()).unwrap_or(u32::MAX),
            publication_candidates: u32::try_from(self.publication_candidates.len())
                .unwrap_or(u32::MAX),
            admitted_publications: u32::try_from(self.admitted_publications.len())
                .unwrap_or(u32::MAX),
            cleaner_writes: u32::try_from(self.cleaner_writes.len()).unwrap_or(u32::MAX),
            queued_read_bytes: self.queued_read_bytes,
            resume_read_bytes_since_admission_read_waited: self
                .resume_read_bytes_since_admission_read_waited,
            read_bytes_since_publication_candidate_waited: self
                .read_bytes_since_publication_candidate_waited,
            read_bytes_since_admitted_publication_advanced: self
                .read_bytes_since_admitted_publication_advanced,
            publication_admitted_since_service: self.publication_admitted_since_service,
        }
    }

    fn insert(
        &mut self,
        command: DirectIoCommand,
        publication_candidate: bool,
    ) -> Result<(), DirectIoScheduleError> {
        validate_command(command)?;
        if self.queued_commands >= self.config.maximum_queued_commands {
            return Err(DirectIoScheduleError::QueueFull);
        }
        if self.command_ids.contains(&command.command_id) {
            return Err(DirectIoScheduleError::DuplicateCommand);
        }
        let order_key = (command.class, command.order);
        if self.order_keys.contains(&order_key) {
            return Err(DirectIoScheduleError::DuplicateOrder);
        }
        if matches!(
            command.class,
            DirectIoClass::ResumeRead | DirectIoClass::AdmissionRead
        ) && command.bytes > self.config.maximum_read_command_bytes
        {
            return Err(DirectIoScheduleError::ReadTooLarge);
        }
        let next_read_bytes = if matches!(
            command.class,
            DirectIoClass::ResumeRead | DirectIoClass::AdmissionRead
        ) {
            self.queued_read_bytes
                .checked_add(command.bytes)
                .ok_or(DirectIoScheduleError::Overflow)?
        } else {
            self.queued_read_bytes
        };
        let queued = Reverse(QueuedCommand::from(command));
        match command.class {
            DirectIoClass::ResumeRead => self.resume_reads.push(queued),
            DirectIoClass::AdmissionRead => {
                if self.admission_reads.is_empty() {
                    self.resume_read_bytes_since_admission_read_waited = 0;
                }
                self.admission_reads.push(queued);
            }
            DirectIoClass::PublicationWrite if publication_candidate => {
                if self.publication_candidates.is_empty() {
                    self.read_bytes_since_publication_candidate_waited = 0;
                }
                self.publication_candidates.push(queued);
            }
            DirectIoClass::PublicationWrite => {
                return Err(DirectIoScheduleError::PublicationRequiresAdmission);
            }
            DirectIoClass::CleanerWrite => self.cleaner_writes.push(queued),
        }
        self.command_ids.insert(command.command_id);
        self.order_keys.insert(order_key);
        self.queued_commands += 1;
        self.queued_read_bytes = next_read_bytes;
        Ok(())
    }

    fn pop_service(
        &mut self,
        class: DirectIoClass,
    ) -> Result<DirectIoCommand, DirectIoScheduleError> {
        let queued = match class {
            DirectIoClass::ResumeRead => self.resume_reads.pop(),
            DirectIoClass::AdmissionRead => self.admission_reads.pop(),
            DirectIoClass::PublicationWrite => self.admitted_publications.pop(),
            DirectIoClass::CleanerWrite => self.cleaner_writes.pop(),
        }
        .ok_or(DirectIoScheduleError::Invariant)?
        .0;
        let command = queued.into_command(class);
        if !self.command_ids.remove(&command.command_id)
            || !self.order_keys.remove(&(class, command.order))
        {
            return Err(DirectIoScheduleError::Invariant);
        }
        self.queued_commands = self
            .queued_commands
            .checked_sub(1)
            .ok_or(DirectIoScheduleError::Invariant)?;
        if matches!(
            class,
            DirectIoClass::ResumeRead | DirectIoClass::AdmissionRead
        ) {
            self.queued_read_bytes = self
                .queued_read_bytes
                .checked_sub(command.bytes)
                .ok_or(DirectIoScheduleError::Invariant)?;
        }
        Ok(command)
    }

    fn record_read_service(&mut self, class: DirectIoClass, bytes: u64) {
        match class {
            DirectIoClass::ResumeRead if !self.admission_reads.is_empty() => {
                self.resume_read_bytes_since_admission_read_waited = self
                    .resume_read_bytes_since_admission_read_waited
                    .saturating_add(bytes)
                    .min(self.config.maximum_resume_read_bytes_before_admission_read);
            }
            DirectIoClass::AdmissionRead => {
                self.resume_read_bytes_since_admission_read_waited = 0;
            }
            _ if self.admission_reads.is_empty() => {
                self.resume_read_bytes_since_admission_read_waited = 0;
            }
            _ => {}
        }
        if !self.publication_candidates.is_empty() {
            self.read_bytes_since_publication_candidate_waited = self
                .read_bytes_since_publication_candidate_waited
                .saturating_add(bytes)
                .min(self.config.maximum_read_bytes_before_publication_admission);
        } else {
            self.read_bytes_since_publication_candidate_waited = 0;
        }
        if !self.admitted_publications.is_empty() {
            self.read_bytes_since_admitted_publication_advanced = self
                .read_bytes_since_admitted_publication_advanced
                .saturating_add(bytes)
                .min(self.config.maximum_read_bytes_before_publication_service);
        } else {
            self.read_bytes_since_admitted_publication_advanced = 0;
        }
    }

    fn admission_read_due(&self) -> bool {
        if self.admission_reads.is_empty() {
            return false;
        }
        let Some(next_resume) = self.resume_reads.peek().map(|queued| queued.0.bytes) else {
            return true;
        };
        self.resume_read_bytes_since_admission_read_waited
            >= self.config.maximum_resume_read_bytes_before_admission_read
            || would_exceed(
                self.resume_read_bytes_since_admission_read_waited,
                next_resume,
                self.config.maximum_resume_read_bytes_before_admission_read,
            )
    }

    fn next_read_bytes(&self, admission_read_due: bool) -> Option<u64> {
        if admission_read_due {
            return self.admission_reads.peek().map(|queued| queued.0.bytes);
        }
        self.resume_reads
            .peek()
            .or_else(|| self.admission_reads.peek())
            .map(|queued| queued.0.bytes)
    }

    fn reserve_publication_resources(&self, resources: &mut DirectIoResources) -> bool {
        let minimum_cq = self
            .config
            .read_reserved_cq_entries
            .saturating_add(DIRECT_PUBLICATION_CQ_RESERVATION);
        if resources.free_buffers <= self.config.read_reserved_buffers
            || resources.free_descriptors <= self.config.read_reserved_descriptors
            || resources.free_cq_entries < minimum_cq
        {
            return false;
        }
        resources.free_buffers -= 1;
        resources.free_descriptors -= 1;
        resources.free_cq_entries -= DIRECT_PUBLICATION_CQ_RESERVATION;
        true
    }

    fn validate_resources(
        &self,
        resources: DirectIoResources,
    ) -> Result<(), DirectIoScheduleError> {
        if resources.free_buffers > self.config.total_buffers
            || resources.free_descriptors > self.config.total_descriptors
            || resources.free_cq_entries > self.config.total_cq_entries
        {
            return Err(DirectIoScheduleError::ResourceState);
        }
        Ok(())
    }
}

fn validate_config(config: DirectIoSchedulerConfig) -> Result<(), DirectIoScheduleError> {
    let required_cq_entries = config
        .total_descriptors
        .checked_mul(2)
        .ok_or(DirectIoScheduleError::Overflow)?;
    let required_read_cq_entries = config
        .read_reserved_descriptors
        .checked_mul(2)
        .ok_or(DirectIoScheduleError::Overflow)?;
    if config.maximum_queued_commands == 0
        || config.maximum_queued_commands > MAX_DIRECT_IO_QUEUED_COMMANDS
        || config.total_buffers == 0
        || config.read_reserved_buffers >= config.total_buffers
        || config.total_descriptors == 0
        || config.read_reserved_descriptors >= config.total_descriptors
        || config.total_cq_entries != required_cq_entries
        || config.read_reserved_cq_entries != required_read_cq_entries
        || config.read_reserved_cq_entries >= config.total_cq_entries
        || config.read_low_watermark_bytes > config.read_high_watermark_bytes
        || config.publication_low_watermark_commands > config.maximum_queued_commands
        || config.maximum_read_command_bytes == 0
        || config.maximum_resume_read_bytes_before_admission_read
            < config.maximum_read_command_bytes
        || config.maximum_read_bytes_before_publication_admission
            < config.maximum_read_command_bytes
        || config.maximum_read_bytes_before_publication_service < config.maximum_read_command_bytes
    {
        return Err(DirectIoScheduleError::Config);
    }
    Ok(())
}

fn preallocated_heap(
    capacity: usize,
) -> Result<BinaryHeap<Reverse<QueuedCommand>>, DirectIoScheduleError> {
    let mut heap = BinaryHeap::new();
    heap.try_reserve_exact(capacity)
        .map_err(|_| DirectIoScheduleError::Allocation)?;
    Ok(heap)
}

fn preallocated_set<T>(capacity: usize) -> Result<HashSet<T>, DirectIoScheduleError>
where
    T: Eq + Hash,
{
    let mut set = HashSet::new();
    set.try_reserve(capacity)
        .map_err(|_| DirectIoScheduleError::Allocation)?;
    Ok(set)
}

fn validate_command(command: DirectIoCommand) -> Result<(), DirectIoScheduleError> {
    if command.command_id == 0
        || command.order.service_epoch == 0
        || command.order.owner_id == 0
        || command.bytes == 0
    {
        return Err(DirectIoScheduleError::Command);
    }
    Ok(())
}

fn would_exceed(current: u64, next: u64, limit: u64) -> bool {
    current.checked_add(next).is_none_or(|value| value > limit)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectIoScheduleError {
    Config,
    Command,
    Class,
    PublicationRequiresAdmission,
    DuplicateCommand,
    DuplicateOrder,
    QueueFull,
    ReadTooLarge,
    ResourceState,
    Allocation,
    Overflow,
    Invariant,
}

impl std::fmt::Display for DirectIoScheduleError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for DirectIoScheduleError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> DirectIoSchedulerConfig {
        DirectIoSchedulerConfig {
            maximum_queued_commands: 32,
            total_buffers: 8,
            read_reserved_buffers: 2,
            total_descriptors: 8,
            read_reserved_descriptors: 2,
            total_cq_entries: 16,
            read_reserved_cq_entries: 4,
            read_low_watermark_bytes: 100,
            read_high_watermark_bytes: 200,
            publication_low_watermark_commands: 0,
            maximum_read_command_bytes: 200,
            maximum_resume_read_bytes_before_admission_read: 200,
            maximum_read_bytes_before_publication_admission: 300,
            maximum_read_bytes_before_publication_service: 200,
        }
    }

    fn resources() -> DirectIoResources {
        DirectIoResources {
            free_buffers: 8,
            free_descriptors: 8,
            free_cq_entries: 16,
        }
    }

    fn command(
        command_id: u64,
        class: DirectIoClass,
        service_epoch: u64,
        owner_id: u64,
        bytes: u64,
    ) -> DirectIoCommand {
        DirectIoCommand {
            command_id,
            class,
            order: DirectIoOrderKey {
                service_epoch,
                owner_id,
                page_ordinal: 0,
                operation_ordinal: 0,
            },
            bytes,
        }
    }

    fn service(decision: Option<DirectIoDecision>) -> DirectIoCommand {
        match decision.unwrap() {
            DirectIoDecision::Service(command) => command,
            other => panic!("expected service, got {other:?}"),
        }
    }

    fn admission(decision: Option<DirectIoDecision>) -> DirectIoCommand {
        match decision.unwrap() {
            DirectIoDecision::PublicationAdmitted(command) => command,
            other => panic!("expected admission, got {other:?}"),
        }
    }

    #[test]
    fn configuration_and_commands_fail_closed() {
        let mut invalid = config();
        invalid.total_cq_entries -= 1;
        assert_eq!(
            DirectIoScheduler::new(invalid).unwrap_err(),
            DirectIoScheduleError::Config
        );
        invalid = config();
        invalid.read_reserved_buffers = invalid.total_buffers;
        assert_eq!(
            DirectIoScheduler::new(invalid).unwrap_err(),
            DirectIoScheduleError::Config
        );
        invalid = config();
        invalid.read_reserved_cq_entries -= 1;
        assert_eq!(
            DirectIoScheduler::new(invalid).unwrap_err(),
            DirectIoScheduleError::Config
        );
        invalid = config();
        invalid.maximum_queued_commands = MAX_DIRECT_IO_QUEUED_COMMANDS + 1;
        assert_eq!(
            DirectIoScheduler::new(invalid).unwrap_err(),
            DirectIoScheduleError::Config
        );
        for selector in 0..3 {
            invalid = config();
            match selector {
                0 => invalid.maximum_resume_read_bytes_before_admission_read = 199,
                1 => invalid.maximum_read_bytes_before_publication_admission = 199,
                2 => invalid.maximum_read_bytes_before_publication_service = 199,
                _ => unreachable!(),
            }
            assert_eq!(
                DirectIoScheduler::new(invalid).unwrap_err(),
                DirectIoScheduleError::Config
            );
        }

        let mut scheduler = DirectIoScheduler::new(config()).unwrap();
        let mut invalid_command = command(0, DirectIoClass::ResumeRead, 1, 1, 1);
        assert_eq!(
            scheduler.enqueue_ready(invalid_command),
            Err(DirectIoScheduleError::Command)
        );
        invalid_command.command_id = 1;
        invalid_command.order.service_epoch = 0;
        assert_eq!(
            scheduler.enqueue_ready(invalid_command),
            Err(DirectIoScheduleError::Command)
        );
        assert_eq!(
            scheduler.enqueue_ready(command(1, DirectIoClass::PublicationWrite, 1, 1, 1)),
            Err(DirectIoScheduleError::PublicationRequiresAdmission)
        );
        assert_eq!(
            scheduler.offer_publication(command(1, DirectIoClass::AdmissionRead, 1, 1, 1)),
            Err(DirectIoScheduleError::Class)
        );
        assert_eq!(
            scheduler.enqueue_ready(command(1, DirectIoClass::ResumeRead, 1, 1, 201)),
            Err(DirectIoScheduleError::ReadTooLarge)
        );
    }

    #[test]
    fn class_priority_and_in_class_order_are_deterministic() {
        let mut scheduler = DirectIoScheduler::new(config()).unwrap();
        scheduler
            .enqueue_ready(command(1, DirectIoClass::AdmissionRead, 2, 1, 10))
            .unwrap();
        scheduler
            .enqueue_ready(command(2, DirectIoClass::ResumeRead, 2, 1, 10))
            .unwrap();
        scheduler
            .enqueue_ready(command(3, DirectIoClass::ResumeRead, 1, 9, 10))
            .unwrap();
        scheduler
            .enqueue_ready(command(4, DirectIoClass::AdmissionRead, 1, 8, 10))
            .unwrap();
        let mut available = resources();
        assert_eq!(
            service(scheduler.next(&mut available).unwrap()).command_id,
            3
        );
        assert_eq!(
            service(scheduler.next(&mut available).unwrap()).command_id,
            2
        );
        assert_eq!(
            service(scheduler.next(&mut available).unwrap()).command_id,
            4
        );
        assert_eq!(
            service(scheduler.next(&mut available).unwrap()).command_id,
            1
        );
        assert_eq!(scheduler.next(&mut available), Ok(None));
    }

    #[test]
    fn resume_reads_cannot_starve_admission_reads() {
        let mut scheduler = DirectIoScheduler::new(config()).unwrap();
        scheduler
            .enqueue_ready(command(1, DirectIoClass::AdmissionRead, 1, 1, 100))
            .unwrap();
        scheduler
            .enqueue_ready(command(2, DirectIoClass::ResumeRead, 1, 2, 100))
            .unwrap();
        scheduler
            .enqueue_ready(command(3, DirectIoClass::ResumeRead, 1, 3, 100))
            .unwrap();
        scheduler
            .enqueue_ready(command(4, DirectIoClass::ResumeRead, 1, 4, 100))
            .unwrap();
        let mut available = resources();
        assert_eq!(
            service(scheduler.next(&mut available).unwrap()).command_id,
            2
        );
        assert_eq!(
            service(scheduler.next(&mut available).unwrap()).command_id,
            3
        );
        assert_eq!(
            scheduler
                .stats()
                .resume_read_bytes_since_admission_read_waited,
            200
        );
        assert_eq!(
            service(scheduler.next(&mut available).unwrap()).command_id,
            1
        );
        assert_eq!(
            scheduler
                .stats()
                .resume_read_bytes_since_admission_read_waited,
            0
        );
        assert_eq!(
            service(scheduler.next(&mut available).unwrap()).command_id,
            4
        );

        let mut scheduler = DirectIoScheduler::new(config()).unwrap();
        scheduler
            .enqueue_ready(command(1, DirectIoClass::AdmissionRead, 1, 1, 100))
            .unwrap();
        scheduler
            .enqueue_ready(command(2, DirectIoClass::ResumeRead, 1, 2, 150))
            .unwrap();
        scheduler
            .enqueue_ready(command(3, DirectIoClass::ResumeRead, 1, 3, 60))
            .unwrap();
        assert_eq!(
            service(scheduler.next(&mut available).unwrap()).command_id,
            2
        );
        assert_eq!(
            service(scheduler.next(&mut available).unwrap()).command_id,
            1
        );
        assert_eq!(
            service(scheduler.next(&mut available).unwrap()).command_id,
            3
        );
    }

    #[test]
    fn duplicate_identity_and_order_are_rejected_without_consuming_capacity() {
        let mut scheduler = DirectIoScheduler::new(config()).unwrap();
        let first = command(1, DirectIoClass::ResumeRead, 1, 1, 10);
        scheduler.enqueue_ready(first).unwrap();
        let mut duplicate_id = command(1, DirectIoClass::AdmissionRead, 1, 2, 10);
        assert_eq!(
            scheduler.enqueue_ready(duplicate_id),
            Err(DirectIoScheduleError::DuplicateCommand)
        );
        duplicate_id.command_id = 2;
        duplicate_id.class = DirectIoClass::ResumeRead;
        duplicate_id.order = first.order;
        assert_eq!(
            scheduler.enqueue_ready(duplicate_id),
            Err(DirectIoScheduleError::DuplicateOrder)
        );
        let stats = scheduler.stats();
        assert_eq!(stats.resume_reads, 1);
        assert_eq!(stats.admission_reads, 0);
        assert_eq!(stats.queued_read_bytes, 10);
    }

    #[test]
    fn queue_and_read_byte_overflow_fail_without_partial_insertion() {
        let mut bounded = config();
        bounded.maximum_queued_commands = 2;
        bounded.maximum_read_command_bytes = u64::MAX;
        bounded.maximum_resume_read_bytes_before_admission_read = u64::MAX;
        bounded.maximum_read_bytes_before_publication_admission = u64::MAX;
        bounded.maximum_read_bytes_before_publication_service = u64::MAX;
        let mut scheduler = DirectIoScheduler::new(bounded).unwrap();
        scheduler
            .enqueue_ready(command(1, DirectIoClass::ResumeRead, 1, 1, u64::MAX))
            .unwrap();
        assert_eq!(
            scheduler.enqueue_ready(command(2, DirectIoClass::ResumeRead, 1, 2, 1)),
            Err(DirectIoScheduleError::Overflow)
        );
        scheduler
            .enqueue_ready(command(2, DirectIoClass::CleanerWrite, 1, 2, 1))
            .unwrap();
        assert_eq!(
            scheduler.enqueue_ready(command(3, DirectIoClass::CleanerWrite, 1, 3, 1)),
            Err(DirectIoScheduleError::QueueFull)
        );
        let stats = scheduler.stats();
        assert_eq!(stats.resume_reads, 1);
        assert_eq!(stats.cleaner_writes, 1);
        assert_eq!(stats.queued_read_bytes, u64::MAX);
    }

    #[test]
    fn configured_queue_capacity_never_grows_after_construction() {
        let mut scheduler = DirectIoScheduler::new(config()).unwrap();
        let resume_capacity = scheduler.resume_reads.capacity();
        let admission_capacity = scheduler.admission_reads.capacity();
        let candidate_capacity = scheduler.publication_candidates.capacity();
        let admitted_capacity = scheduler.admitted_publications.capacity();
        let cleaner_capacity = scheduler.cleaner_writes.capacity();
        let id_capacity = scheduler.command_ids.capacity();
        let order_capacity = scheduler.order_keys.capacity();
        for command_id in 1..=u64::from(config().maximum_queued_commands) {
            scheduler
                .enqueue_ready(command(
                    command_id,
                    DirectIoClass::ResumeRead,
                    1,
                    command_id,
                    1,
                ))
                .unwrap();
        }
        assert_eq!(scheduler.resume_reads.capacity(), resume_capacity);
        assert_eq!(scheduler.admission_reads.capacity(), admission_capacity);
        assert_eq!(
            scheduler.publication_candidates.capacity(),
            candidate_capacity
        );
        assert_eq!(
            scheduler.admitted_publications.capacity(),
            admitted_capacity
        );
        assert_eq!(scheduler.cleaner_writes.capacity(), cleaner_capacity);
        assert_eq!(scheduler.command_ids.capacity(), id_capacity);
        assert_eq!(scheduler.order_keys.capacity(), order_capacity);
    }

    #[test]
    fn ordinary_publication_admission_preserves_every_read_reserve() {
        let mut scheduler = DirectIoScheduler::new(config()).unwrap();
        scheduler
            .offer_publication(command(1, DirectIoClass::PublicationWrite, 1, 1, 50))
            .unwrap();
        let mut exact_reserves = DirectIoResources {
            free_buffers: 2,
            free_descriptors: 2,
            free_cq_entries: 4,
        };
        assert_eq!(scheduler.next(&mut exact_reserves), Ok(None));
        assert_eq!(
            exact_reserves,
            DirectIoResources {
                free_buffers: 2,
                free_descriptors: 2,
                free_cq_entries: 4,
            }
        );

        let mut one_shared_slot = DirectIoResources {
            free_buffers: 3,
            free_descriptors: 3,
            free_cq_entries: 6,
        };
        assert_eq!(
            admission(scheduler.next(&mut one_shared_slot).unwrap()).command_id,
            1
        );
        assert_eq!(
            one_shared_slot,
            DirectIoResources {
                free_buffers: 2,
                free_descriptors: 2,
                free_cq_entries: 4,
            }
        );
    }

    #[test]
    fn every_resource_boundary_preserves_read_reserves() {
        for free_buffers in 0..=8 {
            for free_descriptors in 0..=8 {
                for free_cq_entries in 0..=16 {
                    let mut scheduler = DirectIoScheduler::new(config()).unwrap();
                    scheduler
                        .offer_publication(command(1, DirectIoClass::PublicationWrite, 1, 1, 50))
                        .unwrap();
                    let mut available = DirectIoResources {
                        free_buffers,
                        free_descriptors,
                        free_cq_entries,
                    };
                    let decision = scheduler.next(&mut available).unwrap();
                    let should_admit =
                        free_buffers > 2 && free_descriptors > 2 && free_cq_entries >= 6;
                    assert_eq!(
                        matches!(decision, Some(DirectIoDecision::PublicationAdmitted(_))),
                        should_admit,
                        "buffers={free_buffers} descriptors={free_descriptors} cq={free_cq_entries}"
                    );
                    if should_admit {
                        assert!(available.free_buffers >= 2);
                        assert!(available.free_descriptors >= 2);
                        assert!(available.free_cq_entries >= 4);
                    } else {
                        assert_eq!(
                            available,
                            DirectIoResources {
                                free_buffers,
                                free_descriptors,
                                free_cq_entries,
                            }
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn continuous_reads_cannot_starve_new_publication_admission() {
        let mut scheduler = DirectIoScheduler::new(config()).unwrap();
        scheduler
            .offer_publication(command(1, DirectIoClass::PublicationWrite, 1, 1, 50))
            .unwrap();
        for command_id in 2..=6 {
            scheduler
                .enqueue_ready(command(
                    command_id,
                    DirectIoClass::ResumeRead,
                    1,
                    command_id,
                    100,
                ))
                .unwrap();
        }
        let mut no_shared_capacity = DirectIoResources {
            free_buffers: 2,
            free_descriptors: 2,
            free_cq_entries: 4,
        };
        for expected_id in 2..=4 {
            assert_eq!(
                service(scheduler.next(&mut no_shared_capacity).unwrap()).command_id,
                expected_id
            );
        }
        assert_eq!(
            scheduler
                .stats()
                .read_bytes_since_publication_candidate_waited,
            300
        );
        assert_eq!(
            service(scheduler.next(&mut no_shared_capacity).unwrap()).command_id,
            5
        );

        let mut one_shared_slot = DirectIoResources {
            free_buffers: 3,
            free_descriptors: 3,
            free_cq_entries: 6,
        };
        assert_eq!(
            admission(scheduler.next(&mut one_shared_slot).unwrap()).command_id,
            1
        );
        assert_eq!(
            scheduler
                .stats()
                .read_bytes_since_publication_candidate_waited,
            0
        );
    }

    #[test]
    fn admitted_publication_advances_at_the_exact_read_byte_bound() {
        let mut scheduler = DirectIoScheduler::new(config()).unwrap();
        scheduler
            .offer_publication(command(1, DirectIoClass::PublicationWrite, 1, 1, 50))
            .unwrap();
        for command_id in 2..=7 {
            scheduler
                .enqueue_ready(command(
                    command_id,
                    DirectIoClass::ResumeRead,
                    1,
                    command_id,
                    100,
                ))
                .unwrap();
        }
        let mut available = resources();
        assert_eq!(
            service(scheduler.next(&mut available).unwrap()).command_id,
            2
        );
        assert_eq!(
            service(scheduler.next(&mut available).unwrap()).command_id,
            3
        );
        assert_eq!(
            service(scheduler.next(&mut available).unwrap()).command_id,
            4
        );
        assert_eq!(
            admission(scheduler.next(&mut available).unwrap()).command_id,
            1
        );
        assert_eq!(
            service(scheduler.next(&mut available).unwrap()).command_id,
            5
        );
        assert_eq!(
            service(scheduler.next(&mut available).unwrap()).command_id,
            6
        );
        assert_eq!(
            service(scheduler.next(&mut available).unwrap()).command_id,
            1
        );
        assert_eq!(
            scheduler
                .stats()
                .read_bytes_since_admitted_publication_advanced,
            0
        );
    }

    #[test]
    fn continuous_read_arrivals_obey_both_publication_bounds() {
        let mut continuous = config();
        continuous.read_low_watermark_bytes = 0;
        continuous.read_high_watermark_bytes = 0;
        let mut scheduler = DirectIoScheduler::new(continuous).unwrap();
        scheduler
            .offer_publication(command(1, DirectIoClass::PublicationWrite, 1, 1, 50))
            .unwrap();
        scheduler
            .enqueue_ready(command(2, DirectIoClass::ResumeRead, 1, 2, 100))
            .unwrap();
        let mut available = resources();
        let mut next_read_id = 3;
        for expected_id in 2..=4 {
            assert_eq!(
                service(scheduler.next(&mut available).unwrap()).command_id,
                expected_id
            );
            scheduler
                .enqueue_ready(command(
                    next_read_id,
                    DirectIoClass::ResumeRead,
                    1,
                    next_read_id,
                    100,
                ))
                .unwrap();
            next_read_id += 1;
        }
        assert_eq!(
            admission(scheduler.next(&mut available).unwrap()).command_id,
            1
        );
        for expected_id in 5..=6 {
            assert_eq!(
                service(scheduler.next(&mut available).unwrap()).command_id,
                expected_id
            );
            scheduler
                .enqueue_ready(command(
                    next_read_id,
                    DirectIoClass::ResumeRead,
                    1,
                    next_read_id,
                    100,
                ))
                .unwrap();
            next_read_id += 1;
        }
        assert_eq!(
            service(scheduler.next(&mut available).unwrap()).command_id,
            1
        );
        assert!(scheduler.stats().resume_reads > 0);
    }

    #[test]
    fn variable_read_sizes_never_overshoot_a_available_publication_bound() {
        let mut variable = config();
        variable.read_low_watermark_bytes = 0;
        variable.read_high_watermark_bytes = 0;
        let mut scheduler = DirectIoScheduler::new(variable).unwrap();
        scheduler
            .offer_publication(command(1, DirectIoClass::PublicationWrite, 1, 1, 50))
            .unwrap();
        scheduler
            .enqueue_ready(command(2, DirectIoClass::ResumeRead, 1, 2, 200))
            .unwrap();
        scheduler
            .enqueue_ready(command(3, DirectIoClass::ResumeRead, 1, 3, 150))
            .unwrap();
        scheduler
            .enqueue_ready(command(4, DirectIoClass::ResumeRead, 1, 4, 60))
            .unwrap();
        let mut available = resources();
        assert_eq!(
            service(scheduler.next(&mut available).unwrap()).command_id,
            2
        );
        assert_eq!(
            admission(scheduler.next(&mut available).unwrap()).command_id,
            1
        );
        assert_eq!(
            service(scheduler.next(&mut available).unwrap()).command_id,
            3
        );
        assert_eq!(
            service(scheduler.next(&mut available).unwrap()).command_id,
            1
        );
        assert_eq!(
            service(scheduler.next(&mut available).unwrap()).command_id,
            4
        );
    }

    #[test]
    fn projected_byte_bound_is_exhaustive_and_overflow_safe() {
        for current in 0_u64..=300 {
            for next in 1_u64..=301 {
                assert_eq!(would_exceed(current, next, 300), current + next > 300);
            }
        }
        assert!(would_exceed(u64::MAX, 1, u64::MAX));
        assert!(!would_exceed(u64::MAX - 1, 1, u64::MAX));
    }

    #[test]
    fn one_shared_slot_cannot_admit_two_publications() {
        let mut scheduler = DirectIoScheduler::new(config()).unwrap();
        for command_id in 1..=2 {
            scheduler
                .offer_publication(command(
                    command_id,
                    DirectIoClass::PublicationWrite,
                    1,
                    command_id,
                    50,
                ))
                .unwrap();
        }
        let mut one_shared_slot = DirectIoResources {
            free_buffers: 3,
            free_descriptors: 3,
            free_cq_entries: 6,
        };
        assert_eq!(
            admission(scheduler.next(&mut one_shared_slot).unwrap()).command_id,
            1
        );
        assert_eq!(
            service(scheduler.next(&mut one_shared_slot).unwrap()).command_id,
            1
        );
        assert_eq!(scheduler.next(&mut one_shared_slot), Ok(None));
        assert_eq!(scheduler.stats().publication_candidates, 1);
    }

    #[test]
    fn admission_receipts_cannot_starve_reads_or_accepted_publications() {
        let mut scheduler = DirectIoScheduler::new(config()).unwrap();
        for command_id in 1..=2 {
            scheduler
                .offer_publication(command(
                    command_id,
                    DirectIoClass::PublicationWrite,
                    1,
                    command_id,
                    50,
                ))
                .unwrap();
        }
        scheduler
            .enqueue_ready(command(3, DirectIoClass::ResumeRead, 1, 3, 100))
            .unwrap();
        let mut available = resources();
        assert_eq!(
            admission(scheduler.next(&mut available).unwrap()).command_id,
            1
        );
        assert!(scheduler.stats().publication_admitted_since_service);
        assert_eq!(
            service(scheduler.next(&mut available).unwrap()).command_id,
            3
        );
        assert!(!scheduler.stats().publication_admitted_since_service);
        assert_eq!(
            service(scheduler.next(&mut available).unwrap()).command_id,
            1
        );
        assert_eq!(
            admission(scheduler.next(&mut available).unwrap()).command_id,
            2
        );
        assert_eq!(
            service(scheduler.next(&mut available).unwrap()).command_id,
            2
        );
    }

    #[test]
    fn cleaner_runs_only_below_both_low_watermarks() {
        let mut scheduler = DirectIoScheduler::new(config()).unwrap();
        scheduler
            .enqueue_ready(command(1, DirectIoClass::CleanerWrite, 1, 1, 50))
            .unwrap();
        scheduler
            .enqueue_ready(command(2, DirectIoClass::AdmissionRead, 1, 2, 200))
            .unwrap();
        let mut available = resources();
        assert_eq!(
            service(scheduler.next(&mut available).unwrap()).command_id,
            2
        );
        assert_eq!(
            service(scheduler.next(&mut available).unwrap()).command_id,
            1
        );

        let mut scheduler = DirectIoScheduler::new(config()).unwrap();
        scheduler
            .enqueue_ready(command(1, DirectIoClass::CleanerWrite, 1, 1, 50))
            .unwrap();
        scheduler
            .offer_publication(command(2, DirectIoClass::PublicationWrite, 1, 2, 50))
            .unwrap();
        let mut exact_reserves = DirectIoResources {
            free_buffers: 2,
            free_descriptors: 2,
            free_cq_entries: 4,
        };
        assert_eq!(scheduler.next(&mut exact_reserves), Ok(None));
        assert_eq!(scheduler.stats().cleaner_writes, 1);
    }

    #[test]
    fn invalid_resource_snapshot_is_rejected_without_state_change() {
        let mut scheduler = DirectIoScheduler::new(config()).unwrap();
        scheduler
            .enqueue_ready(command(1, DirectIoClass::ResumeRead, 1, 1, 10))
            .unwrap();
        let before = scheduler.stats();
        let mut invalid = DirectIoResources {
            free_buffers: 9,
            free_descriptors: 8,
            free_cq_entries: 16,
        };
        assert_eq!(
            scheduler.next(&mut invalid),
            Err(DirectIoScheduleError::ResourceState)
        );
        assert_eq!(scheduler.stats(), before);
    }

    #[test]
    fn final_accounting_reaches_zero() {
        let mut scheduler = DirectIoScheduler::new(config()).unwrap();
        scheduler
            .enqueue_ready(command(1, DirectIoClass::ResumeRead, 1, 1, 10))
            .unwrap();
        scheduler
            .enqueue_ready(command(2, DirectIoClass::AdmissionRead, 1, 2, 20))
            .unwrap();
        scheduler
            .offer_publication(command(3, DirectIoClass::PublicationWrite, 1, 3, 30))
            .unwrap();
        scheduler
            .enqueue_ready(command(4, DirectIoClass::CleanerWrite, 1, 4, 40))
            .unwrap();
        let mut available = resources();
        let mut decisions = Vec::new();
        while let Some(decision) = scheduler.next(&mut available).unwrap() {
            decisions.push(decision);
        }
        assert_eq!(decisions.len(), 5);
        assert_eq!(
            scheduler.stats(),
            DirectIoSchedulerStats {
                resume_reads: 0,
                admission_reads: 0,
                publication_candidates: 0,
                admitted_publications: 0,
                cleaner_writes: 0,
                queued_read_bytes: 0,
                resume_read_bytes_since_admission_read_waited: 0,
                read_bytes_since_publication_candidate_waited: 0,
                read_bytes_since_admitted_publication_advanced: 0,
                publication_admitted_since_service: false,
            }
        );
    }
}
