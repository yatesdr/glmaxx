use std::{
    collections::BTreeMap,
    fmt,
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
        mpsc::{self, Receiver, SyncSender, TrySendError},
    },
    thread::{self, JoinHandle},
    time::Duration,
};

use crate::{FileTierStore, RestoredPage, StoreError, Tier, TierRecord, owner_rank};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RestoreRequest {
    pub request_id: u64,
    pub page_key: [u8; 32],
    pub expected_namespace: [u8; 32],
    pub minimum_generation: u64,
    pub page_ordinal: u64,
    pub worker_rank: u8,
}

#[derive(Debug)]
pub struct RestoreResult {
    pub request_id: u64,
    pub page_ordinal: u64,
    pub page: RestoredPage,
}

pub struct RestoreHandle {
    receiver: Receiver<Result<RestoreResult, RestoreError>>,
    outstanding: Arc<AtomicUsize>,
    released: bool,
}

impl RestoreHandle {
    pub fn receive(mut self) -> Result<RestoreResult, RestoreError> {
        let result = self
            .receiver
            .recv()
            .map_err(|_| RestoreError::WorkerClosed)?;
        self.release();
        result
    }

    pub fn receive_timeout(mut self, timeout: Duration) -> Result<RestoreResult, RestoreError> {
        let result = self
            .receiver
            .recv_timeout(timeout)
            .map_err(|error| match error {
                mpsc::RecvTimeoutError::Timeout => RestoreError::Timeout,
                mpsc::RecvTimeoutError::Disconnected => RestoreError::WorkerClosed,
            })?;
        self.release();
        result
    }

    pub fn try_receive(&mut self) -> Result<Option<RestoreResult>, RestoreError> {
        match self.receiver.try_recv() {
            Ok(result) => {
                self.release();
                result.map(Some)
            }
            Err(mpsc::TryRecvError::Empty) => Ok(None),
            Err(mpsc::TryRecvError::Disconnected) => {
                self.release();
                Err(RestoreError::WorkerClosed)
            }
        }
    }

    fn release(&mut self) {
        if !self.released {
            self.outstanding.fetch_sub(1, Ordering::AcqRel);
            self.released = true;
        }
    }
}

impl Drop for RestoreHandle {
    fn drop(&mut self) {
        self.release();
    }
}

struct RestoreCommand {
    request: RestoreRequest,
    response: SyncSender<Result<RestoreResult, RestoreError>>,
}

pub struct RestoreService {
    sender: Option<SyncSender<RestoreCommand>>,
    worker: Option<JoinHandle<()>>,
    outstanding: Arc<AtomicUsize>,
    maximum_outstanding: usize,
}

impl RestoreService {
    pub fn spawn(root: &Path, maximum_outstanding: usize) -> Result<Self, RestoreError> {
        if maximum_outstanding == 0 {
            return Err(RestoreError::Config);
        }
        let mut store = FileTierStore::open(root)?;
        let (sender, receiver) = mpsc::sync_channel::<RestoreCommand>(maximum_outstanding);
        let worker = thread::Builder::new()
            .name("glmaxx-nvme-restore".into())
            .spawn(move || {
                while let Ok(command) = receiver.recv() {
                    let result = restore_one(&mut store, command.request);
                    let _ = command.response.send(result);
                }
            })
            .map_err(RestoreError::Thread)?;
        Ok(Self {
            sender: Some(sender),
            worker: Some(worker),
            outstanding: Arc::new(AtomicUsize::new(0)),
            maximum_outstanding,
        })
    }

