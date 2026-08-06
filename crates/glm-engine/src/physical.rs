use std::{collections::BTreeSet, fmt};

use sha2::{Digest, Sha256};

use crate::{AttentionTransport, Digest32, MAX_MTP_DEPTH};

pub const GRAPH_ARENA_COUNT: usize = 10;
pub const GRAPH_CLASS_SPAN_COUNT: usize = 32;
pub const GRAPH_ARENA_RECORD_BYTES: usize = 32;
pub const GRAPH_CLASS_SPAN_RECORD_BYTES: usize = 48;
pub const GRAPH_BUFFER_USE_RECORD_BYTES: usize = 80;
pub const GRAPH_MEMORY_PLAN_RECORD_BYTES: usize = 480;
pub const DEVICE_ARENA_BINDING_RECORD_BYTES: usize = 48;

const ARENA_TABLE_DOMAIN: &[u8] = b"glmaxx.target-graph-arena-table.v1\0";
const CLASS_SPAN_TABLE_DOMAIN: &[u8] = b"glmaxx.target-graph-class-span-table.v1\0";
const BUFFER_USE_TABLE_DOMAIN: &[u8] = b"glmaxx.target-graph-buffer-use-table.v1\0";
const MEMORY_PLAN_DOMAIN: &[u8] = b"glmaxx.target-graph-memory-plan.v1\0";
const PROFILE_V3_DOMAIN: &[u8] = b"glmaxx.graph-profile.v3\0";
const DEVICE_BINDING_TABLE_DOMAIN: &[u8] = b"glmaxx.target-device-arena-binding-table.v1\0";
const MEMORY_PLAN_MAGIC: &[u8; 8] = b"G5GMPV1\0";

const ARENA_EXECUTOR_FIXED: u32 = 1;
const ARENA_PERSISTENT_MODEL_STATE: u32 = 2;
const ARENA_IMMUTABLE_MODEL_STATE: u32 = 4;
const CLASS_PRESENT: u32 = 1;
const CLASS_ALIAS_REUSE: u32 = 2;
const USE_READ: u16 = 1;
const USE_WRITE: u16 = 2;
const USE_DYNAMIC_INDEXED: u16 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u16)]
pub enum ExecutorArenaRole {
    ResidentWeight = 1,
    CodecMetadata = 2,
    TargetKv = 3,
    TargetIndexer = 4,
    RecurrentState = 5,
    PageTable = 6,
    Arguments = 7,
    Scratch = 8,
    Collectives = 9,
    Status = 11,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GraphArena {
    logical_id: u16,
    role: ExecutorArenaRole,
    flags: u32,
    bytes: u64,
    alignment: u32,
}

impl GraphArena {
    pub fn new(logical_id: u16, bytes: u64, alignment: u32) -> Result<Self, PhysicalPlanError> {
        let (role, flags) = arena_identity(logical_id)?;
        let arena = Self {
            logical_id,
            role,
            flags,
            bytes,
            alignment,
        };
        arena.validate()?;
        Ok(arena)
    }

    #[must_use]
    pub const fn logical_id(self) -> u16 {
        self.logical_id
    }

    #[must_use]
    pub const fn role(self) -> ExecutorArenaRole {
        self.role
    }

    #[must_use]
    pub const fn bytes(self) -> u64 {
        self.bytes
    }

