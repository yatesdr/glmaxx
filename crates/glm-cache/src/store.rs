use std::{
    collections::BTreeMap,
    fmt,
    fs::{self, File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    os::fd::AsRawFd,
    path::{Path, PathBuf},
};

use glm_format::crc32c;
use sha2::{Digest, Sha256};

use crate::{
    JournalEvent, Tier, TierError, TierJournal, TierPiece, TierPieceRecord, TierRecord,
    TierRecordRelation,
};

// v2 makes the incompatible TierPiece::DraftSidecar=3 meaning explicit.
// v1 used piece 3 for the separately hashed draft-KV plane.
const JOURNAL_MAGIC: [u8; 8] = *b"GLTJRNL2";
const JOURNAL_VERSION: u16 = 2;
const JOURNAL_RECORD_BYTES: usize = 512;
const JOURNAL_CRC_OFFSET: usize = 508;
const PIECE_TABLE_OFFSET: usize = 96;
const PIECE_RECORD_BYTES: usize = 56;
const NVME_ALIGNMENT: u64 = 4096;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PagePieceBytes {
    pub piece: TierPiece,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DurablePageRequest {
    pub namespace: [u8; 32],
    pub page_key: [u8; 32],
    pub generation: u64,
    pub mtp: bool,
    pub pieces: Vec<PagePieceBytes>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RestoredPage {
    pub record: TierRecord,
    pub pieces: BTreeMap<TierPiece, Vec<u8>>,
}

pub struct FileTierStore {
    root: PathBuf,
    data: File,
    journal_file: File,
    journal: TierJournal,
    published: BTreeMap<[u8; 32], TierRecord>,
    next_transaction: u64,
    next_data_offset: u64,
    write_poisoned: bool,
}

pub struct FileTierReader {
    data: File,
    published: BTreeMap<[u8; 32], TierRecord>,
}

impl FileTierStore {
    pub fn open(root: &Path) -> Result<Self, StoreError> {
        fs::create_dir_all(root)?;
        let data_path = root.join("pages.dat");
        let journal_path = root.join("journal.log");
        let data = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(false)
            .open(data_path)?;
        let mut journal_file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(false)
            .open(journal_path)?;
        lock_exclusive_writer(&journal_file)?;
        let mut journal_bytes = Vec::new();
        journal_file.seek(SeekFrom::Start(0))?;
        journal_file.read_to_end(&mut journal_bytes)?;
        let valid_journal_bytes = journal_bytes.len() / JOURNAL_RECORD_BYTES * JOURNAL_RECORD_BYTES;
        let (events, next_transaction) = decode_journal(&journal_bytes)?;
        let journal = TierJournal::from_events(events)?;
        let published = journal.recover()?;
        let data_bytes = data.metadata()?.len();
        validate_catalog_extents(&published, data_bytes)?;
        // Never reuse bytes after a crash orphan or an extent retained by an
        // older snapshot. Segment cleaning is the only future reclamation
        // authority; the blocking store remains physically append-only.
        let next_data_offset = align_up(data_bytes, NVME_ALIGNMENT)?;
        if valid_journal_bytes != journal_bytes.len() {
            journal_file
                .set_len(u64::try_from(valid_journal_bytes).map_err(|_| StoreError::Overflow)?)?;
            journal_file.sync_data()?;
        }
        journal_file.seek(SeekFrom::End(0))?;
        Ok(Self {
            root: root.to_path_buf(),
            data,
            journal_file,
            journal,
            published,
            next_transaction,
            next_data_offset,
            write_poisoned: false,
        })
    }

    pub fn publish(&mut self, request: DurablePageRequest) -> Result<TierRecord, StoreError> {
        self.publish_inner(request, None)
    }

    pub fn restore(&mut self, page_key: [u8; 32]) -> Result<Option<RestoredPage>, StoreError> {
        restore_published(&mut self.data, &self.published, page_key)
    }

    #[must_use]
    pub fn record(&self, page_key: [u8; 32]) -> Option<&TierRecord> {
        self.published.get(&page_key)
    }

    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    #[must_use]
    pub fn resident_bytes(&self) -> u64 {
        self.published
            .values()
            .flat_map(|record| &record.pieces)
            .map(|piece| piece.byte_length)
            .sum()
    }

    fn publish_inner(
        &mut self,
        mut request: DurablePageRequest,
        failpoint: Option<WriteFailpoint>,
    ) -> Result<TierRecord, StoreError> {
        if self.write_poisoned {
            return Err(StoreError::WritePoisoned);
        }
        request.pieces.sort_by_key(|piece| piece.piece);
        if request.pieces.is_empty()
            || request
                .pieces
                .windows(2)
                .any(|pair| pair[0].piece == pair[1].piece)
        {
            return Err(StoreError::Pieces);
        }
        let mut piece_records = Vec::with_capacity(request.pieces.len());
        let mut next_offset = self.next_data_offset;
        for piece in &request.pieces {
            if piece.bytes.len()
                != usize::try_from(piece.piece.expected_bytes())
                    .map_err(|_| StoreError::Overflow)?
            {
                return Err(StoreError::Pieces);
            }
            next_offset = align_up(next_offset, NVME_ALIGNMENT)?;
            piece_records.push(TierPieceRecord {
                piece: piece.piece,
                byte_length: u64::try_from(piece.bytes.len()).map_err(|_| StoreError::Overflow)?,
                storage_offset: next_offset,
                sha256: Sha256::digest(&piece.bytes).into(),
            });
            next_offset = next_offset
                .checked_add(piece.piece.expected_bytes())
                .ok_or(StoreError::Overflow)?;
        }
        let record = TierRecord {
            namespace: request.namespace,
            page_key: request.page_key,
            generation: request.generation,
            tier: Tier::Nvme,
            mtp: request.mtp,
            pieces: piece_records,
        };
        record.validate()?;
        if let Some(existing) = self.published.get(&record.page_key) {
            match existing.relation_to(&record).map_err(|error| match error {
                TierError::Collision => StoreError::ContentCollision,
                error => StoreError::Tier(error),
            })? {
                TierRecordRelation::ExactDedup | TierRecordRelation::RetainMtp => {
                    return Ok(existing.clone());
                }
                TierRecordRelation::MtpUpgrade => {
                    if record.generation <= existing.generation {
                        return Err(StoreError::StaleGeneration);
                    }
                }
            }
        }
        let next_data_offset = align_up(next_offset, NVME_ALIGNMENT)?;

        let result = self.publish_prevalidated(request, record, next_data_offset, failpoint);
        if result.is_err() {
            self.write_poisoned = true;
        }
        result
    }

    fn publish_prevalidated(
        &mut self,
        request: DurablePageRequest,
        record: TierRecord,
        next_data_offset: u64,
        failpoint: Option<WriteFailpoint>,
    ) -> Result<TierRecord, StoreError> {
        let transaction = self.next_transaction;
        let next_transaction = self
            .next_transaction
            .checked_add(1)
            .ok_or(StoreError::Overflow)?;
        let in_memory_transaction = self.journal.begin(record.clone())?;
        if in_memory_transaction != transaction {
            return Err(StoreError::JournalSequence);
        }
        self.next_transaction = next_transaction;
        let begin = JournalEvent::Begin {
            transaction,
            record: record.clone(),
        };
        append_journal_record(&mut self.journal_file, &begin)?;
        self.journal_file.sync_data()?;
        if failpoint == Some(WriteFailpoint::BeginJournaled) {
            return Err(StoreError::InjectedCrash);
        }

        for (piece, descriptor) in request.pieces.iter().zip(&record.pieces) {
            self.data.seek(SeekFrom::Start(descriptor.storage_offset))?;
            self.data.write_all(&piece.bytes)?;
        }
        self.data.sync_data()?;
        if failpoint == Some(WriteFailpoint::DataSynced) {
            return Err(StoreError::InjectedCrash);
        }

        for (ordinal, descriptor) in record.pieces.iter().enumerate() {
            self.journal
                .piece_durable(transaction, descriptor.piece, descriptor.sha256)?;
            let event = JournalEvent::PieceDurable {
                transaction,
                piece: descriptor.piece,
                sha256: descriptor.sha256,
            };
            append_journal_record(&mut self.journal_file, &event)?;
            self.journal_file.sync_data()?;
            if ordinal == 0 && failpoint == Some(WriteFailpoint::FirstPieceJournaled) {
                return Err(StoreError::InjectedCrash);
            }
        }
        self.journal.publish(transaction)?;
        append_journal_record(
            &mut self.journal_file,
            &JournalEvent::Publish { transaction },
        )?;
        self.journal_file.sync_data()?;
        self.next_data_offset = next_data_offset;
        self.published.insert(record.page_key, record.clone());
        Ok(record)
    }
}

impl FileTierReader {
    pub fn open(root: &Path) -> Result<Self, StoreError> {
        let mut journal_file = OpenOptions::new()
            .read(true)
            .open(root.join("journal.log"))?;
        lock_shared_snapshot(&journal_file)?;
        let data = OpenOptions::new().read(true).open(root.join("pages.dat"))?;
        let mut journal_bytes = Vec::new();
        journal_file.read_to_end(&mut journal_bytes)?;
        let (events, _) = decode_journal(&journal_bytes)?;
        let published = TierJournal::from_events(events)?.recover()?;
        validate_catalog_extents(&published, data.metadata()?.len())?;
        Ok(Self { data, published })
    }

    pub fn restore(&mut self, page_key: [u8; 32]) -> Result<Option<RestoredPage>, StoreError> {
        restore_published(&mut self.data, &self.published, page_key)
    }
}

fn validate_catalog_extents(
    published: &BTreeMap<[u8; 32], TierRecord>,
    data_bytes: u64,
) -> Result<(), StoreError> {
    let mut extents = Vec::new();
    for record in published.values() {
        for piece in &record.pieces {
            let end = piece
                .storage_offset
                .checked_add(piece.byte_length)
                .ok_or(StoreError::Overflow)?;
            if end > data_bytes {
                return Err(StoreError::CatalogOutOfBounds);
            }
            extents.push((piece.storage_offset, end));
        }
    }
    extents.sort_unstable();
    if extents.windows(2).any(|pair| pair[1].0 < pair[0].1) {
        return Err(StoreError::CatalogOverlap);
    }
    Ok(())
}

fn restore_published(
    data: &mut File,
    published: &BTreeMap<[u8; 32], TierRecord>,
    page_key: [u8; 32],
) -> Result<Option<RestoredPage>, StoreError> {
    let Some(record) = published.get(&page_key).cloned() else {
        return Ok(None);
    };
    let mut pieces = BTreeMap::new();
    for piece in &record.pieces {
        let length = usize::try_from(piece.byte_length).map_err(|_| StoreError::Overflow)?;
        let mut bytes = vec![0_u8; length];
        data.seek(SeekFrom::Start(piece.storage_offset))?;
        data.read_exact(&mut bytes)?;
        let digest: [u8; 32] = Sha256::digest(&bytes).into();
        if digest != piece.sha256 {
            return Err(StoreError::Checksum);
        }
        pieces.insert(piece.piece, bytes);
    }
    Ok(Some(RestoredPage { record, pieces }))
}

fn append_journal_record(file: &mut File, event: &JournalEvent) -> Result<(), StoreError> {
    let bytes = encode_journal_event(event)?;
    file.seek(SeekFrom::End(0))?;
    file.write_all(&bytes)?;
    Ok(())
}

fn lock_exclusive_writer(file: &File) -> Result<(), StoreError> {
    // SAFETY: `file` owns a valid descriptor for the entire call. `flock`
    // neither retains the pointer nor accesses Rust-managed memory.
    let result = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
    if result == 0 {
        return Ok(());
    }
    let error = std::io::Error::last_os_error();
    if error.kind() == std::io::ErrorKind::WouldBlock {
        Err(StoreError::WriterLocked)
    } else {
        Err(StoreError::Io(error))
    }
}

fn lock_shared_snapshot(file: &File) -> Result<(), StoreError> {
    // SAFETY: `file` owns a valid descriptor for the entire call. `flock`
    // neither retains the pointer nor accesses Rust-managed memory.
    let result = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_SH | libc::LOCK_NB) };
    if result == 0 {
        return Ok(());
    }
    let error = std::io::Error::last_os_error();
    if error.kind() == std::io::ErrorKind::WouldBlock {
        Err(StoreError::WriterLocked)
    } else {
        Err(StoreError::Io(error))
    }
}

fn encode_journal_event(event: &JournalEvent) -> Result<[u8; JOURNAL_RECORD_BYTES], StoreError> {
    let mut output = [0_u8; JOURNAL_RECORD_BYTES];
    output[..8].copy_from_slice(&JOURNAL_MAGIC);
    put_u16(&mut output, 8, JOURNAL_VERSION);
    match event {
        JournalEvent::Begin {
            transaction,
            record,
        } => {
            record.validate()?;
            output[10] = 1;
            output[11] = record.tier as u8;
            put_u64(&mut output, 12, *transaction);
            output[20..52].copy_from_slice(&record.namespace);
            output[52..84].copy_from_slice(&record.page_key);
            put_u64(&mut output, 84, record.generation);
            output[92] = u8::from(record.mtp);
            output[93] = u8::try_from(record.pieces.len()).map_err(|_| StoreError::Overflow)?;
            for (ordinal, piece) in record.pieces.iter().enumerate() {
                encode_piece(&mut output, ordinal, *piece)?;
            }
        }
        JournalEvent::PieceDurable {
            transaction,
            piece,
            sha256,
        } => {
            output[10] = 2;
            put_u64(&mut output, 12, *transaction);
            encode_piece(
                &mut output,
                0,
                TierPieceRecord {
                    piece: *piece,
                    byte_length: piece.expected_bytes(),
                    storage_offset: 0,
                    sha256: *sha256,
                },
            )?;
        }
        JournalEvent::Publish { transaction } => {
            output[10] = 3;
            put_u64(&mut output, 12, *transaction);
        }
    }
    let crc = crc32c(&output);
    put_u32(&mut output, JOURNAL_CRC_OFFSET, crc);
    Ok(output)
}

fn decode_journal(bytes: &[u8]) -> Result<(Vec<JournalEvent>, u64), StoreError> {
    let mut events = Vec::with_capacity(bytes.len() / JOURNAL_RECORD_BYTES);
    let mut maximum_transaction = 0_u64;
    for record in bytes.chunks_exact(JOURNAL_RECORD_BYTES) {
        let event = decode_journal_event(record)?;
        let transaction = event_transaction(&event);
        maximum_transaction = maximum_transaction.max(transaction);
        events.push(event);
    }
    let next_transaction = if maximum_transaction == 0 {
        1
    } else {
        maximum_transaction
            .checked_add(1)
            .ok_or(StoreError::Overflow)?
    };
    Ok((events, next_transaction))
}

fn decode_journal_event(bytes: &[u8]) -> Result<JournalEvent, StoreError> {
    if bytes.len() != JOURNAL_RECORD_BYTES {
        return Err(StoreError::JournalEncoding);
    }
    let mut checked = [0_u8; JOURNAL_RECORD_BYTES];
    checked.copy_from_slice(bytes);
    let expected_crc = get_u32(bytes, JOURNAL_CRC_OFFSET);
    checked[JOURNAL_CRC_OFFSET..].fill(0);
    if crc32c(&checked) != expected_crc {
        return Err(StoreError::JournalChecksum);
    }
    if bytes[..8] != JOURNAL_MAGIC || get_u16(bytes, 8) != JOURNAL_VERSION {
        return Err(StoreError::JournalEncoding);
    }
    let transaction = get_u64(bytes, 12);
    if transaction == 0 {
        return Err(StoreError::JournalEncoding);
    }
    match bytes[10] {
        1 => {
            let tier = match bytes[11] {
                1 => Tier::Dram,
                2 => Tier::Nvme,
                _ => return Err(StoreError::JournalEncoding),
            };
            let piece_count = usize::from(bytes[93]);
            if piece_count == 0 || piece_count > 3 {
                return Err(StoreError::JournalEncoding);
            }
            let mut pieces = Vec::with_capacity(piece_count);
            for ordinal in 0..piece_count {
                pieces.push(decode_piece(bytes, ordinal)?);
            }
            let record = TierRecord {
                namespace: bytes[20..52]
                    .try_into()
                    .map_err(|_| StoreError::JournalEncoding)?,
                page_key: bytes[52..84]
                    .try_into()
                    .map_err(|_| StoreError::JournalEncoding)?,
                generation: get_u64(bytes, 84),
                tier,
                mtp: match bytes[92] {
                    0 => false,
                    1 => true,
                    _ => return Err(StoreError::JournalEncoding),
                },
                pieces,
            };
            record.validate()?;
            Ok(JournalEvent::Begin {
                transaction,
                record,
            })
        }
        2 => {
            let piece = decode_piece(bytes, 0)?;
            Ok(JournalEvent::PieceDurable {
                transaction,
                piece: piece.piece,
                sha256: piece.sha256,
            })
        }
        3 => Ok(JournalEvent::Publish { transaction }),
        _ => Err(StoreError::JournalEncoding),
    }
}

fn encode_piece(
    output: &mut [u8; JOURNAL_RECORD_BYTES],
    ordinal: usize,
    piece: TierPieceRecord,
) -> Result<(), StoreError> {
    let offset = PIECE_TABLE_OFFSET
        .checked_add(
            ordinal
                .checked_mul(PIECE_RECORD_BYTES)
                .ok_or(StoreError::Overflow)?,
        )
        .ok_or(StoreError::Overflow)?;
    if offset + PIECE_RECORD_BYTES > JOURNAL_CRC_OFFSET {
        return Err(StoreError::Overflow);
    }
    output[offset] = piece.piece as u8;
    put_u64(output, offset + 8, piece.byte_length);
    put_u64(output, offset + 16, piece.storage_offset);
    output[offset + 24..offset + 56].copy_from_slice(&piece.sha256);
    Ok(())
}

fn decode_piece(bytes: &[u8], ordinal: usize) -> Result<TierPieceRecord, StoreError> {
    let offset = PIECE_TABLE_OFFSET
        .checked_add(
            ordinal
                .checked_mul(PIECE_RECORD_BYTES)
                .ok_or(StoreError::Overflow)?,
        )
        .ok_or(StoreError::Overflow)?;
    let piece = match bytes.get(offset).copied() {
        Some(1) => TierPiece::TargetKv,
        Some(2) => TierPiece::TargetIndexer,
        Some(3) => TierPiece::DraftSidecar,
        _ => return Err(StoreError::JournalEncoding),
    };
    let sha256 = bytes
        .get(offset + 24..offset + 56)
        .ok_or(StoreError::JournalEncoding)?
        .try_into()
        .map_err(|_| StoreError::JournalEncoding)?;
    Ok(TierPieceRecord {
        piece,
        byte_length: get_u64(bytes, offset + 8),
        storage_offset: get_u64(bytes, offset + 16),
        sha256,
    })
}

fn event_transaction(event: &JournalEvent) -> u64 {
    match event {
        JournalEvent::Begin { transaction, .. }
        | JournalEvent::PieceDurable { transaction, .. }
        | JournalEvent::Publish { transaction } => *transaction,
    }
}

fn align_up(value: u64, alignment: u64) -> Result<u64, StoreError> {
    value
        .checked_add(alignment - 1)
        .map(|value| value / alignment * alignment)
        .ok_or(StoreError::Overflow)
}

fn put_u16(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn put_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn put_u64(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

fn get_u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(bytes[offset..offset + 2].try_into().expect("bounded"))
}

fn get_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().expect("bounded"))
}

