use std::{collections::BTreeMap, fmt, path::Path, thread, time::Duration};

use glm_cache::{
    PrefixError, PrefixIndex, PrefixPageKey, Residency, ResidencyConfig, ResidencyError,
    ResidencyManager, RestoreError, RestoreHandle, RestoreService, TierRecord, owner_rank,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RestoredPrefix {
    pub matched_tokens: u32,
    pub page_keys: Vec<PrefixPageKey>,
    pub page_has_draft: Vec<bool>,
}

impl RestoredPrefix {
    pub fn validate(&self) -> Result<(), PrefixRestoreError> {
        let page_tokens =
            u32::try_from(glm_cache::PAGE_TOKENS).map_err(|_| PrefixRestoreError::Overflow)?;
        let expected_pages = usize::try_from(self.matched_tokens / page_tokens)
            .map_err(|_| PrefixRestoreError::Overflow)?;
        if !self.matched_tokens.is_multiple_of(page_tokens)
            || expected_pages != self.page_keys.len()
            || self.page_has_draft.len() != self.page_keys.len()
        {
            return Err(PrefixRestoreError::Record);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PrefixRestoreStatus {
    Pending,
    Ready(RestoredPrefix),
}

enum PendingPageState {
    Pinned,
    Resident,
    Restoring(RestoreHandle),
}

struct PendingPage {
    key: PrefixPageKey,
    rank: u8,
    has_draft: bool,
    state: PendingPageState,
}

struct PendingRestore {
    matched_tokens: u32,
    pages: Vec<PendingPage>,
}

pub struct PrefixRestoreCoordinator {
    index: PrefixIndex,
    ranks: Vec<ResidencyManager>,
    services: Vec<RestoreService>,
    pending: BTreeMap<u64, PendingRestore>,
}

pub(crate) struct PrefixReleasePlan {
    entries: BTreeMap<(u8, [u8; 32]), u32>,
}

impl PrefixRestoreCoordinator {
    pub fn new(
        index: PrefixIndex,
        store_root: &Path,
        per_rank_capacity: ResidencyConfig,
        maximum_outstanding_per_rank: usize,
    ) -> Result<Self, PrefixRestoreError> {
        let mut ranks = Vec::with_capacity(4);
        let mut services = Vec::with_capacity(4);
        for _ in 0..4 {
            ranks.push(ResidencyManager::new(per_rank_capacity)?);
            services.push(RestoreService::spawn(
                store_root,
                maximum_outstanding_per_rank,
            )?);
        }
        Ok(Self {
            index,
            ranks,
            services,
            pending: BTreeMap::new(),
        })
    }

    pub fn register_prefix(
        &mut self,
        tokens: &[u32],
        records: Vec<TierRecord>,
    ) -> Result<Vec<PrefixPageKey>, PrefixRestoreError> {
        let mut candidate_index = self.index.clone();
        let keys = candidate_index.insert(tokens, records.clone())?;
        if keys.len() != records.len()
            || keys
                .iter()
                .zip(&records)
                .any(|(key, record)| key.0 != record.page_key)
        {
            return Err(PrefixRestoreError::Record);
        }
        for (ordinal, record) in records.iter().enumerate() {
            let ordinal = u64::try_from(ordinal).map_err(|_| PrefixRestoreError::Overflow)?;
            self.ranks[usize::from(owner_rank(ordinal))].validate_nvme_registration(record)?;
        }
        for (ordinal, record) in records.into_iter().enumerate() {
            let ordinal = u64::try_from(ordinal).map_err(|_| PrefixRestoreError::Overflow)?;
            self.ranks[usize::from(owner_rank(ordinal))].register_nvme(record)?;
        }
        self.index = candidate_index;
        Ok(keys)
    }

    pub fn restore_longest(
        &mut self,
        request_id: u64,
        tokens: &[u32],
    ) -> Result<RestoredPrefix, PrefixRestoreError> {
        match self.begin_restore_longest(request_id, tokens)? {
            PrefixRestoreStatus::Ready(restored) => return Ok(restored),
            PrefixRestoreStatus::Pending => {}
        }
        loop {
            match self.poll_restore(request_id)? {
                PrefixRestoreStatus::Ready(restored) => return Ok(restored),
                PrefixRestoreStatus::Pending => {
                    thread::park_timeout(Duration::from_millis(1));
                }
            }
        }
    }

    /// Starts all missing rank-owned page reads without waiting for NVMe.
    /// Production admission can poll this state while decode work continues.
    pub fn begin_restore_longest(
        &mut self,
        request_id: u64,
        tokens: &[u32],
    ) -> Result<PrefixRestoreStatus, PrefixRestoreError> {
        self.begin_restore_longest_with_capability(request_id, tokens, false)
    }

    pub fn begin_restore_longest_with_capability(
        &mut self,
        request_id: u64,
        tokens: &[u32],
        require_draft: bool,
    ) -> Result<PrefixRestoreStatus, PrefixRestoreError> {
        if request_id == 0 || self.pending.contains_key(&request_id) {
            return Err(PrefixRestoreError::Busy);
        }
        let Some(matched) = self
            .index
            .longest_match_with_capability(tokens, require_draft)
        else {
            return Ok(PrefixRestoreStatus::Ready(RestoredPrefix {
                matched_tokens: 0,
                page_keys: Vec::new(),
                page_has_draft: Vec::new(),
            }));
        };
        let matched_tokens =
            u32::try_from(matched.matched_tokens).map_err(|_| PrefixRestoreError::Overflow)?;
        let mut pending = PendingRestore {
            matched_tokens,
            pages: Vec::with_capacity(matched.page_keys.len()),
        };
        for (ordinal, &key) in matched.page_keys.iter().enumerate() {
            let ordinal = u64::try_from(ordinal).map_err(|_| PrefixRestoreError::Overflow)?;
            let rank = owner_rank(ordinal);
            let has_draft = self
                .index
                .record(key)
                .ok_or(PrefixRestoreError::Record)?
                .mtp;
            let manager = &mut self.ranks[usize::from(rank)];
            let state = match manager.location(key.0) {
                Some(Residency::Hbm) => {
                    if let Err(error) = manager.pin_hbm(key.0) {
                        self.rollback_pending(&pending)?;
                        return Err(error.into());
                    }
                    PendingPageState::Pinned
                }
                Some(Residency::Dram) => {
                    if let Err(error) = manager
                        .promote_dram(key.0)
                        .and_then(|()| manager.pin_hbm(key.0))
                    {
                        self.rollback_pending(&pending)?;
                        return Err(error.into());
                    }
                    PendingPageState::Pinned
                }
                Some(Residency::Nvme) => {
                    let request = match manager.begin_restore(request_id, key.0, ordinal, rank) {
                        Ok(request) => request,
                        Err(error) => {
                            self.rollback_pending(&pending)?;
                            return Err(error.into());
                        }
                    };
                    let handle = match self.services[usize::from(rank)].try_submit(request) {
                        Ok(handle) => handle,
                        Err(error) => {
                            manager.abort_restore(key.0)?;
                            self.rollback_pending(&pending)?;
                            return Err(error.into());
                        }
                    };
                    PendingPageState::Restoring(handle)
                }
                Some(Residency::Restoring) => {
                    self.rollback_pending(&pending)?;
                    return Err(PrefixRestoreError::Busy);
                }
                None => {
                    self.rollback_pending(&pending)?;
                    return Err(PrefixRestoreError::Record);
                }
            };
            pending.pages.push(PendingPage {
                key,
                rank,
                has_draft,
                state,
            });
        }
        if pending
            .pages
            .iter()
            .all(|page| matches!(page.state, PendingPageState::Pinned))
        {
            return Ok(PrefixRestoreStatus::Ready(RestoredPrefix {
                matched_tokens,
                page_keys: pending.pages.iter().map(|page| page.key).collect(),
                page_has_draft: pending.pages.iter().map(|page| page.has_draft).collect(),
            }));
        }
        self.pending.insert(request_id, pending);
        Ok(PrefixRestoreStatus::Pending)
    }

    pub fn poll_restore(
        &mut self,
        request_id: u64,
    ) -> Result<PrefixRestoreStatus, PrefixRestoreError> {
        let mut pending = self
            .pending
            .remove(&request_id)
            .ok_or(PrefixRestoreError::UnknownRequest)?;
        for page in &mut pending.pages {
            let result = match &mut page.state {
                PendingPageState::Restoring(handle) => handle.try_receive(),
                PendingPageState::Pinned | PendingPageState::Resident => continue,
            };
            match result {
                Ok(Some(result)) => {
                    let manager = &mut self.ranks[usize::from(page.rank)];
                    if let Err(error) = manager.complete_restore(result) {
                        self.rollback_pending(&pending)?;
                        return Err(error.into());
                    }
                    page.state = PendingPageState::Resident;
                    if let Err(error) = manager.pin_hbm(page.key.0) {
                        self.rollback_pending(&pending)?;
                        return Err(error.into());
                    }
                    page.state = PendingPageState::Pinned;
                }
                Ok(None) => {}
                Err(error) => {
                    self.rollback_pending(&pending)?;
                    return Err(error.into());
                }
            }
        }
        if pending
            .pages
            .iter()
            .all(|page| matches!(page.state, PendingPageState::Pinned))
        {
            return Ok(PrefixRestoreStatus::Ready(RestoredPrefix {
                matched_tokens: pending.matched_tokens,
                page_keys: pending.pages.iter().map(|page| page.key).collect(),
                page_has_draft: pending.pages.iter().map(|page| page.has_draft).collect(),
            }));
        }
        self.pending.insert(request_id, pending);
        Ok(PrefixRestoreStatus::Pending)
    }

    pub fn cancel_restore(&mut self, request_id: u64) -> Result<(), PrefixRestoreError> {
        let pending = self
            .pending
            .remove(&request_id)
            .ok_or(PrefixRestoreError::UnknownRequest)?;
        self.rollback_pending(&pending)
    }

    #[must_use]
    pub fn pending_restores(&self) -> usize {
        self.pending.len()
    }

    pub fn release(&mut self, page_keys: &[PrefixPageKey]) -> Result<(), PrefixRestoreError> {
        let plan = self.plan_release_many(std::iter::once(page_keys))?;
        self.commit_release(plan);
        Ok(())
    }

    pub(crate) fn plan_release_many<'a>(
        &self,
        page_sets: impl IntoIterator<Item = &'a [PrefixPageKey]>,
    ) -> Result<PrefixReleasePlan, PrefixRestoreError> {
        let mut entries = BTreeMap::new();
        for page_keys in page_sets {
            for (ordinal, key) in page_keys.iter().copied().enumerate() {
                let ordinal = u64::try_from(ordinal).map_err(|_| PrefixRestoreError::Overflow)?;
                let rank = owner_rank(ordinal);
                let count = entries.entry((rank, key.0)).or_insert(0_u32);
                *count = count.checked_add(1).ok_or(PrefixRestoreError::Overflow)?;
            }
        }
        for (&(rank, page_key), &count) in &entries {
            self.ranks[usize::from(rank)].validate_unpin_count(page_key, count)?;
        }
        Ok(PrefixReleasePlan { entries })
    }

    pub(crate) fn commit_release(&mut self, plan: PrefixReleasePlan) {
        for ((rank, page_key), count) in plan.entries {
            self.ranks[usize::from(rank)]
                .unpin_count(page_key, count)
                .expect("prefix release was preflighted under exclusive coordinator access");
        }
    }

    #[must_use]
    pub fn location(&self, page_ordinal: u64, key: PrefixPageKey) -> Option<Residency> {
        self.ranks[usize::from(owner_rank(page_ordinal))].location(key.0)
    }

    fn rollback_pending(&mut self, pending: &PendingRestore) -> Result<(), PrefixRestoreError> {
        let mut first_error = None;
        for page in pending.pages.iter().rev() {
            let result = match &page.state {
                PendingPageState::Pinned => self.ranks[usize::from(page.rank)].unpin(page.key.0),
                PendingPageState::Restoring(_) => {
                    self.ranks[usize::from(page.rank)].abort_restore(page.key.0)
                }
                PendingPageState::Resident => Ok(()),
            };
            if let Err(error) = result
                && first_error.is_none()
            {
                first_error = Some(error);
            }
        }
        first_error.map_or(Ok(()), |error| Err(error.into()))
    }
}

#[derive(Debug)]
pub enum PrefixRestoreError {
    Record,
    Busy,
    UnknownRequest,
    Overflow,
    Prefix(PrefixError),
    Residency(ResidencyError),
    Restore(RestoreError),
}

impl fmt::Display for PrefixRestoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for PrefixRestoreError {}

impl From<PrefixError> for PrefixRestoreError {
    fn from(value: PrefixError) -> Self {
        Self::Prefix(value)
    }
}

impl From<ResidencyError> for PrefixRestoreError {
    fn from(value: ResidencyError) -> Self {
        Self::Residency(value)
    }
}

impl From<RestoreError> for PrefixRestoreError {
    fn from(value: RestoreError) -> Self {
        Self::Restore(value)
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        time::{Duration, Instant, SystemTime, UNIX_EPOCH},
    };

    use glm_cache::{
        DurablePageRequest, FileTierStore, NamespaceInputs, PagePieceBytes, PrefixNamespace,
        TierPiece,
    };

    use super::*;

    fn temporary_store() -> std::path::PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "glmaxx-async-prefix-{}-{nonce}",
            std::process::id()
        ))
    }

    #[test]
    fn restored_prefix_capability_shape_fails_closed() {
        let key = PrefixPageKey([1; 32]);
        assert!(matches!(
            RestoredPrefix {
                matched_tokens: 64,
                page_keys: vec![key],
                page_has_draft: Vec::new(),
            }
            .validate(),
            Err(PrefixRestoreError::Record)
        ));
        assert!(matches!(
            RestoredPrefix {
                matched_tokens: 63,
                page_keys: vec![key],
                page_has_draft: vec![true],
            }
            .validate(),
            Err(PrefixRestoreError::Record)
        ));
    }

    #[test]
    fn multi_page_restore_is_submitted_without_blocking_admission() {
        let root = temporary_store();
        let namespace = PrefixNamespace::new(NamespaceInputs {
            model_revision_sha256: [1; 32],
            tokenizer_sha256: [2; 32],
            chat_template_sha256: [3; 32],
            weight_policy_hash: [4; 32],
            target_kv_abi_sha256: [5; 32],
            draft_kv_abi_sha256: [6; 32],
            rope_parameters_sha256: [7; 32],
        })
        .unwrap();
        let tokens: Vec<u32> = (0..128).collect();
        let index = PrefixIndex::new(namespace);
        let keys = index.derive_keys(&tokens);
        let mut store = FileTierStore::open(&root).unwrap();
        let mut records = Vec::new();
        for (page, key) in keys.iter().enumerate() {
            records.push(
                store
                    .publish(DurablePageRequest {
                        namespace: namespace.0,
                        page_key: key.0,
                        generation: 1,
                        mtp: true,
                        pieces: [
                            TierPiece::TargetKv,
                            TierPiece::TargetIndexer,
                            TierPiece::DraftSidecar,
                        ]
                        .into_iter()
                        .map(|piece| PagePieceBytes {
                            piece,
                            bytes: vec![
                                u8::try_from(page + 1).unwrap();
                                piece.expected_bytes() as usize
                            ],
                        })
                        .collect(),
                    })
                    .unwrap(),
            );
        }
        let page_bytes = records[0]
            .pieces
            .iter()
            .map(|piece| piece.byte_length)
            .sum();
        drop(store);

        let mut coordinator = PrefixRestoreCoordinator::new(
            index,
            &root,
            ResidencyConfig {
                hbm_bytes: page_bytes,
                dram_bytes: page_bytes,
            },
            2,
        )
        .unwrap();
        coordinator
            .register_prefix(&tokens, records.clone())
            .unwrap();
        assert_eq!(
            coordinator.begin_restore_longest(8, &tokens).unwrap(),
            PrefixRestoreStatus::Pending
        );
        assert_eq!(
            coordinator
                .services
                .iter()
                .map(RestoreService::outstanding)
                .sum::<usize>(),
            2
        );
        coordinator.cancel_restore(8).unwrap();
        assert_eq!(coordinator.pending_restores(), 0);
        for (ordinal, key) in keys.iter().copied().enumerate() {
            assert_eq!(
                coordinator.location(u64::try_from(ordinal).unwrap(), key),
                Some(Residency::Nvme)
            );
        }

        assert_eq!(
            coordinator.begin_restore_longest(9, &tokens).unwrap(),
            PrefixRestoreStatus::Pending
        );
        assert!(matches!(
            coordinator.begin_restore_longest(9, &tokens),
            Err(PrefixRestoreError::Busy)
        ));

        let deadline = Instant::now() + Duration::from_secs(5);
        let restored = loop {
            match coordinator.poll_restore(9).unwrap() {
                PrefixRestoreStatus::Pending => {
                    assert!(Instant::now() < deadline, "restore did not complete");
                    thread::yield_now();
                }
                PrefixRestoreStatus::Ready(restored) => break restored,
            }
        };
        assert_eq!(restored.matched_tokens, 128);
        assert_eq!(restored.page_keys, keys);
        assert_eq!(restored.page_has_draft, [true, true]);
        restored.validate().unwrap();
        assert_eq!(
            coordinator
                .services
                .iter()
                .map(RestoreService::outstanding)
                .sum::<usize>(),
            0
        );
        for (ordinal, key) in keys.iter().copied().enumerate() {
            assert_eq!(
                coordinator.location(u64::try_from(ordinal).unwrap(), key),
                Some(Residency::Hbm)
            );
        }
        let bogus_first = PrefixPageKey([0xee; 32]);
        assert!(matches!(
            coordinator.release(&[bogus_first, keys[1]]),
            Err(PrefixRestoreError::Residency(ResidencyError::Missing))
        ));
        let mut newer_second = records[1].clone();
        newer_second.generation = 2;
        assert_eq!(
            coordinator.ranks[1].validate_nvme_registration(&newer_second),
            Err(ResidencyError::Pinned)
        );
        coordinator.release(&keys).unwrap();
        coordinator.ranks[1].pin_hbm(keys[1].0).unwrap();
        let mut newer_records = records;
        for record in &mut newer_records {
            record.generation = 2;
        }
        assert!(matches!(
            coordinator.register_prefix(&tokens, newer_records),
            Err(PrefixRestoreError::Residency(ResidencyError::Pinned))
        ));
        assert_eq!(coordinator.location(0, keys[0]), Some(Residency::Hbm));
        assert_eq!(coordinator.location(1, keys[1]), Some(Residency::Hbm));
        coordinator.ranks[1].unpin(keys[1].0).unwrap();
        assert!(matches!(
            coordinator.poll_restore(9),
            Err(PrefixRestoreError::UnknownRequest)
        ));
        drop(coordinator);
        fs::remove_dir_all(root).unwrap();
    }
}