    pub fn try_submit(&self, request: RestoreRequest) -> Result<RestoreHandle, RestoreError> {
        if request.request_id == 0
            || request.expected_namespace == [0; 32]
            || request.minimum_generation == 0
            || request.worker_rank >= 4
            || owner_rank(request.page_ordinal) != request.worker_rank
        {
            return Err(RestoreError::Request);
        }
        self.reserve_slot()?;
        let (response, receiver) = mpsc::sync_channel(1);
        let command = RestoreCommand { request, response };
        let Some(sender) = &self.sender else {
            self.outstanding.fetch_sub(1, Ordering::AcqRel);
            return Err(RestoreError::WorkerClosed);
        };
        if let Err(error) = sender.try_send(command) {
            self.outstanding.fetch_sub(1, Ordering::AcqRel);
            return Err(match error {
                TrySendError::Full(_) => RestoreError::Saturated,
                TrySendError::Disconnected(_) => RestoreError::WorkerClosed,
            });
        }
        Ok(RestoreHandle {
            receiver,
            outstanding: Arc::clone(&self.outstanding),
            released: false,
        })
    }

    #[must_use]
    pub fn outstanding(&self) -> usize {
        self.outstanding.load(Ordering::Acquire)
    }

    fn reserve_slot(&self) -> Result<(), RestoreError> {
        self.outstanding
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
                (current < self.maximum_outstanding).then_some(current + 1)
            })
            .map(|_| ())
            .map_err(|_| RestoreError::Saturated)
    }
}

