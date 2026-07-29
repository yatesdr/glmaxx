use std::fmt;

use glm_engine::{
    AttentionTransport, CollectiveKind, CollectiveOp, CollectiveSchedule, GraphEntry, PlanError,
    StepMode, StepPlan, StepPlanRequest, TP_RANK_MASK,
};

use crate::{BatchKind, ScheduledBatch};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SamplingCollective {
    Greedy,
    TopK,
    Mass,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RouteCatalog {
    pub tp_route_id: u16,
    pub dcp_ckv_route_id: u16,
    pub dcp_query_route_id: u16,
    pub dcp_candidate_route_id: u16,
    pub dcp_partial_route_id: u16,
    pub greedy_route_id: u16,
    pub top_k_route_id: u16,
    pub mass_route_id: u16,
    pub packed_ckv_bytes_per_row: u32,
    pub query_bytes_per_row: u32,
    pub candidate_bytes_per_row: u32,
    pub partial_state_bytes_per_row: u32,
    pub tp_reduce_bytes_per_row: u32,
    pub greedy_bytes_per_row: u32,
    pub top_k_bytes_per_row: u32,
    pub mass_bytes_per_row: u32,
}

impl RouteCatalog {
    pub fn validate(self) -> Result<(), CompileError> {
        let routes = [
            self.tp_route_id,
            self.dcp_ckv_route_id,
            self.dcp_query_route_id,
            self.dcp_candidate_route_id,
            self.dcp_partial_route_id,
            self.greedy_route_id,
            self.top_k_route_id,
            self.mass_route_id,
        ];
        let payloads = [
            self.packed_ckv_bytes_per_row,
            self.query_bytes_per_row,
            self.candidate_bytes_per_row,
            self.partial_state_bytes_per_row,
            self.tp_reduce_bytes_per_row,
            self.greedy_bytes_per_row,
            self.top_k_bytes_per_row,
            self.mass_bytes_per_row,
        ];
        if routes.contains(&0) || payloads.contains(&0) {
            return Err(CompileError::Catalog);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompiledStep {
    pub plan: StepPlan,
    pub schedule: CollectiveSchedule,
}

#[derive(Clone, Copy, Debug)]
pub struct StepPlanCompiler {
    epoch: u64,
    routes: RouteCatalog,
}

impl StepPlanCompiler {
    pub fn new(epoch: u64, routes: RouteCatalog) -> Result<Self, CompileError> {
        if epoch == 0 {
            return Err(CompileError::Epoch);
        }
        routes.validate()?;
        Ok(Self { epoch, routes })
    }

    pub fn compile(
        self,
        batch: &ScheduledBatch,
        entry: &GraphEntry,
        sampling: SamplingCollective,
        sequence_table_generation: u64,
    ) -> Result<CompiledStep, CompileError> {
        if sequence_table_generation == 0
            || batch.rows.is_empty()
            || batch.graph_id != entry.graph_id
        {
            return Err(CompileError::Batch);
        }
        let (mode, mtp_depth) = match batch.kind {
            BatchKind::Prefill => (StepMode::Prefill, 0),
            BatchKind::Decode => (StepMode::Decode, 0),
            BatchKind::Verify { depth } => (StepMode::Verify, depth),
        };
        if entry.key.mode != mode || entry.key.mtp_depth != mtp_depth {
            return Err(CompileError::Graph);
        }
        if mode == StepMode::Prefill && sampling != SamplingCollective::Greedy {
            return Err(CompileError::Sampling);
        }

        let mut operations = Vec::new();
        match entry.key.attention_transport {
            AttentionTransport::PrefillCkv => {
                push(
                    &mut operations,
                    CollectiveKind::DcpPackedCkv,
                    self.routes.dcp_ckv_route_id,
                    scaled(self.routes.packed_ckv_bytes_per_row, batch.query_rows)?,
                )?;
            }
            AttentionTransport::PrefillQuery => {
                push(
                    &mut operations,
                    CollectiveKind::DcpQueryGather,
                    self.routes.dcp_query_route_id,
                    scaled(self.routes.query_bytes_per_row, batch.query_rows)?,
                )?;
                push(
                    &mut operations,
                    CollectiveKind::DcpPartialStateReturn,
                    self.routes.dcp_partial_route_id,
                    scaled(self.routes.partial_state_bytes_per_row, batch.query_rows)?,
                )?;
            }
            AttentionTransport::DecodeQueryLse => {
                push(
                    &mut operations,
                    CollectiveKind::DcpQueryGather,
                    self.routes.dcp_query_route_id,
                    scaled(self.routes.query_bytes_per_row, batch.query_rows)?,
                )?;
                push(
                    &mut operations,
                    CollectiveKind::DcpCandidateExchange,
                    self.routes.dcp_candidate_route_id,
                    scaled(self.routes.candidate_bytes_per_row, batch.query_rows)?,
                )?;
                push(
                    &mut operations,
                    CollectiveKind::DcpPartialStateReturn,
                    self.routes.dcp_partial_route_id,
                    scaled(self.routes.partial_state_bytes_per_row, batch.query_rows)?,
                )?;
            }
            AttentionTransport::None => return Err(CompileError::Graph),
        }
        push(
            &mut operations,
            CollectiveKind::TpReduce,
            self.routes.tp_route_id,
            scaled(self.routes.tp_reduce_bytes_per_row, batch.query_rows)?,
        )?;

        let sampling_route_id = if mode == StepMode::Prefill {
            0
        } else {
            let (kind, route, bytes) = match sampling {
                SamplingCollective::Greedy => (
                    CollectiveKind::LogitsArgmax,
                    self.routes.greedy_route_id,
                    self.routes.greedy_bytes_per_row,
                ),
                SamplingCollective::TopK => (
                    CollectiveKind::LogitsTopK,
                    self.routes.top_k_route_id,
                    self.routes.top_k_bytes_per_row,
                ),
                SamplingCollective::Mass => (
                    CollectiveKind::LogitsMass,
                    self.routes.mass_route_id,
                    self.routes.mass_bytes_per_row,
                ),
            };
            push(
                &mut operations,
                kind,
                route,
                scaled(bytes, batch.query_rows)?,
            )?;
            route
        };
        let dcp_route_id = match entry.key.attention_transport {
            AttentionTransport::PrefillCkv => self.routes.dcp_ckv_route_id,
            AttentionTransport::PrefillQuery | AttentionTransport::DecodeQueryLse => {
                self.routes.dcp_query_route_id
            }
            AttentionTransport::None => return Err(CompileError::Graph),
        };
        let required_dcp_routes: Vec<_> = operations
            .iter()
            .filter(|operation| {
                matches!(
                    operation.kind,
                    CollectiveKind::DcpPackedCkv
                        | CollectiveKind::DcpQueryGather
                        | CollectiveKind::DcpCandidateExchange
                        | CollectiveKind::DcpPartialStateReturn
                )
            })
            .map(|operation| operation.route_id)
            .collect();
        if !entry
            .compatible_tp_routes
            .contains(&self.routes.tp_route_id)
            || required_dcp_routes
                .iter()
                .any(|route| !entry.compatible_dcp_routes.contains(route))
            || (sampling_route_id != 0
                && !entry
                    .compatible_sampling_routes
                    .contains(&sampling_route_id))
        {
            return Err(CompileError::Route);
        }
        let active_sequences =
            u16::try_from(batch.rows.len()).map_err(|_| CompileError::Overflow)?;
        let scheduled_prompt_tokens = if mode == StepMode::Prefill {
            batch.query_rows
        } else {
            0
        };
        let schedule = CollectiveSchedule::new(operations)?;
        let plan = StepPlan::build(
            StepPlanRequest {
                epoch: self.epoch,
                step_id: batch.step_id,
                mode,
                active_sequences,
                sequence_bucket: entry.key.sequence_bucket,
                scheduled_prompt_tokens,
                query_rows: batch.query_rows,
                verifier_row_bucket: entry.key.verifier_row_bucket,
                mtp_depth,
                graph_id: entry.graph_id,
                tp_route_id: self.routes.tp_route_id,
                dcp_route_id,
                attention_transport: entry.key.attention_transport,
                sampling_route_id,
                sequence_table_generation,
            },
            &schedule,
        )?;
        Ok(CompiledStep { plan, schedule })
    }
}

fn scaled(bytes_per_row: u32, rows: u32) -> Result<u32, CompileError> {
    bytes_per_row
        .checked_mul(rows)
        .ok_or(CompileError::Overflow)
}

fn push(
    operations: &mut Vec<CollectiveOp>,
    kind: CollectiveKind,
    route_id: u16,
    payload_bytes: u32,
) -> Result<(), CompileError> {
    operations.push(CollectiveOp {
        ordinal: u16::try_from(operations.len()).map_err(|_| CompileError::Overflow)?,
        kind,
        route_id,
        payload_bytes,
        participant_mask: TP_RANK_MASK,
    });
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompileError {
    Epoch,
    Catalog,
    Batch,
    Graph,
    Route,
    Sampling,
    Overflow,
    Plan(PlanError),
}

impl fmt::Display for CompileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for CompileError {}

impl From<PlanError> for CompileError {
    fn from(value: PlanError) -> Self {
        Self::Plan(value)
    }
}

#[cfg(test)]
mod tests {
    use glm_engine::GraphKey;

    use super::*;
    use crate::BatchRow;

    fn routes() -> RouteCatalog {
        RouteCatalog {
            tp_route_id: 10,
            dcp_ckv_route_id: 20,
            dcp_query_route_id: 21,
            dcp_candidate_route_id: 22,
            dcp_partial_route_id: 23,
            greedy_route_id: 30,
            top_k_route_id: 31,
            mass_route_id: 32,
            packed_ckv_bytes_per_row: 1024,
            query_bytes_per_row: 256,
            candidate_bytes_per_row: 128,
            partial_state_bytes_per_row: 512,
            tp_reduce_bytes_per_row: 768,
            greedy_bytes_per_row: 16,
            top_k_bytes_per_row: 4096,
            mass_bytes_per_row: 32,
        }
    }

    fn entry() -> GraphEntry {
        GraphEntry {
            graph_id: 7,
            key: GraphKey {
                mode: StepMode::Verify,
                sequence_bucket: 8,
                verifier_row_bucket: 56,
                mtp_depth: 6,
                attention_transport: AttentionTransport::DecodeQueryLse,
            },
            maximum_active_sequences: 8,
            maximum_prompt_tokens: 0,
            maximum_query_rows: 56,
            compatible_tp_routes: vec![10],
            compatible_dcp_routes: vec![21, 22, 23],
            compatible_sampling_routes: vec![30, 31, 32],
            maximum_scratch_bytes: 1,
            argument_bytes: 1,
            graph_object_bytes: 1,
            resident_module_bytes: 1,
            admission_slo_class: 1,
        }
    }

    #[test]
    fn one_compilation_is_byte_identical_for_all_four_ranks() {
        let batch = ScheduledBatch {
            step_id: 44,
            kind: BatchKind::Verify { depth: 6 },
            graph_id: 7,
            rows: (1..=8)
                .map(|request_id| BatchRow {
                    request_id,
                    prompt_tokens: 0,
                })
                .collect(),
            query_rows: 56,
        };
        let compiler = StepPlanCompiler::new(9, routes()).unwrap();
        let expected = compiler
            .compile(&batch, &entry(), SamplingCollective::TopK, 11)
            .unwrap();
        for _rank in 0..4 {
            let compiled = compiler
                .compile(&batch, &entry(), SamplingCollective::TopK, 11)
                .unwrap();
            assert_eq!(compiled, expected);
            assert_eq!(compiled.schedule.operations().len(), 5);
            compiled.plan.verify(&compiled.schedule).unwrap();
        }
    }

    #[test]
    fn rank_local_or_uncaptured_route_is_impossible_to_compile() {
        let batch = ScheduledBatch {
            step_id: 1,
            kind: BatchKind::Verify { depth: 6 },
            graph_id: 7,
            rows: vec![BatchRow {
                request_id: 1,
                prompt_tokens: 0,
            }],
            query_rows: 7,
        };
        let mut graph = entry();
        graph.compatible_dcp_routes = vec![21, 23];
        assert_eq!(
            StepPlanCompiler::new(1, routes()).unwrap().compile(
                &batch,
                &graph,
                SamplingCollective::Greedy,
                1
            ),
            Err(CompileError::Route)
        );
    }
}
