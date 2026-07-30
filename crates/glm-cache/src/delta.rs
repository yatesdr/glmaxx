use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};

use sha2::{Digest, Sha256};

use crate::{
    MAXIMUM_CONTEXT_TOKENS, PAGE_TOKENS, PageState, PageTableConfig, PhysicalPageId,
    SequencePageError, SequencePageSnapshot, SequencePageTable, SequencePageView, owner_rank,
};

pub const PAGE_TABLE_DELTA_SCHEMA: &str = "glmaxx.page-table-delta.v1";
pub const MAXIMUM_DELTA_SEQUENCES: usize = 64;

const GLOBAL_HASH_DOMAIN: &[u8] = b"glmaxx.page-table-delta.v1\0";
const LOCAL_HASH_DOMAIN: &[u8] = b"glmaxx.page-table-delta.local.v1\0";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RankPageEntry {
    pub ordinal: u64,
    pub owner_rank: u8,
    pub target_local_page_id: u32,
    pub draft_local_page_id: Option<u32>,
    pub state: PageState,
    pub valid_tokens: u8,
}

impl From<SequencePageView> for RankPageEntry {
    fn from(page: SequencePageView) -> Self {
        Self {
            ordinal: page.ordinal,
            owner_rank: page.physical.owner_rank,
            target_local_page_id: page.physical.target_local_page_id,
            draft_local_page_id: page.draft_local_page_id,
            state: page.state,
            valid_tokens: page.valid_tokens,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SequencePageUpdate {
    request_id: u64,
    mtp: bool,
    committed_tokens: u64,
    tentative_tokens: u8,
    page_count_after: u32,
    first_changed_ordinal: u32,
    changed_pages: Box<[RankPageEntry]>,
}

impl SequencePageUpdate {
    #[must_use]
    pub const fn request_id(&self) -> u64 {
        self.request_id
    }

    #[must_use]
    pub const fn mtp(&self) -> bool {
        self.mtp
    }

    #[must_use]
    pub const fn committed_tokens(&self) -> u64 {
        self.committed_tokens
    }

    #[must_use]
    pub const fn tentative_tokens(&self) -> u8 {
        self.tentative_tokens
    }

    #[must_use]
    pub const fn page_count_after(&self) -> u32 {
        self.page_count_after
    }

    #[must_use]
    pub const fn first_changed_ordinal(&self) -> u32 {
        self.first_changed_ordinal
    }

    #[must_use]
    pub fn changed_pages(&self) -> &[RankPageEntry] {
        &self.changed_pages
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PageTableDelta {
    generation_before: u64,
    generation_after: u64,
    updates: Box<[SequencePageUpdate]>,
    removed_sequence_ids: Box<[u64]>,
    global_digest: [u8; 32],
}

impl PageTableDelta {
    pub fn between(
        before: &SequencePageTable,
        after: &SequencePageTable,
        generation_before: u64,
        generation_after: u64,
    ) -> Result<Self, PageTableDeltaError> {
        validate_generations(generation_before, generation_after)?;
        if before.config() != after.config() {
            return Err(PageTableDeltaError::Shape);
        }
        let before = normalized_snapshots(before)?;
        let after = normalized_snapshots(after)?;
        let removed_sequence_ids: Vec<_> = before
            .keys()
            .filter(|request_id| !after.contains_key(request_id))
            .copied()
            .collect();
        let mut updates = Vec::new();
        for (&request_id, next) in &after {
            let prior = before.get(&request_id);
            if prior == Some(next) {
                continue;
            }
            let first_changed = prior.map_or(0, |prior| common_prefix(&prior.pages, &next.pages));
            let first_changed_ordinal =
                u32::try_from(first_changed).map_err(|_| PageTableDeltaError::Overflow)?;
            let page_count_after =
                u32::try_from(next.pages.len()).map_err(|_| PageTableDeltaError::Overflow)?;
            updates.push(SequencePageUpdate {
                request_id,
                mtp: next.mtp,
                committed_tokens: next.committed_tokens,
                tentative_tokens: next.tentative_tokens,
                page_count_after,
                first_changed_ordinal,
                changed_pages: next.pages[first_changed..].to_vec().into_boxed_slice(),
            });
        }
        let mut delta = Self {
            generation_before,
            generation_after,
            updates: updates.into_boxed_slice(),
            removed_sequence_ids: removed_sequence_ids.into_boxed_slice(),
            global_digest: [0; 32],
        };
        delta.validate_shape()?;
        delta.global_digest = delta.compute_global_digest();
        Ok(delta)
    }

    #[must_use]
    pub const fn generation_before(&self) -> u64 {
        self.generation_before
    }

    #[must_use]
    pub const fn generation_after(&self) -> u64 {
        self.generation_after
    }

    #[must_use]
    pub fn updates(&self) -> &[SequencePageUpdate] {
        &self.updates
    }

    #[must_use]
    pub fn removed_sequence_ids(&self) -> &[u64] {
        &self.removed_sequence_ids
    }

    #[must_use]
    pub const fn global_digest(&self) -> [u8; 32] {
        self.global_digest
    }

    pub fn verify(&self) -> Result<(), PageTableDeltaError> {
        self.validate_shape()?;
        if self.compute_global_digest() != self.global_digest {
            return Err(PageTableDeltaError::Digest);
        }
        Ok(())
    }

    pub fn rank_local_digest(&self, rank: u8) -> Result<[u8; 32], PageTableDeltaError> {
        self.verify()?;
        if rank >= 4 {
            return Err(PageTableDeltaError::Rank);
        }
        let mut hasher = Sha256::new();
        hasher.update(LOCAL_HASH_DOMAIN);
        hasher.update([rank]);
        hasher.update(self.global_digest);
        hash_invariant_fields(&mut hasher, self);
        for update in &self.updates {
            for page in update
                .changed_pages
                .iter()
                .filter(|page| page.owner_rank == rank)
            {
                hash_page(&mut hasher, *page);
            }
        }
        Ok(hasher.finalize().into())
    }

    fn validate_shape(&self) -> Result<(), PageTableDeltaError> {
        validate_generations(self.generation_before, self.generation_after)?;
        if (self.updates.is_empty() && self.removed_sequence_ids.is_empty())
            || self.updates.len() > MAXIMUM_DELTA_SEQUENCES
            || self.removed_sequence_ids.len() > MAXIMUM_DELTA_SEQUENCES
            || self
                .updates
                .windows(2)
                .any(|pair| pair[0].request_id >= pair[1].request_id)
            || self
                .removed_sequence_ids
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
        {
            return Err(PageTableDeltaError::Shape);
        }
        let removed: BTreeSet<_> = self.removed_sequence_ids.iter().copied().collect();
        for update in &self.updates {
            if update.request_id == 0
                || removed.contains(&update.request_id)
                || update.tentative_tokens > 7
                || update.committed_tokens > MAXIMUM_CONTEXT_TOKENS
                || update.first_changed_ordinal > update.page_count_after
                || usize::try_from(update.page_count_after - update.first_changed_ordinal).ok()
                    != Some(update.changed_pages.len())
            {
                return Err(PageTableDeltaError::Shape);
            }
            for (offset, page) in update.changed_pages.iter().enumerate() {
                let expected = u64::from(update.first_changed_ordinal)
                    .checked_add(u64::try_from(offset).map_err(|_| PageTableDeltaError::Overflow)?)
                    .ok_or(PageTableDeltaError::Overflow)?;
                if page.ordinal != expected
                    || page.owner_rank != owner_rank(page.ordinal)
                    || page.valid_tokens == 0
                    || u64::from(page.valid_tokens) > PAGE_TOKENS
                    || page.draft_local_page_id.is_some() != update.mtp
                    || !matches!(
                        page.state,
                        PageState::HbmMutable | PageState::HbmTentative | PageState::HbmSealed
                    )
                {
                    return Err(PageTableDeltaError::Page);
                }
            }
        }
        Ok(())
    }

    fn compute_global_digest(&self) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(GLOBAL_HASH_DOMAIN);
        hash_invariant_fields(&mut hasher, self);
        for update in &self.updates {
            for &page in &update.changed_pages {
                hash_page(&mut hasher, page);
            }
        }
        hasher.finalize().into()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct NormalizedSequence {
    mtp: bool,
    committed_tokens: u64,
    tentative_tokens: u8,
    pages: Vec<RankPageEntry>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PageTableMirror {
    config: PageTableConfig,
    generation: u64,
    sequences: BTreeMap<u64, NormalizedSequence>,
}

impl PageTableMirror {
    pub fn from_table(
        table: &SequencePageTable,
        generation: u64,
    ) -> Result<Self, PageTableDeltaError> {
        if generation == 0 {
            return Err(PageTableDeltaError::Generation);
        }
        let mirror = Self {
            config: table.config(),
            generation,
            sequences: normalized_snapshots(table)?,
        };
        mirror.validate()?;
        Ok(mirror)
    }

    #[must_use]
    pub const fn generation(&self) -> u64 {
        self.generation
    }

    #[must_use]
    pub fn sequence_ids(&self) -> Vec<u64> {
        self.sequences.keys().copied().collect()
    }

    pub fn apply(&mut self, delta: &PageTableDelta) -> Result<(), PageTableDeltaError> {
        delta.verify()?;
        if self.generation != delta.generation_before {
            return Err(PageTableDeltaError::Generation);
        }
        let mut candidate = self.clone();
        for &request_id in &delta.removed_sequence_ids {
            if candidate.sequences.remove(&request_id).is_none() {
                return Err(PageTableDeltaError::Sequence);
            }
        }
        for update in &delta.updates {
            let first_changed = usize::try_from(update.first_changed_ordinal)
                .map_err(|_| PageTableDeltaError::Overflow)?;
            if let Some(sequence) = candidate.sequences.get_mut(&update.request_id) {
                if sequence.mtp != update.mtp || first_changed > sequence.pages.len() {
                    return Err(PageTableDeltaError::Sequence);
                }
                sequence.pages.truncate(first_changed);
                sequence.pages.extend_from_slice(&update.changed_pages);
                sequence.committed_tokens = update.committed_tokens;
                sequence.tentative_tokens = update.tentative_tokens;
            } else {
                if first_changed != 0 {
                    return Err(PageTableDeltaError::Sequence);
                }
                candidate.sequences.insert(
                    update.request_id,
                    NormalizedSequence {
                        mtp: update.mtp,
                        committed_tokens: update.committed_tokens,
                        tentative_tokens: update.tentative_tokens,
                        pages: update.changed_pages.to_vec(),
                    },
                );
            }
            if candidate.sequences[&update.request_id].pages.len()
                != usize::try_from(update.page_count_after)
                    .map_err(|_| PageTableDeltaError::Overflow)?
            {
                return Err(PageTableDeltaError::Shape);
            }
        }
        candidate.generation = delta.generation_after;
        candidate.validate()?;
        *self = candidate;
        Ok(())
    }

    fn validate(&self) -> Result<(), PageTableDeltaError> {
        if self.generation == 0 || self.sequences.len() > MAXIMUM_DELTA_SEQUENCES {
            return Err(PageTableDeltaError::Shape);
        }
        let mut physical = BTreeMap::new();
        let mut draft = BTreeMap::new();
        for (&request_id, sequence) in &self.sequences {
            if request_id == 0 {
                return Err(PageTableDeltaError::Sequence);
            }
            validate_sequence(sequence)?;
            for &page in &sequence.pages {
                let id = PhysicalPageId {
                    owner_rank: page.owner_rank,
                    target_local_page_id: page.target_local_page_id,
                };
                if page.target_local_page_id >= self.config.target_pages_per_rank
                    || page
                        .draft_local_page_id
                        .is_some_and(|draft| draft >= self.config.draft_pages_per_rank)
                {
                    return Err(PageTableDeltaError::Page);
                }
                if let Some(prior) = physical.insert(id, page)
                    && prior != page
                {
                    return Err(PageTableDeltaError::Collision);
                }
                if let Some(draft_local_page_id) = page.draft_local_page_id {
                    let key = (page.owner_rank, draft_local_page_id);
                    if let Some(prior) = draft.insert(key, id)
                        && prior != id
                    {
                        return Err(PageTableDeltaError::Collision);
                    }
                }
            }
        }
        Ok(())
    }
}

fn normalized_snapshots(
    table: &SequencePageTable,
) -> Result<BTreeMap<u64, NormalizedSequence>, PageTableDeltaError> {
    table
        .snapshots()?
        .into_iter()
        .map(normalize_snapshot)
        .collect()
}

fn normalize_snapshot(
    snapshot: SequencePageSnapshot,
) -> Result<(u64, NormalizedSequence), PageTableDeltaError> {
    let sequence = NormalizedSequence {
        mtp: snapshot.mtp,
        committed_tokens: snapshot.committed_tokens,
        tentative_tokens: snapshot.tentative_tokens,
        pages: snapshot.pages.into_iter().map(Into::into).collect(),
    };
    validate_sequence(&sequence)?;
    Ok((snapshot.sequence_id, sequence))
}

fn validate_sequence(sequence: &NormalizedSequence) -> Result<(), PageTableDeltaError> {
    let positions = sequence
        .committed_tokens
        .checked_add(u64::from(sequence.tentative_tokens))
        .filter(|&positions| positions <= MAXIMUM_CONTEXT_TOKENS)
        .ok_or(PageTableDeltaError::Overflow)?;
    let expected_pages = usize::try_from(positions.div_ceil(PAGE_TOKENS))
        .map_err(|_| PageTableDeltaError::Overflow)?;
    if sequence.pages.len() != expected_pages {
        return Err(PageTableDeltaError::Shape);
    }
    let mut valid_positions = 0_u64;
    let mut target_ids = BTreeSet::new();
    let mut draft_ids = BTreeSet::new();
    for (ordinal, page) in sequence.pages.iter().enumerate() {
        let ordinal = u64::try_from(ordinal).map_err(|_| PageTableDeltaError::Overflow)?;
        if page.ordinal != ordinal
            || page.owner_rank != owner_rank(ordinal)
            || page.valid_tokens == 0
            || u64::from(page.valid_tokens) > PAGE_TOKENS
            || page.draft_local_page_id.is_some() != sequence.mtp
            || !target_ids.insert((page.owner_rank, page.target_local_page_id))
            || page
                .draft_local_page_id
                .is_some_and(|draft| !draft_ids.insert((page.owner_rank, draft)))
        {
            return Err(PageTableDeltaError::Page);
        }
        let is_last = usize::try_from(ordinal).ok() == sequence.pages.len().checked_sub(1);
        if sequence.tentative_tokens == 0 {
            let expected_state = if is_last && page.valid_tokens < PAGE_TOKENS as u8 {
                PageState::HbmMutable
            } else {
                PageState::HbmSealed
            };
            if page.state != expected_state {
                return Err(PageTableDeltaError::Page);
            }
        } else if !matches!(page.state, PageState::HbmSealed | PageState::HbmTentative) {
            return Err(PageTableDeltaError::Page);
        }
        if !is_last && page.valid_tokens != PAGE_TOKENS as u8 {
            return Err(PageTableDeltaError::Page);
        }
        valid_positions = valid_positions
            .checked_add(u64::from(page.valid_tokens))
            .ok_or(PageTableDeltaError::Overflow)?;
    }
    if valid_positions != positions
        || (sequence.tentative_tokens != 0
            && !sequence
                .pages
                .iter()
                .any(|page| page.state == PageState::HbmTentative))
    {
        return Err(PageTableDeltaError::Page);
    }
    Ok(())
}

fn common_prefix(left: &[RankPageEntry], right: &[RankPageEntry]) -> usize {
    left.iter()
        .zip(right)
        .take_while(|(left, right)| left == right)
        .count()
}

fn validate_generations(before: u64, after: u64) -> Result<(), PageTableDeltaError> {
    if before == 0 || before.checked_add(1) != Some(after) {
        return Err(PageTableDeltaError::Generation);
    }
    Ok(())
}

fn hash_invariant_fields(hasher: &mut Sha256, delta: &PageTableDelta) {
    hasher.update(delta.generation_before.to_le_bytes());
    hasher.update(delta.generation_after.to_le_bytes());
    hasher.update(
        u32::try_from(delta.updates.len())
            .expect("validated delta update count fits u32")
            .to_le_bytes(),
    );
    hasher.update(
        u32::try_from(delta.removed_sequence_ids.len())
            .expect("validated delta removal count fits u32")
            .to_le_bytes(),
    );
    for update in &delta.updates {
        hasher.update(update.request_id.to_le_bytes());
        hasher.update([u8::from(update.mtp)]);
        hasher.update(update.committed_tokens.to_le_bytes());
        hasher.update([update.tentative_tokens]);
        hasher.update(update.page_count_after.to_le_bytes());
        hasher.update(update.first_changed_ordinal.to_le_bytes());
        hasher.update(
            u32::try_from(update.changed_pages.len())
                .expect("validated changed-page count fits u32")
                .to_le_bytes(),
        );
    }
    for request_id in &delta.removed_sequence_ids {
        hasher.update(request_id.to_le_bytes());
    }
}

fn hash_page(hasher: &mut Sha256, page: RankPageEntry) {
    hasher.update(page.ordinal.to_le_bytes());
    hasher.update([page.owner_rank]);
    hasher.update(page.target_local_page_id.to_le_bytes());
    hasher.update(page.draft_local_page_id.unwrap_or(u32::MAX).to_le_bytes());
    hasher.update([page_state_code(page.state)]);
    hasher.update([page.valid_tokens]);
}

const fn page_state_code(state: PageState) -> u8 {
    match state {
        PageState::Free => 0,
        PageState::HbmMutable => 1,
        PageState::HbmTentative => 2,
        PageState::HbmSealed => 3,
        PageState::DramWriting => 4,
        PageState::DramResident => 5,
        PageState::NvmeWriting => 6,
        PageState::NvmeResident => 7,
        PageState::Restoring => 8,
        PageState::Invalid => 9,
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum PageTableDeltaError {
    Generation,
    Shape,
    Sequence,
    Page,
    Rank,
    Collision,
    Digest,
    Overflow,
    Table(SequencePageError),
}

impl fmt::Display for PageTableDeltaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for PageTableDeltaError {}

impl From<SequencePageError> for PageTableDeltaError {
    fn from(value: SequencePageError) -> Self {
        Self::Table(value)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use crate::PageTableConfig;

    use super::*;

    fn table() -> SequencePageTable {
        SequencePageTable::new(PageTableConfig {
            target_pages_per_rank: 8,
            draft_pages_per_rank: 8,
        })
        .unwrap()
    }

    #[test]
    fn delta_reconstructs_tentative_admission_and_removal_atomically() {
        let mut before = table();
        before.admit_with_prefix(1, true, &[]).unwrap();
        before.append_committed(1, 63).unwrap();
        before.admit_with_prefix(2, false, &[]).unwrap();
        before.append_committed(2, 64).unwrap();

        let mut after = before.clone();
        after.begin_tentative(1, 7).unwrap();
        after.remove_sequence(2).unwrap();
        after.admit_with_prefix(3, false, &[]).unwrap();
        after.append_committed(3, 65).unwrap();

        let delta = PageTableDelta::between(&before, &after, 10, 11).unwrap();
        delta.verify().unwrap();
        assert_eq!(delta.removed_sequence_ids(), [2]);
        assert_eq!(
            delta
                .updates()
                .iter()
                .map(SequencePageUpdate::request_id)
                .collect::<Vec<_>>(),
            [1, 3]
        );
        assert_eq!(delta.updates()[0].tentative_tokens(), 7);
        assert_eq!(delta.updates()[0].first_changed_ordinal(), 0);
        assert_eq!(delta.updates()[0].page_count_after(), 2);
        assert_eq!(delta.updates()[1].first_changed_ordinal(), 0);
        assert_eq!(delta.updates()[1].page_count_after(), 2);

        let local_digests: BTreeSet<_> = (0..4)
            .map(|rank| delta.rank_local_digest(rank).unwrap())
            .collect();
        assert_eq!(local_digests.len(), 4);

        let mut mirror = PageTableMirror::from_table(&before, 10).unwrap();
        mirror.apply(&delta).unwrap();
        assert_eq!(mirror, PageTableMirror::from_table(&after, 11).unwrap());
        assert_eq!(mirror.sequence_ids(), [1, 3]);
    }

    #[test]
    fn unchanged_prefix_is_omitted_and_digest_tampering_fails_closed() {
        let mut before = table();
        before.admit_with_prefix(1, true, &[]).unwrap();
        before.append_committed(1, 128).unwrap();
        let mut after = before.clone();
        after.append_committed(1, 1).unwrap();

        let delta = PageTableDelta::between(&before, &after, 7, 8).unwrap();
        assert_eq!(delta.updates().len(), 1);
        assert_eq!(delta.updates()[0].first_changed_ordinal(), 2);
        assert_eq!(delta.updates()[0].changed_pages().len(), 1);
        let mut mirror = PageTableMirror::from_table(&before, 7).unwrap();
        mirror.apply(&delta).unwrap();
        assert_eq!(mirror, PageTableMirror::from_table(&after, 8).unwrap());

        let mut digest_tamper = delta.clone();
        digest_tamper.global_digest[0] ^= 1;
        assert_eq!(digest_tamper.verify(), Err(PageTableDeltaError::Digest));
        let unchanged = mirror.clone();
        assert_eq!(
            mirror.apply(&digest_tamper),
            Err(PageTableDeltaError::Digest)
        );
        assert_eq!(mirror, unchanged);
    }

    #[test]
    fn generation_shape_owner_and_noop_deltas_are_rejected() {
        let mut before = table();
        before.admit_with_prefix(1, false, &[]).unwrap();
        let mut after = before.clone();
        after.append_committed(1, 1).unwrap();

        assert_eq!(
            PageTableDelta::between(&before, &after, 0, 1),
            Err(PageTableDeltaError::Generation)
        );
        assert_eq!(
            PageTableDelta::between(&before, &after, 4, 6),
            Err(PageTableDeltaError::Generation)
        );
        assert_eq!(
            PageTableDelta::between(&before, &before, 4, 5),
            Err(PageTableDeltaError::Shape)
        );

        let mut owner_tamper = PageTableDelta::between(&before, &after, 4, 5).unwrap();
        owner_tamper.updates[0].changed_pages[0].owner_rank ^= 1;
        owner_tamper.global_digest = owner_tamper.compute_global_digest();
        assert_eq!(owner_tamper.verify(), Err(PageTableDeltaError::Page));

        let delta = PageTableDelta::between(&before, &after, 4, 5).unwrap();
        let mut arena_tamper = delta.clone();
        arena_tamper.updates[0].changed_pages[0].target_local_page_id = 8;
        arena_tamper.global_digest = arena_tamper.compute_global_digest();
        arena_tamper.verify().unwrap();
        let mut bounded_mirror = PageTableMirror::from_table(&before, 4).unwrap();
        let unchanged = bounded_mirror.clone();
        assert_eq!(
            bounded_mirror.apply(&arena_tamper),
            Err(PageTableDeltaError::Page)
        );
        assert_eq!(bounded_mirror, unchanged);

        let mut stale_mirror = PageTableMirror::from_table(&before, 3).unwrap();
        let unchanged = stale_mirror.clone();
        assert_eq!(
            stale_mirror.apply(&delta),
            Err(PageTableDeltaError::Generation)
        );
        assert_eq!(stale_mirror, unchanged);
    }
}
