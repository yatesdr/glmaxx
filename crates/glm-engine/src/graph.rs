use std::collections::BTreeSet;
use std::fmt;

use glm_format::KERNEL_ABI;
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::{AttentionTransport, STEP_PLAN_ABI, StepMode, StepPlan};

const GRAPH_PROFILE_DOMAIN: &[u8] = b"glmaxx.graph-profile.v1\0";

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct GraphKey {
    pub mode: StepMode,
    pub sequence_bucket: u16,
    pub verifier_row_bucket: u32,
    pub mtp_depth: u8,
    pub attention_transport: AttentionTransport,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct GraphEntry {
    pub graph_id: u32,
    pub key: GraphKey,
    pub maximum_active_sequences: u16,
    pub maximum_prompt_tokens: u32,
    pub maximum_query_rows: u32,
    pub compatible_tp_routes: Vec<u16>,
    pub compatible_dcp_routes: Vec<u16>,
    pub compatible_sampling_routes: Vec<u16>,
    pub maximum_scratch_bytes: u64,
    pub argument_bytes: u64,
    pub graph_object_bytes: u64,
    pub resident_module_bytes: u64,
    pub admission_slo_class: u16,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct GraphProfile {
    pub schema: &'static str,
    pub step_plan_abi: &'static str,
    pub kernel_abi: &'static str,
    pub entries: Vec<GraphEntry>,
    pub profile_hash: [u8; 32],
}

impl GraphProfile {
    pub fn new(mut entries: Vec<GraphEntry>) -> Result<Self, GraphProfileError> {
        entries.sort_by_key(|entry| entry.graph_id);
        validate_entries(&entries)?;
        let profile_hash = hash_entries(&entries);
        Ok(Self {
            schema: "glmaxx.graph-profile.v1",
            step_plan_abi: STEP_PLAN_ABI,
            kernel_abi: KERNEL_ABI,
            entries,
            profile_hash,
        })
    }

    pub fn verify(&self) -> Result<(), GraphProfileError> {
        if self.schema != "glmaxx.graph-profile.v1"
            || self.step_plan_abi != STEP_PLAN_ABI
            || self.kernel_abi != KERNEL_ABI
        {
            return Err(GraphProfileError::Abi);
        }
        validate_entries(&self.entries)?;
        if hash_entries(&self.entries) != self.profile_hash {
            return Err(GraphProfileError::Hash);
        }
        Ok(())
    }

    pub fn admit(&self, plan: &StepPlan) -> Result<&GraphEntry, GraphProfileError> {
        self.verify()?;
        if plan.mode == StepMode::CacheOnly {
            return Err(GraphProfileError::CacheOnlyHasNoGraph);
        }
        let entry = self
            .entries
            .iter()
            .find(|entry| entry.graph_id == plan.graph_id)
            .ok_or(GraphProfileError::MissingGraph)?;
        let key = GraphKey {
            mode: plan.mode,
            sequence_bucket: plan.sequence_bucket,
            verifier_row_bucket: plan.verifier_row_bucket,
            mtp_depth: plan.mtp_depth,
            attention_transport: plan.attention_transport,
        };
        if entry.key != key
            || plan.active_sequences > entry.maximum_active_sequences
            || plan.scheduled_prompt_tokens > entry.maximum_prompt_tokens
            || plan.query_rows > entry.maximum_query_rows
            || !entry.compatible_tp_routes.contains(&plan.tp_route_id)
            || !entry.compatible_dcp_routes.contains(&plan.dcp_route_id)
            || (plan.sampling_route_id != 0
                && !entry
                    .compatible_sampling_routes
                    .contains(&plan.sampling_route_id))
        {
            return Err(GraphProfileError::Incompatible);
        }
        Ok(entry)
    }
}

fn validate_entries(entries: &[GraphEntry]) -> Result<(), GraphProfileError> {
    if entries.is_empty() {
        return Err(GraphProfileError::Empty);
    }
    let mut graph_ids = BTreeSet::new();
    let mut keys = BTreeSet::new();
    for entry in entries {
        if entry.graph_id == 0
            || !graph_ids.insert(entry.graph_id)
            || !keys.insert(entry.key)
            || entry.maximum_active_sequences == 0
            || entry.maximum_active_sequences > entry.key.sequence_bucket
            || entry.maximum_query_rows == 0
            || entry.admission_slo_class == 0
            || entry.compatible_tp_routes.is_empty()
            || entry.compatible_dcp_routes.is_empty()
            || !strictly_sorted_nonzero(&entry.compatible_tp_routes)
            || !strictly_sorted_nonzero(&entry.compatible_dcp_routes)
            || !strictly_sorted_nonzero(&entry.compatible_sampling_routes)
        {
            return Err(GraphProfileError::Entry);
        }
        if entry.key.mode == StepMode::Prefill {
            if entry.maximum_prompt_tokens == 0
                || entry.key.verifier_row_bucket != 0
                || entry.key.mtp_depth != 0
                || !matches!(
                    entry.key.attention_transport,
                    AttentionTransport::PrefillCkv | AttentionTransport::PrefillQuery
                )
                || !entry.compatible_sampling_routes.is_empty()
            {
                return Err(GraphProfileError::Entry);
            }
        } else if matches!(entry.key.mode, StepMode::Decode | StepMode::Verify) {
            if entry.maximum_prompt_tokens != 0
                || entry.key.attention_transport != AttentionTransport::DecodeQueryLse
                || entry.compatible_sampling_routes.is_empty()
                || (entry.key.mode == StepMode::Decode && entry.key.mtp_depth != 0)
                || (entry.key.mode == StepMode::Verify && entry.key.mtp_depth == 0)
            {
                return Err(GraphProfileError::Entry);
            }
        } else {
            return Err(GraphProfileError::Entry);
        }
    }
    Ok(())
}

fn strictly_sorted_nonzero(values: &[u16]) -> bool {
    values.iter().all(|&value| value != 0) && values.windows(2).all(|window| window[0] < window[1])
}

fn hash_entries(entries: &[GraphEntry]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(GRAPH_PROFILE_DOMAIN);
    hasher.update(Sha256::digest(STEP_PLAN_ABI.as_bytes()));
    hasher.update(Sha256::digest(KERNEL_ABI.as_bytes()));
    hasher.update(
        u32::try_from(entries.len())
            .unwrap_or(u32::MAX)
            .to_le_bytes(),
    );
    for entry in entries {
        hasher.update(entry.graph_id.to_le_bytes());
        hasher.update([entry.key.mode as u8]);
        hasher.update(entry.key.sequence_bucket.to_le_bytes());
        hasher.update(entry.key.verifier_row_bucket.to_le_bytes());
        hasher.update([entry.key.mtp_depth]);
        hasher.update([entry.key.attention_transport as u8]);
        hasher.update(entry.maximum_active_sequences.to_le_bytes());
        hasher.update(entry.maximum_prompt_tokens.to_le_bytes());
        hasher.update(entry.maximum_query_rows.to_le_bytes());
        hash_u16_list(&mut hasher, &entry.compatible_tp_routes);
        hash_u16_list(&mut hasher, &entry.compatible_dcp_routes);
        hash_u16_list(&mut hasher, &entry.compatible_sampling_routes);
        hasher.update(entry.maximum_scratch_bytes.to_le_bytes());
        hasher.update(entry.argument_bytes.to_le_bytes());
        hasher.update(entry.graph_object_bytes.to_le_bytes());
        hasher.update(entry.resident_module_bytes.to_le_bytes());
        hasher.update(entry.admission_slo_class.to_le_bytes());
    }
    hasher.finalize().into()
}

fn hash_u16_list(hasher: &mut Sha256, values: &[u16]) {
    hasher.update(
        u16::try_from(values.len())
            .unwrap_or(u16::MAX)
            .to_le_bytes(),
    );
    for value in values {
        hasher.update(value.to_le_bytes());
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GraphProfileError {
    Empty,
    Abi,
    Entry,
    Hash,
    MissingGraph,
    Incompatible,
    CacheOnlyHasNoGraph,
}

impl fmt::Display for GraphProfileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for GraphProfileError {}

#[cfg(test)]
mod tests {
    use crate::{CollectiveKind, CollectiveOp, CollectiveSchedule, StepPlanRequest, TP_RANK_MASK};

    use super::*;

    fn entry() -> GraphEntry {
        GraphEntry {
            graph_id: 11,
            key: GraphKey {
                mode: StepMode::Decode,
                sequence_bucket: 8,
                verifier_row_bucket: 8,
                mtp_depth: 0,
                attention_transport: AttentionTransport::DecodeQueryLse,
            },
            maximum_active_sequences: 8,
            maximum_prompt_tokens: 0,
            maximum_query_rows: 8,
            compatible_tp_routes: vec![9],
            compatible_dcp_routes: vec![3, 4],
            compatible_sampling_routes: vec![12],
            maximum_scratch_bytes: 64 << 20,
            argument_bytes: 64 << 10,
            graph_object_bytes: 2 << 20,
            resident_module_bytes: 8 << 20,
            admission_slo_class: 1,
        }
    }

    fn plan() -> StepPlan {
        let schedule = CollectiveSchedule::new(vec![CollectiveOp {
            ordinal: 0,
            kind: CollectiveKind::TpReduce,
            route_id: 9,
            payload_bytes: 98_304,
            participant_mask: TP_RANK_MASK,
        }])
        .unwrap();
        StepPlan::build(
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
            },
            &schedule,
        )
        .unwrap()
    }

    #[test]
    fn matching_plan_is_admitted() {
        let profile = GraphProfile::new(vec![entry()]).unwrap();
        assert_eq!(profile.admit(&plan()).unwrap().graph_id, 11);
        assert_eq!(profile.verify(), Ok(()));
    }

    #[test]
    fn missing_or_incompatible_shapes_fail_closed() {
        let mut wrong = entry();
        wrong.maximum_query_rows = 7;
        let profile = GraphProfile::new(vec![wrong]).unwrap();
        assert_eq!(profile.admit(&plan()), Err(GraphProfileError::Incompatible));
    }

    #[test]
    fn duplicate_keys_and_unsorted_routes_are_rejected() {
        let mut duplicate = entry();
        duplicate.graph_id = 12;
        assert_eq!(
            GraphProfile::new(vec![entry(), duplicate]),
            Err(GraphProfileError::Entry)
        );

        let mut unsorted = entry();
        unsorted.compatible_dcp_routes = vec![4, 3];
        assert_eq!(
            GraphProfile::new(vec![unsorted]),
            Err(GraphProfileError::Entry)
        );
    }

    #[test]
    fn profile_bytes_are_order_independent_but_hash_tampering_fails() {
        let mut second = entry();
        second.graph_id = 12;
        second.key.sequence_bucket = 16;
        second.key.verifier_row_bucket = 16;
        second.maximum_active_sequences = 16;
        second.maximum_query_rows = 16;
        let a = GraphProfile::new(vec![entry(), second.clone()]).unwrap();
        let b = GraphProfile::new(vec![second, entry()]).unwrap();
        assert_eq!(a, b);

        let mut corrupt = a;
        corrupt.profile_hash[0] ^= 1;
        assert_eq!(corrupt.verify(), Err(GraphProfileError::Hash));
    }
}
