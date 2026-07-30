use sha2::{Digest, Sha256};

use crate::{TierPiece, TierRecord};

pub const DIRECT_TIER_FORMAT_VERSION: u16 = 1;
pub const DIRECT_IO_ALIGNMENT: u64 = 4_096;
pub const TARGET_KV_EXTENT_OFFSET: u64 = 0;
pub const TARGET_KV_EXTENT_LENGTH: u64 = crate::tier::TARGET_KV_PAGE_BYTES;
pub const TARGET_INDEXER_EXTENT_OFFSET: u64 = 1_839_104;
pub const TARGET_INDEXER_EXTENT_LENGTH: u64 = crate::tier::TARGET_INDEXER_PAGE_BYTES;
pub const DRAFT_SIDECAR_EXTENT_OFFSET: u64 = 2_019_328;
pub const DRAFT_SIDECAR_EXTENT_LENGTH: u64 = crate::tier::DRAFT_SIDECAR_PAGE_BYTES;
pub const TARGET_ONLY_LOGICAL_BYTES: u64 = TARGET_KV_EXTENT_LENGTH + TARGET_INDEXER_EXTENT_LENGTH;
pub const TARGET_ONLY_PHYSICAL_BYTES: u64 = 2_019_328;
pub const MTP_LOGICAL_BYTES: u64 = TARGET_ONLY_LOGICAL_BYTES + DRAFT_SIDECAR_EXTENT_LENGTH;
pub const MTP_PHYSICAL_BYTES: u64 = 2_052_096;

const TARGET_INDEXER_END: u64 = TARGET_INDEXER_EXTENT_OFFSET + TARGET_INDEXER_EXTENT_LENGTH;
const DRAFT_SIDECAR_END: u64 = DRAFT_SIDECAR_EXTENT_OFFSET + DRAFT_SIDECAR_EXTENT_LENGTH;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum DirectTierCapability {
    Target = 1,
    Mtp = 2,
}

impl DirectTierCapability {
    #[must_use]
    pub const fn physical_bytes(self) -> u64 {
        match self {
            Self::Target => TARGET_ONLY_PHYSICAL_BYTES,
            Self::Mtp => MTP_PHYSICAL_BYTES,
        }
    }

