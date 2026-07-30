use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};

use crate::{PAGE_TOKENS, PageState, PrefixPageKey, TierPiece, TierRecord, owner_rank};

pub const MAXIMUM_CONTEXT_TOKENS: u64 = 1_048_576;
pub const MAXIMUM_PHYSICAL_PAGES_PER_RANK: u32 = 1_048_576;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct PhysicalPageId {
    pub owner_rank: u8,
    pub target_local_page_id: u32,
}

/// Logical identity of one prefix page whose durable record has passed the
/// strict tier-record validator.
///
/// This replaces the old caller-supplied `has_draft` boolean. Target and
/// draft capability are now bound to the namespace, generation, and exact
/// logical piece hashes that a restore coordinator validated.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PrefixPageAttachment {
    namespace: [u8; 32],
    key: PrefixPageKey,
    generation: u64,
    target_kv_sha256: [u8; 32],
    target_indexer_sha256: [u8; 32],
    draft_sidecar_sha256: Option<[u8; 32]>,
}

impl PrefixPageAttachment {
    pub fn from_tier_record(record: &TierRecord) -> Result<Self, SequencePageError> {
        record.validate()?;
        Ok(Self {
            namespace: record.namespace,
            key: PrefixPageKey(record.page_key),
            generation: record.generation,
            target_kv_sha256: piece_hash(record, TierPiece::TargetKv)?,
            target_indexer_sha256: piece_hash(record, TierPiece::TargetIndexer)?,
            draft_sidecar_sha256: record
                .mtp
                .then(|| piece_hash(record, TierPiece::DraftSidecar))
                .transpose()?,
        })
    }

    #[must_use]
    pub const fn key(self) -> PrefixPageKey {
        self.key
    }

    #[must_use]
    pub const fn generation(self) -> u64 {
        self.generation
    }

    #[must_use]
    pub const fn has_draft(self) -> bool {
        self.draft_sidecar_sha256.is_some()
    }