    #[must_use]
    pub const fn alignment(self) -> u32 {
        self.alignment
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, PhysicalPlanError> {
        if bytes.len() != GRAPH_ARENA_RECORD_BYTES {
            return Err(PhysicalPlanError::RecordBytes);
        }
        if read_u32(bytes, 20) != 0 || read_u64(bytes, 24) != 0 {
            return Err(PhysicalPlanError::Reserved);
        }
        let logical_id = read_u16(bytes, 0);
        let (role, flags) = arena_identity(logical_id)?;
        if read_u16(bytes, 2) != role as u16 || read_u32(bytes, 4) != flags {
            return Err(PhysicalPlanError::ArenaRole);
        }
        let arena = Self {
            logical_id,
            role,
            flags,
            bytes: read_u64(bytes, 8),
            alignment: read_u32(bytes, 16),
        };
        arena.validate()?;
        Ok(arena)
    }

    #[must_use]
    pub fn to_bytes(self) -> [u8; GRAPH_ARENA_RECORD_BYTES] {
        let mut bytes = [0_u8; GRAPH_ARENA_RECORD_BYTES];
        bytes[0..2].copy_from_slice(&self.logical_id.to_le_bytes());
        bytes[2..4].copy_from_slice(&(self.role as u16).to_le_bytes());
        bytes[4..8].copy_from_slice(&self.flags.to_le_bytes());
        bytes[8..16].copy_from_slice(&self.bytes.to_le_bytes());
        bytes[16..20].copy_from_slice(&self.alignment.to_le_bytes());
        bytes
    }

    fn validate(self) -> Result<(), PhysicalPlanError> {
        let (expected_role, expected_flags) = arena_identity(self.logical_id)?;
        if self.role != expected_role
            || self.flags != expected_flags
            || self.bytes == 0
            || self.alignment < 256
            || !self.alignment.is_power_of_two()
            || !self.bytes.is_multiple_of(u64::from(self.alignment))
        {
            return Err(PhysicalPlanError::Arena);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GraphArenaTable {
    arenas: [GraphArena; GRAPH_ARENA_COUNT],
    digest: Digest32,
}

impl GraphArenaTable {
    pub fn new(
        arena_bytes: [u64; GRAPH_ARENA_COUNT],
        alignments: [u32; GRAPH_ARENA_COUNT],
    ) -> Result<Self, PhysicalPlanError> {
        let arenas: [GraphArena; GRAPH_ARENA_COUNT] = (0..GRAPH_ARENA_COUNT)
            .map(|index| {
                GraphArena::new(
                    u16::try_from(index + 1).map_err(|_| PhysicalPlanError::Overflow)?,
                    arena_bytes[index],
                    alignments[index],
                )
            })
            .collect::<Result<Vec<_>, _>>()?
            .try_into()
            .map_err(|_| PhysicalPlanError::ArenaCount)?;
        let digest = hash_arena_table(&arenas);
        Ok(Self { arenas, digest })
    }

    pub fn from_records(
        records: &[[u8; GRAPH_ARENA_RECORD_BYTES]],
    ) -> Result<Self, PhysicalPlanError> {
        if records.len() != GRAPH_ARENA_COUNT {
            return Err(PhysicalPlanError::ArenaCount);
        }
        let arenas: [GraphArena; GRAPH_ARENA_COUNT] = records
            .iter()
            .map(|record| GraphArena::from_bytes(record))
            .collect::<Result<Vec<_>, _>>()?
            .try_into()
            .map_err(|_| PhysicalPlanError::ArenaCount)?;
        for (index, arena) in arenas.iter().enumerate() {
            if usize::from(arena.logical_id) != index + 1 {
                return Err(PhysicalPlanError::Ordering);
            }
        }
        let digest = hash_arena_table(&arenas);
        Ok(Self { arenas, digest })
    }

    #[must_use]
    pub const fn arenas(&self) -> &[GraphArena; GRAPH_ARENA_COUNT] {
        &self.arenas
    }

    #[must_use]
    pub const fn digest(&self) -> Digest32 {
        self.digest
    }

    fn arena(&self, logical_id: u16) -> Result<GraphArena, PhysicalPlanError> {
        self.arenas
            .get(usize::from(logical_id).saturating_sub(1))
            .copied()
            .filter(|arena| arena.logical_id == logical_id)
            .ok_or(PhysicalPlanError::ArenaRole)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GraphClassSpan {
    class_id: u16,
    arena_id: u16,
    flags: u32,
    offset: u64,
    capacity_bytes: u64,
    maximum_consumed_bytes: u64,
    alignment: u32,
    phase_mask: u16,
    first_node_ordinal: u32,
    last_node_ordinal: u32,
}

impl GraphClassSpan {
    #[allow(clippy::too_many_arguments)]
    pub fn present(
        class_id: u16,
        offset: u64,
        maximum_consumed_bytes: u64,
        alignment: u32,
        phase_mask: u16,
        first_node_ordinal: u32,
        last_node_ordinal: u32,
        alias_reuse: bool,
    ) -> Result<Self, PhysicalPlanError> {
        let arena_id = class_arena(class_id)?;
        let capacity_bytes = align_up(maximum_consumed_bytes, u64::from(alignment))?;
        let span = Self {
            class_id,
            arena_id,
            flags: CLASS_PRESENT | if alias_reuse { CLASS_ALIAS_REUSE } else { 0 },
            offset,
            capacity_bytes,
            maximum_consumed_bytes,
            alignment,
            phase_mask,
            first_node_ordinal,
            last_node_ordinal,
        };
        span.validate()?;
        Ok(span)
    }

    pub fn absent(class_id: u16) -> Result<Self, PhysicalPlanError> {
        Ok(Self {
            class_id,
            arena_id: class_arena(class_id)?,
            flags: 0,
            offset: 0,
            capacity_bytes: 0,
            maximum_consumed_bytes: 0,
            alignment: 0,
            phase_mask: 0,
            first_node_ordinal: 0,
            last_node_ordinal: 0,
        })
    }

    #[must_use]
    pub const fn class_id(self) -> u16 {
        self.class_id
    }

    #[must_use]
    pub const fn arena_id(self) -> u16 {
        self.arena_id
    }

    #[must_use]
    pub const fn is_present(self) -> bool {
        self.flags & CLASS_PRESENT != 0
    }

    #[must_use]
    pub const fn offset(self) -> u64 {
        self.offset
    }

    #[must_use]
    pub const fn capacity_bytes(self) -> u64 {
        self.capacity_bytes
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, PhysicalPlanError> {
        if bytes.len() != GRAPH_CLASS_SPAN_RECORD_BYTES || read_u16(bytes, 38) != 0 {
            return Err(PhysicalPlanError::RecordBytes);
        }
        let span = Self {
            class_id: read_u16(bytes, 0),
            arena_id: read_u16(bytes, 2),
            flags: read_u32(bytes, 4),
            offset: read_u64(bytes, 8),
            capacity_bytes: read_u64(bytes, 16),
            maximum_consumed_bytes: read_u64(bytes, 24),
            alignment: read_u32(bytes, 32),
            phase_mask: read_u16(bytes, 36),
            first_node_ordinal: read_u32(bytes, 40),
            last_node_ordinal: read_u32(bytes, 44),
        };
        span.validate()?;
        Ok(span)
    }

    #[must_use]
    pub fn to_bytes(self) -> [u8; GRAPH_CLASS_SPAN_RECORD_BYTES] {
        let mut bytes = [0_u8; GRAPH_CLASS_SPAN_RECORD_BYTES];
        bytes[0..2].copy_from_slice(&self.class_id.to_le_bytes());
        bytes[2..4].copy_from_slice(&self.arena_id.to_le_bytes());
        bytes[4..8].copy_from_slice(&self.flags.to_le_bytes());
        bytes[8..16].copy_from_slice(&self.offset.to_le_bytes());
        bytes[16..24].copy_from_slice(&self.capacity_bytes.to_le_bytes());
        bytes[24..32].copy_from_slice(&self.maximum_consumed_bytes.to_le_bytes());
        bytes[32..36].copy_from_slice(&self.alignment.to_le_bytes());
        bytes[36..38].copy_from_slice(&self.phase_mask.to_le_bytes());
        bytes[40..44].copy_from_slice(&self.first_node_ordinal.to_le_bytes());
        bytes[44..48].copy_from_slice(&self.last_node_ordinal.to_le_bytes());
        bytes
    }

    fn validate(self) -> Result<(), PhysicalPlanError> {
        if self.arena_id != class_arena(self.class_id)? || self.flags & !3 != 0 {
            return Err(PhysicalPlanError::ClassSpan);
        }
        if self.flags == 0 {
            if self.offset != 0
                || self.capacity_bytes != 0
                || self.maximum_consumed_bytes != 0
                || self.alignment != 0
                || self.phase_mask != 0
                || self.first_node_ordinal != 0
                || self.last_node_ordinal != 0
            {
                return Err(PhysicalPlanError::ClassSpan);
            }
            return Ok(());
        }
        if self.flags & CLASS_PRESENT == 0
            || self.maximum_consumed_bytes == 0
            || self.alignment < 16
            || !self.alignment.is_power_of_two()
            || !self.offset.is_multiple_of(u64::from(self.alignment))
            || self.capacity_bytes
                != align_up(self.maximum_consumed_bytes, u64::from(self.alignment))?
            || self.phase_mask == 0
            || self.first_node_ordinal > self.last_node_ordinal
            || (self.class_id >= 27 && self.flags & CLASS_ALIAS_REUSE != 0)
            || ((28..=30).contains(&self.class_id) && self.last_node_ordinal != u32::MAX)
        {
            return Err(PhysicalPlanError::ClassSpan);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GraphClassSpanTable {
    spans: [GraphClassSpan; GRAPH_CLASS_SPAN_COUNT],
    digest: Digest32,
}

impl GraphClassSpanTable {
    pub fn new(
        spans: [GraphClassSpan; GRAPH_CLASS_SPAN_COUNT],
        arenas: &GraphArenaTable,
    ) -> Result<Self, PhysicalPlanError> {
        for (index, span) in spans.iter().enumerate() {
            span.validate()?;
            if usize::from(span.class_id) != index + 1 {
                return Err(PhysicalPlanError::Ordering);
            }
            if span.is_present() {
                let arena = arenas.arena(span.arena_id)?;
                checked_end(span.offset, span.capacity_bytes)
                    .filter(|&end| end <= arena.bytes)
                    .ok_or(PhysicalPlanError::Bounds)?;
            }
        }
        validate_span_overlaps(&spans)?;
        let digest = hash_class_spans(&spans);
        Ok(Self { spans, digest })
    }

    #[must_use]
    pub const fn spans(&self) -> &[GraphClassSpan; GRAPH_CLASS_SPAN_COUNT] {
        &self.spans
    }

    #[must_use]
    pub const fn digest(&self) -> Digest32 {
        self.digest
    }

    fn span(&self, class_id: u16) -> Result<GraphClassSpan, PhysicalPlanError> {
        self.spans
            .get(usize::from(class_id).saturating_sub(1))
            .copied()
            .filter(|span| span.class_id == class_id)
            .ok_or(PhysicalPlanError::ClassSpan)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GraphBufferUse {
    node_ordinal: u32,
    use_ordinal: u16,
    arena_id: u16,
    class_id: u16,
    access: u16,
    phase_mask: u16,
    flags: u16,
    relative_offset: u64,
    bytes: u64,
    alignment: u32,
    operator_plan_sha256: Digest32,
}

impl GraphBufferUse {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        node_ordinal: u32,
        use_ordinal: u16,
        arena_id: u16,
        class_id: u16,
        read: bool,
        write: bool,
        phase_mask: u16,
        dynamic_indexed: bool,
        relative_offset: u64,
        bytes: u64,
        alignment: u32,
        operator_plan_sha256: Digest32,
    ) -> Result<Self, PhysicalPlanError> {
        let use_record = Self {
            node_ordinal,
            use_ordinal,
            arena_id,
            class_id,
            access: if read { USE_READ } else { 0 } | if write { USE_WRITE } else { 0 },
            phase_mask,
            flags: if dynamic_indexed {
                USE_DYNAMIC_INDEXED
            } else {
                0
            },
            relative_offset,
            bytes,
            alignment,
            operator_plan_sha256,
        };
        use_record.validate()?;
        Ok(use_record)
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, PhysicalPlanError> {
        if bytes.len() != GRAPH_BUFFER_USE_RECORD_BYTES
            || read_u32(bytes, 36) != 0
            || read_u64(bytes, 72) != 0
        {
            return Err(PhysicalPlanError::RecordBytes);
        }
        let use_record = Self {
            node_ordinal: read_u32(bytes, 0),
            use_ordinal: read_u16(bytes, 4),
            arena_id: read_u16(bytes, 6),
            class_id: read_u16(bytes, 8),
            access: read_u16(bytes, 10),
            phase_mask: read_u16(bytes, 12),
            flags: read_u16(bytes, 14),
            relative_offset: read_u64(bytes, 16),
            bytes: read_u64(bytes, 24),
            alignment: read_u32(bytes, 32),
            operator_plan_sha256: bytes[40..72].try_into().expect("fixed record"),
        };
        use_record.validate()?;
        Ok(use_record)
    }

    #[must_use]
    pub fn to_bytes(self) -> [u8; GRAPH_BUFFER_USE_RECORD_BYTES] {
        let mut bytes = [0_u8; GRAPH_BUFFER_USE_RECORD_BYTES];
        bytes[0..4].copy_from_slice(&self.node_ordinal.to_le_bytes());
        bytes[4..6].copy_from_slice(&self.use_ordinal.to_le_bytes());
        bytes[6..8].copy_from_slice(&self.arena_id.to_le_bytes());
        bytes[8..10].copy_from_slice(&self.class_id.to_le_bytes());
        bytes[10..12].copy_from_slice(&self.access.to_le_bytes());
        bytes[12..14].copy_from_slice(&self.phase_mask.to_le_bytes());
        bytes[14..16].copy_from_slice(&self.flags.to_le_bytes());
        bytes[16..24].copy_from_slice(&self.relative_offset.to_le_bytes());
        bytes[24..32].copy_from_slice(&self.bytes.to_le_bytes());
        bytes[32..36].copy_from_slice(&self.alignment.to_le_bytes());
        bytes[40..72].copy_from_slice(&self.operator_plan_sha256);
        bytes
    }

    fn validate(self) -> Result<(), PhysicalPlanError> {
        if self.arena_id == 0
            || self.arena_id > GRAPH_ARENA_COUNT as u16
            || self.access == 0
            || self.access & !(USE_READ | USE_WRITE) != 0
            || self.phase_mask == 0
            || self.flags & !USE_DYNAMIC_INDEXED != 0
            || self.bytes == 0
            || self.alignment < 16
            || !self.alignment.is_power_of_two()
            || !self
                .relative_offset
                .is_multiple_of(u64::from(self.alignment))
        {
            return Err(PhysicalPlanError::BufferUse);
        }
        if self.class_id == 0 {
            if !(5..=10).contains(&self.arena_id) || self.flags != 0 {
                return Err(PhysicalPlanError::BufferUse);
            }
        } else if self.arena_id != class_arena(self.class_id)?
            || (self.flags & USE_DYNAMIC_INDEXED != 0 && !(28..=30).contains(&self.class_id))
        {
            return Err(PhysicalPlanError::BufferUse);
        }
        if matches!(self.arena_id, 8..=10) && self.access != USE_READ {
            return Err(PhysicalPlanError::BufferUse);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GraphBufferUseTable {
    uses: Vec<GraphBufferUse>,
    digest: Digest32,
}

impl GraphBufferUseTable {
    pub fn new(
        uses: Vec<GraphBufferUse>,
        spans: &GraphClassSpanTable,
        arenas: &GraphArenaTable,
    ) -> Result<Self, PhysicalPlanError> {
        if uses.is_empty() || uses.len() > u32::MAX as usize {
            return Err(PhysicalPlanError::UseCount);
        }
        let mut prior_node = None;
        let mut expected_use_ordinal = 0_u16;
        let mut maximum_class_end = [0_u64; GRAPH_CLASS_SPAN_COUNT];
        for use_record in &uses {
            use_record.validate()?;
            if prior_node != Some(use_record.node_ordinal) {
                if prior_node.is_some_and(|node| node >= use_record.node_ordinal) {
                    return Err(PhysicalPlanError::Ordering);
                }
                prior_node = Some(use_record.node_ordinal);
                expected_use_ordinal = 0;
            }
            if use_record.use_ordinal != expected_use_ordinal {
                return Err(PhysicalPlanError::Ordering);
            }
            expected_use_ordinal = expected_use_ordinal
                .checked_add(1)
                .ok_or(PhysicalPlanError::Overflow)?;
            let end = checked_end(use_record.relative_offset, use_record.bytes)
                .ok_or(PhysicalPlanError::Overflow)?;
            if use_record.class_id == 0 {
                if end > arenas.arena(use_record.arena_id)?.bytes {
                    return Err(PhysicalPlanError::Bounds);
                }
            } else {
                let span = spans.span(use_record.class_id)?;
                if !span.is_present() || end > span.capacity_bytes {
                    return Err(PhysicalPlanError::Bounds);
                }
                let index = usize::from(use_record.class_id - 1);
                maximum_class_end[index] = maximum_class_end[index].max(end);
            }
        }
        for span in spans.spans() {
            if span.is_present()
                && maximum_class_end[usize::from(span.class_id - 1)] != span.maximum_consumed_bytes
            {
                return Err(PhysicalPlanError::MaximumUse);
            }
        }
        let digest = hash_buffer_uses(&uses);
        Ok(Self { uses, digest })
    }

    #[must_use]
    pub fn uses(&self) -> &[GraphBufferUse] {
        &self.uses
    }

    #[must_use]
    pub const fn digest(&self) -> Digest32 {
        self.digest
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ExecutorGraphKind {
    Prefill = 1,
    Decode = 2,
    Verify = 3,
}

impl ExecutorGraphKind {
    fn decode(value: u8) -> Result<Self, PhysicalPlanError> {
        match value {
            1 => Ok(Self::Prefill),
            2 => Ok(Self::Decode),
            3 => Ok(Self::Verify),
            _ => Err(PhysicalPlanError::GraphKind),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GraphMemoryPlanRequest {
    pub graph_id: u32,
    pub graph_kind: ExecutorGraphKind,
    pub attention_transport: AttentionTransport,
    pub mtp_depth: u8,
    pub sequence_bucket: u16,
    pub row_bucket: u32,
    pub token_bucket: u32,
    pub identities: [Digest32; 8],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GraphMemoryPlan {
    request: GraphMemoryPlanRequest,
    buffer_use_count: u32,
    graph_scratch_bytes: u64,
    graph_argument_bytes: u64,
    recurrent_state_bytes: u64,
    collective_bytes: u64,
    status_bytes: u64,
    table_digests: [Digest32; 3],
    digest: Digest32,
}

impl GraphMemoryPlan {
    pub fn new(
        request: GraphMemoryPlanRequest,
        arenas: &GraphArenaTable,
        spans: &GraphClassSpanTable,
        uses: &GraphBufferUseTable,
    ) -> Result<Self, PhysicalPlanError> {
        let mut plan = Self {
            request,
            buffer_use_count: u32::try_from(uses.uses.len())
                .map_err(|_| PhysicalPlanError::UseCount)?,
            graph_scratch_bytes: arenas.arena(2)?.bytes,
            graph_argument_bytes: arenas.arena(1)?.bytes,
            recurrent_state_bytes: arenas.arena(5)?.bytes,
            collective_bytes: arenas.arena(6)?.bytes,
            status_bytes: arenas.arena(7)?.bytes,
            table_digests: [arenas.digest(), spans.digest(), uses.digest()],
            digest: [0; 32],
        };
        plan.validate_shape()?;
        plan.digest = digest(MEMORY_PLAN_DOMAIN, &plan.hash_input());
        Ok(plan)
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, PhysicalPlanError> {
        if bytes.len() != GRAPH_MEMORY_PLAN_RECORD_BYTES
            || &bytes[..8] != MEMORY_PLAN_MAGIC
            || read_u16(bytes, 8) != 1
            || usize::from(read_u16(bytes, 10)) != GRAPH_MEMORY_PLAN_RECORD_BYTES
            || read_u16(bytes, 22) != GRAPH_ARENA_COUNT as u16
            || read_u16(bytes, 24) != GRAPH_CLASS_SPAN_COUNT as u16
            || read_u16(bytes, 26) != 0
            || read_u64(bytes, 40) != 0
            || read_u64(bytes, 440) != 0
        {
            return Err(PhysicalPlanError::Header);
        }
        let flags = bytes[19];
        if flags & !1 != 0 {
            return Err(PhysicalPlanError::Reserved);
        }
        let all_identities = read_identities::<11>(bytes, 88);
        let plan = Self {
            request: GraphMemoryPlanRequest {
                graph_id: read_u32(bytes, 12),
                graph_kind: ExecutorGraphKind::decode(bytes[16])?,
                attention_transport: decode_transport(bytes[17])?,
                mtp_depth: bytes[18],
                sequence_bucket: read_u16(bytes, 20),
                row_bucket: read_u32(bytes, 28),
                token_bucket: read_u32(bytes, 32),
                identities: all_identities[..8]
                    .try_into()
                    .expect("fixed identity count"),
            },
            buffer_use_count: read_u32(bytes, 36),
            graph_scratch_bytes: read_u64(bytes, 48),
            graph_argument_bytes: read_u64(bytes, 56),
            recurrent_state_bytes: read_u64(bytes, 64),
            collective_bytes: read_u64(bytes, 72),
            status_bytes: read_u64(bytes, 80),
            table_digests: all_identities[8..]
                .try_into()
                .expect("fixed identity count"),
            digest: bytes[448..480].try_into().expect("fixed record"),
        };
        if (flags & 1 != 0) != plan.mtp_program_present() {
            return Err(PhysicalPlanError::MtpBinding);
        }
        plan.validate_shape()?;
        if digest(MEMORY_PLAN_DOMAIN, &bytes[..448]) != plan.digest {
            return Err(PhysicalPlanError::Hash);
        }
        Ok(plan)
    }

    #[must_use]
    pub fn to_bytes(self) -> [u8; GRAPH_MEMORY_PLAN_RECORD_BYTES] {
        let mut bytes = [0_u8; GRAPH_MEMORY_PLAN_RECORD_BYTES];
        bytes[..448].copy_from_slice(&self.hash_input());
        bytes[448..].copy_from_slice(&self.digest);
        bytes
    }

    #[must_use]
    pub const fn digest(self) -> Digest32 {
        self.digest
    }

    pub fn verify(
        self,
        arenas: &GraphArenaTable,
        spans: &GraphClassSpanTable,
        uses: &GraphBufferUseTable,
    ) -> Result<(), PhysicalPlanError> {
        self.validate_shape()?;
        if self.buffer_use_count != uses.uses.len() as u32
            || self.graph_scratch_bytes != arenas.arena(2)?.bytes
            || self.graph_argument_bytes != arenas.arena(1)?.bytes
            || self.recurrent_state_bytes != arenas.arena(5)?.bytes
            || self.collective_bytes != arenas.arena(6)?.bytes
            || self.status_bytes != arenas.arena(7)?.bytes
            || self.table_digests != [arenas.digest(), spans.digest(), uses.digest()]
            || digest(MEMORY_PLAN_DOMAIN, &self.hash_input()) != self.digest
        {
            return Err(PhysicalPlanError::Binding);
        }
        Ok(())
    }

    fn mtp_program_present(self) -> bool {
        self.request.identities[1] != [0; 32]
    }

    fn validate_shape(self) -> Result<(), PhysicalPlanError> {
        if self.request.graph_id == 0
            || self.request.sequence_bucket == 0
            || self.request.row_bucket == 0
            || self.buffer_use_count == 0
            || [
                self.graph_scratch_bytes,
                self.graph_argument_bytes,
                self.recurrent_state_bytes,
                self.collective_bytes,
                self.status_bytes,
            ]
            .contains(&0)
            || self.request.identities[0] == [0; 32]
            || self.request.identities[2..].contains(&[0; 32])
            || self.table_digests.contains(&[0; 32])
        {
            return Err(PhysicalPlanError::Identity);
        }
        match self.request.graph_kind {
            ExecutorGraphKind::Prefill => {
                if self.request.mtp_depth != 0
                    || self.mtp_program_present()
                    || self.request.token_bucket == 0
                    || !matches!(
                        self.request.attention_transport,
                        AttentionTransport::PrefillCkv | AttentionTransport::PrefillQuery
                    )
                {
                    return Err(PhysicalPlanError::MtpBinding);
                }
            }
            ExecutorGraphKind::Decode => {
                if self.request.mtp_depth != 0
                    || self.mtp_program_present()
                    || self.request.token_bucket != 0
                    || self.request.attention_transport != AttentionTransport::DecodeQueryLse
                {
                    return Err(PhysicalPlanError::MtpBinding);
                }
            }
            ExecutorGraphKind::Verify => {
                if !(1..=MAX_MTP_DEPTH).contains(&self.request.mtp_depth)
                    || !self.mtp_program_present()
                    || self.request.token_bucket != 0
                    || self.request.attention_transport != AttentionTransport::DecodeQueryLse
                {
                    return Err(PhysicalPlanError::MtpBinding);
                }
            }
        }
        Ok(())
    }

    fn hash_input(self) -> [u8; 448] {
        let mut bytes = [0_u8; 448];
        bytes[..8].copy_from_slice(MEMORY_PLAN_MAGIC);
        bytes[8..10].copy_from_slice(&1_u16.to_le_bytes());
        bytes[10..12].copy_from_slice(&(GRAPH_MEMORY_PLAN_RECORD_BYTES as u16).to_le_bytes());
        bytes[12..16].copy_from_slice(&self.request.graph_id.to_le_bytes());
        bytes[16] = self.request.graph_kind as u8;
        bytes[17] = self.request.attention_transport as u8;
        bytes[18] = self.request.mtp_depth;
        bytes[19] = u8::from(self.mtp_program_present());
        bytes[20..22].copy_from_slice(&self.request.sequence_bucket.to_le_bytes());
        bytes[22..24].copy_from_slice(&(GRAPH_ARENA_COUNT as u16).to_le_bytes());
        bytes[24..26].copy_from_slice(&(GRAPH_CLASS_SPAN_COUNT as u16).to_le_bytes());
        bytes[28..32].copy_from_slice(&self.request.row_bucket.to_le_bytes());
        bytes[32..36].copy_from_slice(&self.request.token_bucket.to_le_bytes());
        bytes[36..40].copy_from_slice(&self.buffer_use_count.to_le_bytes());
        bytes[48..56].copy_from_slice(&self.graph_scratch_bytes.to_le_bytes());
        bytes[56..64].copy_from_slice(&self.graph_argument_bytes.to_le_bytes());
        bytes[64..72].copy_from_slice(&self.recurrent_state_bytes.to_le_bytes());
        bytes[72..80].copy_from_slice(&self.collective_bytes.to_le_bytes());
        bytes[80..88].copy_from_slice(&self.status_bytes.to_le_bytes());
        write_identities(&mut bytes, 88, &self.request.identities);
        write_identities(&mut bytes, 344, &self.table_digests);
        bytes
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GraphProfileV3 {
    parent_profile_sha256: Digest32,
    entries: Vec<(u32, Digest32)>,
    digest: Digest32,
}

impl GraphProfileV3 {
    pub fn new(
        parent_profile_sha256: Digest32,
        mut entries: Vec<(u32, Digest32)>,
    ) -> Result<Self, PhysicalPlanError> {
        if parent_profile_sha256 == [0; 32]
            || entries.is_empty()
            || entries.len() > u32::MAX as usize
        {
            return Err(PhysicalPlanError::Identity);
        }
        entries.sort_by_key(|entry| entry.0);
        if entries
            .iter()
            .any(|(graph_id, plan)| *graph_id == 0 || *plan == [0; 32])
            || entries.windows(2).any(|pair| pair[0].0 >= pair[1].0)
        {
            return Err(PhysicalPlanError::Ordering);
        }
        let digest = hash_profile_v3(parent_profile_sha256, &entries);
        Ok(Self {
            parent_profile_sha256,
            entries,
            digest,
        })
    }

    #[must_use]
    pub const fn digest(&self) -> Digest32 {
        self.digest
    }

    pub fn admit(&self, graph_id: u32, plan_sha256: Digest32) -> Result<(), PhysicalPlanError> {
        if self
            .entries
            .binary_search_by_key(&graph_id, |entry| entry.0)
            .ok()
            .and_then(|index| self.entries.get(index))
            .is_none_or(|entry| entry.1 != plan_sha256)
        {
            return Err(PhysicalPlanError::Binding);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DeviceArenaBinding {
    logical_id: u16,
    role: ExecutorArenaRole,
    native_arena_id: u32,
    device_base_address: u64,
    native_arena_bytes: u64,
    generation: u64,
    native_alignment: u32,
    rank: u8,
}

impl DeviceArenaBinding {
    pub fn new(
        arena: GraphArena,
        native_arena_id: u32,
        device_base_address: u64,
        generation: u64,
        rank: u8,
    ) -> Result<Self, PhysicalPlanError> {
        let binding = Self {
            logical_id: arena.logical_id,
            role: arena.role,
            native_arena_id,
            device_base_address,
            native_arena_bytes: arena.bytes,
            generation,
            native_alignment: arena.alignment,
            rank,
        };
        binding.validate(arena)?;
        Ok(binding)
    }

    #[must_use]
    pub fn to_bytes(self) -> [u8; DEVICE_ARENA_BINDING_RECORD_BYTES] {
        let mut bytes = [0_u8; DEVICE_ARENA_BINDING_RECORD_BYTES];
        bytes[0..2].copy_from_slice(&self.logical_id.to_le_bytes());
        bytes[2..4].copy_from_slice(&(self.role as u16).to_le_bytes());
        bytes[4..8].copy_from_slice(&self.native_arena_id.to_le_bytes());
        bytes[8..16].copy_from_slice(&self.device_base_address.to_le_bytes());
        bytes[16..24].copy_from_slice(&self.native_arena_bytes.to_le_bytes());
        bytes[24..32].copy_from_slice(&self.generation.to_le_bytes());
        bytes[32..36].copy_from_slice(&self.native_alignment.to_le_bytes());
        bytes[36] = self.rank;
        bytes
    }

    fn validate(self, arena: GraphArena) -> Result<(), PhysicalPlanError> {
        if self.logical_id != arena.logical_id
            || self.role != arena.role
            || self.native_arena_id == 0
            || self.device_base_address == 0
            || self.native_arena_bytes != arena.bytes
            || self.generation == 0
            || self.native_alignment != arena.alignment
            || self.rank >= 4
            || !self
                .device_base_address
                .is_multiple_of(u64::from(self.native_alignment))
            || checked_end(self.device_base_address, self.native_arena_bytes).is_none()
        {
            return Err(PhysicalPlanError::DeviceBinding);
        }
        Ok(())
    }
}

pub fn device_arena_binding_digest(
    rank: u8,
    bindings: &[DeviceArenaBinding; GRAPH_ARENA_COUNT],
    arenas: &GraphArenaTable,
) -> Result<Digest32, PhysicalPlanError> {
    if rank >= 4 {
        return Err(PhysicalPlanError::Rank);
    }
    let mut native_ids = BTreeSet::new();
    for (index, binding) in bindings.iter().enumerate() {
        let arena = arenas.arenas[index];
        binding.validate(arena)?;
        if binding.rank != rank
            || usize::from(binding.logical_id) != index + 1
            || !native_ids.insert(binding.native_arena_id)
        {
            return Err(PhysicalPlanError::DeviceBinding);
        }
    }
    let mut hasher = Sha256::new();
    hasher.update(DEVICE_BINDING_TABLE_DOMAIN);
    hasher.update([rank, 0]);
    hasher.update((GRAPH_ARENA_COUNT as u16).to_le_bytes());
    hasher.update([0; 4]);
    for binding in bindings {
        hasher.update(binding.to_bytes());
    }
    Ok(hasher.finalize().into())
}

fn arena_identity(logical_id: u16) -> Result<(ExecutorArenaRole, u32), PhysicalPlanError> {
    match logical_id {
        1 => Ok((ExecutorArenaRole::Arguments, ARENA_EXECUTOR_FIXED)),
        2 => Ok((ExecutorArenaRole::Scratch, ARENA_EXECUTOR_FIXED)),
        3 => Ok((ExecutorArenaRole::TargetKv, ARENA_PERSISTENT_MODEL_STATE)),
        4 => Ok((
            ExecutorArenaRole::TargetIndexer,
            ARENA_PERSISTENT_MODEL_STATE,
        )),
        5 => Ok((
            ExecutorArenaRole::RecurrentState,
            ARENA_PERSISTENT_MODEL_STATE,
        )),
        6 => Ok((ExecutorArenaRole::Collectives, ARENA_EXECUTOR_FIXED)),
        7 => Ok((ExecutorArenaRole::Status, ARENA_EXECUTOR_FIXED)),
        8 => Ok((
            ExecutorArenaRole::ResidentWeight,
            ARENA_IMMUTABLE_MODEL_STATE,
        )),
        9 => Ok((
            ExecutorArenaRole::CodecMetadata,
            ARENA_IMMUTABLE_MODEL_STATE,
        )),
        10 => Ok((ExecutorArenaRole::PageTable, ARENA_PERSISTENT_MODEL_STATE)),
        _ => Err(PhysicalPlanError::ArenaRole),
    }
}

fn class_arena(class_id: u16) -> Result<u16, PhysicalPlanError> {
    match class_id {
        1..=26 | 31..=32 => Ok(2),
        27 => Ok(1),
        28 => Ok(3),
        29 => Ok(4),
        30 => Ok(5),
        _ => Err(PhysicalPlanError::ClassSpan),
    }
}

fn validate_span_overlaps(
    spans: &[GraphClassSpan; GRAPH_CLASS_SPAN_COUNT],
) -> Result<(), PhysicalPlanError> {
    for (index, left) in spans.iter().enumerate() {
        if !left.is_present() {
            continue;
        }
        let left_end =
            checked_end(left.offset, left.capacity_bytes).ok_or(PhysicalPlanError::Overflow)?;
        for right in &spans[index + 1..] {
            if !right.is_present() || left.arena_id != right.arena_id {
                continue;
            }
            let right_end = checked_end(right.offset, right.capacity_bytes)
                .ok_or(PhysicalPlanError::Overflow)?;
            if left.offset < right_end && right.offset < left_end {
                let declared_reuse = left.arena_id == 2
                    && left.flags & CLASS_ALIAS_REUSE != 0
                    && right.flags & CLASS_ALIAS_REUSE != 0;
                if !declared_reuse
                    || ranges_overlap(
                        left.first_node_ordinal,
                        left.last_node_ordinal,
                        right.first_node_ordinal,
                        right.last_node_ordinal,
                    )
                {
                    return Err(PhysicalPlanError::Overlap);
                }
            }
        }
    }
    Ok(())
}

fn ranges_overlap(left_start: u32, left_end: u32, right_start: u32, right_end: u32) -> bool {
    left_start <= right_end && right_start <= left_end
}

fn hash_arena_table(arenas: &[GraphArena; GRAPH_ARENA_COUNT]) -> Digest32 {
    let mut hasher = Sha256::new();
    hasher.update(ARENA_TABLE_DOMAIN);
    hasher.update((GRAPH_ARENA_COUNT as u16).to_le_bytes());
    for arena in arenas {
        hasher.update(arena.to_bytes());
    }
    hasher.finalize().into()
}

fn hash_class_spans(spans: &[GraphClassSpan; GRAPH_CLASS_SPAN_COUNT]) -> Digest32 {
    let mut hasher = Sha256::new();
    hasher.update(CLASS_SPAN_TABLE_DOMAIN);
    hasher.update((GRAPH_CLASS_SPAN_COUNT as u16).to_le_bytes());
    for span in spans {
        hasher.update(span.to_bytes());
    }
    hasher.finalize().into()
}

fn hash_buffer_uses(uses: &[GraphBufferUse]) -> Digest32 {
    let mut hasher = Sha256::new();
    hasher.update(BUFFER_USE_TABLE_DOMAIN);
    hasher.update(u32::try_from(uses.len()).unwrap_or(u32::MAX).to_le_bytes());
    for use_record in uses {
        hasher.update(use_record.to_bytes());
    }
    hasher.finalize().into()
}

fn hash_profile_v3(parent: Digest32, entries: &[(u32, Digest32)]) -> Digest32 {
    let mut hasher = Sha256::new();
    hasher.update(PROFILE_V3_DOMAIN);
    hasher.update(parent);
    hasher.update(
        u32::try_from(entries.len())
            .unwrap_or(u32::MAX)
            .to_le_bytes(),
    );
    for (graph_id, plan) in entries {
        hasher.update(graph_id.to_le_bytes());
        hasher.update(plan);
    }
    hasher.finalize().into()
}

fn decode_transport(value: u8) -> Result<AttentionTransport, PhysicalPlanError> {
    match value {
        1 => Ok(AttentionTransport::PrefillCkv),
        2 => Ok(AttentionTransport::PrefillQuery),
        3 => Ok(AttentionTransport::DecodeQueryLse),
        _ => Err(PhysicalPlanError::Transport),
    }
}

fn checked_end(offset: u64, bytes: u64) -> Option<u64> {
    offset.checked_add(bytes)
}

fn align_up(value: u64, alignment: u64) -> Result<u64, PhysicalPlanError> {
    if alignment == 0 || !alignment.is_power_of_two() {
        return Err(PhysicalPlanError::Alignment);
    }
    value
        .checked_add(alignment - 1)
        .map(|rounded| rounded & !(alignment - 1))
        .ok_or(PhysicalPlanError::Overflow)
}

fn read_u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(bytes[offset..offset + 2].try_into().expect("fixed record"))
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().expect("fixed record"))
}

fn read_u64(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(bytes[offset..offset + 8].try_into().expect("fixed record"))
}

fn read_identities<const N: usize>(bytes: &[u8], offset: usize) -> [Digest32; N] {
    std::array::from_fn(|index| {
        let start = offset + index * 32;
        bytes[start..start + 32].try_into().expect("fixed record")
    })
}

fn write_identities<const N: usize>(bytes: &mut [u8], offset: usize, identities: &[Digest32; N]) {
    for (index, identity) in identities.iter().enumerate() {
        let start = offset + index * 32;
        bytes[start..start + 32].copy_from_slice(identity);
    }
}

fn digest(domain: &[u8], bytes: &[u8]) -> Digest32 {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update(bytes);
    hasher.finalize().into()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PhysicalPlanError {
    RecordBytes,
    Header,
    Reserved,
    ArenaCount,
    ArenaRole,
    Arena,
    ClassSpan,
    BufferUse,
    UseCount,
    Ordering,
    Alignment,
    Bounds,
    Overlap,
    MaximumUse,
    GraphKind,
    Transport,
    MtpBinding,
    Identity,
    Hash,
    Binding,
    DeviceBinding,
    Rank,
    Overflow,
}

impl fmt::Display for PhysicalPlanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for PhysicalPlanError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest_value(value: u8) -> Digest32 {
        [value; 32]
    }

    fn arenas() -> GraphArenaTable {
        GraphArenaTable::new(
            [256, 4096, 4096, 4096, 4096, 4096, 256, 4096, 4096, 4096],
            [256; GRAPH_ARENA_COUNT],
        )
        .unwrap()
    }

    fn spans(arenas: &GraphArenaTable) -> GraphClassSpanTable {
        let spans = std::array::from_fn(|index| {
            let class_id = u16::try_from(index + 1).unwrap();
            if class_id == 1 {
                GraphClassSpan::present(class_id, 0, 256, 256, 1, 1, 1, false).unwrap()
            } else {
                GraphClassSpan::absent(class_id).unwrap()
            }
        });
        GraphClassSpanTable::new(spans, arenas).unwrap()
    }

    fn uses(spans: &GraphClassSpanTable, arenas: &GraphArenaTable) -> GraphBufferUseTable {
        GraphBufferUseTable::new(
            vec![
                GraphBufferUse::new(
                    1,
                    0,
                    2,
                    1,
                    true,
                    true,
                    1,
                    false,
                    0,
                    256,
                    256,
                    digest_value(9),
                )
                .unwrap(),
                GraphBufferUse::new(
                    2,
                    0,
                    8,
                    0,
                    true,
                    false,
                    1,
                    false,
                    0,
                    256,
                    256,
                    digest_value(10),
                )
                .unwrap(),
            ],
            spans,
            arenas,
        )
        .unwrap()
    }

    #[test]
    fn physical_records_and_plan_round_trip() {
        let arenas = arenas();
        for arena in arenas.arenas() {
            assert_eq!(GraphArena::from_bytes(&arena.to_bytes()), Ok(*arena));
        }
        let spans = spans(&arenas);
        for span in spans.spans() {
            assert_eq!(GraphClassSpan::from_bytes(&span.to_bytes()), Ok(*span));
        }
        let uses = uses(&spans, &arenas);
        for use_record in uses.uses() {
            assert_eq!(
                GraphBufferUse::from_bytes(&use_record.to_bytes()),
                Ok(*use_record)
            );
        }
        let request = GraphMemoryPlanRequest {
            graph_id: 7,
            graph_kind: ExecutorGraphKind::Decode,
            attention_transport: AttentionTransport::DecodeQueryLse,
            mtp_depth: 0,
            sequence_bucket: 1,
            row_bucket: 1,
            token_bucket: 0,
            identities: [
                digest_value(1),
                [0; 32],
                digest_value(3),
                digest_value(4),
                digest_value(5),
                digest_value(6),
                digest_value(7),
                digest_value(8),
            ],
        };
        let plan = GraphMemoryPlan::new(request, &arenas, &spans, &uses).unwrap();
        let bytes = plan.to_bytes();
        assert_eq!(bytes.len(), GRAPH_MEMORY_PLAN_RECORD_BYTES);
        assert_eq!(GraphMemoryPlan::from_bytes(&bytes), Ok(plan));
        assert_eq!(plan.verify(&arenas, &spans, &uses), Ok(()));
    }

    #[test]
    fn one_byte_short_arena_and_use_are_rejected() {
        let arenas = arenas();
        let spans = spans(&arenas);
        let mut arena_bytes = arenas.arenas()[1].to_bytes();
        arena_bytes[8..16].copy_from_slice(&255_u64.to_le_bytes());
        assert!(GraphArena::from_bytes(&arena_bytes).is_err());

        let use_record = GraphBufferUse::new(
            1,
            0,
            2,
            1,
            true,
            false,
            1,
            false,
            0,
            256,
            256,
            digest_value(1),
        )
        .unwrap();
        let mut short = use_record.to_bytes();
        short[24..32].copy_from_slice(&255_u64.to_le_bytes());
        let decoded = GraphBufferUse::from_bytes(&short).unwrap();
        assert_eq!(
            GraphBufferUseTable::new(vec![decoded], &spans, &arenas),
            Err(PhysicalPlanError::MaximumUse)
        );
    }

    #[test]
    fn profile_and_rank_bindings_are_exact() {
        let arenas = arenas();
        let profile = GraphProfileV3::new(
            digest_value(1),
            vec![(8, digest_value(8)), (7, digest_value(7))],
        )
        .unwrap();
        assert_eq!(profile.admit(7, digest_value(7)), Ok(()));
        assert_eq!(
            profile.admit(7, digest_value(8)),
            Err(PhysicalPlanError::Binding)
        );

        let bindings = std::array::from_fn(|index| {
            DeviceArenaBinding::new(
                arenas.arenas()[index],
                u32::try_from(index + 1).unwrap(),
                0x1_0000_0000 + u64::try_from(index).unwrap() * 0x1_0000,
                11,
                2,
            )
            .unwrap()
        });
        assert_ne!(
            device_arena_binding_digest(2, &bindings, &arenas).unwrap(),
            [0; 32]
        );
    }

    #[test]
    fn verify_requires_mtp_program_and_decode_transport() {
        let arenas = arenas();
        let spans = spans(&arenas);
        let uses = uses(&spans, &arenas);
        let mut identities = [digest_value(1); 8];
        identities[1] = [0; 32];
        let request = GraphMemoryPlanRequest {
            graph_id: 1,
            graph_kind: ExecutorGraphKind::Verify,
            attention_transport: AttentionTransport::DecodeQueryLse,
            mtp_depth: 3,
            sequence_bucket: 1,
            row_bucket: 4,
            token_bucket: 0,
            identities,
        };
        assert_eq!(
            GraphMemoryPlan::new(request, &arenas, &spans, &uses),
            Err(PhysicalPlanError::MtpBinding)
        );
    }
}
