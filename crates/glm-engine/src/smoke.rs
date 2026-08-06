use std::{fmt, sync::Arc};

use glm_cache::{PageTableConfig, PageTableDelta, SequencePageError, SequencePageTable};
use serde::Serialize;

use crate::{
    AttentionTransport, BatchSmokeProgram, CollectiveKind, CollectiveOp, CollectiveSchedule,
    GLM_52_OUTPUT_VOCABULARY, PlanError, SequenceStepInput, StepInput, StepInputError, StepMode,
    StepPlan, StepPlanRequest, StepSampling, TP_RANK_MASK, Tp4WorkerPool, WorkerError,
    WorkerExecutionPosture,
};

pub const BATCH_SMOKE_ROWS: usize = 4;
pub const BATCH_SMOKE_DECODE_STEPS: usize = 16;
const PREFILL_GRAPH_ID: u32 = 1;
const DECODE_GRAPH_ID: u32 = 2;
const TP_ROUTE_ID: u16 = 1;
const DCP_ROUTE_ID: u16 = 1;
const SAMPLING_ROUTE_ID: u16 = 2;
const PHYSICAL_PAGES_PER_RANK: u32 = 64;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CpuBatchSmokeEvidence {
    pub schema: &'static str,
    pub execution_posture: &'static str,
    pub program_sha256: [u8; 32],
    pub request_ids: [u64; BATCH_SMOKE_ROWS],
    pub prompt_token_counts: [u32; BATCH_SMOKE_ROWS],
    pub prefill_plan_sha256: [u8; 32],
    pub prefill_output_sha256: [u8; 32],
    pub decode_plan_sha256: [[u8; 32]; BATCH_SMOKE_DECODE_STEPS],
    pub decode_output_sha256: [[u8; 32]; BATCH_SMOKE_DECODE_STEPS],
    pub generated_token_ids: [Vec<u32>; BATCH_SMOKE_ROWS],
    pub final_page_table_generation: u64,
    pub tp4_consensus_steps: u32,
}

impl CpuBatchSmokeEvidence {
    pub fn validate(&self) -> Result<(), BatchSmokeError> {
        if self.schema != "glmaxx.cpu-tp4-batch-smoke.v1"
            || self.execution_posture != "cpu-reference-not-model-inference"
            || self.program_sha256 == [0; 32]
            || self.request_ids.contains(&0)
            || self.prompt_token_counts.contains(&0)
            || self.prefill_plan_sha256 == [0; 32]
            || self.prefill_output_sha256 == [0; 32]
            || self.decode_plan_sha256.contains(&[0; 32])
            || self.decode_output_sha256.contains(&[0; 32])
            || self
                .generated_token_ids
                .iter()
                .any(|tokens| tokens.len() != BATCH_SMOKE_DECODE_STEPS)
            || self
                .generated_token_ids
                .iter()
                .flatten()
                .any(|&token| token >= GLM_52_OUTPUT_VOCABULARY)
            || self.final_page_table_generation != 34
            || self.tp4_consensus_steps != 17
        {
            return Err(BatchSmokeError::Evidence);
        }
        Ok(())
    }
}