    fn relation_to(self, candidate: Self) -> Result<AttachmentRelation, SequencePageError> {
        if self.namespace != candidate.namespace
            || self.key != candidate.key
            || self.target_kv_sha256 != candidate.target_kv_sha256
            || self.target_indexer_sha256 != candidate.target_indexer_sha256
        {
            return Err(SequencePageError::Prefix);
        }
        match (self.draft_sidecar_sha256, candidate.draft_sidecar_sha256) {
            (None, None) => Ok(AttachmentRelation::Exact),
            (Some(_), None) => Ok(AttachmentRelation::RetainDraft),
            (None, Some(_)) if candidate.generation > self.generation => {
                Ok(AttachmentRelation::DraftUpgrade)
            }
            (Some(current), Some(next)) if current == next => Ok(AttachmentRelation::Exact),
            (None, Some(_)) | (Some(_), Some(_)) => Err(SequencePageError::Prefix),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AttachmentRelation {
    Exact,
    RetainDraft,
    DraftUpgrade,
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SequencePageSnapshot {
    pub sequence_id: u64,
    pub mtp: bool,
    pub committed_tokens: u64,
    pub tentative_tokens: u8,
    pub pages: Vec<SequencePageView>,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PageReuseQuarantineStats {
    pub target_pages: [u32; 4],
    pub draft_pages: [u32; 4],
    pub bound_generation: Option<u64>,
}

impl PageReuseQuarantineStats {
    #[must_use]
    pub fn is_empty(self) -> bool {
        self.target_pages == [0; 4] && self.draft_pages == [0; 4]
    }
}

#[derive(Clone, Debug)]
struct PhysicalPage {
    draft_local_page_id: Option<u32>,
    state: PageState,
    valid_tokens: u8,
    references: u32,
    prefix: Option<(PrefixPageAttachment, u64)>,
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
    quarantined_target: [BTreeSet<u32>; 4],
    quarantined_draft: [BTreeSet<u32>; 4],
    quarantine_generation: Option<u64>,
    physical: BTreeMap<PhysicalPageId, PhysicalPage>,
    prefixes: BTreeMap<PrefixPageKey, PhysicalPageId>,
    sequences: BTreeMap<u64, Sequence>,
}

impl SequencePageTable {
    pub fn new(config: PageTableConfig) -> Result<Self, SequencePageError> {
        if config.target_pages_per_rank == 0
            || config.target_pages_per_rank > MAXIMUM_PHYSICAL_PAGES_PER_RANK
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
            quarantined_target: std::array::from_fn(|_| BTreeSet::new()),
            quarantined_draft: std::array::from_fn(|_| BTreeSet::new()),
            quarantine_generation: None,
            physical: BTreeMap::new(),
            prefixes: BTreeMap::new(),
            sequences: BTreeMap::new(),
        })
    }

    #[must_use]
    pub const fn config(&self) -> PageTableConfig {
        self.config
    }

    #[must_use]
    pub fn reuse_quarantine_stats(&self) -> PageReuseQuarantineStats {
        PageReuseQuarantineStats {
            target_pages: std::array::from_fn(|rank| {
                u32::try_from(self.quarantined_target[rank].len())
                    .expect("arena configuration bounds quarantine cardinality to u32")
            }),
            draft_pages: std::array::from_fn(|rank| {
                u32::try_from(self.quarantined_draft[rank].len())
                    .expect("arena configuration bounds quarantine cardinality to u32")
            }),
            bound_generation: self.quarantine_generation,
        }
    }

    /// Binds every currently retired physical ID to the one page-table
    /// generation whose four-rank acknowledgement permits allocator reuse.
    ///
    /// An empty quarantine needs no receipt and returns `false`.
    pub fn bind_reuse_quarantine(&mut self, generation: u64) -> Result<bool, SequencePageError> {
        if generation == 0 || self.quarantine_generation.is_some() {
            return Err(SequencePageError::Transaction);
        }
        if self.reuse_quarantine_stats().is_empty() {
            return Ok(false);
        }
        self.quarantine_generation = Some(generation);
        Ok(true)
    }

    /// Makes retired IDs allocatable only after the caller has verified all
    /// four rank acknowledgements for the exact bound generation.
    pub fn acknowledge_reuse_quarantine(
        &mut self,
        generation: u64,
    ) -> Result<PageReuseQuarantineStats, SequencePageError> {
        if self.quarantine_generation != Some(generation) {
            return Err(SequencePageError::Transaction);
        }
        let retired = self.reuse_quarantine_stats();
        for rank in 0..4 {
            self.free_target[rank].append(&mut self.quarantined_target[rank]);
            self.free_draft[rank].append(&mut self.quarantined_draft[rank]);
        }
        self.quarantine_generation = None;
        Ok(retired)
    }

    /// Attaches only complete sealed pages. Missing keys allocate restored HBM
    /// slots; existing keys acquire another active reference.
    pub fn admit_with_prefix(
        &mut self,
        sequence_id: u64,
        mtp: bool,
        prefix_pages: &[PrefixPageAttachment],
    ) -> Result<(), SequencePageError> {
        self.require_unbound_quarantine()?;
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
            for (ordinal, &attachment) in prefix_pages.iter().enumerate() {
                let key = attachment.key();
                if mtp && !attachment.has_draft() {
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
                            && page.prefix.is_some_and(|(current, current_ordinal)| {
                                current_ordinal == ordinal
                                    && current.relation_to(attachment).is_ok()
                            })
                    });
                    if !valid {
                        return Err(SequencePageError::Prefix);
                    }
                    let relation = self
                        .physical
                        .get(&physical)
                        .and_then(|page| page.prefix)
                        .ok_or(SequencePageError::Invariant)?
                        .0
                        .relation_to(attachment)?;
                    if relation == AttachmentRelation::DraftUpgrade {
                        self.physical
                            .get_mut(&physical)
                            .ok_or(SequencePageError::Invariant)?
                            .prefix = Some((attachment, ordinal));
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
                    page.prefix = Some((attachment, ordinal));
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
        self.require_unbound_quarantine()?;
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
        self.require_unbound_quarantine()?;
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
        self.require_unbound_quarantine()?;
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
        self.require_unbound_quarantine()?;
        let transaction = self
            .sequences
            .get(&sequence_id)
            .and_then(|sequence| sequence.tentative.clone())
            .ok_or(SequencePageError::Transaction)?;
        if committed_tokens == 0 || committed_tokens > transaction.reserved_tokens {
            return Err(SequencePageError::Transaction);
        }
        let snapshot = self.clone();
        let result = self.commit_tentative_inner(sequence_id, committed_tokens, transaction);
        if let Err(error) = result {
            *self = snapshot;
            return Err(error);
        }
        Ok(())
    }

    pub fn rollback_tentative(&mut self, sequence_id: u64) -> Result<(), SequencePageError> {
        self.require_unbound_quarantine()?;
        let snapshot = self.clone();
        let result = self.rollback_tentative_inner(sequence_id);
        if let Err(error) = result {
            *self = snapshot;
            return Err(error);
        }
        Ok(())
    }

    pub fn remove_sequence(&mut self, sequence_id: u64) -> Result<(), SequencePageError> {
        self.require_unbound_quarantine()?;
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

    pub fn snapshots(&self) -> Result<Vec<SequencePageSnapshot>, SequencePageError> {
        self.sequences
            .iter()
            .map(|(&sequence_id, sequence)| {
                Ok(SequencePageSnapshot {
                    sequence_id,
                    mtp: sequence.mtp,
                    committed_tokens: sequence.committed_tokens,
                    tentative_tokens: sequence
                        .tentative
                        .as_ref()
                        .map_or(0, |tail| tail.reserved_tokens),
                    pages: self.pages(sequence_id)?,
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
        let new_committed_tokens = sequence
            .committed_tokens
            .checked_add(token_count)
            .filter(|&tokens| tokens <= MAXIMUM_CONTEXT_TOKENS)
            .ok_or(SequencePageError::Transaction)?;
        if sequence.tentative.is_some() {
            return Err(SequencePageError::Transaction);
        }
        let mut remaining = token_count;
        if let Some(physical) = sequence.pages.last().map(|page| page.physical) {
            let page = self
                .physical
                .get_mut(&physical)
                .ok_or(SequencePageError::Invariant)?;
            if page.state == PageState::HbmMutable
                && page.references == 1
                && page.valid_tokens < PAGE_TOKENS as u8
            {
                let available = PAGE_TOKENS
                    .checked_sub(u64::from(page.valid_tokens))
                    .ok_or(SequencePageError::Invariant)?;
                let appended = remaining.min(available);
                page.valid_tokens = page
                    .valid_tokens
                    .checked_add(u8::try_from(appended).map_err(|_| SequencePageError::Overflow)?)
                    .ok_or(SequencePageError::Overflow)?;
                if page.valid_tokens == PAGE_TOKENS as u8 {
                    page.state = page.state.transition(PageState::HbmSealed)?;
                }
                remaining = remaining
                    .checked_sub(appended)
                    .ok_or(SequencePageError::Invariant)?;
            } else if page.state != PageState::HbmSealed || page.valid_tokens != PAGE_TOKENS as u8 {
                return Err(SequencePageError::State);
            }
        }
        while remaining != 0 {
            let sequence = &self.sequences[&sequence_id];
            let ordinal =
                u64::try_from(sequence.pages.len()).map_err(|_| SequencePageError::Overflow)?;
            let physical = self.allocate_page(ordinal, sequence.mtp)?;
            let appended = remaining.min(PAGE_TOKENS);
            let page = self
                .physical
                .get_mut(&physical)
                .ok_or(SequencePageError::Invariant)?;
            page.valid_tokens = u8::try_from(appended).map_err(|_| SequencePageError::Overflow)?;
            if appended == PAGE_TOKENS {
                page.state = page.state.transition(PageState::HbmSealed)?;
            }
            self.sequences
                .get_mut(&sequence_id)
                .ok_or(SequencePageError::Sequence)?
                .pages
                .push(SequencePage { ordinal, physical });
            remaining = remaining
                .checked_sub(appended)
                .ok_or(SequencePageError::Invariant)?;
        }
        self.sequences
            .get_mut(&sequence_id)
            .ok_or(SequencePageError::Sequence)?
            .committed_tokens = new_committed_tokens;
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

    fn commit_tentative_inner(
        &mut self,
        sequence_id: u64,
        committed_tokens: u8,
        transaction: TentativeTail,
    ) -> Result<(), SequencePageError> {
        let new_committed_tokens = transaction
            .original_committed_tokens
            .checked_add(u64::from(committed_tokens))
            .filter(|&tokens| tokens <= MAXIMUM_CONTEXT_TOKENS)
            .ok_or(SequencePageError::Transaction)?;
        let desired_page_count = usize::try_from(new_committed_tokens.div_ceil(PAGE_TOKENS))
            .map_err(|_| SequencePageError::Overflow)?;
        let sequence = self
            .sequences
            .get_mut(&sequence_id)
            .ok_or(SequencePageError::Sequence)?;
        let retained_transaction = sequence
            .tentative
            .take()
            .ok_or(SequencePageError::Transaction)?;
        if retained_transaction.original_committed_tokens != transaction.original_committed_tokens
            || retained_transaction.original_page_count != transaction.original_page_count
            || retained_transaction.original_tail_tokens != transaction.original_tail_tokens
            || retained_transaction.reserved_tokens != transaction.reserved_tokens
            || desired_page_count > sequence.pages.len()
        {
            return Err(SequencePageError::Invariant);
        }
        while self
            .sequences
            .get(&sequence_id)
            .ok_or(SequencePageError::Sequence)?
            .pages
            .len()
            > desired_page_count
        {
            let page = self
                .sequences
                .get_mut(&sequence_id)
                .ok_or(SequencePageError::Sequence)?
                .pages
                .pop()
                .ok_or(SequencePageError::Invariant)?;
            self.release_page(page.physical)?;
        }
        for ordinal in 0..desired_page_count {
            let entry = self.sequences[&sequence_id].pages[ordinal];
            let is_tail = ordinal + 1 == desired_page_count;
            let expected_valid = if is_tail {
                let tail = new_committed_tokens % PAGE_TOKENS;
                if tail == 0 {
                    PAGE_TOKENS as u8
                } else {
                    u8::try_from(tail).map_err(|_| SequencePageError::Overflow)?
                }
            } else {
                PAGE_TOKENS as u8
            };
            let expected_state = if expected_valid == PAGE_TOKENS as u8 {
                PageState::HbmSealed
            } else {
                PageState::HbmMutable
            };
            let page = self
                .physical
                .get_mut(&entry.physical)
                .ok_or(SequencePageError::Invariant)?;
            if page.state == PageState::HbmTentative {
                page.state = page.state.transition(expected_state)?;
                page.valid_tokens = expected_valid;
            } else if page.state != expected_state || page.valid_tokens != expected_valid {
                return Err(SequencePageError::State);
            }
        }
        self.sequences
            .get_mut(&sequence_id)
            .ok_or(SequencePageError::Sequence)?
            .committed_tokens = new_committed_tokens;
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
        if let Some((attachment, _ordinal)) = page.prefix
            && self.prefixes.remove(&attachment.key()) != Some(physical)
        {
            return Err(SequencePageError::Invariant);
        }
        let rank = usize::from(physical.owner_rank);
        if self.free_target[rank].contains(&physical.target_local_page_id)
            || !self.quarantined_target[rank].insert(physical.target_local_page_id)
        {
            return Err(SequencePageError::Invariant);
        }
        if let Some(draft) = page.draft_local_page_id
            && (self.free_draft[rank].contains(&draft)
                || !self.quarantined_draft[rank].insert(draft))
        {
            return Err(SequencePageError::Invariant);
        }
        Ok(())
    }

    fn require_unbound_quarantine(&self) -> Result<(), SequencePageError> {
        if self.quarantine_generation.is_some() {
            Err(SequencePageError::Transaction)
        } else {
            Ok(())
        }
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
    Tier(crate::TierError),
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

impl From<crate::TierError> for SequencePageError {
    fn from(value: crate::TierError) -> Self {
        Self::Tier(value)
    }
}

fn piece_hash(record: &TierRecord, piece: TierPiece) -> Result<[u8; 32], SequencePageError> {
    record
        .pieces
        .iter()
        .find(|candidate| candidate.piece == piece)
        .map(|candidate| candidate.sha256)
        .ok_or(SequencePageError::Prefix)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn attachment(
        key: PrefixPageKey,
        generation: u64,
        target_marker: u8,
        draft_marker: Option<u8>,
    ) -> PrefixPageAttachment {
        let mut pieces = vec![
            crate::TierPieceRecord {
                piece: TierPiece::TargetKv,
                byte_length: TierPiece::TargetKv.expected_bytes(),
                storage_offset: 0,
                sha256: [target_marker; 32],
            },
            crate::TierPieceRecord {
                piece: TierPiece::TargetIndexer,
                byte_length: TierPiece::TargetIndexer.expected_bytes(),
                storage_offset: TierPiece::TargetKv.expected_bytes(),
                sha256: [target_marker.wrapping_add(1); 32],
            },
        ];
        if let Some(draft_marker) = draft_marker {
            pieces.push(crate::TierPieceRecord {
                piece: TierPiece::DraftSidecar,
                byte_length: TierPiece::DraftSidecar.expected_bytes(),
                storage_offset: TierPiece::TargetKv.expected_bytes()
                    + TierPiece::TargetIndexer.expected_bytes(),
                sha256: [draft_marker; 32],
            });
        }
        PrefixPageAttachment::from_tier_record(&TierRecord {
            namespace: [0x55; 32],
            page_key: key.0,
            generation,
            tier: crate::Tier::Dram,
            mtp: draft_marker.is_some(),
            pieces,
        })
        .unwrap()
    }

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
    fn page_granular_append_matches_single_token_reference_at_every_boundary() {
        for token_count in 1_u64..=257 {
            let config = PageTableConfig {
                target_pages_per_rank: 4,
                draft_pages_per_rank: 4,
            };
            let mut bulk = SequencePageTable::new(config).unwrap();
            let mut reference = SequencePageTable::new(config).unwrap();
            bulk.admit_with_prefix(1, true, &[]).unwrap();
            reference.admit_with_prefix(1, true, &[]).unwrap();

            bulk.append_committed(1, token_count).unwrap();
            for _ in 0..token_count {
                reference.append_committed(1, 1).unwrap();
            }

            assert_eq!(bulk.committed_tokens(1), Some(token_count));
            assert_eq!(bulk.pages(1).unwrap(), reference.pages(1).unwrap());
            assert_eq!(bulk.stats().unwrap(), reference.stats().unwrap());
        }
    }

    #[test]
    fn every_tail_occupancy_and_mtp_depth_reserves_exactly_one_position_per_token() {
        for tail_tokens in 0_u64..PAGE_TOKENS {
            for depth in 1_u8..=7 {
                let mut table = SequencePageTable::new(PageTableConfig {
                    target_pages_per_rank: 4,
                    draft_pages_per_rank: 4,
                })
                .unwrap();
                table.admit_with_prefix(1, true, &[]).unwrap();
                let committed = PAGE_TOKENS + tail_tokens;
                table.append_committed(1, committed).unwrap();
                let before = table.pages(1).unwrap();

                table.begin_tentative(1, depth).unwrap();
                let tentative = table.pages(1).unwrap();
                let expected_positions = committed + u64::from(depth);
                assert_eq!(
                    tentative
                        .iter()
                        .map(|page| u64::from(page.valid_tokens))
                        .sum::<u64>(),
                    expected_positions,
                    "tail={tail_tokens} depth={depth}"
                );
                assert_eq!(
                    u64::try_from(tentative.len()).unwrap(),
                    expected_positions.div_ceil(PAGE_TOKENS),
                    "tail={tail_tokens} depth={depth}"
                );
                assert!(
                    tentative
                        .iter()
                        .all(|page| page.valid_tokens <= PAGE_TOKENS as u8),
                    "tail={tail_tokens} depth={depth}"
                );
                assert!(
                    tentative
                        .iter()
                        .all(|page| page.draft_local_page_id.is_some()),
                    "tail={tail_tokens} depth={depth}"
                );

                table.rollback_tentative(1).unwrap();
                assert_eq!(
                    table.pages(1).unwrap(),
                    before,
                    "tail={tail_tokens} depth={depth}"
                );
                assert_eq!(table.committed_tokens(1), Some(committed));
            }
        }
    }

    #[test]
    fn fork_shares_sealed_pages_and_copies_a_mutable_tail() {
        let mut table = SequencePageTable::new(PageTableConfig {
            target_pages_per_rank: 4,
            draft_pages_per_rank: 4,
        })
        .unwrap();
        table
            .admit_with_prefix(
                1,
                true,
                &[attachment(PrefixPageKey([9; 32]), 1, 9, Some(11))],
            )
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
            table.quarantined_target[usize::from(first.owner_rank)]
                .contains(&first.target_local_page_id)
        );
        assert!(
            table.quarantined_target[usize::from(second.owner_rank)]
                .contains(&second.target_local_page_id)
        );
        assert_eq!(
            table.acknowledge_reuse_quarantine(9),
            Err(SequencePageError::Transaction)
        );
        assert!(table.bind_reuse_quarantine(9).unwrap());
        assert_eq!(
            table.append_committed(1, 1),
            Err(SequencePageError::Transaction)
        );
        let retired = table.acknowledge_reuse_quarantine(9).unwrap();
        assert_eq!(retired.bound_generation, Some(9));
        assert!(
            table.free_target[usize::from(first.owner_rank)].contains(&first.target_local_page_id)
        );
        assert!(
            table.free_target[usize::from(second.owner_rank)]
                .contains(&second.target_local_page_id)
        );
    }

    #[test]
    fn tentative_commit_keeps_accepted_ids_and_quarantines_only_rejected_pages() {
        let mut table = SequencePageTable::new(PageTableConfig {
            target_pages_per_rank: 2,
            draft_pages_per_rank: 2,
        })
        .unwrap();
        table.admit_with_prefix(1, true, &[]).unwrap();
        table.append_committed(1, 63).unwrap();
        table.begin_tentative(1, 7).unwrap();
        let reserved = table.pages(1).unwrap();
        assert_eq!(reserved.len(), 2);
        let accepted_tail = reserved[1];

        table.commit_tentative(1, 3).unwrap();
        let committed = table.pages(1).unwrap();
        assert_eq!(committed.len(), 2);
        assert_eq!(committed[1].physical, accepted_tail.physical);
        assert_eq!(
            committed[1].draft_local_page_id,
            accepted_tail.draft_local_page_id
        );
        assert!(table.reuse_quarantine_stats().is_empty());

        table.begin_tentative(1, 7).unwrap();
        let second_reservation = table.pages(1).unwrap();
        assert_eq!(second_reservation.len(), 2);
        table.commit_tentative(1, 1).unwrap();
        assert_eq!(
            table.pages(1).unwrap()[1].physical,
            second_reservation[1].physical
        );
        assert!(table.reuse_quarantine_stats().is_empty());

        table.append_committed(1, 59).unwrap();
        table.begin_tentative(1, 7).unwrap();
        let third_reservation = table.pages(1).unwrap();
        assert_eq!(third_reservation.len(), 3);
        let rejected_page = third_reservation[2];
        table.commit_tentative(1, 1).unwrap();
        assert_eq!(table.pages(1).unwrap().len(), 2);
        let quarantine = table.reuse_quarantine_stats();
        assert_eq!(
            quarantine.target_pages[usize::from(rejected_page.physical.owner_rank)],
            1
        );
        assert_eq!(
            quarantine.draft_pages[usize::from(rejected_page.physical.owner_rank)],
            1
        );
        assert!(table.bind_reuse_quarantine(12).unwrap());
        assert_eq!(
            table.acknowledge_reuse_quarantine(11),
            Err(SequencePageError::Transaction)
        );
        table.acknowledge_reuse_quarantine(12).unwrap();
        assert!(table.reuse_quarantine_stats().is_empty());
    }

    #[test]
    fn removed_target_and_draft_ids_cannot_aba_before_exact_generation_ack() {
        let mut table = SequencePageTable::new(PageTableConfig {
            target_pages_per_rank: 1,
            draft_pages_per_rank: 1,
        })
        .unwrap();
        table.admit_with_prefix(1, true, &[]).unwrap();
        table.append_committed(1, 1).unwrap();
        let retired = table.pages(1).unwrap()[0];
        table.remove_sequence(1).unwrap();
        assert_eq!(
            table.reuse_quarantine_stats(),
            PageReuseQuarantineStats {
                target_pages: [1, 0, 0, 0],
                draft_pages: [1, 0, 0, 0],
                bound_generation: None,
            }
        );

        table.admit_with_prefix(2, true, &[]).unwrap();
        assert_eq!(
            table.append_committed(2, 1),
            Err(SequencePageError::Capacity)
        );
        table.remove_sequence(2).unwrap();

        assert!(table.bind_reuse_quarantine(44).unwrap());
        assert_eq!(
            table.acknowledge_reuse_quarantine(43),
            Err(SequencePageError::Transaction)
        );
        assert_eq!(
            table.admit_with_prefix(3, true, &[]),
            Err(SequencePageError::Transaction)
        );
        table.acknowledge_reuse_quarantine(44).unwrap();

        table.admit_with_prefix(3, true, &[]).unwrap();
        table.append_committed(3, 1).unwrap();
        let reused = table.pages(3).unwrap()[0];
        assert_eq!(reused.physical, retired.physical);
        assert_eq!(reused.draft_local_page_id, retired.draft_local_page_id);
    }

    #[test]
    fn prefix_keys_are_bound_to_one_page_ordinal() {
        let key = PrefixPageKey([4; 32]);
        let mut table = SequencePageTable::new(PageTableConfig {
            target_pages_per_rank: 2,
            draft_pages_per_rank: 0,
        })
        .unwrap();
        let first = attachment(key, 1, 4, None);
        table.admit_with_prefix(1, false, &[first]).unwrap();
        let before = table.stats().unwrap();
        assert_eq!(
            table.admit_with_prefix(
                2,
                false,
                &[attachment(PrefixPageKey([5; 32]), 1, 5, None), first,],
            ),
            Err(SequencePageError::Prefix)
        );
        assert_eq!(table.stats().unwrap(), before);
        assert_eq!(
            table.admit_with_prefix(2, false, &[first, first]),
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
        let target = attachment(key, 1, 7, None);
        let mtp = attachment(key, 2, 7, Some(9));
        table.admit_with_prefix(1, false, &[target]).unwrap();
        let target_only = table.pages(1).unwrap();
        assert_eq!(target_only[0].draft_local_page_id, None);

        table.admit_with_prefix(2, true, &[mtp]).unwrap();
        let mtp_pages = table.pages(2).unwrap();
        assert_eq!(target_only[0].physical, mtp_pages[0].physical);
        assert!(mtp_pages[0].draft_local_page_id.is_some());
        assert_eq!(mtp_pages[0].references, 2);
        assert_eq!(table.stats().unwrap().target_pages_used, [1, 0, 0, 0]);
        assert_eq!(table.stats().unwrap().draft_pages_used, [1, 0, 0, 0]);

        table.remove_sequence(2).unwrap();
        table.remove_sequence(1).unwrap();
        assert_eq!(table.stats().unwrap().target_pages_used, [0; 4]);
        assert_eq!(table.stats().unwrap().draft_pages_used, [0; 4]);
    }

    #[test]
    fn prefix_attachment_binds_generation_and_every_logical_piece_hash() {
        let key = PrefixPageKey([8; 32]);
        let target = attachment(key, 4, 8, None);
        let stale_upgrade = attachment(key, 4, 8, Some(10));
        let valid_upgrade = attachment(key, 5, 8, Some(10));
        let wrong_target = attachment(key, 6, 9, Some(10));
        let wrong_draft = attachment(key, 6, 8, Some(11));
        let mut table = SequencePageTable::new(PageTableConfig {
            target_pages_per_rank: 1,
            draft_pages_per_rank: 1,
        })
        .unwrap();
        table.admit_with_prefix(1, false, &[target]).unwrap();
        let before = table.stats().unwrap();

        assert_eq!(
            table.admit_with_prefix(2, true, &[stale_upgrade]),
            Err(SequencePageError::Prefix)
        );
        assert_eq!(
            table.admit_with_prefix(2, true, &[wrong_target]),
            Err(SequencePageError::Prefix)
        );
        assert_eq!(table.stats().unwrap(), before);

        table.admit_with_prefix(2, true, &[valid_upgrade]).unwrap();
        let upgraded = table.stats().unwrap();
        assert_eq!(upgraded.target_pages_used, [1, 0, 0, 0]);
        assert_eq!(upgraded.draft_pages_used, [1, 0, 0, 0]);
        assert_eq!(
            table.admit_with_prefix(3, true, &[wrong_draft]),
            Err(SequencePageError::Prefix)
        );
        assert_eq!(table.stats().unwrap(), upgraded);
    }
}