    #[must_use]
    pub const fn logical_bytes(self) -> u64 {
        match self {
            Self::Target => TARGET_ONLY_LOGICAL_BYTES,
            Self::Mtp => MTP_LOGICAL_BYTES,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectPieceRecord {
    pub piece: TierPiece,
    pub extent_offset: u64,
    pub logical_length: u64,
    pub sha256: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectExtentRecord {
    pub format_version: u16,
    pub namespace: [u8; 32],
    pub page_key: [u8; 32],
    pub durable_revision: u64,
    pub capability: DirectTierCapability,
    pub segment_id: u64,
    pub physical_offset: u64,
    pub physical_length: u64,
    pub physical_sha256: [u8; 32],
    pub pieces: Vec<DirectPieceRecord>,
}

impl DirectExtentRecord {
    pub fn validate(&self) -> Result<(), DirectExtentError> {
        if self.format_version != DIRECT_TIER_FORMAT_VERSION {
            return Err(DirectExtentError::Version);
        }
        if self.namespace == [0; 32]
            || self.page_key == [0; 32]
            || self.durable_revision == 0
            || self.segment_id == 0
        {
            return Err(DirectExtentError::Identity);
        }
        if self.physical_length != self.capability.physical_bytes()
            || self.physical_sha256 == [0; 32]
        {
            return Err(DirectExtentError::Physical);
        }
        validate_direct_io_span(
            DIRECT_IO_ALIGNMENT,
            self.physical_offset,
            self.physical_length,
            self.capability.physical_bytes(),
        )?;
        let expected = expected_piece_layout(self.capability);
        if self.pieces.len() != expected.len() {
            return Err(DirectExtentError::Pieces);
        }
        for (observed, &(piece, offset, length)) in self.pieces.iter().zip(expected) {
            if observed.piece != piece
                || observed.extent_offset != offset
                || observed.logical_length != length
                || observed.sha256 == [0; 32]
                || !observed.extent_offset.is_multiple_of(DIRECT_IO_ALIGNMENT)
                || observed
                    .extent_offset
                    .checked_add(observed.logical_length)
                    .ok_or(DirectExtentError::Overflow)?
                    > self.physical_length
            {
                return Err(DirectExtentError::Pieces);
            }
        }
        Ok(())
    }

    /// The retained blocking-store record has no segment identity, complete
    /// physical extent, or physical digest. It therefore has an explicit
    /// migration boundary and can never be relabeled as direct format.
    pub fn try_from_blocking_store(record: &TierRecord) -> Result<Self, DirectExtentError> {
        record
            .validate()
            .map_err(|_| DirectExtentError::LegacyRecord)?;
        Err(DirectExtentError::MigrationRequired)
    }
}

pub struct DirectPagePieces<'a> {
    pub target_kv: &'a [u8],
    pub target_indexer: &'a [u8],
    pub draft_sidecar: Option<&'a [u8]>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectExtentView<'a> {
    pub target_kv: &'a [u8],
    pub target_indexer: &'a [u8],
    pub draft_sidecar: Option<&'a [u8]>,
}

#[derive(Debug)]
pub struct DirectExtentBuffer {
    storage: Vec<u8>,
    aligned_start: usize,
    length: usize,
}

impl DirectExtentBuffer {
    pub fn new(capability: DirectTierCapability) -> Result<Self, DirectExtentError> {
        let length = usize::try_from(capability.physical_bytes())
            .map_err(|_| DirectExtentError::Overflow)?;
        let alignment =
            usize::try_from(DIRECT_IO_ALIGNMENT).map_err(|_| DirectExtentError::Overflow)?;
        let storage_length = length
            .checked_add(alignment - 1)
            .ok_or(DirectExtentError::Overflow)?;
        let mut storage = Vec::new();
        storage
            .try_reserve_exact(storage_length)
            .map_err(|_| DirectExtentError::Allocation)?;
        storage.resize(storage_length, 0);
        let address = storage.as_ptr() as usize;
        let aligned_address = address
            .checked_add(alignment - 1)
            .ok_or(DirectExtentError::Overflow)?
            / alignment
            * alignment;
        let aligned_start = aligned_address
            .checked_sub(address)
            .ok_or(DirectExtentError::Overflow)?;
        let buffer = Self {
            storage,
            aligned_start,
            length,
        };
        validate_direct_io_span(
            buffer.as_slice().as_ptr() as u64,
            0,
            capability.physical_bytes(),
            capability.physical_bytes(),
        )?;
        Ok(buffer)
    }

    #[must_use]
    pub fn as_slice(&self) -> &[u8] {
        &self.storage[self.aligned_start..self.aligned_start + self.length]
    }

    #[must_use]
    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        &mut self.storage[self.aligned_start..self.aligned_start + self.length]
    }

    #[must_use]
    pub const fn len(&self) -> usize {
        self.length
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.length == 0
    }
}

pub fn encode_direct_extent(
    namespace: [u8; 32],
    page_key: [u8; 32],
    durable_revision: u64,
    segment_id: u64,
    physical_offset: u64,
    pieces: DirectPagePieces<'_>,
) -> Result<(DirectExtentRecord, DirectExtentBuffer), DirectExtentError> {
    let capability = if pieces.draft_sidecar.is_some() {
        DirectTierCapability::Mtp
    } else {
        DirectTierCapability::Target
    };
    if pieces.target_kv.len() != TARGET_KV_EXTENT_LENGTH as usize
        || pieces.target_indexer.len() != TARGET_INDEXER_EXTENT_LENGTH as usize
        || pieces
            .draft_sidecar
            .is_some_and(|draft| draft.len() != DRAFT_SIDECAR_EXTENT_LENGTH as usize)
    {
        return Err(DirectExtentError::Pieces);
    }
    validate_direct_io_span(
        DIRECT_IO_ALIGNMENT,
        physical_offset,
        capability.physical_bytes(),
        capability.physical_bytes(),
    )?;

    let mut buffer = DirectExtentBuffer::new(capability)?;
    let output = buffer.as_mut_slice();
    output.fill(0);
    copy_piece(output, TARGET_KV_EXTENT_OFFSET, pieces.target_kv)?;
    copy_piece(output, TARGET_INDEXER_EXTENT_OFFSET, pieces.target_indexer)?;
    if let Some(draft) = pieces.draft_sidecar {
        copy_piece(output, DRAFT_SIDECAR_EXTENT_OFFSET, draft)?;
    }
    validate_zero_padding(output, capability)?;

    let mut piece_records = vec![
        DirectPieceRecord {
            piece: TierPiece::TargetKv,
            extent_offset: TARGET_KV_EXTENT_OFFSET,
            logical_length: TARGET_KV_EXTENT_LENGTH,
            sha256: Sha256::digest(pieces.target_kv).into(),
        },
        DirectPieceRecord {
            piece: TierPiece::TargetIndexer,
            extent_offset: TARGET_INDEXER_EXTENT_OFFSET,
            logical_length: TARGET_INDEXER_EXTENT_LENGTH,
            sha256: Sha256::digest(pieces.target_indexer).into(),
        },
    ];
    if let Some(draft) = pieces.draft_sidecar {
        piece_records.push(DirectPieceRecord {
            piece: TierPiece::DraftSidecar,
            extent_offset: DRAFT_SIDECAR_EXTENT_OFFSET,
            logical_length: DRAFT_SIDECAR_EXTENT_LENGTH,
            sha256: Sha256::digest(draft).into(),
        });
    }
    let record = DirectExtentRecord {
        format_version: DIRECT_TIER_FORMAT_VERSION,
        namespace,
        page_key,
        durable_revision,
        capability,
        segment_id,
        physical_offset,
        physical_length: capability.physical_bytes(),
        physical_sha256: Sha256::digest(buffer.as_slice()).into(),
        pieces: piece_records,
    };
    record.validate()?;
    Ok((record, buffer))
}

pub fn decode_direct_extent<'a>(
    record: &DirectExtentRecord,
    extent: &'a [u8],
) -> Result<DirectExtentView<'a>, DirectExtentError> {
    record.validate()?;
    validate_direct_io_span(
        extent.as_ptr() as u64,
        record.physical_offset,
        extent.len() as u64,
        record.physical_length,
    )?;
    if Sha256::digest(extent).as_slice() != record.physical_sha256 {
        return Err(DirectExtentError::PhysicalChecksum);
    }
    validate_zero_padding(extent, record.capability)?;
    for piece in &record.pieces {
        let bytes = piece_bytes(extent, piece.extent_offset, piece.logical_length)?;
        if Sha256::digest(bytes).as_slice() != piece.sha256 {
            return Err(DirectExtentError::PieceChecksum(piece.piece));
        }
    }
    Ok(DirectExtentView {
        target_kv: piece_bytes(extent, TARGET_KV_EXTENT_OFFSET, TARGET_KV_EXTENT_LENGTH)?,
        target_indexer: piece_bytes(
            extent,
            TARGET_INDEXER_EXTENT_OFFSET,
            TARGET_INDEXER_EXTENT_LENGTH,
        )?,
        draft_sidecar: if record.capability == DirectTierCapability::Mtp {
            Some(piece_bytes(
                extent,
                DRAFT_SIDECAR_EXTENT_OFFSET,
                DRAFT_SIDECAR_EXTENT_LENGTH,
            )?)
        } else {
            None
        },
    })
}

pub fn validate_direct_io_span(
    address: u64,
    file_offset: u64,
    length: u64,
    expected_length: u64,
) -> Result<(), DirectExtentError> {
    if address == 0 || !address.is_multiple_of(DIRECT_IO_ALIGNMENT) {
        return Err(DirectExtentError::AddressAlignment);
    }
    if !file_offset.is_multiple_of(DIRECT_IO_ALIGNMENT) {
        return Err(DirectExtentError::OffsetAlignment);
    }
    if length == 0 || !length.is_multiple_of(DIRECT_IO_ALIGNMENT) || length != expected_length {
        return Err(DirectExtentError::LengthAlignment);
    }
    Ok(())
}

fn expected_piece_layout(capability: DirectTierCapability) -> &'static [(TierPiece, u64, u64)] {
    const TARGET: &[(TierPiece, u64, u64)] = &[
        (
            TierPiece::TargetKv,
            TARGET_KV_EXTENT_OFFSET,
            TARGET_KV_EXTENT_LENGTH,
        ),
        (
            TierPiece::TargetIndexer,
            TARGET_INDEXER_EXTENT_OFFSET,
            TARGET_INDEXER_EXTENT_LENGTH,
        ),
    ];
    const MTP: &[(TierPiece, u64, u64)] = &[
        (
            TierPiece::TargetKv,
            TARGET_KV_EXTENT_OFFSET,
            TARGET_KV_EXTENT_LENGTH,
        ),
        (
            TierPiece::TargetIndexer,
            TARGET_INDEXER_EXTENT_OFFSET,
            TARGET_INDEXER_EXTENT_LENGTH,
        ),
        (
            TierPiece::DraftSidecar,
            DRAFT_SIDECAR_EXTENT_OFFSET,
            DRAFT_SIDECAR_EXTENT_LENGTH,
        ),
    ];
    match capability {
        DirectTierCapability::Target => TARGET,
        DirectTierCapability::Mtp => MTP,
    }
}