pub fn run_cpu_mtp0_batch_smoke(
    pool: &Tp4WorkerPool,
    program: BatchSmokeProgram,
    request_ids: [u64; BATCH_SMOKE_ROWS],
    prompts: [Vec<u32>; BATCH_SMOKE_ROWS],
) -> Result<CpuBatchSmokeEvidence, BatchSmokeError> {
    if pool.execution_posture() != WorkerExecutionPosture::CpuReference {
        return Err(BatchSmokeError::Posture);
    }
    validate_fixture(program, request_ids, &prompts)?;

    let mut generation = 1_u64;
    let empty = SequencePageTable::new(PageTableConfig {
        target_pages_per_rank: PHYSICAL_PAGES_PER_RANK,
        draft_pages_per_rank: 0,
    })?;
    pool.initialize_page_table(Arc::new(empty.clone()), generation)?;

    let mut committed = empty.clone();
    for (request_id, prompt) in request_ids.into_iter().zip(&prompts) {
        committed.admit_with_prefix(request_id, false, &[])?;
        committed.append_committed(
            request_id,
            u64::try_from(prompt.len()).map_err(|_| BatchSmokeError::Overflow)?,
        )?;
    }
    let prefill_generation = generation.checked_add(1).ok_or(BatchSmokeError::Overflow)?;
    let prefill_delta = Arc::new(PageTableDelta::between(
        &empty,
        &committed,
        generation,
        prefill_generation,
    )?);
    let prefill_schedule = prefill_schedule()?;
    let prompt_token_count = prompts.iter().try_fold(0_u32, |sum, prompt| {
        sum.checked_add(u32::try_from(prompt.len()).map_err(|_| BatchSmokeError::Overflow)?)
            .ok_or(BatchSmokeError::Overflow)
    })?;
    let prefill_plan = StepPlan::build(
        StepPlanRequest {
            epoch: 1,
            step_id: 1,
            mode: StepMode::Prefill,
            active_sequences: BATCH_SMOKE_ROWS as u16,
            sequence_bucket: BATCH_SMOKE_ROWS as u16,
            scheduled_prompt_tokens: prompt_token_count,
            query_rows: prompt_token_count,
            verifier_row_bucket: 0,
            mtp_depth: 0,
            graph_id: PREFILL_GRAPH_ID,
            tp_route_id: TP_ROUTE_ID,
            dcp_route_id: DCP_ROUTE_ID,
            attention_transport: AttentionTransport::PrefillQuery,
            sampling_route_id: 0,
            sequence_table_generation: prefill_generation,
        },
        &prefill_schedule,
    )?;
    let mut prompt_payload_offset = 0_u32;
    let prefill_rows = request_ids
        .into_iter()
        .zip(&prompts)
        .map(|(request_id, prompt)| {
            let prompt_tokens_this_step =
                u32::try_from(prompt.len()).map_err(|_| BatchSmokeError::Overflow)?;
            let row = SequenceStepInput {
                request_id,
                context_tokens_before: 0,
                generated_tokens_before: 0,
                maximum_new_tokens: BATCH_SMOKE_DECODE_STEPS as u32,
                prompt_payload_offset,
                prompt_tokens_this_step,
                configured_mtp_depth: 0,
                effective_mtp_depth: 0,
                sampling: StepSampling::greedy(request_id),
            };
            prompt_payload_offset = prompt_payload_offset
                .checked_add(prompt_tokens_this_step)
                .ok_or(BatchSmokeError::Overflow)?;
            Ok(row)
        })
        .collect::<Result<Vec<_>, BatchSmokeError>>()?;
    let prompt_token_ids = prompts.iter().flatten().copied().collect();
    let prefill_input = Arc::new(StepInput::new(
        &prefill_plan,
        &prefill_schedule,
        &prefill_delta,
        prefill_rows,
        prompt_token_ids,
    )?);
    let prefill_outcome = pool
        .try_submit_bound(prefill_plan, prefill_schedule, prefill_input, prefill_delta)?
        .receive()?;
    generation = prefill_generation;

    let mut decode_plan_sha256 = [[0_u8; 32]; BATCH_SMOKE_DECODE_STEPS];
    let mut decode_output_sha256 = [[0_u8; 32]; BATCH_SMOKE_DECODE_STEPS];
    let mut generated_token_ids: [Vec<u32>; BATCH_SMOKE_ROWS] =
        std::array::from_fn(|_| Vec::with_capacity(BATCH_SMOKE_DECODE_STEPS));

    for decode_index in 0..BATCH_SMOKE_DECODE_STEPS {
        let mut reserved = committed.clone();
        for request_id in request_ids {
            reserved.begin_tentative(request_id, 1)?;
        }
        let reservation_generation = generation.checked_add(1).ok_or(BatchSmokeError::Overflow)?;
        let reservation_delta = Arc::new(PageTableDelta::between(
            &committed,
            &reserved,
            generation,
            reservation_generation,
        )?);
        let schedule = decode_schedule()?;
        let plan = StepPlan::build(
            StepPlanRequest {
                epoch: 1,
                step_id: u64::try_from(decode_index + 2).map_err(|_| BatchSmokeError::Overflow)?,
                mode: StepMode::Decode,
                active_sequences: BATCH_SMOKE_ROWS as u16,
                sequence_bucket: BATCH_SMOKE_ROWS as u16,
                scheduled_prompt_tokens: 0,
                query_rows: BATCH_SMOKE_ROWS as u32,
                verifier_row_bucket: BATCH_SMOKE_ROWS as u32,
                mtp_depth: 0,
                graph_id: DECODE_GRAPH_ID,
                tp_route_id: TP_ROUTE_ID,
                dcp_route_id: DCP_ROUTE_ID,
                attention_transport: AttentionTransport::DecodeQueryLse,
                sampling_route_id: SAMPLING_ROUTE_ID,
                sequence_table_generation: reservation_generation,
            },
            &schedule,
        )?;
        let rows = request_ids
            .into_iter()
            .zip(&prompts)
            .map(|(request_id, prompt)| {
                let context_tokens_before = prompt
                    .len()
                    .checked_add(decode_index)
                    .and_then(|value| u32::try_from(value).ok())
                    .ok_or(BatchSmokeError::Overflow)?;
                Ok(SequenceStepInput {
                    request_id,
                    context_tokens_before,
                    generated_tokens_before: u32::try_from(decode_index)
                        .map_err(|_| BatchSmokeError::Overflow)?,
                    maximum_new_tokens: BATCH_SMOKE_DECODE_STEPS as u32,
                    prompt_payload_offset: 0,
                    prompt_tokens_this_step: 0,
                    configured_mtp_depth: 0,
                    effective_mtp_depth: 0,
                    sampling: StepSampling::greedy(request_id),
                })
            })
            .collect::<Result<Vec<_>, BatchSmokeError>>()?;
        let input = Arc::new(StepInput::new(
            &plan,
            &schedule,
            &reservation_delta,
            rows,
            vec![],
        )?);
        let outcome = pool
            .try_submit_bound(plan, schedule, input, reservation_delta)?
            .receive()?;
        for (row, committed_tokens) in outcome.output.sequences().iter().enumerate() {
            let &[token_id] = committed_tokens.token_ids() else {
                return Err(BatchSmokeError::Output);
            };
            generated_token_ids[row].push(token_id);
        }
        decode_plan_sha256[decode_index] = outcome.plan_hash;
        decode_output_sha256[decode_index] = outcome.output_digest;

        let mut next_committed = reserved.clone();
        for request_id in request_ids {
            next_committed.commit_tentative(request_id, 1)?;
        }
        let commit_generation = reservation_generation
            .checked_add(1)
            .ok_or(BatchSmokeError::Overflow)?;
        let commit_delta = Arc::new(PageTableDelta::between(
            &reserved,
            &next_committed,
            reservation_generation,
            commit_generation,
        )?);
        pool.apply_page_delta(commit_delta)?;
        committed = next_committed;
        generation = commit_generation;
    }

    let evidence = CpuBatchSmokeEvidence {
        schema: "glmaxx.cpu-tp4-batch-smoke.v1",
        execution_posture: "cpu-reference-not-model-inference",
        program_sha256: program.digest(),
        request_ids,
        prompt_token_counts: prompts.map(|prompt| prompt.len() as u32),
        prefill_plan_sha256: prefill_outcome.plan_hash,
        prefill_output_sha256: prefill_outcome.output_digest,
        decode_plan_sha256,
        decode_output_sha256,
        generated_token_ids,
        final_page_table_generation: generation,
        tp4_consensus_steps: 1 + BATCH_SMOKE_DECODE_STEPS as u32,
    };
    evidence.validate()?;
    Ok(evidence)
}

