use std::{fmt, path::Path};

use glm_cache::{
    PrefixError, PrefixIndex, PrefixPageKey, Residency, ResidencyConfig, ResidencyError,
    ResidencyManager, RestoreError, RestoreService, TierRecord, owner_rank,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RestoredPrefix {
    pub matched_tokens: u32,
    pub page_keys: Vec<PrefixPageKey>,
}

pub struct PrefixRestoreCoordinator {
    index: PrefixIndex,
    ranks: Vec<ResidencyManager>,
    services: Vec<RestoreService>,
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
        })
    }

    pub fn register_prefix(
        &mut self,
        tokens: &[u32],
        records: Vec<TierRecord>,
    ) -> Result<Vec<PrefixPageKey>, PrefixRestoreError> {
        let keys = self.index.derive_keys(tokens);
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
            self.ranks[usize::from(owner_rank(ordinal))].register_nvme(record.clone())?;
        }
        self.index.insert(tokens, records).map_err(Into::into)
    }

    pub fn restore_longest(
        &mut self,
        request_id: u64,
        tokens: &[u32],
    ) -> Result<RestoredPrefix, PrefixRestoreError> {
        let Some(matched) = self.index.longest_match(tokens) else {
            return Ok(RestoredPrefix {
                matched_tokens: 0,
                page_keys: Vec::new(),
            });
        };
        let mut pinned = Vec::with_capacity(matched.page_keys.len());
        for (ordinal, &key) in matched.page_keys.iter().enumerate() {
            let ordinal = u64::try_from(ordinal).map_err(|_| PrefixRestoreError::Overflow)?;
            if let Err(error) = self.make_hbm(request_id, key, ordinal) {
                self.release(&pinned)?;
                return Err(error);
            }
            pinned.push(key);
        }
        Ok(RestoredPrefix {
            matched_tokens: u32::try_from(matched.matched_tokens)
                .map_err(|_| PrefixRestoreError::Overflow)?,
            page_keys: pinned,
        })
    }

    pub fn release(&mut self, page_keys: &[PrefixPageKey]) -> Result<(), PrefixRestoreError> {
        for (ordinal, key) in page_keys.iter().copied().enumerate().rev() {
            let ordinal = u64::try_from(ordinal).map_err(|_| PrefixRestoreError::Overflow)?;
            self.ranks[usize::from(owner_rank(ordinal))].unpin(key.0)?;
        }
        Ok(())
    }

    #[must_use]
    pub fn location(&self, page_ordinal: u64, key: PrefixPageKey) -> Option<Residency> {
        self.ranks[usize::from(owner_rank(page_ordinal))].location(key.0)
    }

    fn make_hbm(
        &mut self,
        request_id: u64,
        key: PrefixPageKey,
        page_ordinal: u64,
    ) -> Result<(), PrefixRestoreError> {
        let rank = owner_rank(page_ordinal);
        let manager = &mut self.ranks[usize::from(rank)];
        match manager.location(key.0) {
            Some(Residency::Hbm) => manager.pin_hbm(key.0)?,
            Some(Residency::Dram) => {
                manager.promote_dram(key.0)?;
                manager.pin_hbm(key.0)?;
            }
            Some(Residency::Nvme) => {
                let request = manager.begin_restore(request_id, key.0, page_ordinal, rank)?;
                let handle = match self.services[usize::from(rank)].try_submit(request) {
                    Ok(handle) => handle,
                    Err(error) => {
                        manager.abort_restore(key.0)?;
                        return Err(error.into());
                    }
                };
                let restored = match handle.receive() {
                    Ok(restored) => restored,
                    Err(error) => {
                        manager.abort_restore(key.0)?;
                        return Err(error.into());
                    }
                };
                if let Err(error) = manager.complete_restore(restored) {
                    manager.abort_restore(key.0)?;
                    return Err(error.into());
                }
                manager.pin_hbm(key.0)?;
            }
            Some(Residency::Restoring) => return Err(PrefixRestoreError::Busy),
            None => return Err(PrefixRestoreError::Record),
        }
        Ok(())
    }
}

#[derive(Debug)]
pub enum PrefixRestoreError {
    Record,
    Busy,
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
