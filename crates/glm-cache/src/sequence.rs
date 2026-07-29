use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};

use crate::{PAGE_TOKENS, PageState, PrefixPageKey, owner_rank};

pub const MAXIMUM_CONTEXT_TOKENS: u64 = 1_048_576;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct PhysicalPageId {
    pub owner_rank: u8,
    pub target_local_page_id: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PageTableConfig {
    pub target_pages_per_rank: u32,
    pub draft_pages_per_rank: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SequencePageView {
    pub ordinal: u64,
    pub physical: PhysicalPageId,
    pub draft_local_page_id: Option<u32>,
    pub state: PageState,
    pub valid_tokens: u8,
    pub references: u32,
    pub shared_prefix: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PageTableStats {
    pub target_pages_used: [u32; 4],
    pub draft_pages_used: [u32; 4],
    pub active_sequences: u32,
    pub active_positions: u64,
    pub maximum_target_only_sequence_tokens: u64,
    pub maximum_mtp_sequence_tokens: u64,
}

#[derive(Clone, Debug)]
struct PhysicalPage {
    draft_local_page_id: Option<u32>,
    state: PageState,
    valid_tokens: u8,
    references: u32,
    prefix: Option<(PrefixPageKey, u64)>,
}

#[derive(Clone, Copy, Debug)]
struct SequencePage {
    ordinal: u64,
    physical: PhysicalPageId,
}

#[derive(Clone, Debug)]
struct TentativeTail {
    original_committed_tokens: u64,
    original_page_count: usize,
    original_tail_tokens: u8,
    reserved_tokens: u8,
}

#[derive(Clone, Debug)]
struct Sequence {
    mtp: bool,
    committed_tokens: u64,
    pages: Vec<SequencePage>,
    tentative: Option<TentativeTail>,
}

/// CPU metadata oracle for the active DCP4 KV page table.
///
/// Payload bytes remain owned by the eventual rank allocator. This type
/// proves slot capacity, ownership, sharing, COW, and transactional reachability.
#[derive(Clone, Debug)]
pub struct SequencePageTable {
    config: PageTableConfig,
    free_target: [BTreeSet<u32>; 4],
    free_draft: [BTreeSet<u32>; 4],
    physical: BTreeMap<PhysicalPageId, PhysicalPage>,
    prefixes: BTreeMap<PrefixPageKey, PhysicalPageId>,
    sequences: BTreeMap<u64, Sequence>,
}

impl SequencePageTable {
    pub fn new(config: PageTableConfig) -> Result<Self, SequencePageError> {
        if config.target_pages_per_rank == 0
            || config.target_pages_per_rank > 1_048_576
            || config.draft_pages_per_rank > config.target_pages_per_rank
        {
            return Err(SequencePageError::Config);
        }
        let free_target = std::array::from_fn(|_| (0..config.target_pages_per_rank).collect());
        let free_draft = std::array::from_fn(|_| (0..config.draft_pages_per_rank).collect());
        Ok(Self {
            config,
            free_target,
            free_draft,
            physical: BTreeMap::new(),
            prefixes: BTreeMap::new(),
            sequences: BTreeMap::new(),
        })
    }

    /// Attaches only complete sealed pages. Missing keys allocate restored HBM
    /// slots; existing keys acquire another active reference.
    pub fn admit_with_prefix(
        &mut self,
        sequence_id: u64,
        mtp: bool,
        prefix_pages: &[(PrefixPageKey, bool)],
    ) -> Result<(), SequencePageError> {
        if sequence_id == 0
            || self.sequences.contains_key(&sequence_id)
            || prefix_pages.len() > maximum_pages()
        {
            return Err(SequencePageError::Sequence);
        }
        let snapshot = self.clone();
        let result = (|| {
            let mut pages = Vec::with_capacity(prefix_pages.len());
            let mut seen = BTreeSet::new();
            for (ordinal, &(key, has_draft)) in prefix_pages.iter().enumerate() {
                if mtp && !has_draft {
                    return Err(SequencePageError::MissingDraft);
                }
                if !seen.insert(key) {
                    return Err(SequencePageError::Prefix);
                }
                let ordinal = u64::try_from(ordinal).map_err(|_| SequencePageError::Overflow)?;
                let physical = if let Some(&physical) = self.prefixes.get(&key) {
                    let valid = self.physical.get(&physical).is_some_and(|page| {
                        page.state == PageState::HbmSealed
                            && page.valid_tokens == PAGE_TOKENS as u8
                            && physical.owner_rank == owner_rank(ordinal)
                            && page.prefix == Some((key, ordinal))
                    });
                    if !valid {
                        return Err(SequencePageError::Prefix);
                    }
                    if mtp && self.physical[&physical].draft_local_page_id.is_none() {
                        let draft = self.free_draft[usize::from(physical.owner_rank)]
                            .pop_first()
                            .ok_or(SequencePageError::Capacity)?;
                        self.physical
                            .get_mut(&physical)
                            .ok_or(SequencePageError::Invariant)?
                            .draft_local_page_id = Some(draft);
                    }
                    let page = self
                        .physical
                        .get_mut(&physical)
                        .ok_or(SequencePageError::Invariant)?;
                    page.references = page
                        .references
                        .checked_add(1)
                        .ok_or(SequencePageError::Overflow)?;
                    physical
                } else {
                    let physical = self.allocate_page(ordinal, mtp)?;
                    let page = self
                        .physical
                        .get_mut(&physical)
                        .ok_or(SequencePageError::Invariant)?;
                    page.valid_tokens = PAGE_TOKENS as u8;
                    page.state = page.state.transition(PageState::HbmSealed)?;
                    page.prefix = Some((key, ordinal));
                    self.prefixes.insert(key, physical);
                    physical
                };
                pages.push(SequencePage { ordinal, physical });
            }
            let committed_tokens = u64::try_from(prefix_pages.len())
                .ok()
                .and_then(|pages| pages.checked_mul(PAGE_TOKENS))
                .ok_or(SequencePageError::Overflow)?;
            self.sequences.insert(
                sequence_id,
                Sequence {
                    mtp,
                    committed_tokens,
                    pages,
                    tentative: None,
                },
            );
            Ok(())
        })();
        if let Err(error) = result {
            *self = snapshot;
            return Err(error);
        }
        Ok(())
    }

    pub fn append_committed(
        &mut self,
        sequence_id: u64,
        token_count: u64,
    ) -> Result<(), SequencePageError> {
        if token_count == 0 {
            return Ok(());
        }
        let snapshot = self.clone();
        let result = self.append_committed_inner(sequence_id, token_count);
        if let Err(error) = result {
            *self = snapshot;
            return Err(error);
        }
        Ok(())
    }

    /// Forks a session. Full sealed pages become shared; a partial mutable
    /// tail receives a private physical page on its deterministic owner.
    pub fn fork_sequence(
        &mut self,
        source_id: u64,
        destination_id: u64,
    ) -> Result<(), SequencePageError> {
        if destination_id == 0 || self.sequences.contains_key(&destination_id) {
            return Err(SequencePageError::Sequence);
        }
        let source = self
            .sequences
            .get(&source_id)
            .cloned()
            .ok_or(SequencePageError::Sequence)?;
        if source.tentative.is_some() {
            return Err(SequencePageError::Transaction);
        }
        let snapshot = self.clone();
        let result = (|| {
            let mut pages = Vec::with_capacity(source.pages.len());
            for source_page in &source.pages {
                let page = self
                    .physical
                    .get(&source_page.physical)
                    .cloned()
                    .ok_or(SequencePageError::Invariant)?;
                if page.state == PageState::HbmSealed && page.valid_tokens == PAGE_TOKENS as u8 {
                    let shared = self
                        .physical
                        .get_mut(&source_page.physical)
                        .ok_or(SequencePageError::Invariant)?;
                    shared.references = shared
                        .references
                        .checked_add(1)
                        .ok_or(SequencePageError::Overflow)?;
                    pages.push(*source_page);
                } else if page.state == PageState::HbmMutable
                    && page.valid_tokens < PAGE_TOKENS as u8
                    && page.references == 1
                {
                    let physical = self.allocate_page(source_page.ordinal, source.mtp)?;
                    self.physical
                        .get_mut(&physical)
                        .ok_or(SequencePageError::Invariant)?
                        .valid_tokens = page.valid_tokens;
                    pages.push(SequencePage {
                        ordinal: source_page.ordinal,
                        physical,
                    });
                } else {
                    return Err(SequencePageError::State);
                }
            }
            self.sequences.insert(
                destination_id,
                Sequence {
                    mtp: source.mtp,
                    committed_tokens: source.committed_tokens,
                    pages,
                    tentative: None,
                },
            );
            Ok(())
        })();
        if let Err(error) = result {
            *self = snapshot;
            return Err(error);
        }
        Ok(())
    }

    pub fn begin_tentative(
        &mut self,
        sequence_id: u64,
        requested_tokens: u8,
    ) -> Result<(), SequencePageError> {
        if requested_tokens == 0 || requested_tokens > 7 {
            return Err(SequencePageError::Transaction);
        }
        let snapshot = self.clone();
        let result = (|| {
            let sequence = self
                .sequences
                .get(&sequence_id)
                .ok_or(SequencePageError::Sequence)?;
            if sequence.tentative.is_some()
                || sequence
                    .committed_tokens
                    .checked_add(u64::from(requested_tokens))
                    .is_none_or(|tokens| tokens > MAXIMUM_CONTEXT_TOKENS)
            {
                return Err(SequencePageError::Transaction);
            }
            let original_committed_tokens = sequence.committed_tokens;
            let original_page_count = sequence.pages.len();
            let original_tail_tokens = sequence
                .pages
                .last()
                .and_then(|page| self.physical.get(&page.physical))
                .map_or(0, |page| page.valid_tokens);
            self.reserve_tentative(sequence_id, requested_tokens)?;
            self.sequences
                .get_mut(&sequence_id)
                .ok_or(SequencePageError::Sequence)?
                .tentative = Some(TentativeTail {
                original_committed_tokens,
                original_page_count,
                original_tail_tokens,
                reserved_tokens: requested_tokens,
            });
            Ok(())
        })();
        if let Err(error) = result {
            *self = snapshot;
            return Err(error);
        }
        Ok(())
    }

    pub fn commit_tentative(
        &mut self,
        sequence_id: u64,
        committed_tokens: u8,
    ) -> Result<(), SequencePageError> {
        let transaction = self
            .sequences
            .get(&sequence_id)
            .and_then(|sequence| sequence.tentative.clone())
            .ok_or(SequencePageError::Transaction)?;
        if committed_tokens == 0 || committed_tokens > transaction.reserved_tokens {
            return Err(SequencePageError::Transaction);
        }
        let snapshot = self.clone();
        let result = self
            .rollback_tentative_inner(sequence_id)
            .and_then(|()| self.append_committed_inner(sequence_id, u64::from(committed_tokens)));
        if let Err(error) = result {
            *self = snapshot;
            return Err(error);
        }
        Ok(())
    }

    pub fn rollback_tentative(&mut self, sequence_id: u64) -> Result<(), SequencePageError> {
        let snapshot = self.clone();
        let result = self.rollback_tentative_inner(sequence_id);
        if let Err(error) = result {
            *self = snapshot;
            return Err(error);
        }
        Ok(())
    }

    pub fn remove_sequence(&mut self, sequence_id: u64) -> Result<(), SequencePageError> {
        let snapshot = self.clone();
        let result = (|| {
            let sequence = self
                .sequences
                .remove(&sequence_id)
                .ok_or(SequencePageError::Sequence)?;
            if sequence.tentative.is_some() {
                return Err(SequencePageError::Transaction);
            }
            for page in sequence.pages.into_iter().rev() {
                self.release_page(page.physical)?;
            }
            Ok(())
        })();
        if let Err(error) = result {
            *self = snapshot;
            return Err(error);
        }
        Ok(())
    }

    #[must_use]
    pub fn committed_tokens(&self, sequence_id: u64) -> Option<u64> {
        self.sequences
            .get(&sequence_id)
            .map(|sequence| sequence.committed_tokens)
    }

    pub fn pages(&self, sequence_id: u64) -> Result<Vec<SequencePageView>, SequencePageError> {
        let sequence = self
            .sequences
            .get(&sequence_id)
            .ok_or(SequencePageError::Sequence)?;
        sequence
            .pages
            .iter()
            .map(|entry| {
                let page = self
                    .physical
                    .get(&entry.physical)
                    .ok_or(SequencePageError::Invariant)?;
                Ok(SequencePageView {
                    ordinal: entry.ordinal,
                    physical: entry.physical,
                    draft_local_page_id: page.draft_local_page_id,
                    state: page.state,
                    valid_tokens: page.valid_tokens,
                    references: page.references,
                    shared_prefix: page.prefix.is_some(),
                })
            })
            .collect()
    }

    pub fn stats(&self) -> Result<PageTableStats, SequencePageError> {
        let mut target_pages_used = [0_u32; 4];
        let mut draft_pages_used = [0_u32; 4];
        for (physical, page) in &self.physical {
            target_pages_used[usize::from(physical.owner_rank)] += 1;
            if page.draft_local_page_id.is_some() {
                draft_pages_used[usize::from(physical.owner_rank)] += 1;
            }
        }
        let active_positions = self.sequences.values().try_fold(0_u64, |total, sequence| {
            total
                .checked_add(sequence.committed_tokens)
                .ok_or(SequencePageError::Overflow)
        })?;
        Ok(PageTableStats {
            target_pages_used,
            draft_pages_used,
            active_sequences: u32::try_from(self.sequences.len())
                .map_err(|_| SequencePageError::Overflow)?,
            active_positions,
            maximum_target_only_sequence_tokens: maximum_sequence_tokens(
                self.config.target_pages_per_rank,
            )?,
            maximum_mtp_sequence_tokens: maximum_sequence_tokens(
                self.config
                    .target_pages_per_rank
                    .min(self.config.draft_pages_per_rank),
            )?,
        })
    }

    fn append_committed_inner(
        &mut self,
        sequence_id: u64,
        token_count: u64,
    ) -> Result<(), SequencePageError> {
        let sequence = self
            .sequences
            .get(&sequence_id)
            .ok_or(SequencePageError::Sequence)?;
        if sequence.tentative.is_some()
            || sequence
                .committed_tokens
                .checked_add(token_count)
                .is_none_or(|tokens| tokens > MAXIMUM_CONTEXT_TOKENS)
        {
            return Err(SequencePageError::Transaction);
        }
        for _ in 0..token_count {
            let need_page = self.sequences[&sequence_id]
                .pages
                .last()
                .is_none_or(|entry| {
                    self.physical[&entry.physical].valid_tokens == PAGE_TOKENS as u8
                });
            if need_page {
                let sequence = &self.sequences[&sequence_id];
                let ordinal =
                    u64::try_from(sequence.pages.len()).map_err(|_| SequencePageError::Overflow)?;
                let physical = self.allocate_page(ordinal, sequence.mtp)?;
                self.sequences
                    .get_mut(&sequence_id)
                    .ok_or(SequencePageError::Sequence)?
                    .pages
                    .push(SequencePage { ordinal, physical });
            }
            let physical = self.sequences[&sequence_id]
                .pages
                .last()
                .ok_or(SequencePageError::Invariant)?
                .physical;
            let page = self
                .physical
                .get_mut(&physical)
                .ok_or(SequencePageError::Invariant)?;
            if page.state != PageState::HbmMutable || page.references != 1 {
                return Err(SequencePageError::State);
            }
            page.valid_tokens = page
                .valid_tokens
                .checked_add(1)
                .ok_or(SequencePageError::Overflow)?;
            if page.valid_tokens == PAGE_TOKENS as u8 {
                page.state = page.state.transition(PageState::HbmSealed)?;
            }
            self.sequences
                .get_mut(&sequence_id)
                .ok_or(SequencePageError::Sequence)?
                .committed_tokens += 1;
        }
        Ok(())
    }

    fn reserve_tentative(
        &mut self,
        sequence_id: u64,
        requested_tokens: u8,
    ) -> Result<(), SequencePageError> {
        for _ in 0..requested_tokens {
            let need_page = self.sequences[&sequence_id]
                .pages
                .last()
                .is_none_or(|entry| {
                    self.physical[&entry.physical].valid_tokens == PAGE_TOKENS as u8
                });
            if need_page {
                let sequence = &self.sequences[&sequence_id];
                let ordinal =
                    u64::try_from(sequence.pages.len()).map_err(|_| SequencePageError::Overflow)?;
                let physical = self.allocate_page(ordinal, sequence.mtp)?;
                self.sequences
                    .get_mut(&sequence_id)
                    .ok_or(SequencePageError::Sequence)?
                    .pages
                    .push(SequencePage { ordinal, physical });
            }
            let physical = self.sequences[&sequence_id]
                .pages
                .last()
                .ok_or(SequencePageError::Invariant)?
                .physical;
            let page = self
                .physical
                .get_mut(&physical)
                .ok_or(SequencePageError::Invariant)?;
            if page.references != 1
                || !matches!(page.state, PageState::HbmMutable | PageState::HbmTentative)
            {
                return Err(SequencePageError::State);
            }
            if page.state == PageState::HbmMutable {
                page.state = page.state.transition(PageState::HbmTentative)?;
            }
            page.valid_tokens = page
                .valid_tokens
                .checked_add(1)
                .ok_or(SequencePageError::Overflow)?;
        }
        Ok(())
    }

    fn rollback_tentative_inner(&mut self, sequence_id: u64) -> Result<(), SequencePageError> {
        let transaction = self
            .sequences
            .get_mut(&sequence_id)
            .and_then(|sequence| sequence.tentative.take())
            .ok_or(SequencePageError::Transaction)?;
        let extra_pages = self.sequences[&sequence_id]
            .pages
            .len()
            .checked_sub(transaction.original_page_count)
            .ok_or(SequencePageError::Invariant)?;
        for _ in 0..extra_pages {
            let page = self
                .sequences
                .get_mut(&sequence_id)
                .ok_or(SequencePageError::Sequence)?
                .pages
                .pop()
                .ok_or(SequencePageError::Invariant)?;
            self.release_page(page.physical)?;
        }
        if transaction.original_page_count != 0 {
            let physical = self.sequences[&sequence_id]
                .pages
                .last()
                .ok_or(SequencePageError::Invariant)?
                .physical;
            let page = self
                .physical
                .get_mut(&physical)
                .ok_or(SequencePageError::Invariant)?;
            if page.state == PageState::HbmTentative {
                page.state = page.state.transition(PageState::HbmMutable)?;
                page.valid_tokens = transaction.original_tail_tokens;
            } else if page.valid_tokens != PAGE_TOKENS as u8 || page.state != PageState::HbmSealed {
                return Err(SequencePageError::State);
            }
        }
        self.sequences
            .get_mut(&sequence_id)
            .ok_or(SequencePageError::Sequence)?
            .committed_tokens = transaction.original_committed_tokens;
        Ok(())
    }

    fn allocate_page(
        &mut self,
        ordinal: u64,
        mtp: bool,
    ) -> Result<PhysicalPageId, SequencePageError> {
        let rank = owner_rank(ordinal);
        let target_local_page_id = self.free_target[usize::from(rank)]
            .pop_first()
            .ok_or(SequencePageError::Capacity)?;
        let draft_local_page_id = if mtp {
            match self.free_draft[usize::from(rank)].pop_first() {
                Some(page) => Some(page),
                None => {
                    self.free_target[usize::from(rank)].insert(target_local_page_id);
                    return Err(SequencePageError::Capacity);
                }
            }
        } else {
            None
        };
        let physical = PhysicalPageId {
            owner_rank: rank,
            target_local_page_id,
        };
        if self
            .physical
            .insert(
                physical,
                PhysicalPage {
                    draft_local_page_id,
                    state: PageState::Free.transition(PageState::HbmMutable)?,
                    valid_tokens: 0,
                    references: 1,
                    prefix: None,
                },
            )
            .is_some()
        {
            return Err(SequencePageError::Invariant);
        }
        Ok(physical)
    }

    fn release_page(&mut self, physical: PhysicalPageId) -> Result<(), SequencePageError> {
        let page = self
            .physical
            .get_mut(&physical)
            .ok_or(SequencePageError::Invariant)?;
        page.references = page
            .references
            .checked_sub(1)
            .ok_or(SequencePageError::Invariant)?;
        if page.references != 0 {
            return Ok(());
        }
        let page = self
            .physical
            .remove(&physical)
            .ok_or(SequencePageError::Invariant)?;
        if let Some((key, _ordinal)) = page.prefix
            && self.prefixes.remove(&key) != Some(physical)
        {
            return Err(SequencePageError::Invariant);
        }
        self.free_target[usize::from(physical.owner_rank)].insert(physical.target_local_page_id);
        if let Some(draft) = page.draft_local_page_id {
            self.free_draft[usize::from(physical.owner_rank)].insert(draft);
        }
        Ok(())
    }
}

const fn maximum_pages() -> usize {
    (MAXIMUM_CONTEXT_TOKENS / PAGE_TOKENS) as usize
}

fn maximum_sequence_tokens(pages_per_rank: u32) -> Result<u64, SequencePageError> {
    u64::from(pages_per_rank)
        .checked_mul(4)
        .and_then(|pages| pages.checked_mul(PAGE_TOKENS))
        .map(|tokens| tokens.min(MAXIMUM_CONTEXT_TOKENS))
        .ok_or(SequencePageError::Overflow)
}

#[derive(Debug, Eq, PartialEq)]
pub enum SequencePageError {
    Config,
    Sequence,
    Prefix,
    MissingDraft,
    Capacity,
    State,
    Transaction,
    Invariant,
    Overflow,
    Transition(crate::PageTransitionError),
}

impl fmt::Display for SequencePageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for SequencePageError {}

impl From<crate::PageTransitionError> for SequencePageError {
    fn from(value: crate::PageTransitionError) -> Self {
        Self::Transition(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_million_positions_fill_exactly_balanced_dcp4_slots() {
        let mut table = SequencePageTable::new(PageTableConfig {
            target_pages_per_rank: 4_096,
            draft_pages_per_rank: 4_096,
        })
        .unwrap();
        table.admit_with_prefix(1, true, &[]).unwrap();
        table.append_committed(1, MAXIMUM_CONTEXT_TOKENS).unwrap();
        let stats = table.stats().unwrap();
        assert_eq!(stats.target_pages_used, [4_096; 4]);
        assert_eq!(stats.draft_pages_used, [4_096; 4]);
        assert_eq!(
            stats.maximum_target_only_sequence_tokens,
            MAXIMUM_CONTEXT_TOKENS
        );
        assert_eq!(stats.maximum_mtp_sequence_tokens, MAXIMUM_CONTEXT_TOKENS);
        assert_eq!(
            table.append_committed(1, 1),
            Err(SequencePageError::Transaction)
        );
    }

    #[test]
    fn fork_shares_sealed_pages_and_copies_a_mutable_tail() {
        let mut table = SequencePageTable::new(PageTableConfig {
            target_pages_per_rank: 4,
            draft_pages_per_rank: 4,
        })
        .unwrap();
        table
            .admit_with_prefix(1, true, &[(PrefixPageKey([9; 32]), true)])
            .unwrap();
        table.append_committed(1, 10).unwrap();
        table.fork_sequence(1, 2).unwrap();
        let first = table.pages(1).unwrap();
        let second = table.pages(2).unwrap();
        assert_eq!(first[0].physical, second[0].physical);
        assert_eq!(first[0].references, 2);
        assert_ne!(first[1].physical, second[1].physical);
        assert_eq!(first[1].valid_tokens, 10);
        assert_eq!(second[1].valid_tokens, 10);
        table.append_committed(1, 1).unwrap();
        assert_eq!(table.pages(1).unwrap()[1].valid_tokens, 11);
        assert_eq!(table.pages(2).unwrap()[1].valid_tokens, 10);
    }

    #[test]
    fn tentative_target_and_draft_pages_commit_or_rollback_together() {
        let mut table = SequencePageTable::new(PageTableConfig {
            target_pages_per_rank: 2,
            draft_pages_per_rank: 2,
        })
        .unwrap();
        table.admit_with_prefix(1, true, &[]).unwrap();
        table.append_committed(1, 63).unwrap();
        let before = table.pages(1).unwrap();
        table.begin_tentative(1, 7).unwrap();
        let tentative = table.pages(1).unwrap();
        assert_eq!(tentative.len(), 2);
        assert!(
            tentative
                .iter()
                .all(|page| page.state == PageState::HbmTentative)
        );
        table.rollback_tentative(1).unwrap();
        assert_eq!(table.pages(1).unwrap(), before);
        assert_eq!(table.committed_tokens(1), Some(63));

        table.begin_tentative(1, 7).unwrap();
        table.commit_tentative(1, 3).unwrap();
        assert_eq!(table.committed_tokens(1), Some(66));
        let committed = table.pages(1).unwrap();
        assert_eq!(committed.len(), 2);
        assert_eq!(committed[0].state, PageState::HbmSealed);
        assert_eq!(committed[1].state, PageState::HbmMutable);
        assert!(
            committed
                .iter()
                .all(|page| page.draft_local_page_id.is_some())
        );
    }

    #[test]
    fn failed_cross_page_reservation_is_atomic() {
        let mut table = SequencePageTable::new(PageTableConfig {
            target_pages_per_rank: 1,
            draft_pages_per_rank: 0,
        })
        .unwrap();
        table.admit_with_prefix(1, false, &[]).unwrap();
        table.append_committed(1, 255).unwrap();
        let before = table.pages(1).unwrap();
        assert_eq!(
            table.begin_tentative(1, 2),
            Err(SequencePageError::Capacity)
        );
        assert_eq!(table.pages(1).unwrap(), before);
        assert_eq!(table.committed_tokens(1), Some(255));
        let stats = table.stats().unwrap();
        assert_eq!(stats.maximum_target_only_sequence_tokens, 256);
        assert_eq!(stats.maximum_mtp_sequence_tokens, 0);
    }

    #[test]
    fn failed_sequence_removal_restores_every_page_and_is_retryable() {
        let mut table = SequencePageTable::new(PageTableConfig {
            target_pages_per_rank: 2,
            draft_pages_per_rank: 0,
        })
        .unwrap();
        table.admit_with_prefix(1, false, &[]).unwrap();
        table.append_committed(1, 65).unwrap();
        let pages = table.sequences[&1].pages.clone();
        let first = pages[0].physical;
        let second = pages[1].physical;
        let missing = table.physical.remove(&first).unwrap();
        assert!(
            !table.free_target[usize::from(second.owner_rank)]
                .contains(&second.target_local_page_id)
        );

        assert_eq!(table.remove_sequence(1), Err(SequencePageError::Invariant));
        assert_eq!(table.sequences[&1].pages.len(), 2);
        assert_eq!(table.physical[&second].references, 1);
        assert!(
            !table.free_target[usize::from(second.owner_rank)]
                .contains(&second.target_local_page_id)
        );

        assert!(table.physical.insert(first, missing).is_none());
        table.remove_sequence(1).unwrap();
        assert!(!table.sequences.contains_key(&1));
        assert!(!table.physical.contains_key(&first));
        assert!(!table.physical.contains_key(&second));
        assert!(
            table.free_target[usize::from(first.owner_rank)].contains(&first.target_local_page_id)
        );
        assert!(
            table.free_target[usize::from(second.owner_rank)]
                .contains(&second.target_local_page_id)
        );
    }

    #[test]
    fn prefix_keys_are_bound_to_one_page_ordinal() {
        let key = PrefixPageKey([4; 32]);
        let mut table = SequencePageTable::new(PageTableConfig {
            target_pages_per_rank: 2,
            draft_pages_per_rank: 0,
        })
        .unwrap();
        table.admit_with_prefix(1, false, &[(key, false)]).unwrap();
        let before = table.stats().unwrap();
        assert_eq!(
            table.admit_with_prefix(2, false, &[(PrefixPageKey([5; 32]), false), (key, false)]),
            Err(SequencePageError::Prefix)
        );
        assert_eq!(table.stats().unwrap(), before);
        assert_eq!(
            table.admit_with_prefix(2, false, &[(key, false), (key, false)]),
            Err(SequencePageError::Prefix)
        );
    }

    #[test]
    fn shared_target_prefix_upgrades_to_mtp_without_duplication() {
        let key = PrefixPageKey([7; 32]);
        let mut table = SequencePageTable::new(PageTableConfig {
            target_pages_per_rank: 1,
            draft_pages_per_rank: 1,
        })
        .unwrap();
        table.admit_with_prefix(1, false, &[(key, true)]).unwrap();
        let target_only = table.pages(1).unwrap();
        assert_eq!(target_only[0].draft_local_page_id, None);

        table.admit_with_prefix(2, true, &[(key, true)]).unwrap();
        let mtp = table.pages(2).unwrap();
        assert_eq!(target_only[0].physical, mtp[0].physical);
        assert!(mtp[0].draft_local_page_id.is_some());
        assert_eq!(mtp[0].references, 2);
        assert_eq!(table.stats().unwrap().target_pages_used, [1, 0, 0, 0]);
        assert_eq!(table.stats().unwrap().draft_pages_used, [1, 0, 0, 0]);

        table.remove_sequence(2).unwrap();
        table.remove_sequence(1).unwrap();
        assert_eq!(table.stats().unwrap().target_pages_used, [0; 4]);
        assert_eq!(table.stats().unwrap().draft_pages_used, [0; 4]);
    }
}
