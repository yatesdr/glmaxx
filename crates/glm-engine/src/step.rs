use std::fmt;

use serde::Serialize;
use sha2::{Digest, Sha256};

pub const STEP_PLAN_ABI: &str = "glmaxx.step-plan.v1";
pub const STEP_PLAN_HASH_INPUT_BYTES: usize = 85;
pub const STEP_PLAN_RECORD_BYTES: usize = STEP_PLAN_HASH_INPUT_BYTES + 32;
pub const MAX_ACTIVE_SEQUENCES: u16 = 64;
pub const MAX_VERIFIER_ROWS: u32 = 448;
pub const MAX_MTP_DEPTH: u8 = 6;
pub const TP_RANK_MASK: u8 = 0x0f;

const PLAN_HASH_DOMAIN: &[u8] = b"glmaxx.step-plan.v1\0";
const COLLECTIVE_HASH_DOMAIN: &[u8] = b"glmaxx.collective-schedule.v1\0";

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[repr(u8)]
#[serde(rename_all = "kebab-case")]
pub enum StepMode {
    Prefill = 1,
    Decode = 2,
    Verify = 3,
    Mixed = 4,
    CacheOnly = 5,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[repr(u8)]
#[serde(rename_all = "kebab-case")]
pub enum AttentionTransport {
    None = 0,
    PrefillCkv = 1,
    PrefillQuery = 2,
    DecodeQueryLse = 3,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[repr(u8)]
#[serde(rename_all = "kebab-case")]
pub enum CollectiveKind {
    TpReduce = 1,
    DcpPackedCkv = 2,
    DcpQueryGather = 3,
    DcpCandidateExchange = 4,
    DcpPartialStateReturn = 5,
    LogitsArgmax = 6,
    LogitsTopK = 7,
    LogitsMass = 8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct CollectiveOp {
    pub ordinal: u16,
    pub kind: CollectiveKind,
    pub route_id: u16,
    pub payload_bytes: u32,
    pub participant_mask: u8,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CollectiveSchedule {
    operations: Vec<CollectiveOp>,
    hash: [u8; 32],
}

impl CollectiveSchedule {
    pub fn new(operations: Vec<CollectiveOp>) -> Result<Self, PlanError> {
        if operations.len() > usize::from(u16::MAX) {
            return Err(PlanError::CollectiveCount);
        }
        for (expected, operation) in operations.iter().enumerate() {
            if usize::from(operation.ordinal) != expected {
                return Err(PlanError::CollectiveOrdinal);
            }
            if operation.route_id == 0
                || operation.payload_bytes == 0
                || operation.participant_mask == 0
                || operation.participant_mask & !TP_RANK_MASK != 0
            {
                return Err(PlanError::CollectiveOperation);
            }
        }
        let hash = collective_hash(&operations);
        Ok(Self { operations, hash })
    }

    #[must_use]
    pub fn empty() -> Self {
        Self {
            operations: Vec::new(),
            hash: collective_hash(&[]),
        }
    }

    #[must_use]
    pub fn operations(&self) -> &[CollectiveOp] {
        &self.operations
    }

    #[must_use]
    pub const fn hash(&self) -> [u8; 32] {
        self.hash
    }

    pub fn verify(&self) -> Result<(), PlanError> {
        if collective_hash(&self.operations) != self.hash {
            return Err(PlanError::CollectiveHash);
        }
        Self::new(self.operations.clone()).map(|_| ())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StepPlanRequest {
    pub epoch: u64,
    pub step_id: u64,
    pub mode: StepMode,
    pub active_sequences: u16,
    pub sequence_bucket: u16,
    pub scheduled_prompt_tokens: u32,
    pub query_rows: u32,
    pub verifier_row_bucket: u32,
    pub mtp_depth: u8,
    pub graph_id: u32,
    pub tp_route_id: u16,
    pub dcp_route_id: u16,
    pub attention_transport: AttentionTransport,
    pub sampling_route_id: u16,
    pub sequence_table_generation: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct StepPlan {
    pub epoch: u64,
    pub step_id: u64,
    pub mode: StepMode,
    pub active_sequences: u16,
    pub sequence_bucket: u16,
    pub scheduled_prompt_tokens: u32,
    pub query_rows: u32,
    pub verifier_row_bucket: u32,
    pub mtp_depth: u8,
    pub graph_id: u32,
    pub tp_route_id: u16,
    pub dcp_route_id: u16,
    pub attention_transport: AttentionTransport,
    pub sampling_route_id: u16,
    pub sequence_table_generation: u64,
    pub collective_schedule_hash: [u8; 32],
    pub plan_hash: [u8; 32],
}

impl StepPlan {
    pub fn build(
        request: StepPlanRequest,
        schedule: &CollectiveSchedule,
    ) -> Result<Self, PlanError> {
        schedule.verify()?;
        validate_request(request, schedule)?;
        let mut plan = Self {
            epoch: request.epoch,
            step_id: request.step_id,
            mode: request.mode,
            active_sequences: request.active_sequences,
            sequence_bucket: request.sequence_bucket,
            scheduled_prompt_tokens: request.scheduled_prompt_tokens,
            query_rows: request.query_rows,
            verifier_row_bucket: request.verifier_row_bucket,
            mtp_depth: request.mtp_depth,
            graph_id: request.graph_id,
            tp_route_id: request.tp_route_id,
            dcp_route_id: request.dcp_route_id,
            attention_transport: request.attention_transport,
            sampling_route_id: request.sampling_route_id,
            sequence_table_generation: request.sequence_table_generation,
            collective_schedule_hash: schedule.hash(),
            plan_hash: [0; 32],
        };
        plan.plan_hash = plan.compute_hash();
        Ok(plan)
    }

    #[must_use]
    pub fn canonical_hash_input(&self) -> [u8; STEP_PLAN_HASH_INPUT_BYTES] {
        let mut output = [0_u8; STEP_PLAN_HASH_INPUT_BYTES];
        let mut cursor = 0;
        append(&mut output, &mut cursor, &self.epoch.to_le_bytes());
        append(&mut output, &mut cursor, &self.step_id.to_le_bytes());
        append(&mut output, &mut cursor, &[self.mode as u8]);
        append(
            &mut output,
            &mut cursor,
            &self.active_sequences.to_le_bytes(),
        );
        append(
            &mut output,
            &mut cursor,
            &self.sequence_bucket.to_le_bytes(),
        );
        append(
            &mut output,
            &mut cursor,
            &self.scheduled_prompt_tokens.to_le_bytes(),
        );
        append(&mut output, &mut cursor, &self.query_rows.to_le_bytes());
        append(
            &mut output,
            &mut cursor,
            &self.verifier_row_bucket.to_le_bytes(),
        );
        append(&mut output, &mut cursor, &[self.mtp_depth]);
        append(&mut output, &mut cursor, &self.graph_id.to_le_bytes());
        append(&mut output, &mut cursor, &self.tp_route_id.to_le_bytes());
        append(&mut output, &mut cursor, &self.dcp_route_id.to_le_bytes());
        append(&mut output, &mut cursor, &[self.attention_transport as u8]);
        append(
            &mut output,
            &mut cursor,
            &self.sampling_route_id.to_le_bytes(),
        );
        append(
            &mut output,
            &mut cursor,
            &self.sequence_table_generation.to_le_bytes(),
        );
        append(&mut output, &mut cursor, &self.collective_schedule_hash);
        debug_assert_eq!(cursor, STEP_PLAN_HASH_INPUT_BYTES);
        output
    }

    #[must_use]
    pub fn canonical_record(&self) -> [u8; STEP_PLAN_RECORD_BYTES] {
        let mut output = [0_u8; STEP_PLAN_RECORD_BYTES];
        output[..STEP_PLAN_HASH_INPUT_BYTES].copy_from_slice(&self.canonical_hash_input());
        output[STEP_PLAN_HASH_INPUT_BYTES..].copy_from_slice(&self.plan_hash);
        output
    }

    pub fn verify(&self, schedule: &CollectiveSchedule) -> Result<(), PlanError> {
        schedule.verify()?;
        if schedule.hash() != self.collective_schedule_hash {
            return Err(PlanError::CollectiveHash);
        }
        if self.compute_hash() != self.plan_hash {
            return Err(PlanError::PlanHash);
        }
        validate_request(self.request(), schedule)
    }

    #[must_use]
    pub const fn request(&self) -> StepPlanRequest {
        StepPlanRequest {
            epoch: self.epoch,
            step_id: self.step_id,
            mode: self.mode,
            active_sequences: self.active_sequences,
            sequence_bucket: self.sequence_bucket,
            scheduled_prompt_tokens: self.scheduled_prompt_tokens,
            query_rows: self.query_rows,
            verifier_row_bucket: self.verifier_row_bucket,
            mtp_depth: self.mtp_depth,
            graph_id: self.graph_id,
            tp_route_id: self.tp_route_id,
            dcp_route_id: self.dcp_route_id,
            attention_transport: self.attention_transport,
            sampling_route_id: self.sampling_route_id,
            sequence_table_generation: self.sequence_table_generation,
        }
    }

    fn compute_hash(&self) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(PLAN_HASH_DOMAIN);
        hasher.update(self.canonical_hash_input());
        hasher.finalize().into()
    }
}

fn validate_request(
    request: StepPlanRequest,
    schedule: &CollectiveSchedule,
) -> Result<(), PlanError> {
    if request.mtp_depth > MAX_MTP_DEPTH {
        return Err(PlanError::MtpDepth);
    }
    match request.mode {
        StepMode::CacheOnly => {
            if request.active_sequences != 0
                || request.sequence_bucket != 0
                || request.scheduled_prompt_tokens != 0
                || request.query_rows != 0
                || request.verifier_row_bucket != 0
                || request.mtp_depth != 0
                || request.graph_id != 0
                || request.tp_route_id != 0
                || request.dcp_route_id != 0
                || request.attention_transport != AttentionTransport::None
                || request.sampling_route_id != 0
                || request.sequence_table_generation != 0
                || !schedule.operations().is_empty()
            {
                return Err(PlanError::NonCanonicalUnused);
            }
            return Ok(());
        }
        StepMode::Mixed => return Err(PlanError::MixedContractUnreviewed),
        StepMode::Prefill | StepMode::Decode | StepMode::Verify => {}
    }

    if request.active_sequences == 0
        || request.active_sequences > MAX_ACTIVE_SEQUENCES
        || !is_sequence_bucket(request.sequence_bucket)
        || request.sequence_bucket < request.active_sequences
        || request.query_rows == 0
        || request.query_rows > MAX_VERIFIER_ROWS
        || request.graph_id == 0
        || request.tp_route_id == 0
        || request.dcp_route_id == 0
        || request.sequence_table_generation == 0
        || schedule.operations().is_empty()
    {
        return Err(PlanError::Shape);
    }

    match request.mode {
        StepMode::Prefill => {
            if request.scheduled_prompt_tokens == 0
                || request.query_rows != request.scheduled_prompt_tokens
                || request.verifier_row_bucket != 0
                || request.mtp_depth != 0
                || !matches!(
                    request.attention_transport,
                    AttentionTransport::PrefillCkv | AttentionTransport::PrefillQuery
                )
                || request.sampling_route_id != 0
            {
                return Err(PlanError::Mode);
            }
        }
        StepMode::Decode => {
            if request.scheduled_prompt_tokens != 0
                || request.query_rows != u32::from(request.active_sequences)
                || request.verifier_row_bucket < request.query_rows
                || request.verifier_row_bucket > MAX_VERIFIER_ROWS
                || request.mtp_depth != 0
                || request.attention_transport != AttentionTransport::DecodeQueryLse
                || request.sampling_route_id == 0
            {
                return Err(PlanError::Mode);
            }
        }
        StepMode::Verify => {
            let expected_rows = u32::from(request.active_sequences)
                .checked_mul(u32::from(request.mtp_depth) + 1)
                .ok_or(PlanError::Shape)?;
            if request.scheduled_prompt_tokens != 0
                || request.mtp_depth == 0
                || request.query_rows != expected_rows
                || request.verifier_row_bucket < request.query_rows
                || request.verifier_row_bucket > MAX_VERIFIER_ROWS
                || request.attention_transport != AttentionTransport::DecodeQueryLse
                || request.sampling_route_id == 0
            {
                return Err(PlanError::Mode);
            }
        }
        StepMode::Mixed | StepMode::CacheOnly => unreachable!(),
    }
    Ok(())
}

const fn is_sequence_bucket(value: u16) -> bool {
    matches!(value, 1 | 2 | 4 | 8 | 16 | 32 | 64)
}

fn collective_hash(operations: &[CollectiveOp]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(COLLECTIVE_HASH_DOMAIN);
    hasher.update(
        u16::try_from(operations.len())
            .unwrap_or(u16::MAX)
            .to_le_bytes(),
    );
    for operation in operations {
        hasher.update(operation.ordinal.to_le_bytes());
        hasher.update([operation.kind as u8]);
        hasher.update(operation.route_id.to_le_bytes());
        hasher.update(operation.payload_bytes.to_le_bytes());
        hasher.update([operation.participant_mask]);
    }
    hasher.finalize().into()
}

fn append<const N: usize>(output: &mut [u8; N], cursor: &mut usize, bytes: &[u8]) {
    let end = *cursor + bytes.len();
    output[*cursor..end].copy_from_slice(bytes);
    *cursor = end;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlanError {
    CollectiveCount,
    CollectiveOrdinal,
    CollectiveOperation,
    CollectiveHash,
    PlanHash,
    MtpDepth,
    Shape,
    Mode,
    NonCanonicalUnused,
    MixedContractUnreviewed,
}

impl fmt::Display for PlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for PlanError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn schedule() -> CollectiveSchedule {
        CollectiveSchedule::new(vec![
            CollectiveOp {
                ordinal: 0,
                kind: CollectiveKind::DcpQueryGather,
                route_id: 3,
                payload_bytes: 32_768,
                participant_mask: TP_RANK_MASK,
            },
            CollectiveOp {
                ordinal: 1,
                kind: CollectiveKind::DcpPartialStateReturn,
                route_id: 4,
                payload_bytes: 98_304,
                participant_mask: TP_RANK_MASK,
            },
            CollectiveOp {
                ordinal: 2,
                kind: CollectiveKind::TpReduce,
                route_id: 9,
                payload_bytes: 98_304,
                participant_mask: TP_RANK_MASK,
            },
            CollectiveOp {
                ordinal: 3,
                kind: CollectiveKind::LogitsArgmax,
                route_id: 12,
                payload_bytes: 128,
                participant_mask: TP_RANK_MASK,
            },
        ])
        .unwrap()
    }

    fn decode_request() -> StepPlanRequest {
        StepPlanRequest {
            epoch: 7,
            step_id: 42,
            mode: StepMode::Decode,
            active_sequences: 8,
            sequence_bucket: 8,
            scheduled_prompt_tokens: 0,
            query_rows: 8,
            verifier_row_bucket: 8,
            mtp_depth: 0,
            graph_id: 11,
            tp_route_id: 9,
            dcp_route_id: 3,
            attention_transport: AttentionTransport::DecodeQueryLse,
            sampling_route_id: 12,
            sequence_table_generation: 99,
        }
    }

    #[test]
    fn plan_is_byte_stable_and_self_verifying() {
        let schedule = schedule();
        let first = StepPlan::build(decode_request(), &schedule).unwrap();
        let second = StepPlan::build(decode_request(), &schedule).unwrap();
        assert_eq!(first.canonical_record(), second.canonical_record());
        assert_eq!(first.canonical_hash_input().len(), 85);
        assert_eq!(first.canonical_record().len(), 117);
        assert_eq!(first.verify(&schedule), Ok(()));
    }

    #[test]
    fn rank_local_schedule_change_is_rejected() {
        let schedule = schedule();
        let plan = StepPlan::build(decode_request(), &schedule).unwrap();
        let mut changed = schedule.operations().to_vec();
        changed[2].route_id = 10;
        let changed = CollectiveSchedule::new(changed).unwrap();
        assert_eq!(plan.verify(&changed), Err(PlanError::CollectiveHash));
    }

    #[test]
    fn tampered_plan_is_rejected() {
        let schedule = schedule();
        let mut plan = StepPlan::build(decode_request(), &schedule).unwrap();
        plan.step_id += 1;
        assert_eq!(plan.verify(&schedule), Err(PlanError::PlanHash));
    }

    #[test]
    fn cache_only_requires_every_compute_field_to_be_zero() {
        let request = StepPlanRequest {
            epoch: 1,
            step_id: 2,
            mode: StepMode::CacheOnly,
            active_sequences: 0,
            sequence_bucket: 0,
            scheduled_prompt_tokens: 0,
            query_rows: 0,
            verifier_row_bucket: 0,
            mtp_depth: 0,
            graph_id: 0,
            tp_route_id: 0,
            dcp_route_id: 0,
            attention_transport: AttentionTransport::None,
            sampling_route_id: 0,
            sequence_table_generation: 0,
        };
        assert!(StepPlan::build(request, &CollectiveSchedule::empty()).is_ok());
        assert_eq!(
            StepPlan::build(
                StepPlanRequest {
                    graph_id: 1,
                    ..request
                },
                &CollectiveSchedule::empty()
            ),
            Err(PlanError::NonCanonicalUnused)
        );
    }

    #[test]
    fn verifier_rows_are_derived_not_rank_local() {
        let schedule = schedule();
        let valid = StepPlanRequest {
            mode: StepMode::Verify,
            query_rows: 56,
            verifier_row_bucket: 64,
            mtp_depth: 6,
            ..decode_request()
        };
        assert!(StepPlan::build(valid, &schedule).is_ok());
        assert_eq!(
            StepPlan::build(
                StepPlanRequest {
                    query_rows: 55,
                    ..valid
                },
                &schedule
            ),
            Err(PlanError::Mode)
        );
    }

    #[test]
    fn mixed_mode_fails_until_its_attention_contract_is_reviewed() {
        let schedule = schedule();
        assert_eq!(
            StepPlan::build(
                StepPlanRequest {
                    mode: StepMode::Mixed,
                    ..decode_request()
                },
                &schedule
            ),
            Err(PlanError::MixedContractUnreviewed)
        );
    }

    #[test]
    fn malformed_collective_schedules_fail_closed() {
        let invalid = CollectiveSchedule::new(vec![CollectiveOp {
            ordinal: 1,
            kind: CollectiveKind::TpReduce,
            route_id: 1,
            payload_bytes: 1,
            participant_mask: TP_RANK_MASK,
        }]);
        assert_eq!(invalid, Err(PlanError::CollectiveOrdinal));
    }
}
