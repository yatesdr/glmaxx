use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};

use crate::{
    DRAFT_COMMITTED_RECORD_BYTES, INDEXER_GROUPS, INDEXER_RECORD_BYTES, KV_RECORD_BYTES,
    PAGE_TOKENS, TARGET_LAYERS,
};

type RecoveryState = (TierRecord, BTreeMap<TierPiece, [u8; 32]>, bool);

pub const TARGET_KV_PAGE_BYTES: u64 = PAGE_TOKENS * TARGET_LAYERS * KV_RECORD_BYTES;
pub const TARGET_INDEXER_PAGE_BYTES: u64 = PAGE_TOKENS * INDEXER_GROUPS * INDEXER_RECORD_BYTES;
pub const DRAFT_KV_PAGE_BYTES: u64 = PAGE_TOKENS * KV_RECORD_BYTES;
pub const DRAFT_INDEXER_PAGE_BYTES: u64 = PAGE_TOKENS * INDEXER_RECORD_BYTES;
pub const DRAFT_SIDECAR_PAGE_BYTES: u64 = PAGE_TOKENS * DRAFT_COMMITTED_RECORD_BYTES;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum Tier {
    Dram = 1,
    Nvme = 2,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum TierPiece {
    TargetKv = 1,
    TargetIndexer = 2,
    DraftSidecar = 3,
}

impl TierPiece {
    #[must_use]
    pub const fn expected_bytes(self) -> u64 {
        match self {
            Self::TargetKv => TARGET_KV_PAGE_BYTES,
            Self::TargetIndexer => TARGET_INDEXER_PAGE_BYTES,
            Self::DraftSidecar => DRAFT_SIDECAR_PAGE_BYTES,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TierPieceRecord {
    pub piece: TierPiece,
    pub byte_length: u64,
    pub storage_offset: u64,
    pub sha256: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TierRecord {
    pub namespace: [u8; 32],
    pub page_key: [u8; 32],
    pub generation: u64,
    pub tier: Tier,
    pub mtp: bool,
    pub pieces: Vec<TierPieceRecord>,
}

impl TierRecord {
    pub fn validate(&self) -> Result<(), TierError> {
        if self.namespace == [0; 32] || self.page_key == [0; 32] || self.generation == 0 {
            return Err(TierError::Identity);
        }
        let required = if self.mtp {
            vec![
                TierPiece::TargetKv,
                TierPiece::TargetIndexer,
                TierPiece::DraftSidecar,
            ]
        } else {
            vec![TierPiece::TargetKv, TierPiece::TargetIndexer]
        };
        if self.pieces.len() != required.len() {
            return Err(TierError::Pieces);
        }
        let alignment = match self.tier {
            Tier::Dram => 64,
            Tier::Nvme => 4096,
        };
        let mut seen = BTreeSet::new();
        let mut ranges = Vec::with_capacity(self.pieces.len());
        for piece in &self.pieces {
            if !required.contains(&piece.piece)
                || !seen.insert(piece.piece)
                || piece.byte_length != piece.piece.expected_bytes()
                || !piece.storage_offset.is_multiple_of(alignment)
                || piece.sha256 == [0; 32]
            {
                return Err(TierError::Pieces);
            }
            let end = piece
                .storage_offset
                .checked_add(piece.byte_length)
                .ok_or(TierError::Overflow)?;
            if ranges
                .iter()
                .any(|&(start, prior_end)| piece.storage_offset < prior_end && start < end)
            {
                return Err(TierError::Pieces);
            }
            ranges.push((piece.storage_offset, end));
        }
        Ok(())
    }
}

pub fn encode_draft_sidecar_payload(
    draft_kv: &[u8],
    draft_indexer: &[u8],
) -> Result<Vec<u8>, TierError> {
    if draft_kv.len() != DRAFT_KV_PAGE_BYTES as usize
        || draft_indexer.len() != DRAFT_INDEXER_PAGE_BYTES as usize
    {
        return Err(TierError::Pieces);
    }
    let mut output = Vec::with_capacity(DRAFT_SIDECAR_PAGE_BYTES as usize);
    for token in 0..PAGE_TOKENS as usize {
        let kv_start = token * KV_RECORD_BYTES as usize;
        let indexer_start = token * INDEXER_RECORD_BYTES as usize;
        output.extend_from_slice(&draft_kv[kv_start..kv_start + KV_RECORD_BYTES as usize]);
        output.extend_from_slice(
            &draft_indexer[indexer_start..indexer_start + INDEXER_RECORD_BYTES as usize],
        );
    }
    Ok(output)
}

pub fn decode_draft_sidecar_payload(payload: &[u8]) -> Result<(Vec<u8>, Vec<u8>), TierError> {
    if payload.len() != DRAFT_SIDECAR_PAGE_BYTES as usize {
        return Err(TierError::Pieces);
    }
    let mut draft_kv = Vec::with_capacity(DRAFT_KV_PAGE_BYTES as usize);
    let mut draft_indexer = Vec::with_capacity(DRAFT_INDEXER_PAGE_BYTES as usize);
    for token_record in payload.chunks_exact(DRAFT_COMMITTED_RECORD_BYTES as usize) {
        draft_kv.extend_from_slice(&token_record[..KV_RECORD_BYTES as usize]);
        draft_indexer.extend_from_slice(&token_record[KV_RECORD_BYTES as usize..]);
    }
    Ok((draft_kv, draft_indexer))
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum JournalEvent {
    Begin {
        transaction: u64,
        record: TierRecord,
    },
    PieceDurable {
        transaction: u64,
        piece: TierPiece,
        sha256: [u8; 32],
    },
    Publish {
        transaction: u64,
    },
}

#[derive(Clone, Debug, Default)]
pub struct TierJournal {
    events: Vec<JournalEvent>,
    next_transaction: u64,
}

impl TierJournal {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            events: Vec::new(),
            next_transaction: 1,
        }
    }

    pub fn begin(&mut self, record: TierRecord) -> Result<u64, TierError> {
        record.validate()?;
        let transaction = self.next_transaction;
        self.next_transaction = self
            .next_transaction
            .checked_add(1)
            .ok_or(TierError::Overflow)?;
        self.events.push(JournalEvent::Begin {
            transaction,
            record,
        });
        Ok(transaction)
    }

    pub fn piece_durable(
        &mut self,
        transaction: u64,
        piece: TierPiece,
        observed_sha256: [u8; 32],
    ) -> Result<(), TierError> {
        let record = self.open_record(transaction)?;
        let expected = record
            .pieces
            .iter()
            .find(|entry| entry.piece == piece)
            .ok_or(TierError::Pieces)?;
        if observed_sha256 != expected.sha256 {
            return Err(TierError::Checksum);
        }
        if self.events.iter().any(|event| {
            matches!(
                event,
                JournalEvent::PieceDurable {
                    transaction: event_transaction,
                    piece: event_piece,
                    ..
                } if *event_transaction == transaction && *event_piece == piece
            )
        }) {
            return Err(TierError::Journal);
        }
        self.events.push(JournalEvent::PieceDurable {
            transaction,
            piece,
            sha256: observed_sha256,
        });
        Ok(())
    }

    pub fn publish(&mut self, transaction: u64) -> Result<(), TierError> {
        let record = self.open_record(transaction)?;
        let durable: BTreeSet<_> = self
            .events
            .iter()
            .filter_map(|event| match event {
                JournalEvent::PieceDurable {
                    transaction: event_transaction,
                    piece,
                    ..
                } if *event_transaction == transaction => Some(*piece),
                _ => None,
            })
            .collect();
        if record
            .pieces
            .iter()
            .any(|piece| !durable.contains(&piece.piece))
        {
            return Err(TierError::NotDurable);
        }
        self.events.push(JournalEvent::Publish { transaction });
        Ok(())
    }

    #[must_use]
    pub fn events(&self) -> &[JournalEvent] {
        &self.events
    }

    pub fn from_events(events: Vec<JournalEvent>) -> Result<Self, TierError> {
        let journal = Self {
            next_transaction: events
                .iter()
                .map(|event| match event {
                    JournalEvent::Begin { transaction, .. }
                    | JournalEvent::PieceDurable { transaction, .. }
                    | JournalEvent::Publish { transaction } => *transaction,
                })
                .max()
                .unwrap_or(0)
                .checked_add(1)
                .ok_or(TierError::Overflow)?,
            events,
        };
        journal.recover()?;
        Ok(journal)
    }

    /// Replays only fully durable published records. Begun or partially
    /// written records are crash orphans and intentionally remain invisible.
    pub fn recover(&self) -> Result<BTreeMap<[u8; 32], TierRecord>, TierError> {
        let mut transactions: BTreeMap<u64, RecoveryState> = BTreeMap::new();
        for event in &self.events {
            match event {
                JournalEvent::Begin {
                    transaction,
                    record,
                } => {
                    record.validate()?;
                    if transactions
                        .insert(*transaction, (record.clone(), BTreeMap::new(), false))
                        .is_some()
                    {
                        return Err(TierError::Journal);
                    }
                }
                JournalEvent::PieceDurable {
                    transaction,
                    piece,
                    sha256,
                } => {
                    let (record, durable, published) = transactions
                        .get_mut(transaction)
                        .ok_or(TierError::Journal)?;
                    if *published {
                        return Err(TierError::Journal);
                    }
                    let expected = record
                        .pieces
                        .iter()
                        .find(|entry| entry.piece == *piece)
                        .ok_or(TierError::Pieces)?;
                    if expected.sha256 != *sha256 || durable.insert(*piece, *sha256).is_some() {
                        return Err(TierError::Checksum);
                    }
                }
                JournalEvent::Publish { transaction } => {
                    let (record, durable, published) = transactions
                        .get_mut(transaction)
                        .ok_or(TierError::Journal)?;
                    if *published
                        || record
                            .pieces
                            .iter()
                            .any(|piece| !durable.contains_key(&piece.piece))
                    {
                        return Err(TierError::NotDurable);
                    }
                    *published = true;
                }
            }
        }
        let mut recovered: BTreeMap<[u8; 32], TierRecord> = BTreeMap::new();
        for (_, (record, _, published)) in transactions {
            if published
                && recovered
                    .get(&record.page_key)
                    .is_none_or(|existing| existing.generation < record.generation)
            {
                recovered.insert(record.page_key, record);
            }
        }
        Ok(recovered)
    }

    fn open_record(&self, transaction: u64) -> Result<&TierRecord, TierError> {
        if self.events.iter().any(
            |event| matches!(event, JournalEvent::Publish { transaction: id } if *id == transaction),
        ) {
            return Err(TierError::AlreadyPublished);
        }
        self.events
            .iter()
            .find_map(|event| match event {
                JournalEvent::Begin {
                    transaction: id,
                    record,
                } if *id == transaction => Some(record),
                _ => None,
            })
            .ok_or(TierError::Journal)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TierError {
    Identity,
    Pieces,
    Checksum,
    NotDurable,
    AlreadyPublished,
    Journal,
    Overflow,
}

impl fmt::Display for TierError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for TierError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(mtp: bool) -> TierRecord {
        let pieces = [
            TierPiece::TargetKv,
            TierPiece::TargetIndexer,
            TierPiece::DraftSidecar,
        ];
        TierRecord {
            namespace: [0x11; 32],
            page_key: [0x22; 32],
            generation: 7,
            tier: Tier::Nvme,
            mtp,
            pieces: pieces[..if mtp { 3 } else { 2 }]
                .iter()
                .enumerate()
                .map(|(ordinal, &piece)| TierPieceRecord {
                    piece,
                    byte_length: piece.expected_bytes(),
                    storage_offset: ordinal as u64 * 2 * 1024 * 1024,
                    sha256: [ordinal as u8 + 1; 32],
                })
                .collect(),
        }
    }

    #[test]
    fn page_piece_arithmetic_includes_target_indexer_and_mtp_home() {
        assert_eq!(TARGET_KV_PAGE_BYTES, 1_837_056);
        assert_eq!(TARGET_INDEXER_PAGE_BYTES, 177_408);
        assert_eq!(DRAFT_KV_PAGE_BYTES, 23_552);
        assert_eq!(DRAFT_INDEXER_PAGE_BYTES, 8_448);
        assert_eq!(DRAFT_SIDECAR_PAGE_BYTES, 32_000);
        record(true).validate().unwrap();
    }

    #[test]
    fn draft_sidecar_is_one_token_major_round_trip_payload() {
        let draft_kv: Vec<_> = (0..DRAFT_KV_PAGE_BYTES)
            .map(|index| (index / KV_RECORD_BYTES) as u8)
            .collect();
        let draft_indexer: Vec<_> = (0..DRAFT_INDEXER_PAGE_BYTES)
            .map(|index| 0x80 | (index / INDEXER_RECORD_BYTES) as u8)
            .collect();
        let payload = encode_draft_sidecar_payload(&draft_kv, &draft_indexer).unwrap();
        assert_eq!(payload.len(), 32_000);
        for token in 0..PAGE_TOKENS as usize {
            let record = &payload[token * 500..(token + 1) * 500];
            assert!(record[..368].iter().all(|&byte| byte == token as u8));
            assert!(
                record[368..]
                    .iter()
                    .all(|&byte| byte == (0x80 | token as u8))
            );
        }
        assert_eq!(
            decode_draft_sidecar_payload(&payload).unwrap(),
            (draft_kv, draft_indexer)
        );
        assert!(decode_draft_sidecar_payload(&payload[..payload.len() - 1]).is_err());
    }

    #[test]
    fn tier_piece_ranges_must_not_overlap() {
        let mut record = record(true);
        record.pieces[2].storage_offset = record.pieces[1].storage_offset;
        assert_eq!(record.validate(), Err(TierError::Pieces));
    }

    #[test]
    fn publish_requires_every_piece_to_be_durable() {
        let mut journal = TierJournal::new();
        let record = record(true);
        let transaction = journal.begin(record.clone()).unwrap();
        journal
            .piece_durable(transaction, TierPiece::TargetKv, record.pieces[0].sha256)
            .unwrap();
        assert_eq!(journal.publish(transaction), Err(TierError::NotDurable));
        for piece in &record.pieces[1..] {
            journal
                .piece_durable(transaction, piece.piece, piece.sha256)
                .unwrap();
        }
        journal.publish(transaction).unwrap();
        assert_eq!(
            journal.recover().unwrap().get(&record.page_key),
            Some(&record)
        );
    }

    #[test]
    fn replay_ignores_crash_orphans_and_rejects_false_publication() {
        let mut journal = TierJournal::new();
        let record = record(false);
        let transaction = journal.begin(record.clone()).unwrap();
        journal
            .piece_durable(transaction, TierPiece::TargetKv, record.pieces[0].sha256)
            .unwrap();
        assert!(journal.recover().unwrap().is_empty());

        let mut corrupt = journal.events().to_vec();
        corrupt.push(JournalEvent::Publish { transaction });
        assert_eq!(
            TierJournal::from_events(corrupt).unwrap_err(),
            TierError::NotDurable
        );
    }

    #[test]
    fn checksum_mismatch_never_becomes_visible() {
        let mut journal = TierJournal::new();
        let transaction = journal.begin(record(false)).unwrap();
        assert_eq!(
            journal.piece_durable(transaction, TierPiece::TargetKv, [0xff; 32]),
            Err(TierError::Checksum)
        );
    }

    #[test]
    fn duplicate_durable_piece_is_rejected_before_publication() {
        let mut journal = TierJournal::new();
        let record = record(false);
        let transaction = journal.begin(record.clone()).unwrap();
        journal
            .piece_durable(transaction, TierPiece::TargetKv, record.pieces[0].sha256)
            .unwrap();
        assert_eq!(
            journal.piece_durable(transaction, TierPiece::TargetKv, record.pieces[0].sha256),
            Err(TierError::Journal)
        );
    }
}
