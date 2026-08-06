use std::fmt;

use sha2::{Digest, Sha256};

use crate::AttentionTransport;

pub type Digest32 = [u8; 32];

pub const REPLAY_PROGRAM_RECORD_BYTES: usize = 480;
pub const M4_PROGRAM_RECORD_BYTES: usize = 544;
pub const BATCH_SMOKE_PROGRAM_RECORD_BYTES: usize = 672;

const REPLAY_MAGIC: &[u8; 8] = b"G5M3RP2\0";
const M4_MAGIC: &[u8; 8] = b"G5M4PR1\0";
const BATCH_SMOKE_MAGIC: &[u8; 8] = b"G5BSPV1\0";
const REPLAY_DIGEST_DOMAIN: &[u8] = b"glmaxx.m3-replay-program.v2\0";
const M4_DIGEST_DOMAIN: &[u8] = b"glmaxx.m4-program.v1\0";
const BATCH_SMOKE_DIGEST_DOMAIN: &[u8] = b"glmaxx.full-batch-smoke-program.v1\0";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ProgramMode {
    Decode = 1,
    Prefill = 2,
}

impl ProgramMode {
    fn decode(value: u8) -> Result<Self, ProgramRecordError> {
        match value {
            1 => Ok(Self::Decode),
            2 => Ok(Self::Prefill),
            _ => Err(ProgramRecordError::Mode),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ExecutionPath {
    Eager = 1,
    Captured = 2,
}

impl ExecutionPath {
    fn decode(value: u8) -> Result<Self, ProgramRecordError> {
        match value {
            1 => Ok(Self::Eager),
            2 => Ok(Self::Captured),
            _ => Err(ProgramRecordError::ExecutionPath),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ReplayProfile {
    CapacityExl3 = 1,
    HybridServe = 2,
    Nvfp4Laboratory = 3,
}

impl ReplayProfile {
    fn decode(value: u8) -> Result<Self, ProgramRecordError> {
        match value {
            1 => Ok(Self::CapacityExl3),
            2 => Ok(Self::HybridServe),
            3 => Ok(Self::Nvfp4Laboratory),
            _ => Err(ProgramRecordError::Profile),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ProductionProfile {
    CapacityExl3 = 1,
    HybridServe = 2,
}

impl ProductionProfile {
    fn decode(value: u8) -> Result<Self, ProgramRecordError> {
        match value {
            1 => Ok(Self::CapacityExl3),
            2 => Ok(Self::HybridServe),
            _ => Err(ProgramRecordError::Profile),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReplayProgram {
    mode: ProgramMode,
    real_query_rows: u32,
    graph_row_bucket: u32,
    sequence_bucket: u32,
    attention_transport: AttentionTransport,
    execution_path: ExecutionPath,
    profile: ReplayProfile,
    identities: [Digest32; 14],
}

impl ReplayProgram {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        mode: ProgramMode,
        real_query_rows: u32,
        graph_row_bucket: u32,
        sequence_bucket: u32,
        attention_transport: AttentionTransport,
        execution_path: ExecutionPath,
        profile: ReplayProfile,
        identities: [Digest32; 14],
    ) -> Result<Self, ProgramRecordError> {
        let program = Self {
            mode,
            real_query_rows,
            graph_row_bucket,
            sequence_bucket,
            attention_transport,
            execution_path,
            profile,
            identities,
        };
        program.validate()?;
        Ok(program)
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, ProgramRecordError> {
        if bytes.len() != REPLAY_PROGRAM_RECORD_BYTES {
            return Err(ProgramRecordError::RecordBytes);
        }
        if &bytes[..8] != REPLAY_MAGIC
            || read_u16(bytes, 8) != 2
            || usize::from(read_u16(bytes, 10)) != REPLAY_PROGRAM_RECORD_BYTES
        {
            return Err(ProgramRecordError::Header);
        }
        if bytes[13] != 6 || bytes[14] != 7 || bytes[15] != 4 || bytes[31] != 0 {
            return Err(ProgramRecordError::FixedField);
        }
        let program = Self {
            mode: ProgramMode::decode(bytes[12])?,
            real_query_rows: read_u32(bytes, 16),
            graph_row_bucket: read_u32(bytes, 20),
            sequence_bucket: read_u32(bytes, 24),
            attention_transport: decode_transport(bytes[28])?,
            execution_path: ExecutionPath::decode(bytes[29])?,
            profile: ReplayProfile::decode(bytes[30])?,
            identities: read_identities::<14>(bytes, 32),
        };
        program.validate()?;
        Ok(program)
    }

    #[must_use]
    pub fn to_bytes(self) -> [u8; REPLAY_PROGRAM_RECORD_BYTES] {
        let mut bytes = [0_u8; REPLAY_PROGRAM_RECORD_BYTES];
        bytes[..8].copy_from_slice(REPLAY_MAGIC);
        bytes[8..10].copy_from_slice(&2_u16.to_le_bytes());
        bytes[10..12].copy_from_slice(&(REPLAY_PROGRAM_RECORD_BYTES as u16).to_le_bytes());
        bytes[12] = self.mode as u8;
        bytes[13] = 6;
        bytes[14] = 7;
        bytes[15] = 4;
        bytes[16..20].copy_from_slice(&self.real_query_rows.to_le_bytes());
        bytes[20..24].copy_from_slice(&self.graph_row_bucket.to_le_bytes());
        bytes[24..28].copy_from_slice(&self.sequence_bucket.to_le_bytes());
        bytes[28] = self.attention_transport as u8;
        bytes[29] = self.execution_path as u8;
        bytes[30] = self.profile as u8;
        write_identities(&mut bytes, 32, &self.identities);
        bytes
    }

    #[must_use]
    pub fn digest(self) -> Digest32 {
        digest(REPLAY_DIGEST_DOMAIN, &self.to_bytes())
    }

    pub fn validate(self) -> Result<(), ProgramRecordError> {
        validate_shape(
            self.mode,
            self.real_query_rows,
            self.graph_row_bucket,
            self.sequence_bucket,
            self.attention_transport,
        )?;
        validate_identities(&self.identities)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct M4Program {
    mode: ProgramMode,
    real_query_rows: u32,
    graph_row_bucket: u32,
    attention_transport: AttentionTransport,
    execution_path: ExecutionPath,
    identities: [Digest32; 16],
}

impl M4Program {
    pub fn new(
        mode: ProgramMode,
        real_query_rows: u32,
        graph_row_bucket: u32,
        attention_transport: AttentionTransport,
        execution_path: ExecutionPath,
        identities: [Digest32; 16],
    ) -> Result<Self, ProgramRecordError> {
        let program = Self {
            mode,
            real_query_rows,
            graph_row_bucket,
            attention_transport,
            execution_path,
            identities,
        };
        program.validate()?;
        Ok(program)
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, ProgramRecordError> {
        if bytes.len() != M4_PROGRAM_RECORD_BYTES {
            return Err(ProgramRecordError::RecordBytes);
        }
        if &bytes[..8] != M4_MAGIC
            || read_u16(bytes, 8) != 1
            || usize::from(read_u16(bytes, 10)) != M4_PROGRAM_RECORD_BYTES
        {
            return Err(ProgramRecordError::Header);
        }
        if bytes[13] != 4
            || bytes[14] != ReplayProfile::Nvfp4Laboratory as u8
            || bytes[15] != 0
            || read_u32(bytes, 24) != 1
            || read_u16(bytes, 30) != 0
        {
            return Err(ProgramRecordError::FixedField);
        }
        let program = Self {
            mode: ProgramMode::decode(bytes[12])?,
            real_query_rows: read_u32(bytes, 16),
            graph_row_bucket: read_u32(bytes, 20),
            attention_transport: decode_transport(bytes[28])?,
            execution_path: ExecutionPath::decode(bytes[29])?,
            identities: read_identities::<16>(bytes, 32),
        };
        program.validate()?;
        Ok(program)
    }

    #[must_use]
    pub fn to_bytes(self) -> [u8; M4_PROGRAM_RECORD_BYTES] {
        let mut bytes = [0_u8; M4_PROGRAM_RECORD_BYTES];
        bytes[..8].copy_from_slice(M4_MAGIC);
        bytes[8..10].copy_from_slice(&1_u16.to_le_bytes());
        bytes[10..12].copy_from_slice(&(M4_PROGRAM_RECORD_BYTES as u16).to_le_bytes());
        bytes[12] = self.mode as u8;
        bytes[13] = 4;
        bytes[14] = ReplayProfile::Nvfp4Laboratory as u8;
        bytes[16..20].copy_from_slice(&self.real_query_rows.to_le_bytes());
        bytes[20..24].copy_from_slice(&self.graph_row_bucket.to_le_bytes());
        bytes[24..28].copy_from_slice(&1_u32.to_le_bytes());
        bytes[28] = self.attention_transport as u8;
        bytes[29] = self.execution_path as u8;
        write_identities(&mut bytes, 32, &self.identities);
        bytes
    }

    #[must_use]
    pub fn digest(self) -> Digest32 {
        digest(M4_DIGEST_DOMAIN, &self.to_bytes())
    }

    pub fn validate(self) -> Result<(), ProgramRecordError> {
        validate_shape(
            self.mode,
            self.real_query_rows,
            self.graph_row_bucket,
            1,
            self.attention_transport,
        )?;
        if self.mode == ProgramMode::Decode && self.real_query_rows != 1 {
            return Err(ProgramRecordError::Shape);
        }
        validate_identities(&self.identities)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BatchSmokeProgram {
    profile: ProductionProfile,
    prefill_row_bucket: u32,
    identities: [Digest32; 20],
}

impl BatchSmokeProgram {
    pub fn new(
        profile: ProductionProfile,
        prefill_row_bucket: u32,
        identities: [Digest32; 20],
    ) -> Result<Self, ProgramRecordError> {
        let program = Self {
            profile,
            prefill_row_bucket,
            identities,
        };
        program.validate()?;
        Ok(program)
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, ProgramRecordError> {
        if bytes.len() != BATCH_SMOKE_PROGRAM_RECORD_BYTES {
            return Err(ProgramRecordError::RecordBytes);
        }
        if &bytes[..8] != BATCH_SMOKE_MAGIC
            || read_u16(bytes, 8) != 1
            || usize::from(read_u16(bytes, 10)) != BATCH_SMOKE_PROGRAM_RECORD_BYTES
        {
            return Err(ProgramRecordError::Header);
        }
        if bytes[13] != 4
            || bytes[14] != 0
            || bytes[15] != 1
            || read_u16(bytes, 16) != 4
            || read_u16(bytes, 18) != 16
            || read_u32(bytes, 24) != 4
            || read_u16(bytes, 28) != 4
            || read_u16(bytes, 30) != 0
        {
            return Err(ProgramRecordError::FixedField);
        }
        let program = Self {
            profile: ProductionProfile::decode(bytes[12])?,
            prefill_row_bucket: read_u32(bytes, 20),
            identities: read_identities::<20>(bytes, 32),
        };
        program.validate()?;
        Ok(program)
    }

    #[must_use]
    pub fn to_bytes(self) -> [u8; BATCH_SMOKE_PROGRAM_RECORD_BYTES] {
        let mut bytes = [0_u8; BATCH_SMOKE_PROGRAM_RECORD_BYTES];
        bytes[..8].copy_from_slice(BATCH_SMOKE_MAGIC);
        bytes[8..10].copy_from_slice(&1_u16.to_le_bytes());
        bytes[10..12].copy_from_slice(&(BATCH_SMOKE_PROGRAM_RECORD_BYTES as u16).to_le_bytes());
        bytes[12] = self.profile as u8;
        bytes[13] = 4;
        bytes[15] = 1;
        bytes[16..18].copy_from_slice(&4_u16.to_le_bytes());
        bytes[18..20].copy_from_slice(&16_u16.to_le_bytes());
        bytes[20..24].copy_from_slice(&self.prefill_row_bucket.to_le_bytes());
        bytes[24..28].copy_from_slice(&4_u32.to_le_bytes());
        bytes[28..30].copy_from_slice(&4_u16.to_le_bytes());
        write_identities(&mut bytes, 32, &self.identities);
        bytes
    }

    #[must_use]
    pub fn digest(self) -> Digest32 {
        digest(BATCH_SMOKE_DIGEST_DOMAIN, &self.to_bytes())
    }

    #[must_use]
    pub const fn profile(self) -> ProductionProfile {
        self.profile
    }

    #[must_use]
    pub const fn prefill_row_bucket(self) -> u32 {
        self.prefill_row_bucket
    }

    #[must_use]
    pub const fn operation_manifest_sha256(self) -> Digest32 {
        self.identities[0]
    }

    #[must_use]
    pub const fn checkpoint_sha256(self) -> Digest32 {
        self.identities[1]
    }

    #[must_use]
    pub const fn rank_set_load_plan_sha256(self) -> Digest32 {
        self.identities[2]
    }

    #[must_use]
    pub const fn resident_weight_generation_sha256(self) -> Digest32 {
        self.identities[3]
    }

    #[must_use]
    pub const fn target_program_sha256(self) -> Digest32 {
        self.identities[4]
    }

    #[must_use]
    pub const fn graph_profile_v3_sha256(self) -> Digest32 {
        self.identities[5]
    }

    #[must_use]
    pub const fn graph_memory_plan_set_sha256(self) -> Digest32 {
        self.identities[6]
    }

    #[must_use]
    pub const fn executor_program_set_sha256(self) -> Digest32 {
        self.identities[7]
    }

    #[must_use]
    pub const fn module_set_capability_sha256(self) -> Digest32 {
        self.identities[8]
    }

    #[must_use]
    pub const fn rank_set_resource_budget_sha256(self) -> Digest32 {
        self.identities[9]
    }

    #[must_use]
    pub const fn system_memory_plan_sha256(self) -> Digest32 {
        self.identities[10]
    }

    #[must_use]
    pub const fn collective_schedule_set_sha256(self) -> Digest32 {
        self.identities[11]
    }

    pub fn validate(self) -> Result<(), ProgramRecordError> {
        if self.prefill_row_bucket == 0 || self.prefill_row_bucket < 4 {
            return Err(ProgramRecordError::Shape);
        }
        validate_identities(&self.identities)
    }
}

fn validate_shape(
    mode: ProgramMode,
    real_query_rows: u32,
    graph_row_bucket: u32,
    sequence_bucket: u32,
    transport: AttentionTransport,
) -> Result<(), ProgramRecordError> {
    if real_query_rows == 0
        || graph_row_bucket < real_query_rows
        || sequence_bucket == 0
        || real_query_rows < sequence_bucket
    {
        return Err(ProgramRecordError::Shape);
    }
    let transport_matches = match mode {
        ProgramMode::Decode => transport == AttentionTransport::DecodeQueryLse,
        ProgramMode::Prefill => matches!(
            transport,
            AttentionTransport::PrefillCkv | AttentionTransport::PrefillQuery
        ),
    };
    if !transport_matches {
        return Err(ProgramRecordError::Transport);
    }
    Ok(())
}

fn validate_identities<const N: usize>(
    identities: &[Digest32; N],
) -> Result<(), ProgramRecordError> {
    if identities.contains(&[0; 32]) {
        return Err(ProgramRecordError::Identity);
    }
    Ok(())
}

fn decode_transport(value: u8) -> Result<AttentionTransport, ProgramRecordError> {
    match value {
        1 => Ok(AttentionTransport::PrefillCkv),
        2 => Ok(AttentionTransport::PrefillQuery),
        3 => Ok(AttentionTransport::DecodeQueryLse),
        _ => Err(ProgramRecordError::Transport),
    }
}

fn read_u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(bytes[offset..offset + 2].try_into().expect("fixed record"))
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().expect("fixed record"))
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
pub enum ProgramRecordError {
    RecordBytes,
    Header,
    FixedField,
    Mode,
    ExecutionPath,
    Profile,
    Transport,
    Shape,
    Identity,
}

impl fmt::Display for ProgramRecordError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for ProgramRecordError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn identities<const N: usize>() -> [Digest32; N] {
        std::array::from_fn(|index| [u8::try_from(index + 1).unwrap(); 32])
    }

    #[test]
    fn replay_program_round_trips_exact_record() {
        let program = ReplayProgram::new(
            ProgramMode::Decode,
            4,
            4,
            4,
            AttentionTransport::DecodeQueryLse,
            ExecutionPath::Captured,
            ReplayProfile::CapacityExl3,
            identities(),
        )
        .unwrap();
        let bytes = program.to_bytes();
        assert_eq!(bytes.len(), REPLAY_PROGRAM_RECORD_BYTES);
        assert_eq!(ReplayProgram::from_bytes(&bytes), Ok(program));
        assert_ne!(program.digest(), [0; 32]);
    }

    #[test]
    fn m4_program_round_trips_both_shapes() {
        for program in [
            M4Program::new(
                ProgramMode::Decode,
                1,
                1,
                AttentionTransport::DecodeQueryLse,
                ExecutionPath::Eager,
                identities(),
            )
            .unwrap(),
            M4Program::new(
                ProgramMode::Prefill,
                64,
                64,
                AttentionTransport::PrefillQuery,
                ExecutionPath::Captured,
                identities(),
            )
            .unwrap(),
        ] {
            let bytes = program.to_bytes();
            assert_eq!(bytes.len(), M4_PROGRAM_RECORD_BYTES);
            assert_eq!(M4Program::from_bytes(&bytes), Ok(program));
        }
    }

    #[test]
    fn batch_smoke_program_round_trips_both_production_profiles() {
        for profile in [
            ProductionProfile::CapacityExl3,
            ProductionProfile::HybridServe,
        ] {
            let program = BatchSmokeProgram::new(profile, 256, identities()).unwrap();
            let bytes = program.to_bytes();
            assert_eq!(bytes.len(), BATCH_SMOKE_PROGRAM_RECORD_BYTES);
            assert_eq!(BatchSmokeProgram::from_bytes(&bytes), Ok(program));
        }
    }

    #[test]
    fn reserved_and_fixed_fields_fail_closed() {
        let replay = ReplayProgram::new(
            ProgramMode::Decode,
            1,
            1,
            1,
            AttentionTransport::DecodeQueryLse,
            ExecutionPath::Captured,
            ReplayProfile::HybridServe,
            identities(),
        )
        .unwrap();
        let mut replay_bytes = replay.to_bytes();
        replay_bytes[31] = 1;
        assert_eq!(
            ReplayProgram::from_bytes(&replay_bytes),
            Err(ProgramRecordError::FixedField)
        );

        let batch =
            BatchSmokeProgram::new(ProductionProfile::CapacityExl3, 4, identities()).unwrap();
        let mut batch_bytes = batch.to_bytes();
        batch_bytes[30] = 1;
        assert_eq!(
            BatchSmokeProgram::from_bytes(&batch_bytes),
            Err(ProgramRecordError::FixedField)
        );
    }

    #[test]
    fn transport_shape_and_zero_identity_fail_closed() {
        assert_eq!(
            ReplayProgram::new(
                ProgramMode::Decode,
                1,
                1,
                1,
                AttentionTransport::PrefillQuery,
                ExecutionPath::Captured,
                ReplayProfile::CapacityExl3,
                identities(),
            ),
            Err(ProgramRecordError::Transport)
        );
        assert_eq!(
            M4Program::new(
                ProgramMode::Decode,
                2,
                2,
                AttentionTransport::DecodeQueryLse,
                ExecutionPath::Captured,
                identities(),
            ),
            Err(ProgramRecordError::Shape)
        );
        let mut zero_identity = identities::<20>();
        zero_identity[7] = [0; 32];
        assert_eq!(
            BatchSmokeProgram::new(ProductionProfile::HybridServe, 4, zero_identity),
            Err(ProgramRecordError::Identity)
        );
    }
}
