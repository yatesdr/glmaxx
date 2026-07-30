use std::{collections::BTreeSet, fmt};

use glm_cache::{
    MAXIMUM_CONTEXT_TOKENS, PAGE_TABLE_DELTA_SCHEMA, PageTableDelta, PageTableDeltaError,
};
use sha2::{Digest, Sha256};

use crate::{
    CollectiveKind, CollectiveSchedule, MAX_ACTIVE_SEQUENCES, MAX_MTP_DEPTH, PlanError, StepMode,
    StepPlan,
};

pub const STEP_INPUT_SCHEMA: &str = "glmaxx.step-input.v1";

const INPUT_HASH_DOMAIN: &[u8] = b"glmaxx.step-input.v1\0";
const ONE_F32_BITS: u32 = 1.0_f32.to_bits();

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum StepSamplingKind {
    Greedy = 1,
    TopK = 2,
    Mass = 3,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StepSampling {
    pub kind: StepSamplingKind,
    pub temperature_bits: u32,
    pub top_p_bits: u32,
    pub top_k: u16,
    pub seed: u64,
    pub rng_counter_before: u64,
}

impl StepSampling {
    #[must_use]
    pub const fn greedy(seed: u64) -> Self {
        Self {
            kind: StepSamplingKind::Greedy,
            temperature_bits: 0,
            top_p_bits: ONE_F32_BITS,
            top_k: 0,
            seed,
            rng_counter_before: 0,
        }
    }

    fn validate(self) -> Result<(), StepInputError> {
        let temperature = f32::from_bits(self.temperature_bits);
        let top_p = f32::from_bits(self.top_p_bits);
        if !temperature.is_finite()
            || !top_p.is_finite()
            || is_negative_zero(self.temperature_bits)
            || is_negative_zero(self.top_p_bits)
        {
            return Err(StepInputError::Sampling);
        }
        match self.kind {
            StepSamplingKind::Greedy => {
                if self.temperature_bits != 0
                    || self.top_p_bits != ONE_F32_BITS
                    || self.top_k != 0
                    || self.rng_counter_before != 0
                {
                    return Err(StepInputError::Sampling);
                }
            }
            StepSamplingKind::TopK => {
                if temperature <= 0.0
                    || !(0.0 < top_p && top_p <= 1.0)
                    || !(1..=256).contains(&self.top_k)
                {
                    return Err(StepInputError::Sampling);
                }
            }
            StepSamplingKind::Mass => {
                if temperature <= 0.0 || self.top_p_bits != ONE_F32_BITS || self.top_k != 0 {
                    return Err(StepInputError::Sampling);
                }
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SequenceStepInput {
    pub request_id: u64,
    pub context_tokens_before: u32,
    pub generated_tokens_before: u32,
    pub maximum_new_tokens: u32,
    pub prompt_payload_offset: u32,
    pub prompt_tokens_this_step: u32,
    pub configured_mtp_depth: u8,
    pub effective_mtp_depth: u8,
    pub sampling: StepSampling,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StepInput {
    sequence_table_generation: u64,
    page_table_delta_digest: [u8; 32],
    rows: Box<[SequenceStepInput]>,
    prompt_token_ids: Box<[u32]>,
    canonical_hash: [u8; 32],
}

impl StepInput {
    pub fn new(
        plan: &StepPlan,
        schedule: &CollectiveSchedule,
        delta: &PageTableDelta,
        rows: Vec<SequenceStepInput>,
        prompt_token_ids: Vec<u32>,
    ) -> Result<Self, StepInputError> {
        plan.verify(schedule)?;
        delta.verify()?;
        let mut input = Self {
            sequence_table_generation: delta.generation_after(),
            page_table_delta_digest: delta.global_digest(),
            rows: rows.into_boxed_slice(),
            prompt_token_ids: prompt_token_ids.into_boxed_slice(),
            canonical_hash: [0; 32],
        };
        input.validate_shape(plan, schedule, delta)?;
        input.canonical_hash = input.compute_hash();
        Ok(input)
    }

    #[must_use]
    pub const fn sequence_table_generation(&self) -> u64 {
        self.sequence_table_generation
    }

    #[must_use]
    pub const fn page_table_delta_digest(&self) -> [u8; 32] {
        self.page_table_delta_digest
    }

    #[must_use]
    pub fn rows(&self) -> &[SequenceStepInput] {
        &self.rows
    }

    #[must_use]
    pub fn prompt_token_ids(&self) -> &[u32] {
        &self.prompt_token_ids
    }

    #[must_use]
    pub const fn canonical_hash(&self) -> [u8; 32] {
        self.canonical_hash
    }

    pub fn verify(
        &self,
        plan: &StepPlan,
        schedule: &CollectiveSchedule,
        delta: &PageTableDelta,
    ) -> Result<(), StepInputError> {
        plan.verify(schedule)?;
        delta.verify()?;
        self.validate_shape(plan, schedule, delta)?;
        if self.compute_hash() != self.canonical_hash {
            return Err(StepInputError::Hash);
        }
        Ok(())
    }

    fn validate_shape(
        &self,
        plan: &StepPlan,
        schedule: &CollectiveSchedule,
        delta: &PageTableDelta,
    ) -> Result<(), StepInputError> {
        if self.sequence_table_generation == 0
            || self.sequence_table_generation != plan.sequence_table_generation
            || self.sequence_table_generation != delta.generation_after()
            || self.page_table_delta_digest != delta.global_digest()
            || self.rows.len() != usize::from(plan.active_sequences)
            || self.rows.len() > usize::from(MAX_ACTIVE_SEQUENCES)
            || delta.updates().len() != self.rows.len()
            || !delta.removed_sequence_ids().is_empty()
        {
            return Err(StepInputError::Binding);
        }
        if matches!(plan.mode, StepMode::Mixed | StepMode::CacheOnly) {
            return Err(StepInputError::Mode);
        }
        let mut request_ids = BTreeSet::new();
        let mut prompt_cursor = 0_u32;
        for row in &self.rows {
            row.sampling.validate()?;
            if row.request_id == 0
                || !request_ids.insert(row.request_id)
                || row.maximum_new_tokens == 0
                || row.generated_tokens_before >= row.maximum_new_tokens
                || row.configured_mtp_depth > MAX_MTP_DEPTH
                || row.effective_mtp_depth > row.configured_mtp_depth
                || row.prompt_payload_offset != prompt_cursor
            {
                return Err(StepInputError::Row);
            }
            prompt_cursor = prompt_cursor
                .checked_add(row.prompt_tokens_this_step)
                .ok_or(StepInputError::Overflow)?;
            let remaining = row
                .maximum_new_tokens
                .checked_sub(row.generated_tokens_before)
                .ok_or(StepInputError::Row)?;
            let positions_after_prompt = u64::from(row.context_tokens_before)
                .checked_add(u64::from(row.prompt_tokens_this_step))
                .ok_or(StepInputError::Overflow)?;
            if positions_after_prompt
                .checked_add(u64::from(remaining))
                .is_none_or(|positions| positions > MAXIMUM_CONTEXT_TOKENS)
            {
                return Err(StepInputError::Context);
            }
            match plan.mode {
                StepMode::Prefill => {
                    if row.prompt_tokens_this_step == 0
                        || row.generated_tokens_before != 0
                        || row.effective_mtp_depth != 0
                    {
                        return Err(StepInputError::Mode);
                    }
                }
                StepMode::Decode => {
                    if row.prompt_tokens_this_step != 0 || row.effective_mtp_depth != 0 {
                        return Err(StepInputError::Mode);
                    }
                }
                StepMode::Verify => {
                    if row.prompt_tokens_this_step != 0
                        || row.effective_mtp_depth != plan.mtp_depth
                        || row.configured_mtp_depth < plan.mtp_depth
                    {
                        return Err(StepInputError::Mode);
                    }
                }
                StepMode::Mixed | StepMode::CacheOnly => unreachable!(),
            }
            let update = delta
                .updates()
                .iter()
                .find(|update| update.request_id() == row.request_id)
                .ok_or(StepInputError::Binding)?;
            let expected_committed = match plan.mode {
                StepMode::Prefill => u64::from(row.context_tokens_before)
                    .checked_add(u64::from(row.prompt_tokens_this_step))
                    .ok_or(StepInputError::Overflow)?,
                StepMode::Decode | StepMode::Verify => u64::from(row.context_tokens_before),
                StepMode::Mixed | StepMode::CacheOnly => unreachable!(),
            };
            let expected_tentative = match plan.mode {
                StepMode::Prefill => 0,
                StepMode::Decode => 1,
                StepMode::Verify => plan.mtp_depth + 1,
                StepMode::Mixed | StepMode::CacheOnly => unreachable!(),
            };
            if u32::from(expected_tentative) > remaining
                || update.mtp() != (row.configured_mtp_depth != 0)
                || update.committed_tokens() != expected_committed
                || update.tentative_tokens() != expected_tentative
            {
                return Err(StepInputError::Binding);
            }
        }
        let prompt_count =
            u32::try_from(self.prompt_token_ids.len()).map_err(|_| StepInputError::Overflow)?;
        if prompt_cursor != prompt_count {
            return Err(StepInputError::Prompt);
        }
        match plan.mode {
            StepMode::Prefill => {
                if prompt_count != plan.scheduled_prompt_tokens || prompt_count != plan.query_rows {
                    return Err(StepInputError::Prompt);
                }
            }
            StepMode::Decode | StepMode::Verify => {
                if prompt_count != 0 || plan.scheduled_prompt_tokens != 0 {
                    return Err(StepInputError::Prompt);
                }
                let sampling_kind =
                    schedule_sampling_kind(plan, schedule)?.ok_or(StepInputError::Sampling)?;
                if self
                    .rows
                    .iter()
                    .any(|row| row.sampling.kind != sampling_kind)
                {
                    return Err(StepInputError::Sampling);
                }
            }
            StepMode::Mixed | StepMode::CacheOnly => unreachable!(),
        }
        Ok(())
    }

    fn compute_hash(&self) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(INPUT_HASH_DOMAIN);
        hasher.update(Sha256::digest(PAGE_TABLE_DELTA_SCHEMA.as_bytes()));
        hasher.update(self.sequence_table_generation.to_le_bytes());
        hasher.update(self.page_table_delta_digest);
        hasher.update(
            u16::try_from(self.rows.len())
                .expect("validated StepInput row count fits u16")
                .to_le_bytes(),
        );
        hasher.update(
            u32::try_from(self.prompt_token_ids.len())
                .expect("validated StepInput prompt count fits u32")
                .to_le_bytes(),
        );
        for row in &self.rows {
            hasher.update(row.request_id.to_le_bytes());
            hasher.update(row.context_tokens_before.to_le_bytes());
            hasher.update(row.generated_tokens_before.to_le_bytes());
            hasher.update(row.maximum_new_tokens.to_le_bytes());
            hasher.update(row.prompt_payload_offset.to_le_bytes());
            hasher.update(row.prompt_tokens_this_step.to_le_bytes());
            hasher.update([row.configured_mtp_depth]);
            hasher.update([row.effective_mtp_depth]);
            hasher.update([row.sampling.kind as u8]);
            hasher.update(row.sampling.temperature_bits.to_le_bytes());
            hasher.update(row.sampling.top_p_bits.to_le_bytes());
            hasher.update(row.sampling.top_k.to_le_bytes());
            hasher.update(row.sampling.seed.to_le_bytes());
            hasher.update(row.sampling.rng_counter_before.to_le_bytes());
        }
        for token_id in &self.prompt_token_ids {
            hasher.update(token_id.to_le_bytes());
        }
        hasher.finalize().into()
    }
}

const fn is_negative_zero(bits: u32) -> bool {
    bits == (-0.0_f32).to_bits()
}

fn schedule_sampling_kind(
    plan: &StepPlan,
    schedule: &CollectiveSchedule,
) -> Result<Option<StepSamplingKind>, StepInputError> {
    let mut sampling_kind = None;
    for operation in schedule.operations() {
        let kind = match operation.kind {
            CollectiveKind::LogitsArgmax => Some(StepSamplingKind::Greedy),
            CollectiveKind::LogitsTopK => Some(StepSamplingKind::TopK),
            CollectiveKind::LogitsMass => Some(StepSamplingKind::Mass),
            _ => None,
        };
        let Some(kind) = kind else {
            continue;
        };
        if operation.route_id != plan.sampling_route_id || sampling_kind.replace(kind).is_some() {
            return Err(StepInputError::Sampling);
        }
    }
    if plan.mode == StepMode::Prefill {
        if sampling_kind.is_some() || plan.sampling_route_id != 0 {
            return Err(StepInputError::Sampling);
        }
        return Ok(None);
    }
    Ok(sampling_kind)
}

#[derive(Debug, Eq, PartialEq)]
pub enum StepInputError {
    Binding,
    Row,
    Prompt,
    Sampling,
    Context,
    Mode,
    Hash,
    Overflow,
    Plan(PlanError),
    Delta(PageTableDeltaError),
}

impl fmt::Display for StepInputError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for StepInputError {}

impl From<PageTableDeltaError> for StepInputError {
    fn from(value: PageTableDeltaError) -> Self {
        Self::Delta(value)
    }
}

impl From<PlanError> for StepInputError {
    fn from(value: PlanError) -> Self {
        Self::Plan(value)
    }
}

#[cfg(test)]
mod tests {
    use glm_cache::{PageTableConfig, SequencePageTable};

    use crate::{
        AttentionTransport, CollectiveKind, CollectiveOp, CollectiveSchedule, StepPlanRequest,
        TP_RANK_MASK,
    };

    use super::*;

    fn schedule(sampling: Option<CollectiveKind>) -> CollectiveSchedule {
        let mut operations = vec![CollectiveOp {
            ordinal: 0,
            kind: CollectiveKind::TpReduce,
            route_id: 1,
            payload_bytes: 32,
            participant_mask: TP_RANK_MASK,
        }];
        if let Some(kind) = sampling {
            operations.push(CollectiveOp {
                ordinal: 1,
                kind,
                route_id: 2,
                payload_bytes: 32,
                participant_mask: TP_RANK_MASK,
            });
        }
        CollectiveSchedule::new(operations).unwrap()
    }

    fn plan(
        mode: StepMode,
        sequences: u16,
        rows: u32,
        depth: u8,
        sampling: Option<CollectiveKind>,
    ) -> (StepPlan, CollectiveSchedule) {
        let schedule = schedule(sampling);
        let plan = StepPlan::build(
            StepPlanRequest {
                epoch: 1,
                step_id: 9,
                mode,
                active_sequences: sequences,
                sequence_bucket: if sequences == 1 { 1 } else { 4 },
                scheduled_prompt_tokens: if mode == StepMode::Prefill { rows } else { 0 },
                query_rows: rows,
                verifier_row_bucket: if mode == StepMode::Prefill { 0 } else { rows },
                mtp_depth: depth,
                graph_id: 1,
                tp_route_id: 1,
                dcp_route_id: 1,
                attention_transport: if mode == StepMode::Prefill {
                    AttentionTransport::PrefillQuery
                } else {
                    AttentionTransport::DecodeQueryLse
                },
                sampling_route_id: if mode == StepMode::Prefill { 0 } else { 2 },
                sequence_table_generation: 5,
            },
            &schedule,
        )
        .unwrap();
        (plan, schedule)
    }

    fn table() -> SequencePageTable {
        SequencePageTable::new(PageTableConfig {
            target_pages_per_rank: 8,
            draft_pages_per_rank: 8,
        })
        .unwrap()
    }

    #[test]
    fn multi_row_prefill_hash_binds_prompt_rows_and_page_delta() {
        let mut before = table();
        before.admit_with_prefix(11, false, &[]).unwrap();
        before.append_committed(11, 64).unwrap();
        before.admit_with_prefix(22, true, &[]).unwrap();
        before.append_committed(22, 64).unwrap();
        let mut after = before.clone();
        after.append_committed(11, 2).unwrap();
        after.append_committed(22, 3).unwrap();
        let delta = PageTableDelta::between(&before, &after, 4, 5).unwrap();
        let (plan, schedule) = plan(StepMode::Prefill, 2, 5, 0, None);
        let rows = vec![
            SequenceStepInput {
                request_id: 22,
                context_tokens_before: 64,
                generated_tokens_before: 0,
                maximum_new_tokens: 10,
                prompt_payload_offset: 0,
                prompt_tokens_this_step: 3,
                configured_mtp_depth: 6,
                effective_mtp_depth: 0,
                sampling: StepSampling::greedy(22),
            },
            SequenceStepInput {
                request_id: 11,
                context_tokens_before: 64,
                generated_tokens_before: 0,
                maximum_new_tokens: 10,
                prompt_payload_offset: 3,
                prompt_tokens_this_step: 2,
                configured_mtp_depth: 0,
                effective_mtp_depth: 0,
                sampling: StepSampling::greedy(11),
            },
        ];
        let input =
            StepInput::new(&plan, &schedule, &delta, rows.clone(), vec![1, 2, 3, 4, 5]).unwrap();
        input.verify(&plan, &schedule, &delta).unwrap();
        let duplicate =
            StepInput::new(&plan, &schedule, &delta, rows.clone(), vec![1, 2, 3, 4, 5]).unwrap();
        assert_eq!(input.canonical_hash(), duplicate.canonical_hash());
        let changed = StepInput::new(&plan, &schedule, &delta, rows, vec![1, 2, 3, 4, 6]).unwrap();
        assert_ne!(input.canonical_hash(), changed.canonical_hash());
        assert_eq!(input.rows()[0].request_id, 22);
        assert_eq!(input.prompt_token_ids(), [1, 2, 3, 4, 5]);
    }

    #[test]
    fn configured_mtp6_tail_binds_to_effective_mtp0_reservation() {
        let mut before = table();
        before.admit_with_prefix(7, true, &[]).unwrap();
        before.append_committed(7, 64).unwrap();
        let mut after = before.clone();
        after.begin_tentative(7, 1).unwrap();
        let delta = PageTableDelta::between(&before, &after, 4, 5).unwrap();
        let (plan, schedule) = plan(
            StepMode::Decode,
            1,
            1,
            0,
            Some(CollectiveKind::LogitsArgmax),
        );
        let input = StepInput::new(
            &plan,
            &schedule,
            &delta,
            vec![SequenceStepInput {
                request_id: 7,
                context_tokens_before: 64,
                generated_tokens_before: 0,
                maximum_new_tokens: 1,
                prompt_payload_offset: 0,
                prompt_tokens_this_step: 0,
                configured_mtp_depth: 6,
                effective_mtp_depth: 0,
                sampling: StepSampling::greedy(99),
            }],
            vec![],
        )
        .unwrap();
        input.verify(&plan, &schedule, &delta).unwrap();
    }

    #[test]
    fn verify_sampling_context_and_hash_tampering_fail_closed() {
        let mut before = table();
        before.admit_with_prefix(7, true, &[]).unwrap();
        before.append_committed(7, 64).unwrap();
        let mut after = before.clone();
        after.begin_tentative(7, 6).unwrap();
        let delta = PageTableDelta::between(&before, &after, 4, 5).unwrap();
        let (plan, schedule) = plan(StepMode::Verify, 1, 6, 5, Some(CollectiveKind::LogitsTopK));
        let row = SequenceStepInput {
            request_id: 7,
            context_tokens_before: 64,
            generated_tokens_before: 2,
            maximum_new_tokens: 8,
            prompt_payload_offset: 0,
            prompt_tokens_this_step: 0,
            configured_mtp_depth: 6,
            effective_mtp_depth: 5,
            sampling: StepSampling {
                kind: StepSamplingKind::TopK,
                temperature_bits: 0.8_f32.to_bits(),
                top_p_bits: 0.9_f32.to_bits(),
                top_k: 32,
                seed: 4,
                rng_counter_before: 9,
            },
        };
        let mut input = StepInput::new(&plan, &schedule, &delta, vec![row], vec![]).unwrap();
        input.verify(&plan, &schedule, &delta).unwrap();
        input.canonical_hash[0] ^= 1;
        assert_eq!(
            input.verify(&plan, &schedule, &delta),
            Err(StepInputError::Hash)
        );

        let mut invalid_sampling = row;
        invalid_sampling.sampling.top_p_bits = (-0.0_f32).to_bits();
        assert_eq!(
            StepInput::new(&plan, &schedule, &delta, vec![invalid_sampling], vec![]),
            Err(StepInputError::Sampling)
        );

        let mut invalid_context = row;
        invalid_context.context_tokens_before = u32::try_from(MAXIMUM_CONTEXT_TOKENS).unwrap();
        assert_eq!(
            StepInput::new(&plan, &schedule, &delta, vec![invalid_context], vec![]),
            Err(StepInputError::Context)
        );

        let mut other_after = before.clone();
        other_after.begin_tentative(7, 5).unwrap();
        let other_delta = PageTableDelta::between(&before, &other_after, 4, 5).unwrap();
        let valid = StepInput::new(&plan, &schedule, &delta, vec![row], vec![]).unwrap();
        assert_eq!(
            valid.verify(&plan, &schedule, &other_delta),
            Err(StepInputError::Binding)
        );

        let mut route_mismatch = row;
        route_mismatch.sampling.kind = StepSamplingKind::Mass;
        route_mismatch.sampling.top_p_bits = ONE_F32_BITS;
        route_mismatch.sampling.top_k = 0;
        assert_eq!(
            StepInput::new(&plan, &schedule, &delta, vec![route_mismatch], vec![]),
            Err(StepInputError::Sampling)
        );

        let mut insufficient_output_capacity = row;
        insufficient_output_capacity.maximum_new_tokens = 7;
        assert_eq!(
            StepInput::new(
                &plan,
                &schedule,
                &delta,
                vec![insufficient_output_capacity],
                vec![],
            ),
            Err(StepInputError::Binding)
        );
    }

    #[test]
    fn every_sampling_class_has_one_fail_closed_canonical_form() {
        let top_k = StepSampling {
            kind: StepSamplingKind::TopK,
            temperature_bits: 0.5_f32.to_bits(),
            top_p_bits: 0.75_f32.to_bits(),
            top_k: 256,
            seed: 1,
            rng_counter_before: 2,
        };
        let mass = StepSampling {
            kind: StepSamplingKind::Mass,
            temperature_bits: 1.0_f32.to_bits(),
            top_p_bits: ONE_F32_BITS,
            top_k: 0,
            seed: 3,
            rng_counter_before: 4,
        };
        assert_eq!(StepSampling::greedy(9).validate(), Ok(()));
        assert_eq!(top_k.validate(), Ok(()));
        assert_eq!(mass.validate(), Ok(()));

        let invalid = [
            StepSampling {
                rng_counter_before: 1,
                ..StepSampling::greedy(9)
            },
            StepSampling {
                temperature_bits: f32::NAN.to_bits(),
                ..top_k
            },
            StepSampling { top_k: 0, ..top_k },
            StepSampling {
                top_p_bits: 0.0_f32.to_bits(),
                ..top_k
            },
            StepSampling {
                top_p_bits: 0.9_f32.to_bits(),
                ..mass
            },
            StepSampling { top_k: 1, ..mass },
        ];
        for sampling in invalid {
            assert_eq!(sampling.validate(), Err(StepInputError::Sampling));
        }
    }
}