fn validate_fixture(
    program: BatchSmokeProgram,
    request_ids: [u64; BATCH_SMOKE_ROWS],
    prompts: &[Vec<u32>; BATCH_SMOKE_ROWS],
) -> Result<(), BatchSmokeError> {
    let unique_request_ids = request_ids
        .into_iter()
        .collect::<std::collections::BTreeSet<_>>();
    let prompt_tokens = prompts.iter().try_fold(0_u32, |sum, prompt| {
        if prompt.is_empty()
            || prompt
                .iter()
                .any(|&token_id| token_id >= GLM_52_OUTPUT_VOCABULARY)
        {
            return Err(BatchSmokeError::Fixture);
        }
        sum.checked_add(u32::try_from(prompt.len()).map_err(|_| BatchSmokeError::Overflow)?)
            .ok_or(BatchSmokeError::Overflow)
    })?;
    if unique_request_ids.len() != BATCH_SMOKE_ROWS
        || unique_request_ids.contains(&0)
        || prompt_tokens > program.prefill_row_bucket()
    {
        return Err(BatchSmokeError::Fixture);
    }
    Ok(())
}

fn prefill_schedule() -> Result<CollectiveSchedule, PlanError> {
    CollectiveSchedule::new(vec![CollectiveOp {
        ordinal: 0,
        kind: CollectiveKind::TpReduce,
        route_id: TP_ROUTE_ID,
        payload_bytes: 49_152,
        participant_mask: TP_RANK_MASK,
    }])
}

fn decode_schedule() -> Result<CollectiveSchedule, PlanError> {
    CollectiveSchedule::new(vec![
        CollectiveOp {
            ordinal: 0,
            kind: CollectiveKind::TpReduce,
            route_id: TP_ROUTE_ID,
            payload_bytes: 49_152,
            participant_mask: TP_RANK_MASK,
        },
        CollectiveOp {
            ordinal: 1,
            kind: CollectiveKind::LogitsArgmax,
            route_id: SAMPLING_ROUTE_ID,
            payload_bytes: 32,
            participant_mask: TP_RANK_MASK,
        },
    ])
}