fn copy_piece(output: &mut [u8], offset: u64, source: &[u8]) -> Result<(), DirectExtentError> {
    let start = usize::try_from(offset).map_err(|_| DirectExtentError::Overflow)?;
    let end = start
        .checked_add(source.len())
        .ok_or(DirectExtentError::Overflow)?;
    output
        .get_mut(start..end)
        .ok_or(DirectExtentError::Physical)?
        .copy_from_slice(source);
    Ok(())
}

fn piece_bytes(extent: &[u8], offset: u64, length: u64) -> Result<&[u8], DirectExtentError> {
    let start = usize::try_from(offset).map_err(|_| DirectExtentError::Overflow)?;
    let length = usize::try_from(length).map_err(|_| DirectExtentError::Overflow)?;
    let end = start
        .checked_add(length)
        .ok_or(DirectExtentError::Overflow)?;
    extent.get(start..end).ok_or(DirectExtentError::Physical)
}

fn validate_zero_padding(
    extent: &[u8],
    capability: DirectTierCapability,
) -> Result<(), DirectExtentError> {
    let target_kv_end = TARGET_KV_EXTENT_LENGTH as usize;
    let target_indexer_start = TARGET_INDEXER_EXTENT_OFFSET as usize;
    let target_indexer_end = TARGET_INDEXER_END as usize;
    let target_physical_end = TARGET_ONLY_PHYSICAL_BYTES as usize;
    if extent
        .get(target_kv_end..target_indexer_start)
        .ok_or(DirectExtentError::Physical)?
        .iter()
        .any(|&byte| byte != 0)
    {
        return Err(DirectExtentError::Padding);
    }
    match capability {
        DirectTierCapability::Target => {
            if extent.len() != target_physical_end
                || extent
                    .get(target_indexer_end..target_physical_end)
                    .ok_or(DirectExtentError::Physical)?
                    .iter()
                    .any(|&byte| byte != 0)
            {
                return Err(DirectExtentError::Padding);
            }
        }
        DirectTierCapability::Mtp => {
            let draft_start = DRAFT_SIDECAR_EXTENT_OFFSET as usize;
            let draft_end = DRAFT_SIDECAR_END as usize;
            let mtp_physical_end = MTP_PHYSICAL_BYTES as usize;
            if extent.len() != mtp_physical_end
                || extent
                    .get(target_indexer_end..draft_start)
                    .ok_or(DirectExtentError::Physical)?
                    .iter()
                    .any(|&byte| byte != 0)
                || extent
                    .get(draft_end..mtp_physical_end)
                    .ok_or(DirectExtentError::Physical)?
                    .iter()
                    .any(|&byte| byte != 0)
            {
                return Err(DirectExtentError::Padding);
            }
        }
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectExtentError {
    Version,
    Identity,
    AddressAlignment,
    OffsetAlignment,
    LengthAlignment,
    Pieces,
    Physical,
    PhysicalChecksum,
    PieceChecksum(TierPiece),
    Padding,
    LegacyRecord,
    MigrationRequired,
    Allocation,
    Overflow,
}

impl std::fmt::Display for DirectExtentError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for DirectExtentError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Tier, TierPieceRecord};

    fn bytes(length: u64, seed: u8) -> Vec<u8> {
        (0..length)
            .map(|index| seed.wrapping_add((index % 251) as u8))
            .collect()
    }

    fn encode(mtp: bool) -> (DirectExtentRecord, DirectExtentBuffer, Vec<Vec<u8>>) {
        let target_kv = bytes(TARGET_KV_EXTENT_LENGTH, 3);
        let target_indexer = bytes(TARGET_INDEXER_EXTENT_LENGTH, 5);
        let draft = mtp.then(|| bytes(DRAFT_SIDECAR_EXTENT_LENGTH, 7));
        let (record, buffer) = encode_direct_extent(
            [1; 32],
            [2; 32],
            4,
            9,
            DIRECT_IO_ALIGNMENT * 3,
            DirectPagePieces {
                target_kv: &target_kv,
                target_indexer: &target_indexer,
                draft_sidecar: draft.as_deref(),
            },
        )
        .unwrap();
        let mut source = vec![target_kv, target_indexer];
        if let Some(draft) = draft {
            source.push(draft);
        }
        (record, buffer, source)
    }

    #[test]
    fn target_and_mtp_extents_have_exact_layout_hashes_and_zero_padding() {
        for mtp in [false, true] {
            let (record, buffer, pieces) = encode(mtp);
            let expected_capability = if mtp {
                DirectTierCapability::Mtp
            } else {
                DirectTierCapability::Target
            };
            assert_eq!(record.capability, expected_capability);
            assert_eq!(record.physical_length, expected_capability.physical_bytes());
            assert_eq!(
                record.physical_length / DIRECT_IO_ALIGNMENT,
                if mtp { 501 } else { 493 }
            );
            assert_eq!(
                expected_capability.logical_bytes(),
                if mtp { 2_046_464 } else { 2_014_464 }
            );
            assert_eq!(buffer.len() as u64, record.physical_length);
            assert!(buffer.as_slice().as_ptr().addr().is_multiple_of(4_096));
            let decoded = decode_direct_extent(&record, buffer.as_slice()).unwrap();
            assert_eq!(decoded.target_kv, pieces[0]);
            assert_eq!(decoded.target_indexer, pieces[1]);
            assert_eq!(decoded.draft_sidecar, pieces.get(2).map(Vec::as_slice));
        }
    }

    #[test]
    fn alignment_and_metadata_lies_fail_closed() {
        assert_eq!(
            validate_direct_io_span(
                4_097,
                0,
                TARGET_ONLY_PHYSICAL_BYTES,
                TARGET_ONLY_PHYSICAL_BYTES
            ),
            Err(DirectExtentError::AddressAlignment)
        );
        assert_eq!(
            validate_direct_io_span(
                DIRECT_IO_ALIGNMENT,
                1,
                TARGET_ONLY_PHYSICAL_BYTES,
                TARGET_ONLY_PHYSICAL_BYTES,
            ),
            Err(DirectExtentError::OffsetAlignment)
        );
        assert_eq!(
            validate_direct_io_span(
                DIRECT_IO_ALIGNMENT,
                0,
                TARGET_ONLY_PHYSICAL_BYTES - 1,
                TARGET_ONLY_PHYSICAL_BYTES,
            ),
            Err(DirectExtentError::LengthAlignment)
        );

        let (record, buffer, _) = encode(false);
        let mut mutation = record.clone();
        mutation.physical_length += DIRECT_IO_ALIGNMENT;
        assert_eq!(mutation.validate(), Err(DirectExtentError::Physical));
        let mut mutation = record.clone();
        mutation.pieces.swap(0, 1);
        assert_eq!(mutation.validate(), Err(DirectExtentError::Pieces));
        let mut mutation = record.clone();
        mutation.pieces[1].extent_offset -= DIRECT_IO_ALIGNMENT;
        assert_eq!(mutation.validate(), Err(DirectExtentError::Pieces));
        assert!(decode_direct_extent(&record, buffer.as_slice()).is_ok());
    }

    #[test]
    fn every_padding_position_is_required_zero() {
        for mtp in [false, true] {
            let (record, mut buffer, _) = encode(mtp);
            let ranges: Vec<std::ops::Range<usize>> = if mtp {
                vec![
                    TARGET_KV_EXTENT_LENGTH as usize..TARGET_INDEXER_EXTENT_OFFSET as usize,
                    TARGET_INDEXER_END as usize..DRAFT_SIDECAR_EXTENT_OFFSET as usize,
                    DRAFT_SIDECAR_END as usize..MTP_PHYSICAL_BYTES as usize,
                ]
            } else {
                vec![
                    TARGET_KV_EXTENT_LENGTH as usize..TARGET_INDEXER_EXTENT_OFFSET as usize,
                    TARGET_INDEXER_END as usize..TARGET_ONLY_PHYSICAL_BYTES as usize,
                ]
            };
            for range in ranges {
                for index in range.clone() {
                    buffer.as_mut_slice()[index] = 1;
                    assert_eq!(
                        validate_zero_padding(buffer.as_slice(), record.capability),
                        Err(DirectExtentError::Padding)
                    );
                    buffer.as_mut_slice()[index] = 0;
                }
                buffer.as_mut_slice()[range.start] = 1;
                let mut resigned = record.clone();
                resigned.physical_sha256 = Sha256::digest(buffer.as_slice()).into();
                assert_eq!(
                    decode_direct_extent(&resigned, buffer.as_slice()),
                    Err(DirectExtentError::Padding)
                );
                buffer.as_mut_slice()[range.start] = 0;
            }
        }
    }

    #[test]
    fn physical_and_each_piece_checksum_are_independent() {
        let (record, mut buffer, _) = encode(true);
        let cases = [
            (TierPiece::TargetKv, TARGET_KV_EXTENT_OFFSET),
            (TierPiece::TargetIndexer, TARGET_INDEXER_EXTENT_OFFSET),
            (TierPiece::DraftSidecar, DRAFT_SIDECAR_EXTENT_OFFSET),
        ];
        for (piece, offset) in cases {
            let index = offset as usize;
            buffer.as_mut_slice()[index] ^= 1;
            assert_eq!(
                decode_direct_extent(&record, buffer.as_slice()),
                Err(DirectExtentError::PhysicalChecksum)
            );
            let mut resigned = record.clone();
            resigned.physical_sha256 = Sha256::digest(buffer.as_slice()).into();
            assert_eq!(
                decode_direct_extent(&resigned, buffer.as_slice()),
                Err(DirectExtentError::PieceChecksum(piece))
            );
            buffer.as_mut_slice()[index] ^= 1;
        }
    }

    #[test]
    fn blocking_store_record_requires_explicit_migration() {
        let legacy = TierRecord {
            namespace: [1; 32],
            page_key: [2; 32],
            generation: 1,
            tier: Tier::Nvme,
            mtp: false,
            pieces: vec![
                TierPieceRecord {
                    piece: TierPiece::TargetKv,
                    byte_length: TARGET_KV_EXTENT_LENGTH,
                    storage_offset: 0,
                    sha256: [3; 32],
                },
                TierPieceRecord {
                    piece: TierPiece::TargetIndexer,
                    byte_length: TARGET_INDEXER_EXTENT_LENGTH,
                    storage_offset: 1_839_104,
                    sha256: [4; 32],
                },
            ],
        };
        assert_eq!(
            DirectExtentRecord::try_from_blocking_store(&legacy),
            Err(DirectExtentError::MigrationRequired)
        );
    }
}
