//! Deterministic continuous-batching scheduler simulation.
//!
//! The simulator uses the same immutable graph-profile shapes as the runtime.
//! It intentionally emits separate prefill, decode, and verify steps until a
//! mixed-step attention contract has passed review.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};

use glm_engine::{GraphEntry, GraphProfile, StepMode};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TenantConfig {
    pub tenant: u32,
    pub weight: u16,
    pub maximum_active_requests: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RequestSpec {
    pub id: u64,
    pub tenant: u32,
    pub prompt_tokens: u32,
    pub maximum_new_tokens: u32,
    pub mtp_depth: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RequestState {
    WaitingPrefill,
    Decoding,
    Finished,
    Cancelled,
    Failed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BatchKind {
    Prefill,
    Decode,
    Verify { depth: u8 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BatchRow {
    pub request_id: u64,
    pub prompt_tokens: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScheduledBatch {
    pub step_id: u64,
    pub kind: BatchKind,
    pub graph_id: u32,
    pub rows: Vec<BatchRow>,
    pub query_rows: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SchedulerConfig {
    pub maximum_batch_sequences: u16,
    pub maximum_prefill_tokens: u32,
    pub maximum_decode_burst: u16,
}

#[derive(Clone, Debug)]
struct Request {
    spec: RequestSpec,
    state: RequestState,
    prompt_done: u32,
    generated: u32,
}

#[derive(Clone, Debug)]
struct Tenant {
    config: TenantConfig,
    service_units: u64,
}

#[derive(Clone, Debug)]
pub struct Scheduler {
    config: SchedulerConfig,
    profile: GraphProfile,
    tenants: BTreeMap<u32, Tenant>,
    requests: BTreeMap<u64, Request>,
    cancellations: BTreeSet<u64>,
    inflight: Option<ScheduledBatch>,
    next_step_id: u64,
    decode_burst: u16,
}

impl Scheduler {
    pub fn new(
        config: SchedulerConfig,
        profile: GraphProfile,
        tenant_configs: Vec<TenantConfig>,
    ) -> Result<Self, SchedulerError> {
        profile.verify().map_err(|_| SchedulerError::GraphProfile)?;
        if config.maximum_batch_sequences == 0
            || config.maximum_batch_sequences > 64
            || config.maximum_prefill_tokens == 0
            || config.maximum_decode_burst == 0
        {
            return Err(SchedulerError::Config);
        }
        let mut tenants = BTreeMap::new();
        for tenant in tenant_configs {
            if tenant.tenant == 0
                || tenant.weight == 0
                || tenant.maximum_active_requests == 0
                || tenants
                    .insert(
                        tenant.tenant,
                        Tenant {
                            config: tenant,
                            service_units: 0,
                        },
                    )
                    .is_some()
            {
                return Err(SchedulerError::Tenant);
            }
        }
        if tenants.is_empty() {
            return Err(SchedulerError::Tenant);
        }
        Ok(Self {
            config,
            profile,
            tenants,
            requests: BTreeMap::new(),
            cancellations: BTreeSet::new(),
            inflight: None,
            next_step_id: 1,
            decode_burst: 0,
        })
    }

    pub fn admit(&mut self, spec: RequestSpec) -> Result<(), SchedulerError> {
        if spec.id == 0
            || spec.prompt_tokens == 0
            || spec.maximum_new_tokens == 0
            || spec.mtp_depth > 6
            || self.requests.contains_key(&spec.id)
        {
            return Err(SchedulerError::Request);
        }
        let tenant = self
            .tenants
            .get(&spec.tenant)
            .ok_or(SchedulerError::Tenant)?;
        let active = self
            .requests
            .values()
            .filter(|request| {
                request.spec.tenant == spec.tenant
                    && !matches!(
                        request.state,
                        RequestState::Finished | RequestState::Cancelled | RequestState::Failed
                    )
            })
            .count();
        if active >= usize::from(tenant.config.maximum_active_requests) {
            return Err(SchedulerError::TenantLimit);
        }
        if !self.has_graph_for_request(spec) {
            return Err(SchedulerError::UncapturedShape);
        }
        self.requests.insert(
            spec.id,
            Request {
                spec,
                state: RequestState::WaitingPrefill,
                prompt_done: 0,
                generated: 0,
            },
        );
        Ok(())
    }

    pub fn cancel(&mut self, request_id: u64) -> Result<(), SchedulerError> {
        let request = self
            .requests
            .get(&request_id)
            .ok_or(SchedulerError::UnknownRequest)?;
        if matches!(
            request.state,
            RequestState::Finished | RequestState::Cancelled | RequestState::Failed
        ) {
            return Ok(());
        }
        self.cancellations.insert(request_id);
        Ok(())
    }

    /// Selects one captured-graph step. Cancellation is applied before the
    /// selection, which is the simulator's collective-safe step boundary.
    pub fn next_batch(&mut self) -> Result<Option<ScheduledBatch>, SchedulerError> {
        if self.inflight.is_some() {
            return Err(SchedulerError::Inflight);
        }
        self.apply_cancellations();
        let has_prefill = self
            .requests
            .values()
            .any(|request| request.state == RequestState::WaitingPrefill);
        let has_decode = self
            .requests
            .values()
            .any(|request| request.state == RequestState::Decoding);
        if !has_prefill && !has_decode {
            return Ok(None);
        }

        let choose_prefill =
            has_prefill && (!has_decode || self.decode_burst >= self.config.maximum_decode_burst);
        let batch = if choose_prefill {
            self.build_prefill_batch()?
        } else {
            self.build_decode_batch()?
        };
        self.inflight = Some(batch.clone());
        Ok(Some(batch))
    }

    pub fn complete_batch(&mut self, success: bool) -> Result<(), SchedulerError> {
        let batch = self.inflight.take().ok_or(SchedulerError::NoInflight)?;
        for row in &batch.rows {
            let request = self
                .requests
                .get_mut(&row.request_id)
                .ok_or(SchedulerError::UnknownRequest)?;
            if !success {
                request.state = RequestState::Failed;
                continue;
            }
            let tenant = self
                .tenants
                .get_mut(&request.spec.tenant)
                .ok_or(SchedulerError::Tenant)?;
            match batch.kind {
                BatchKind::Prefill => {
                    request.prompt_done = request
                        .prompt_done
                        .checked_add(row.prompt_tokens)
                        .ok_or(SchedulerError::Overflow)?;
                    tenant.service_units = tenant
                        .service_units
                        .checked_add(u64::from(row.prompt_tokens))
                        .ok_or(SchedulerError::Overflow)?;
                    if request.prompt_done == request.spec.prompt_tokens {
                        request.state = RequestState::Decoding;
                    }
                }
                BatchKind::Decode | BatchKind::Verify { .. } => {
                    request.generated = request
                        .generated
                        .checked_add(1)
                        .ok_or(SchedulerError::Overflow)?;
                    tenant.service_units = tenant
                        .service_units
                        .checked_add(1)
                        .ok_or(SchedulerError::Overflow)?;
                    if request.generated == request.spec.maximum_new_tokens {
                        request.state = RequestState::Finished;
                    }
                }
            }
        }
        match batch.kind {
            BatchKind::Prefill => self.decode_burst = 0,
            BatchKind::Decode | BatchKind::Verify { .. } => {
                self.decode_burst = self.decode_burst.saturating_add(1);
            }
        }
        Ok(())
    }

    #[must_use]
    pub fn request_state(&self, request_id: u64) -> Option<RequestState> {
        self.requests.get(&request_id).map(|request| request.state)
    }

    fn build_prefill_batch(&mut self) -> Result<ScheduledBatch, SchedulerError> {
        let mut eligible = self.ordered_requests(RequestState::WaitingPrefill, None);
        eligible.truncate(usize::from(self.config.maximum_batch_sequences));
        let mut rows = Vec::new();
        let mut available = self.config.maximum_prefill_tokens;
        for request_id in eligible {
            if available == 0 {
                break;
            }
            let request = &self.requests[&request_id];
            let remaining = request.spec.prompt_tokens - request.prompt_done;
            let tokens = remaining.min(available);
            if tokens != 0 {
                rows.push(BatchRow {
                    request_id,
                    prompt_tokens: tokens,
                });
                available -= tokens;
            }
        }
        self.finalize_batch(BatchKind::Prefill, rows)
    }

    fn build_decode_batch(&mut self) -> Result<ScheduledBatch, SchedulerError> {
        let first = self
            .ordered_requests(RequestState::Decoding, None)
            .into_iter()
            .next()
            .ok_or(SchedulerError::NoRunnable)?;
        let depth = self.requests[&first].spec.mtp_depth;
        let mut eligible = self.ordered_requests(RequestState::Decoding, Some(depth));
        eligible.truncate(usize::from(self.config.maximum_batch_sequences));
        let kind = if depth == 0 {
            BatchKind::Decode
        } else {
            BatchKind::Verify { depth }
        };
        loop {
            let rows: Vec<_> = eligible
                .iter()
                .map(|&request_id| BatchRow {
                    request_id,
                    prompt_tokens: 0,
                })
                .collect();
            if let Ok(batch) = self.finalize_batch(kind, rows) {
                return Ok(batch);
            }
            eligible.pop();
            if eligible.is_empty() {
                return Err(SchedulerError::UncapturedShape);
            }
        }
    }

    fn finalize_batch(
        &mut self,
        kind: BatchKind,
        rows: Vec<BatchRow>,
    ) -> Result<ScheduledBatch, SchedulerError> {
        if rows.is_empty() {
            return Err(SchedulerError::NoRunnable);
        }
        let active = u16::try_from(rows.len()).map_err(|_| SchedulerError::Overflow)?;
        let query_rows = match kind {
            BatchKind::Prefill => rows.iter().try_fold(0_u32, |sum, row| {
                sum.checked_add(row.prompt_tokens)
                    .ok_or(SchedulerError::Overflow)
            })?,
            BatchKind::Decode => u32::from(active),
            BatchKind::Verify { depth } => u32::from(active)
                .checked_mul(u32::from(depth) + 1)
                .ok_or(SchedulerError::Overflow)?,
        };
        let graph = self
            .best_graph(kind, active, query_rows)
            .ok_or(SchedulerError::UncapturedShape)?;
        let batch = ScheduledBatch {
            step_id: self.next_step_id,
            kind,
            graph_id: graph.graph_id,
            rows,
            query_rows,
        };
        self.next_step_id = self
            .next_step_id
            .checked_add(1)
            .ok_or(SchedulerError::Overflow)?;
        Ok(batch)
    }

    fn best_graph(&self, kind: BatchKind, active: u16, query_rows: u32) -> Option<&GraphEntry> {
        let (mode, depth) = match kind {
            BatchKind::Prefill => (StepMode::Prefill, 0),
            BatchKind::Decode => (StepMode::Decode, 0),
            BatchKind::Verify { depth } => (StepMode::Verify, depth),
        };
        self.profile
            .entries
            .iter()
            .filter(|entry| {
                entry.key.mode == mode
                    && entry.key.mtp_depth == depth
                    && entry.maximum_active_sequences >= active
                    && entry.maximum_query_rows >= query_rows
                    && (mode != StepMode::Prefill || entry.maximum_prompt_tokens >= query_rows)
            })
            .min_by_key(|entry| {
                (
                    entry.key.sequence_bucket,
                    entry.key.verifier_row_bucket,
                    entry.maximum_query_rows,
                    entry.graph_id,
                )
            })
    }

    fn ordered_requests(&self, state: RequestState, depth: Option<u8>) -> Vec<u64> {
        let mut ids: Vec<_> = self
            .requests
            .values()
            .filter(|request| {
                request.state == state && depth.is_none_or(|depth| request.spec.mtp_depth == depth)
            })
            .map(|request| request.spec.id)
            .collect();
        ids.sort_by(|left, right| {
            let left_request = &self.requests[left];
            let right_request = &self.requests[right];
            let left_tenant = &self.tenants[&left_request.spec.tenant];
            let right_tenant = &self.tenants[&right_request.spec.tenant];
            // Compare service/weight without floating point.
            let left_score =
                u128::from(left_tenant.service_units) * u128::from(right_tenant.config.weight);
            let right_score =
                u128::from(right_tenant.service_units) * u128::from(left_tenant.config.weight);
            (left_score, left_request.spec.tenant, *left).cmp(&(
                right_score,
                right_request.spec.tenant,
                *right,
            ))
        });
        ids
    }

    fn has_graph_for_request(&self, spec: RequestSpec) -> bool {
        let prefill = self
            .profile
            .entries
            .iter()
            .any(|entry| entry.key.mode == StepMode::Prefill);
        let decode_mode = if spec.mtp_depth == 0 {
            StepMode::Decode
        } else {
            StepMode::Verify
        };
        let decode = self.profile.entries.iter().any(|entry| {
            entry.key.mode == decode_mode
                && entry.key.mtp_depth == spec.mtp_depth
                && entry.maximum_query_rows > u32::from(spec.mtp_depth)
        });
        prefill && decode
    }

    fn apply_cancellations(&mut self) {
        for request_id in std::mem::take(&mut self.cancellations) {
            if let Some(request) = self.requests.get_mut(&request_id)
                && !matches!(request.state, RequestState::Finished | RequestState::Failed)
            {
                request.state = RequestState::Cancelled;
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SchedulerError {
    Config,
    GraphProfile,
    Tenant,
    TenantLimit,
    Request,
    UnknownRequest,
    UncapturedShape,
    NoRunnable,
    Inflight,
    NoInflight,
    Overflow,
}

impl fmt::Display for SchedulerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for SchedulerError {}

#[cfg(test)]
mod tests {
    use glm_engine::{AttentionTransport, GraphEntry, GraphKey};

    use super::*;

    fn entry(
        graph_id: u32,
        mode: StepMode,
        sequence_bucket: u16,
        rows: u32,
        depth: u8,
    ) -> GraphEntry {
        GraphEntry {
            graph_id,
            key: GraphKey {
                mode,
                sequence_bucket,
                verifier_row_bucket: if mode == StepMode::Prefill { 0 } else { rows },
                mtp_depth: depth,
                attention_transport: if mode == StepMode::Prefill {
                    AttentionTransport::PrefillQuery
                } else {
                    AttentionTransport::DecodeQueryLse
                },
            },
            maximum_active_sequences: sequence_bucket,
            maximum_prompt_tokens: if mode == StepMode::Prefill { rows } else { 0 },
            maximum_query_rows: rows,
            compatible_tp_routes: vec![1],
            compatible_dcp_routes: vec![1],
            compatible_sampling_routes: if mode == StepMode::Prefill {
                vec![]
            } else {
                vec![1]
            },
            maximum_scratch_bytes: 1,
            argument_bytes: 1,
            graph_object_bytes: 1,
            resident_module_bytes: 1,
            admission_slo_class: 1,
        }
    }

    fn scheduler() -> Scheduler {
        let profile = GraphProfile::new(vec![
            entry(1, StepMode::Prefill, 4, 32, 0),
            entry(2, StepMode::Decode, 4, 4, 0),
            entry(3, StepMode::Verify, 4, 28, 6),
        ])
        .unwrap();
        Scheduler::new(
            SchedulerConfig {
                maximum_batch_sequences: 4,
                maximum_prefill_tokens: 32,
                maximum_decode_burst: 2,
            },
            profile,
            vec![
                TenantConfig {
                    tenant: 1,
                    weight: 1,
                    maximum_active_requests: 16,
                },
                TenantConfig {
                    tenant: 2,
                    weight: 2,
                    maximum_active_requests: 16,
                },
            ],
        )
        .unwrap()
    }

    #[test]
    fn continuously_batches_prefill_decode_and_mtp6() {
        let mut scheduler = scheduler();
        scheduler
            .admit(RequestSpec {
                id: 1,
                tenant: 1,
                prompt_tokens: 8,
                maximum_new_tokens: 2,
                mtp_depth: 0,
            })
            .unwrap();
        scheduler
            .admit(RequestSpec {
                id: 2,
                tenant: 2,
                prompt_tokens: 12,
                maximum_new_tokens: 2,
                mtp_depth: 6,
            })
            .unwrap();
        let prefill = scheduler.next_batch().unwrap().unwrap();
        assert_eq!(prefill.kind, BatchKind::Prefill);
        assert_eq!(prefill.rows.len(), 2);
        scheduler.complete_batch(true).unwrap();

        let first_decode = scheduler.next_batch().unwrap().unwrap();
        assert_eq!(first_decode.kind, BatchKind::Verify { depth: 6 });
        assert_eq!(first_decode.query_rows, 7);
        scheduler.complete_batch(true).unwrap();
        let second_decode = scheduler.next_batch().unwrap().unwrap();
        scheduler.complete_batch(true).unwrap();
        let third_decode = scheduler.next_batch().unwrap().unwrap();
        assert_eq!(second_decode.kind, BatchKind::Verify { depth: 6 });
        assert_eq!(third_decode.kind, BatchKind::Decode);
    }

    #[test]
    fn cancellation_waits_for_the_step_boundary() {
        let mut scheduler = scheduler();
        scheduler
            .admit(RequestSpec {
                id: 1,
                tenant: 1,
                prompt_tokens: 8,
                maximum_new_tokens: 8,
                mtp_depth: 0,
            })
            .unwrap();
        scheduler.next_batch().unwrap().unwrap();
        scheduler.cancel(1).unwrap();
        assert_eq!(
            scheduler.request_state(1),
            Some(RequestState::WaitingPrefill)
        );
        scheduler.complete_batch(true).unwrap();
        assert_eq!(scheduler.request_state(1), Some(RequestState::Decoding));
        assert!(scheduler.next_batch().unwrap().is_none());
        assert_eq!(scheduler.request_state(1), Some(RequestState::Cancelled));
    }

    #[test]
    fn uncaptured_mtp_depth_is_rejected_at_admission() {
        let mut scheduler = scheduler();
        assert_eq!(
            scheduler.admit(RequestSpec {
                id: 9,
                tenant: 1,
                prompt_tokens: 1,
                maximum_new_tokens: 1,
                mtp_depth: 3,
            }),
            Err(SchedulerError::UncapturedShape)
        );
    }

    #[test]
    fn decode_burst_bound_admits_waiting_prefill() {
        let mut scheduler = scheduler();
        scheduler
            .admit(RequestSpec {
                id: 1,
                tenant: 1,
                prompt_tokens: 1,
                maximum_new_tokens: 8,
                mtp_depth: 0,
            })
            .unwrap();
        scheduler.next_batch().unwrap();
        scheduler.complete_batch(true).unwrap();
        scheduler.next_batch().unwrap();
        scheduler.complete_batch(true).unwrap();
        scheduler
            .admit(RequestSpec {
                id: 2,
                tenant: 2,
                prompt_tokens: 1,
                maximum_new_tokens: 1,
                mtp_depth: 0,
            })
            .unwrap();
        scheduler.next_batch().unwrap();
        scheduler.complete_batch(true).unwrap();
        let batch = scheduler.next_batch().unwrap().unwrap();
        assert_eq!(batch.kind, BatchKind::Prefill);
        assert_eq!(batch.rows[0].request_id, 2);
    }

    #[test]
    fn weighted_fair_order_does_not_starve_tenants() {
        let mut scheduler = scheduler();
        for (id, tenant) in [(1, 1), (2, 2)] {
            scheduler
                .admit(RequestSpec {
                    id,
                    tenant,
                    prompt_tokens: 1,
                    maximum_new_tokens: 20,
                    mtp_depth: 0,
                })
                .unwrap();
        }
        scheduler.next_batch().unwrap();
        scheduler.complete_batch(true).unwrap();
        // Both fit each decode graph, so fairness includes both whenever both
        // are runnable; neither can be starved by a hot tenant.
        for _ in 0..5 {
            let batch = scheduler.next_batch().unwrap().unwrap();
            assert_eq!(batch.rows.len(), 2);
            scheduler.complete_batch(true).unwrap();
        }
    }
}