impl Drop for RestoreService {
    fn drop(&mut self) {
        self.sender.take();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn restore_one(
    store: &mut FileTierStore,
    request: RestoreRequest,
) -> Result<RestoreResult, RestoreError> {
    let page = store
        .restore(request.page_key)?
        .ok_or(RestoreError::Missing)?;
    if page.record.namespace != request.expected_namespace
        || page.record.generation < request.minimum_generation
    {
        return Err(RestoreError::Stale);
    }
    Ok(RestoreResult {
        request_id: request.request_id,
        page_ordinal: request.page_ordinal,
        page,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Residency {
    Hbm,
    Dram,
    Nvme,
    Restoring,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResidencyConfig {
    pub hbm_bytes: u64,
    pub dram_bytes: u64,
}

#[derive(Debug)]
struct ResidentEntry {
    record: TierRecord,
    residency: Residency,
    restored: Option<RestoredPage>,
    pending_restore: Option<PendingRestoreIdentity>,
    pin_count: u32,
    last_touch: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PendingRestoreIdentity {
    request_id: u64,
    page_ordinal: u64,
}

#[derive(Debug)]
pub struct ResidencyManager {
    config: ResidencyConfig,
    entries: BTreeMap<[u8; 32], ResidentEntry>,
    hbm_bytes: u64,
    dram_bytes: u64,
    clock: u64,
}

struct HbmAdmissionPlan {
    victims: BTreeMap<[u8; 32], Residency>,
    hbm_bytes: u64,
    dram_bytes: u64,
}

impl ResidencyManager {
    pub fn new(config: ResidencyConfig) -> Result<Self, ResidencyError> {
        if config.hbm_bytes == 0 {
            return Err(ResidencyError::Config);
        }
        Ok(Self {
            config,
            entries: BTreeMap::new(),
            hbm_bytes: 0,
            dram_bytes: 0,
            clock: 0,
        })
    }

    pub fn register_nvme(&mut self, record: TierRecord) -> Result<(), ResidencyError> {
        self.validate_nvme_registration(&record)?;
        let mut next_hbm_bytes = self.hbm_bytes;
        let mut next_dram_bytes = self.dram_bytes;
        if let Some(existing) = self.entries.get(&record.page_key) {
            let bytes = entry_bytes(&existing.record)?;
            match existing.residency {
                Residency::Hbm => {
                    next_hbm_bytes = next_hbm_bytes
                        .checked_sub(bytes)
                        .ok_or(ResidencyError::Overflow)?;
                }
                Residency::Dram => {
                    next_dram_bytes = next_dram_bytes
                        .checked_sub(bytes)
                        .ok_or(ResidencyError::Overflow)?;
                }
                Residency::Nvme => {}
                Residency::Restoring => return Err(ResidencyError::State),
            }
        }
        self.entries.insert(
            record.page_key,
            ResidentEntry {
                record,
                residency: Residency::Nvme,
                restored: None,
                pending_restore: None,
                pin_count: 0,
                last_touch: 0,
            },
        );
        self.hbm_bytes = next_hbm_bytes;
        self.dram_bytes = next_dram_bytes;
        Ok(())
    }

    pub fn validate_nvme_registration(&self, record: &TierRecord) -> Result<(), ResidencyError> {
        record.validate().map_err(|_| ResidencyError::Record)?;
        if record.tier != Tier::Nvme {
            return Err(ResidencyError::Record);
        }
        if let Some(entry) = self.entries.get(&record.page_key) {
            if entry.record.generation >= record.generation {
                return Err(ResidencyError::Stale);
            }
            if entry.pin_count != 0 {
                return Err(ResidencyError::Pinned);
            }
            if entry.residency == Residency::Restoring {
                return Err(ResidencyError::State);
            }
            entry_bytes(&entry.record)?;
        }
        Ok(())
    }

    pub fn begin_restore(
        &mut self,
        request_id: u64,
        page_key: [u8; 32],
        page_ordinal: u64,
        worker_rank: u8,
    ) -> Result<RestoreRequest, ResidencyError> {
        if request_id == 0 || worker_rank >= 4 || owner_rank(page_ordinal) != worker_rank {
            return Err(ResidencyError::Request);
        }
        let entry = self
            .entries
            .get_mut(&page_key)
            .ok_or(ResidencyError::Missing)?;
        if entry.residency != Residency::Nvme || entry_bytes(&entry.record)? > self.config.hbm_bytes
        {
            return Err(ResidencyError::State);
        }
        entry.residency = Residency::Restoring;
        entry.pending_restore = Some(PendingRestoreIdentity {
            request_id,
            page_ordinal,
        });
        Ok(RestoreRequest {
            request_id,
            page_key,
            expected_namespace: entry.record.namespace,
            minimum_generation: entry.record.generation,
            page_ordinal,
            worker_rank,
        })
    }

    pub fn abort_restore(&mut self, page_key: [u8; 32]) -> Result<(), ResidencyError> {
        let entry = self
            .entries
            .get_mut(&page_key)
            .ok_or(ResidencyError::Missing)?;
        if entry.residency != Residency::Restoring {
            return Err(ResidencyError::State);
        }
        entry.residency = Residency::Nvme;
        entry.pending_restore = None;
        Ok(())
    }

    pub fn validate_abort_restore_identity(
        &self,
        page_key: [u8; 32],
        request_id: u64,
        page_ordinal: u64,
    ) -> Result<(), ResidencyError> {
        let entry = self.entries.get(&page_key).ok_or(ResidencyError::Missing)?;
        if entry.residency != Residency::Restoring
            || entry.pending_restore
                != Some(PendingRestoreIdentity {
                    request_id,
                    page_ordinal,
                })
        {
            return Err(ResidencyError::State);
        }
        Ok(())
    }

    pub fn abort_restore_identity(
        &mut self,
        page_key: [u8; 32],
        request_id: u64,
        page_ordinal: u64,
    ) -> Result<(), ResidencyError> {
        self.validate_abort_restore_identity(page_key, request_id, page_ordinal)?;
        let entry = self
            .entries
            .get_mut(&page_key)
            .ok_or(ResidencyError::Missing)?;
        entry.residency = Residency::Nvme;
        entry.pending_restore = None;
        Ok(())
    }

    pub fn complete_restore(&mut self, result: RestoreResult) -> Result<(), ResidencyError> {
        let page_key = result.page.record.page_key;
        let entry = self.entries.get(&page_key).ok_or(ResidencyError::Missing)?;
        if entry.residency != Residency::Restoring
            || entry.pending_restore
                != Some(PendingRestoreIdentity {
                    request_id: result.request_id,
                    page_ordinal: result.page_ordinal,
                })
            || entry.record != result.page.record
        {
            return Err(ResidencyError::Stale);
        }
        let bytes = entry_bytes(&entry.record)?;
        let next_clock = self.clock.checked_add(1).ok_or(ResidencyError::Overflow)?;
        let plan = self.plan_hbm_admission(bytes, 0, Some(page_key))?;
        let entry = self
            .entries
            .get_mut(&page_key)
            .ok_or(ResidencyError::Missing)?;
        entry.residency = Residency::Hbm;
        entry.restored = Some(result.page);
        entry.pending_restore = None;
        entry.last_touch = next_clock;
        self.apply_hbm_admission(plan);
        self.clock = next_clock;
        Ok(())
    }

    pub fn promote_dram(&mut self, page_key: [u8; 32]) -> Result<(), ResidencyError> {
        let entry = self.entries.get(&page_key).ok_or(ResidencyError::Missing)?;
        if entry.residency != Residency::Dram {
            return Err(ResidencyError::State);
        }
        let bytes = entry_bytes(&entry.record)?;
        let next_clock = self.clock.checked_add(1).ok_or(ResidencyError::Overflow)?;
        let plan = self.plan_hbm_admission(bytes, bytes, Some(page_key))?;
        let entry = self
            .entries
            .get_mut(&page_key)
            .ok_or(ResidencyError::Missing)?;
        entry.residency = Residency::Hbm;
        entry.last_touch = next_clock;
        self.apply_hbm_admission(plan);
        self.clock = next_clock;
        Ok(())
    }

    pub fn pin_hbm(&mut self, page_key: [u8; 32]) -> Result<(), ResidencyError> {
        let entry = self.entries.get(&page_key).ok_or(ResidencyError::Missing)?;
        if entry.residency != Residency::Hbm {
            return Err(ResidencyError::State);
        }
        let next_pin_count = entry
            .pin_count
            .checked_add(1)
            .ok_or(ResidencyError::Overflow)?;
        let next_clock = self.clock.checked_add(1).ok_or(ResidencyError::Overflow)?;
        let entry = self
            .entries
            .get_mut(&page_key)
            .ok_or(ResidencyError::Missing)?;
        entry.pin_count = next_pin_count;
        entry.last_touch = next_clock;
        self.clock = next_clock;
        Ok(())
    }

    pub fn unpin(&mut self, page_key: [u8; 32]) -> Result<(), ResidencyError> {
        self.unpin_count(page_key, 1)
    }

    pub fn validate_unpin_count(
        &self,
        page_key: [u8; 32],
        count: u32,
    ) -> Result<(), ResidencyError> {
        if count == 0 {
            return Err(ResidencyError::Request);
        }
        let entry = self.entries.get(&page_key).ok_or(ResidencyError::Missing)?;
        if entry.residency != Residency::Hbm || entry.pin_count < count {
            return Err(ResidencyError::State);
        }
        Ok(())
    }

    pub fn unpin_count(&mut self, page_key: [u8; 32], count: u32) -> Result<(), ResidencyError> {
        self.validate_unpin_count(page_key, count)?;
        let entry = self
            .entries
            .get_mut(&page_key)
            .ok_or(ResidencyError::Missing)?;
        entry.pin_count = entry
            .pin_count
            .checked_sub(count)
            .ok_or(ResidencyError::State)?;
        Ok(())
    }

    #[must_use]
    pub fn location(&self, page_key: [u8; 32]) -> Option<Residency> {
        self.entries.get(&page_key).map(|entry| entry.residency)
    }

    #[must_use]
    pub const fn hbm_bytes(&self) -> u64 {
        self.hbm_bytes
    }

    #[must_use]
    pub const fn dram_bytes(&self) -> u64 {
        self.dram_bytes
    }

    fn plan_hbm_admission(
        &self,
        incoming_bytes: u64,
        dram_release_bytes: u64,
        excluded: Option<[u8; 32]>,
    ) -> Result<HbmAdmissionPlan, ResidencyError> {
        let mut hbm_after = self.hbm_bytes;
        let mut dram_after = self
            .dram_bytes
            .checked_sub(dram_release_bytes)
            .ok_or(ResidencyError::Overflow)?;
        if hbm_after
            .checked_add(incoming_bytes)
            .ok_or(ResidencyError::Overflow)?
            <= self.config.hbm_bytes
        {
            return Ok(HbmAdmissionPlan {
                victims: BTreeMap::new(),
                hbm_bytes: hbm_after + incoming_bytes,
                dram_bytes: dram_after,
            });
        }

        let mut candidates = self
            .entries
            .iter()
            .filter(|(key, entry)| {
                Some(**key) != excluded && entry.residency == Residency::Hbm && entry.pin_count == 0
            })
            .map(|(key, entry)| Ok((*key, entry.last_touch, entry_bytes(&entry.record)?)))
            .collect::<Result<Vec<_>, ResidencyError>>()?;
        candidates.sort_by_key(|(key, last_touch, _)| (*last_touch, *key));

        let mut victims = BTreeMap::new();
        for (key, _, bytes) in candidates {
            if hbm_after
                .checked_add(incoming_bytes)
                .ok_or(ResidencyError::Overflow)?
                <= self.config.hbm_bytes
            {
                break;
            }
            hbm_after = hbm_after
                .checked_sub(bytes)
                .ok_or(ResidencyError::Overflow)?;
            let destination = if dram_after
                .checked_add(bytes)
                .ok_or(ResidencyError::Overflow)?
                <= self.config.dram_bytes
            {
                dram_after += bytes;
                Residency::Dram
            } else {
                Residency::Nvme
            };
            victims.insert(key, destination);
        }
        if hbm_after
            .checked_add(incoming_bytes)
            .ok_or(ResidencyError::Overflow)?
            > self.config.hbm_bytes
        {
            return Err(ResidencyError::Pinned);
        }
        let hbm_bytes = hbm_after
            .checked_add(incoming_bytes)
            .ok_or(ResidencyError::Overflow)?;
        Ok(HbmAdmissionPlan {
            victims,
            hbm_bytes,
            dram_bytes: dram_after,
        })
    }

    fn apply_hbm_admission(&mut self, plan: HbmAdmissionPlan) {
        for (key, entry) in &mut self.entries {
            let Some(destination) = plan.victims.get(key) else {
                continue;
            };
            entry.residency = *destination;
            if *destination == Residency::Nvme {
                entry.restored = None;
            }
        }
        self.hbm_bytes = plan.hbm_bytes;
        self.dram_bytes = plan.dram_bytes;
    }
}

fn entry_bytes(record: &TierRecord) -> Result<u64, ResidencyError> {
    record.pieces.iter().try_fold(0_u64, |total, piece| {
        total
            .checked_add(piece.byte_length)
            .ok_or(ResidencyError::Overflow)
    })
}

#[derive(Debug)]
pub enum RestoreError {
    Config,
    Request,
    Saturated,
    Missing,
    Stale,
    Timeout,
    WorkerClosed,
    Store(StoreError),
    Thread(std::io::Error),
}

impl fmt::Display for RestoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for RestoreError {}

impl From<StoreError> for RestoreError {
    fn from(value: StoreError) -> Self {
        Self::Store(value)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResidencyError {
    Config,
    Record,
    Request,
    Missing,
    State,
    Stale,
    Pinned,
    Overflow,
}

impl fmt::Display for ResidencyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for ResidencyError {}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    use crate::{DurablePageRequest, PagePieceBytes, TierPiece};

    use super::*;

    fn temporary_store(name: &str) -> std::path::PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("glmaxx-{name}-{}-{nonce}", std::process::id()))
    }

    fn page(key: u8) -> DurablePageRequest {
        DurablePageRequest {
            namespace: [0x19; 32],
            page_key: [key; 32],
            generation: 1,
            mtp: false,
            pieces: [TierPiece::TargetKv, TierPiece::TargetIndexer]
                .into_iter()
                .map(|piece| PagePieceBytes {
                    piece,
                    bytes: vec![key; piece.expected_bytes() as usize],
                })
                .collect(),
        }
    }

    fn mtp_page(key: u8) -> DurablePageRequest {
        let mut request = page(key);
        request.mtp = true;
        request.pieces.push(PagePieceBytes {
            piece: TierPiece::DraftSidecar,
            bytes: vec![key; TierPiece::DraftSidecar.expected_bytes() as usize],
        });
        request
    }

    #[test]
    fn bounded_restore_validates_owner_namespace_and_generation() {
        let root = temporary_store("restore-service");
        let mut store = FileTierStore::open(&root).unwrap();
        let record = store.publish(page(0x21)).unwrap();
        drop(store);

        let service = RestoreService::spawn(&root, 1).unwrap();
        let request = RestoreRequest {
            request_id: 7,
            page_key: record.page_key,
            expected_namespace: record.namespace,
            minimum_generation: record.generation,
            page_ordinal: 5,
            worker_rank: 1,
        };
        let handle = service.try_submit(request).unwrap();
        assert_eq!(service.outstanding(), 1);
        assert!(matches!(
            service.try_submit(request),
            Err(RestoreError::Saturated)
        ));
        let result = handle.receive().unwrap();
        assert_eq!(result.request_id, 7);
        assert_eq!(result.page.record, record);
        assert_eq!(service.outstanding(), 0);
        assert!(matches!(
            service.try_submit(RestoreRequest {
                worker_rank: 2,
                ..request
            }),
            Err(RestoreError::Request)
        ));
        drop(service);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn lru_demotes_to_dram_then_nvme_and_never_evicts_pins() {
        let root = temporary_store("residency");
        let mut store = FileTierStore::open(&root).unwrap();
        let first = store.publish(page(0x31)).unwrap();
        let second = store.publish(page(0x32)).unwrap();
        let third = store.publish(page(0x33)).unwrap();
        let page_bytes = entry_bytes(&first).unwrap();
        let mut manager = ResidencyManager::new(ResidencyConfig {
            hbm_bytes: page_bytes,
            dram_bytes: page_bytes,
        })
        .unwrap();
        for record in [&first, &second, &third] {
            manager.register_nvme(record.clone()).unwrap();
        }

        for (request_id, ordinal, record) in [(1, 0, &first), (2, 1, &second)] {
            manager
                .begin_restore(request_id, record.page_key, ordinal, owner_rank(ordinal))
                .unwrap();
            let restored = store.restore(record.page_key).unwrap().unwrap();
            manager
                .complete_restore(RestoreResult {
                    request_id,
                    page_ordinal: ordinal,
                    page: restored,
                })
                .unwrap();
        }
        assert_eq!(manager.location(first.page_key), Some(Residency::Dram));
        assert_eq!(manager.location(second.page_key), Some(Residency::Hbm));
        manager.pin_hbm(second.page_key).unwrap();
        manager
            .begin_restore(3, third.page_key, 2, owner_rank(2))
            .unwrap();
        let restored = store.restore(third.page_key).unwrap().unwrap();
        assert_eq!(
            manager.complete_restore(RestoreResult {
                request_id: 3,
                page_ordinal: 2,
                page: restored,
            }),
            Err(ResidencyError::Pinned)
        );
        manager.abort_restore(third.page_key).unwrap();
        manager.unpin(second.page_key).unwrap();

        manager
            .begin_restore(4, third.page_key, 2, owner_rank(2))
            .unwrap();
        let restored = store.restore(third.page_key).unwrap().unwrap();
        manager
            .complete_restore(RestoreResult {
                request_id: 4,
                page_ordinal: 2,
                page: restored,
            })
            .unwrap();
        assert_eq!(manager.location(second.page_key), Some(Residency::Nvme));
        assert_eq!(manager.location(third.page_key), Some(Residency::Hbm));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn newer_registration_preserves_pins_and_releases_resident_accounting() {
        let root = temporary_store("residency-generation");
        let mut store = FileTierStore::open(&root).unwrap();
        let first = store.publish(page(0x41)).unwrap();
        let page_bytes = entry_bytes(&first).unwrap();
        let mut manager = ResidencyManager::new(ResidencyConfig {
            hbm_bytes: page_bytes,
            dram_bytes: page_bytes,
        })
        .unwrap();
        manager.register_nvme(first.clone()).unwrap();
        manager
            .begin_restore(1, first.page_key, 0, owner_rank(0))
            .unwrap();
        manager
            .complete_restore(RestoreResult {
                request_id: 1,
                page_ordinal: 0,
                page: store.restore(first.page_key).unwrap().unwrap(),
            })
            .unwrap();
        manager.pin_hbm(first.page_key).unwrap();

        let mut newer = first.clone();
        newer.generation = 2;
        assert_eq!(
            manager.validate_nvme_registration(&newer),
            Err(ResidencyError::Pinned)
        );
        assert_eq!(
            manager.register_nvme(newer.clone()),
            Err(ResidencyError::Pinned)
        );
        assert_eq!(manager.location(first.page_key), Some(Residency::Hbm));
        assert_eq!(manager.hbm_bytes(), page_bytes);

        manager.unpin(first.page_key).unwrap();
        manager.register_nvme(newer).unwrap();
        assert_eq!(manager.location(first.page_key), Some(Residency::Nvme));
        assert_eq!(manager.hbm_bytes(), 0);
        assert_eq!(manager.dram_bytes(), 0);
        assert_eq!(manager.register_nvme(first), Err(ResidencyError::Stale));
        assert_eq!(manager.location([0x41; 32]), Some(Residency::Nvme));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn restore_completion_is_bound_to_request_ordinal_and_exact_record() {
        let root = temporary_store("restore-identity");
        let mut store = FileTierStore::open(&root).unwrap();
        let record = store.publish(page(0x51)).unwrap();
        let page_bytes = entry_bytes(&record).unwrap();
        let mut manager = ResidencyManager::new(ResidencyConfig {
            hbm_bytes: page_bytes,
            dram_bytes: page_bytes,
        })
        .unwrap();
        manager.register_nvme(record.clone()).unwrap();

        assert_eq!(
            manager.begin_restore(0, record.page_key, 0, 0),
            Err(ResidencyError::Request)
        );
        assert_eq!(
            manager.begin_restore(7, record.page_key, 0, 1),
            Err(ResidencyError::Request)
        );
        assert_eq!(manager.location(record.page_key), Some(Residency::Nvme));

        manager
            .begin_restore(7, record.page_key, 0, owner_rank(0))
            .unwrap();
        let restored = store.restore(record.page_key).unwrap().unwrap();
        assert_eq!(
            manager.complete_restore(RestoreResult {
                request_id: 8,
                page_ordinal: 0,
                page: restored.clone(),
            }),
            Err(ResidencyError::Stale)
        );
        assert_eq!(
            manager.complete_restore(RestoreResult {
                request_id: 7,
                page_ordinal: 4,
                page: restored.clone(),
            }),
            Err(ResidencyError::Stale)
        );
        let mut wrong_record = restored.clone();
        wrong_record.record.generation += 1;
        assert_eq!(
            manager.complete_restore(RestoreResult {
                request_id: 7,
                page_ordinal: 0,
                page: wrong_record,
            }),
            Err(ResidencyError::Stale)
        );
        assert_eq!(
            manager.location(record.page_key),
            Some(Residency::Restoring)
        );
        assert_eq!(
            manager.abort_restore_identity(record.page_key, 8, 0),
            Err(ResidencyError::State)
        );
        assert_eq!(
            manager.abort_restore_identity(record.page_key, 7, 4),
            Err(ResidencyError::State)
        );
        assert_eq!(
            manager.location(record.page_key),
            Some(Residency::Restoring)
        );

        manager
            .complete_restore(RestoreResult {
                request_id: 7,
                page_ordinal: 0,
                page: restored,
            })
            .unwrap();
        assert_eq!(manager.location(record.page_key), Some(Residency::Hbm));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn failed_multi_victim_admission_does_not_demote_any_page() {
        let root = temporary_store("residency-atomic-room");
        let mut store = FileTierStore::open(&root).unwrap();
        let first = store.publish(page(0x61)).unwrap();
        let second = store.publish(page(0x62)).unwrap();
        let incoming = store.publish(mtp_page(0x63)).unwrap();
        let page_bytes = entry_bytes(&first).unwrap();
        assert_eq!(entry_bytes(&second).unwrap(), page_bytes);
        assert!(entry_bytes(&incoming).unwrap() > page_bytes);

        let hbm_capacity = page_bytes.checked_mul(2).unwrap();
        let mut manager = ResidencyManager::new(ResidencyConfig {
            hbm_bytes: hbm_capacity,
            dram_bytes: hbm_capacity,
        })
        .unwrap();
        for record in [&first, &second, &incoming] {
            manager.register_nvme(record.clone()).unwrap();
        }
        for (request_id, ordinal, record) in [(1, 0, &first), (2, 4, &second)] {
            manager
                .begin_restore(request_id, record.page_key, ordinal, owner_rank(ordinal))
                .unwrap();
            manager
                .complete_restore(RestoreResult {
                    request_id,
                    page_ordinal: ordinal,
                    page: store.restore(record.page_key).unwrap().unwrap(),
                })
                .unwrap();
        }
        manager.pin_hbm(second.page_key).unwrap();
        manager
            .begin_restore(3, incoming.page_key, 8, owner_rank(8))
            .unwrap();
        assert_eq!(
            manager.complete_restore(RestoreResult {
                request_id: 3,
                page_ordinal: 8,
                page: store.restore(incoming.page_key).unwrap().unwrap(),
            }),
            Err(ResidencyError::Pinned)
        );

        assert_eq!(manager.location(first.page_key), Some(Residency::Hbm));
        assert_eq!(manager.location(second.page_key), Some(Residency::Hbm));
        assert_eq!(
            manager.location(incoming.page_key),
            Some(Residency::Restoring)
        );
        assert_eq!(manager.hbm_bytes(), hbm_capacity);
        assert_eq!(manager.dram_bytes(), 0);

        manager.abort_restore(incoming.page_key).unwrap();
        manager.unpin(second.page_key).unwrap();
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn successful_multi_victim_admission_commits_one_bounded_plan() {
        let root = temporary_store("residency-atomic-room-success");
        let mut store = FileTierStore::open(&root).unwrap();
        let first = store.publish(page(0x71)).unwrap();
        let second = store.publish(page(0x72)).unwrap();
        let incoming = store.publish(mtp_page(0x73)).unwrap();
        let page_bytes = entry_bytes(&first).unwrap();
        let incoming_bytes = entry_bytes(&incoming).unwrap();
        let hbm_capacity = page_bytes.checked_mul(2).unwrap();
        let mut manager = ResidencyManager::new(ResidencyConfig {
            hbm_bytes: hbm_capacity,
            dram_bytes: hbm_capacity,
        })
        .unwrap();
        for record in [&first, &second, &incoming] {
            manager.register_nvme(record.clone()).unwrap();
        }
        for (request_id, ordinal, record) in [(1, 0, &first), (2, 4, &second)] {
            manager
                .begin_restore(request_id, record.page_key, ordinal, owner_rank(ordinal))
                .unwrap();
            manager
                .complete_restore(RestoreResult {
                    request_id,
                    page_ordinal: ordinal,
                    page: store.restore(record.page_key).unwrap().unwrap(),
                })
                .unwrap();
        }
        manager
            .begin_restore(3, incoming.page_key, 8, owner_rank(8))
            .unwrap();
        manager
            .complete_restore(RestoreResult {
                request_id: 3,
                page_ordinal: 8,
                page: store.restore(incoming.page_key).unwrap().unwrap(),
            })
            .unwrap();

        assert_eq!(manager.location(first.page_key), Some(Residency::Dram));
        assert_eq!(manager.location(second.page_key), Some(Residency::Dram));
        assert_eq!(manager.location(incoming.page_key), Some(Residency::Hbm));
        assert_eq!(manager.hbm_bytes(), incoming_bytes);
        assert_eq!(manager.dram_bytes(), hbm_capacity);
        fs::remove_dir_all(root).unwrap();
    }
}