#[derive(Debug)]
pub enum BatchSmokeError {
    Posture,
    Fixture,
    Evidence,
    Output,
    Overflow,
    Page(SequencePageError),
    Delta(glm_cache::PageTableDeltaError),
    Plan(PlanError),
    Input(StepInputError),
    Worker(WorkerError),
}

impl fmt::Display for BatchSmokeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for BatchSmokeError {}

impl From<SequencePageError> for BatchSmokeError {
    fn from(error: SequencePageError) -> Self {
        Self::Page(error)
    }
}

impl From<glm_cache::PageTableDeltaError> for BatchSmokeError {
    fn from(error: glm_cache::PageTableDeltaError) -> Self {
        Self::Delta(error)
    }
}

impl From<PlanError> for BatchSmokeError {
    fn from(error: PlanError) -> Self {
        Self::Plan(error)
    }
}

impl From<StepInputError> for BatchSmokeError {
    fn from(error: StepInputError) -> Self {
        Self::Input(error)
    }
}

impl From<WorkerError> for BatchSmokeError {
    fn from(error: WorkerError) -> Self {
        Self::Worker(error)
    }
}

#[cfg(test)]
mod tests {
    use crate::{Digest32, ProductionProfile};

    use super::*;

    fn program() -> BatchSmokeProgram {
        let identities: [Digest32; 20] =
            std::array::from_fn(|index| [u8::try_from(index + 1).unwrap(); 32]);
        BatchSmokeProgram::new(ProductionProfile::CapacityExl3, 64, identities).unwrap()
    }

    fn prompts() -> [Vec<u32>; BATCH_SMOKE_ROWS] {
        [vec![1, 2, 3], vec![4, 5], vec![6, 7, 8, 9], vec![10]]
    }

    #[test]
    fn four_row_prefill_and_sixteen_decode_steps_are_deterministic() {
        let request_ids = [101, 102, 103, 104];
        let first_pool = Tp4WorkerPool::spawn_cpu(2, None).unwrap();
        let first =
            run_cpu_mtp0_batch_smoke(&first_pool, program(), request_ids, prompts()).unwrap();
        drop(first_pool);
        let second_pool = Tp4WorkerPool::spawn_cpu(2, None).unwrap();
        let second =
            run_cpu_mtp0_batch_smoke(&second_pool, program(), request_ids, prompts()).unwrap();
        assert_eq!(first, second);
        assert_eq!(
            first.generated_token_ids.map(|tokens| tokens.len()),
            [16; 4]
        );
        assert_eq!(first.final_page_table_generation, 34);
        assert_eq!(first.tp4_consensus_steps, 17);
    }

    #[test]
    fn wrong_posture_duplicate_rows_and_oversized_prefill_fail_closed() {
        let custom =
            std::array::from_fn(|_| Box::new(MockExecutor) as Box<dyn crate::RankExecutor + Send>);
        let custom_pool = Tp4WorkerPool::spawn(1, custom).unwrap();
        assert!(matches!(
            run_cpu_mtp0_batch_smoke(&custom_pool, program(), [1, 2, 3, 4], prompts()),
            Err(BatchSmokeError::Posture)
        ));

        let cpu_pool = Tp4WorkerPool::spawn_cpu(1, None).unwrap();
        assert!(matches!(
            run_cpu_mtp0_batch_smoke(&cpu_pool, program(), [1, 1, 3, 4], prompts()),
            Err(BatchSmokeError::Fixture)
        ));
        let too_large = std::array::from_fn(|_| vec![1; 17]);
        assert!(matches!(
            run_cpu_mtp0_batch_smoke(&cpu_pool, program(), [1, 2, 3, 4], too_large),
            Err(BatchSmokeError::Fixture)
        ));
    }

    struct MockExecutor;

    impl crate::RankExecutor for MockExecutor {
        fn execute(
            &mut self,
            _rank: u8,
            _plan: &StepPlan,
            _schedule: &CollectiveSchedule,
        ) -> Result<crate::StepOutput, crate::RankExecutionError> {
            Err(crate::RankExecutionError::Invariant)
        }

        fn execute_bound(
            &mut self,
            _rank: u8,
            _plan: &StepPlan,
            _schedule: &CollectiveSchedule,
            _input: &StepInput,
        ) -> Result<crate::StepOutput, crate::RankExecutionError> {
            Err(crate::RankExecutionError::Invariant)
        }
    }
}