fn get_u64(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(bytes[offset..offset + 8].try_into().expect("bounded"))
}

#[derive(Debug)]
pub enum StoreError {
    Io(std::io::Error),
    Tier(TierError),
    Pieces,
    Checksum,
    ContentCollision,
    StaleGeneration,
    JournalSequence,
    JournalEncoding,
    JournalChecksum,
    CatalogOverlap,
    CatalogOutOfBounds,
    Overflow,
    InjectedCrash,
    WritePoisoned,
    WriterLocked,
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for StoreError {}

impl From<std::io::Error> for StoreError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<TierError> for StoreError {
    fn from(value: TierError) -> Self {
        Self::Tier(value)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WriteFailpoint {
    BeginJournaled,
    DataSynced,
    FirstPieceJournaled,
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    fn temporary_store(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("glmaxx-{name}-{}-{nonce}", std::process::id()))
    }

    fn request(key: u8, generation: u64, mtp: bool) -> DurablePageRequest {
        let pieces = [
            TierPiece::TargetKv,
            TierPiece::TargetIndexer,
            TierPiece::DraftSidecar,
        ];
        DurablePageRequest {
            namespace: [0x11; 32],
            page_key: [key; 32],
            generation,
            mtp,
            pieces: pieces[..if mtp { 3 } else { 2 }]
                .iter()
                .enumerate()
                .map(|(ordinal, &piece)| PagePieceBytes {
                    piece,
                    bytes: vec![key.wrapping_add(ordinal as u8); piece.expected_bytes() as usize],
                })
                .collect(),
        }
    }

    #[test]
    fn published_page_survives_close_reopen_and_hash_validation() {
        let root = temporary_store("durable");
        let mut store = FileTierStore::open(&root).unwrap();
        let record = store.publish(request(0x22, 7, true)).unwrap();
        assert_eq!(record.pieces.len(), 3);
        assert!(
            record
                .pieces
                .iter()
                .all(|piece| piece.storage_offset.is_multiple_of(4096))
        );
        drop(store);

        let mut reopened = FileTierStore::open(&root).unwrap();
        let restored = reopened.restore([0x22; 32]).unwrap().unwrap();
        assert_eq!(restored.record.generation, 7);
        assert_eq!(restored.pieces.len(), 3);
        assert_eq!(
            restored.pieces[&TierPiece::DraftSidecar][0],
            0x22_u8.wrapping_add(2)
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn journal_lock_enforces_one_live_store_writer() {
        let root = temporary_store("single-writer");
        let mut first = FileTierStore::open(&root).unwrap();
        first.publish(request(0x19, 1, false)).unwrap();

        assert!(matches!(
            FileTierStore::open(&root),
            Err(StoreError::WriterLocked)
        ));
        assert!(matches!(
            FileTierReader::open(&root),
            Err(StoreError::WriterLocked)
        ));
        assert!(first.restore([0x19; 32]).unwrap().is_some());
        drop(first);

        let mut readers = (0..4)
            .map(|_| FileTierReader::open(&root).unwrap())
            .collect::<Vec<_>>();
        assert!(
            readers
                .iter_mut()
                .all(|reader| reader.restore([0x19; 32]).unwrap().is_some())
        );
        let mut second = FileTierStore::open(&root).unwrap();
        second.publish(request(0x1a, 1, false)).unwrap();
        assert!(second.restore([0x19; 32]).unwrap().is_some());
        assert!(second.restore([0x1a; 32]).unwrap().is_some());
        assert!(
            readers
                .iter_mut()
                .all(|reader| reader.restore([0x1a; 32]).unwrap().is_none())
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn crash_before_publication_leaves_only_invisible_orphans() {
        for (ordinal, failpoint) in [
            WriteFailpoint::BeginJournaled,
            WriteFailpoint::DataSynced,
            WriteFailpoint::FirstPieceJournaled,
        ]
        .into_iter()
        .enumerate()
        {
            let root = temporary_store(&format!("crash-{ordinal}"));
            let mut store = FileTierStore::open(&root).unwrap();
            assert!(matches!(
                store.publish_inner(request(0x30 + ordinal as u8, 1, false), Some(failpoint)),
                Err(StoreError::InjectedCrash)
            ));
            assert!(matches!(
                store.publish(request(0x40 + ordinal as u8, 1, false)),
                Err(StoreError::WritePoisoned)
            ));
            drop(store);
            let mut reopened = FileTierStore::open(&root).unwrap();
            assert!(
                reopened
                    .restore([0x30 + ordinal as u8; 32])
                    .unwrap()
                    .is_none()
            );
            fs::remove_dir_all(root).unwrap();
        }
    }

    #[test]
    fn failed_publication_poison_writes_until_replay_but_not_preflight_errors() {
        let root = temporary_store("poisoned-writer");
        let mut store = FileTierStore::open(&root).unwrap();

        assert!(matches!(
            store.publish_inner(
                DurablePageRequest {
                    pieces: Vec::new(),
                    ..request(0x20, 1, false)
                },
                None
            ),
            Err(StoreError::Pieces)
        ));
        store.publish(request(0x20, 1, false)).unwrap();
        let deduplicated = store.publish(request(0x20, 1, false)).unwrap();
        assert_eq!(deduplicated.generation, 1);
        store.publish(request(0x20, 2, false)).unwrap();

        assert!(matches!(
            store.publish_inner(
                request(0x21, 1, false),
                Some(WriteFailpoint::FirstPieceJournaled)
            ),
            Err(StoreError::InjectedCrash)
        ));
        let journal_bytes_after_failure = store.journal_file.metadata().unwrap().len();
        let data_bytes_after_failure = store.data.metadata().unwrap().len();
        assert!(matches!(
            store.publish(request(0x22, 1, false)),
            Err(StoreError::WritePoisoned)
        ));
        assert_eq!(
            store.journal_file.metadata().unwrap().len(),
            journal_bytes_after_failure
        );
        assert_eq!(
            store.data.metadata().unwrap().len(),
            data_bytes_after_failure
        );
        assert!(store.restore([0x20; 32]).unwrap().is_some());
        drop(store);

        let mut reopened = FileTierStore::open(&root).unwrap();
        assert!(reopened.restore([0x21; 32]).unwrap().is_none());
        reopened.publish(request(0x22, 1, false)).unwrap();
        assert!(reopened.restore([0x22; 32]).unwrap().is_some());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn durable_content_dedup_upgrade_and_collision_are_preflighted() {
        let root = temporary_store("content-identity");
        let mut store = FileTierStore::open(&root).unwrap();
        let target = store.publish(request(0x43, 1, false)).unwrap();
        let journal_after_target = store.journal_file.metadata().unwrap().len();
        let data_after_target = store.data.metadata().unwrap().len();

        let deduplicated = store.publish(request(0x43, 9, false)).unwrap();
        assert_eq!(deduplicated, target);
        assert_eq!(
            store.journal_file.metadata().unwrap().len(),
            journal_after_target
        );
        assert_eq!(store.data.metadata().unwrap().len(), data_after_target);

        assert!(matches!(
            store.publish(request(0x43, 1, true)),
            Err(StoreError::StaleGeneration)
        ));
        assert_eq!(
            store.journal_file.metadata().unwrap().len(),
            journal_after_target
        );
        assert_eq!(store.data.metadata().unwrap().len(), data_after_target);

        let upgrade = store.publish(request(0x43, 2, true)).unwrap();
        assert!(upgrade.mtp);
        assert_eq!(upgrade.generation, 2);
        let journal_after_upgrade = store.journal_file.metadata().unwrap().len();
        let data_after_upgrade = store.data.metadata().unwrap().len();

        for candidate in [request(0x43, 3, false), request(0x43, 4, true)] {
            assert_eq!(store.publish(candidate).unwrap(), upgrade);
            assert_eq!(
                store.journal_file.metadata().unwrap().len(),
                journal_after_upgrade
            );
            assert_eq!(store.data.metadata().unwrap().len(), data_after_upgrade);
        }

        let mut conflicting_target = request(0x43, 5, true);
        conflicting_target.pieces[0].bytes[0] ^= 1;
        assert!(matches!(
            store.publish(conflicting_target),
            Err(StoreError::ContentCollision)
        ));
        let mut conflicting_draft = request(0x43, 5, true);
        conflicting_draft.pieces[2].bytes[0] ^= 1;
        assert!(matches!(
            store.publish(conflicting_draft),
            Err(StoreError::ContentCollision)
        ));
        assert_eq!(
            store.journal_file.metadata().unwrap().len(),
            journal_after_upgrade
        );
        assert_eq!(store.data.metadata().unwrap().len(), data_after_upgrade);
        assert_eq!(store.record([0x43; 32]), Some(&upgrade));

        store.publish(request(0x44, 1, false)).unwrap();
        drop(store);
        let reopened = FileTierStore::open(&root).unwrap();
        assert_eq!(reopened.record([0x43; 32]), Some(&upgrade));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn data_corruption_fails_closed() {
        let root = temporary_store("corrupt");
        let mut store = FileTierStore::open(&root).unwrap();
        store.publish(request(0x44, 2, false)).unwrap();
        let first_offset = store.record([0x44; 32]).unwrap().pieces[0].storage_offset;
        store.data.seek(SeekFrom::Start(first_offset)).unwrap();
        store.data.write_all(&[0xff]).unwrap();
        store.data.sync_data().unwrap();
        assert!(matches!(
            store.restore([0x44; 32]),
            Err(StoreError::Checksum)
        ));
        drop(store);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn startup_rejects_cross_page_extent_overlap() {
        let root = temporary_store("catalog-overlap");
        let mut store = FileTierStore::open(&root).unwrap();
        let first = store.publish(request(0x51, 1, false)).unwrap();
        store.publish(request(0x52, 1, false)).unwrap();
        drop(store);

        let journal_path = root.join("journal.log");
        let journal_bytes = fs::read(&journal_path).unwrap();
        let (mut events, _) = decode_journal(&journal_bytes).unwrap();
        let second = events
            .iter_mut()
            .find_map(|event| match event {
                JournalEvent::Begin {
                    transaction: 2,
                    record,
                } => Some(record),
                _ => None,
            })
            .unwrap();
        second.pieces[0].storage_offset = first.pieces[0].storage_offset;
        second.validate().unwrap();

        let mut rewritten = Vec::with_capacity(journal_bytes.len());
        for event in &events {
            rewritten.extend_from_slice(&encode_journal_event(event).unwrap());
        }
        fs::write(&journal_path, rewritten).unwrap();

        assert!(matches!(
            FileTierStore::open(&root),
            Err(StoreError::CatalogOverlap)
        ));
        assert!(matches!(
            FileTierReader::open(&root),
            Err(StoreError::CatalogOverlap)
        ));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn startup_rejects_catalog_extent_beyond_data_file() {
        let root = temporary_store("catalog-bounds");
        let mut store = FileTierStore::open(&root).unwrap();
        store.publish(request(0x53, 1, false)).unwrap();
        drop(store);

        let data_path = root.join("pages.dat");
        let data = OpenOptions::new().write(true).open(&data_path).unwrap();
        let truncated_bytes = data.metadata().unwrap().len() - 1;
        data.set_len(truncated_bytes).unwrap();
        data.sync_data().unwrap();
        drop(data);

        assert!(matches!(
            FileTierStore::open(&root),
            Err(StoreError::CatalogOutOfBounds)
        ));
        assert!(matches!(
            FileTierReader::open(&root),
            Err(StoreError::CatalogOutOfBounds)
        ));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn resumed_publication_preserves_every_byte_before_physical_eof() {
        let root = temporary_store("physical-append");
        let mut store = FileTierStore::open(&root).unwrap();
        store.publish(request(0x54, 1, false)).unwrap();
        drop(store);

        let data_path = root.join("pages.dat");
        let garbage_start = fs::metadata(&data_path).unwrap().len();
        let garbage = [0xa5_u8; 8192];
        let mut data = OpenOptions::new().append(true).open(&data_path).unwrap();
        data.write_all(&garbage).unwrap();
        data.sync_data().unwrap();
        drop(data);
        let prior_physical_eof = fs::metadata(&data_path).unwrap().len();

        let mut reopened = FileTierStore::open(&root).unwrap();
        let second = reopened.publish(request(0x55, 1, false)).unwrap();
        assert!(
            second.pieces[0].storage_offset
                >= align_up(prior_physical_eof, NVME_ALIGNMENT).unwrap()
        );
        let mut preserved = [0_u8; 8192];
        reopened.data.seek(SeekFrom::Start(garbage_start)).unwrap();
        reopened.data.read_exact(&mut preserved).unwrap();
        assert_eq!(preserved, garbage);
        drop(reopened);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn torn_trailing_journal_record_is_ignored() {
        let root = temporary_store("torn");
        let mut store = FileTierStore::open(&root).unwrap();
        store.publish(request(0x55, 1, false)).unwrap();
        drop(store);
        let journal_path = root.join("journal.log");
        let mut journal = OpenOptions::new().append(true).open(&journal_path).unwrap();
        journal.write_all(&[0xaa; 113]).unwrap();
        journal.sync_data().unwrap();
        drop(journal);
        let length_with_tail = fs::metadata(&journal_path).unwrap().len();

        let mut reader = FileTierReader::open(&root).unwrap();
        assert!(reader.restore([0x55; 32]).unwrap().is_some());
        drop(reader);
        assert_eq!(fs::metadata(&journal_path).unwrap().len(), length_with_tail);

        let mut reopened = FileTierStore::open(&root).unwrap();
        assert!(reopened.restore([0x55; 32]).unwrap().is_some());
        assert_eq!(
            fs::metadata(&journal_path).unwrap().len(),
            length_with_tail - 113
        );
        reopened.publish(request(0x56, 1, false)).unwrap();
        drop(reopened);

        let mut second_reopen = FileTierStore::open(&root).unwrap();
        assert!(second_reopen.restore([0x55; 32]).unwrap().is_some());
        assert!(second_reopen.restore([0x56; 32]).unwrap().is_some());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn complete_corrupt_trailing_journal_record_is_never_ignored() {
        let root = temporary_store("complete-corrupt-tail");
        let mut store = FileTierStore::open(&root).unwrap();
        store.publish(request(0x56, 1, false)).unwrap();
        drop(store);

        let journal_path = root.join("journal.log");
        let mut journal = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&journal_path)
            .unwrap();
        let final_byte_offset = journal.metadata().unwrap().len() - 1;
        journal.seek(SeekFrom::Start(final_byte_offset)).unwrap();
        let mut final_byte = [0_u8; 1];
        journal.read_exact(&mut final_byte).unwrap();
        final_byte[0] ^= 1;
        journal.seek(SeekFrom::Start(final_byte_offset)).unwrap();
        journal.write_all(&final_byte).unwrap();
        journal.sync_data().unwrap();
        drop(journal);

        assert!(matches!(
            FileTierStore::open(&root),
            Err(StoreError::JournalChecksum)
        ));
        assert!(matches!(
            FileTierReader::open(&root),
            Err(StoreError::JournalChecksum)
        ));
        fs::remove_dir_all(root).unwrap();

        let root = temporary_store("complete-garbage-tail");
        let mut store = FileTierStore::open(&root).unwrap();
        store.publish(request(0x57, 1, false)).unwrap();
        drop(store);
        let mut journal = OpenOptions::new()
            .append(true)
            .open(root.join("journal.log"))
            .unwrap();
        journal.write_all(&[0xaa; JOURNAL_RECORD_BYTES]).unwrap();
        journal.sync_data().unwrap();
        drop(journal);
        assert!(matches!(
            FileTierStore::open(&root),
            Err(StoreError::JournalChecksum)
        ));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn v1_journal_fails_closed_after_unified_draft_sidecar_change() {
        let event = JournalEvent::Begin {
            transaction: 1,
            record: TierRecord {
                namespace: [0x11; 32],
                page_key: [0x22; 32],
                generation: 1,
                tier: Tier::Nvme,
                mtp: true,
                pieces: request(0x22, 1, true)
                    .pieces
                    .into_iter()
                    .enumerate()
                    .map(|(ordinal, piece)| TierPieceRecord {
                        piece: piece.piece,
                        byte_length: piece.piece.expected_bytes(),
                        storage_offset: ordinal as u64 * 2 * 1024 * 1024,
                        sha256: [ordinal as u8 + 1; 32],
                    })
                    .collect(),
            },
        };
        let current = encode_journal_event(&event).unwrap();
        assert_eq!(&current[..8], b"GLTJRNL2");
        assert_eq!(get_u16(&current, 8), 2);

        let mut stale = current;
        stale[..8].copy_from_slice(b"GLTJRNL1");
        put_u16(&mut stale, 8, 1);
        stale[JOURNAL_CRC_OFFSET..].fill(0);
        let crc = crc32c(&stale);
        put_u32(&mut stale, JOURNAL_CRC_OFFSET, crc);
        assert!(matches!(
            decode_journal_event(&stale),
            Err(StoreError::JournalEncoding)
        ));
    }
}
