use std::collections::BTreeSet;
use std::fmt;

use glm_cache::{
    DRAFT_INDEXER_GROUPS, INDEXER_GROUPS, INDEXER_RECORD_BYTES, KV_RECORD_BYTES, TARGET_LAYERS,
};
use serde::Serialize;

pub const GIB: u64 = 1 << 30;
pub const MIN_LOCAL_CAPACITY_TOKENS: u64 = 262_144;
pub const MIN_ESCROW_BYTES: u64 = GIB;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProfileClass {
    Nvfp4Laboratory,
    CapacityExl3,
    HybridServe,
}

impl ProfileClass {
    const fn is_serving(self) -> bool {
        matches!(self, Self::CapacityExl3 | Self::HybridServe)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RankMemoryInput {
    pub rank: u8,
    pub profile: ProfileClass,
    pub mtp_enabled: bool,
    pub measured_usable_hbm_bytes: u64,
    pub weight_bytes: u64,
    pub module_and_context_bytes: u64,
    pub graph_resident_bytes: u64,
    pub maximum_prefill_workspace_bytes: u64,
    pub maximum_verifier_workspace_bytes: u64,
    pub collective_bytes: u64,
    pub staging_bytes: u64,
    pub model_metadata_bytes: u64,
    pub page_table_bytes: u64,
    pub allocator_padding_bytes: u64,
    pub escrow_bytes: u64,
    pub target_committed_slots: u64,
    pub target_slack_slots: u64,
    pub draft_committed_slots: u64,
    pub draft_tentative_slots: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct MemoryTerms {
    pub weights: u64,
    pub modules_and_contexts: u64,
    pub graphs: u64,
    pub maximum_workspace: u64,
    pub collectives: u64,
    pub staging: u64,
    pub target_kv: u64,
    pub target_indexer_keys: u64,
    pub draft_kv: u64,
    pub draft_indexer_keys: u64,
    pub model_metadata: u64,
    pub page_tables: u64,
    pub allocator_padding: u64,
    pub escrow: u64,
}

impl MemoryTerms {
    pub fn required(self) -> Result<u64, MemoryPlanError> {
        [
            self.weights,
            self.modules_and_contexts,
            self.graphs,
            self.maximum_workspace,
            self.collectives,
            self.staging,
            self.target_kv,
            self.target_indexer_keys,
            self.draft_kv,
            self.draft_indexer_keys,
            self.model_metadata,
            self.page_tables,
            self.allocator_padding,
            self.escrow,
        ]
        .into_iter()
        .try_fold(0_u64, |sum, term| {
            sum.checked_add(term).ok_or(MemoryPlanError::Overflow)
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct RankMemoryPlan {
    pub rank: u8,
    pub profile: ProfileClass,
    pub mtp_enabled: bool,
    pub measured_usable_hbm_bytes: u64,
    pub target_committed_slots: u64,
    pub target_slack_slots: u64,
    pub target_slots: u64,
    pub draft_committed_slots: u64,
    pub draft_tentative_slots: u64,
    pub draft_slots: u64,
    pub terms: MemoryTerms,
    pub required_bytes: u64,
    pub headroom_bytes: u64,
}

impl RankMemoryPlan {
    pub fn build(input: RankMemoryInput) -> Result<Self, MemoryPlanError> {
        if input.rank >= 4 {
            return Err(MemoryPlanError::Rank);
        }
        let target_slots = input
            .target_committed_slots
            .checked_add(input.target_slack_slots)
            .ok_or(MemoryPlanError::Overflow)?;
        let draft_slots = input
            .draft_committed_slots
            .checked_add(input.draft_tentative_slots)
            .ok_or(MemoryPlanError::Overflow)?;
        if input.profile.is_serving()
            && (input.target_committed_slots < MIN_LOCAL_CAPACITY_TOKENS
                || input.escrow_bytes < MIN_ESCROW_BYTES)
        {
            return Err(MemoryPlanError::ServingFloor);
        }
        if input.mtp_enabled {
            if input.profile.is_serving() && input.draft_committed_slots < MIN_LOCAL_CAPACITY_TOKENS
            {
                return Err(MemoryPlanError::DraftFloor);
            }
        } else if draft_slots != 0 {
            return Err(MemoryPlanError::UnexpectedDraft);
        }

        let terms = MemoryTerms {
            weights: input.weight_bytes,
            modules_and_contexts: input.module_and_context_bytes,
            graphs: input.graph_resident_bytes,
            maximum_workspace: input
                .maximum_prefill_workspace_bytes
                .max(input.maximum_verifier_workspace_bytes),
            collectives: input.collective_bytes,
            staging: input.staging_bytes,
            target_kv: bytes_for(target_slots, TARGET_LAYERS, KV_RECORD_BYTES)?,
            target_indexer_keys: bytes_for(target_slots, INDEXER_GROUPS, INDEXER_RECORD_BYTES)?,
            draft_kv: if input.mtp_enabled {
                bytes_for(draft_slots, 1, KV_RECORD_BYTES)?
            } else {
                0
            },
            draft_indexer_keys: if input.mtp_enabled {
                bytes_for(draft_slots, DRAFT_INDEXER_GROUPS, INDEXER_RECORD_BYTES)?
            } else {
                0
            },
            model_metadata: input.model_metadata_bytes,
            page_tables: input.page_table_bytes,
            allocator_padding: input.allocator_padding_bytes,
            escrow: input.escrow_bytes,
        };
        let required_bytes = terms.required()?;
        let headroom_bytes = input
            .measured_usable_hbm_bytes
            .checked_sub(required_bytes)
            .ok_or(MemoryPlanError::DoesNotFit {
                rank: input.rank,
                required: required_bytes,
                available: input.measured_usable_hbm_bytes,
            })?;
        Ok(Self {
            rank: input.rank,
            profile: input.profile,
            mtp_enabled: input.mtp_enabled,
            measured_usable_hbm_bytes: input.measured_usable_hbm_bytes,
            target_committed_slots: input.target_committed_slots,
            target_slack_slots: input.target_slack_slots,
            target_slots,
            draft_committed_slots: input.draft_committed_slots,
            draft_tentative_slots: input.draft_tentative_slots,
            draft_slots,
            terms,
            required_bytes,
            headroom_bytes,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SystemMemoryPlan {
    pub schema: &'static str,
    pub ranks: Vec<RankMemoryPlan>,
    pub aggregate_required_bytes: u64,
    pub minimum_rank_headroom_bytes: u64,
    pub admitted_local_committed_slots: u64,
}

pub fn plan_system_memory(
    inputs: Vec<RankMemoryInput>,
) -> Result<SystemMemoryPlan, MemoryPlanError> {
    if inputs.len() != 4 {
        return Err(MemoryPlanError::RankCount);
    }
    let mut ranks = inputs
        .into_iter()
        .map(RankMemoryPlan::build)
        .collect::<Result<Vec<_>, _>>()?;
    ranks.sort_by_key(|plan| plan.rank);
    let unique: BTreeSet<u8> = ranks.iter().map(|plan| plan.rank).collect();
    if unique != BTreeSet::from([0, 1, 2, 3]) {
        return Err(MemoryPlanError::RankSet);
    }
    let profile = ranks[0].profile;
    let mtp_enabled = ranks[0].mtp_enabled;
    if ranks
        .iter()
        .any(|plan| plan.profile != profile || plan.mtp_enabled != mtp_enabled)
    {
        return Err(MemoryPlanError::ProfileMismatch);
    }
    let aggregate_required_bytes = ranks.iter().try_fold(0_u64, |sum, plan| {
        sum.checked_add(plan.required_bytes)
            .ok_or(MemoryPlanError::Overflow)
    })?;
    let minimum_rank_headroom_bytes = ranks
        .iter()
        .map(|plan| plan.headroom_bytes)
        .min()
        .ok_or(MemoryPlanError::RankCount)?;
    let admitted_local_committed_slots = ranks
        .iter()
        .map(|plan| plan.target_committed_slots)
        .min()
        .ok_or(MemoryPlanError::RankCount)?;
    Ok(SystemMemoryPlan {
        schema: "glmaxx.system-memory-plan.v1",
        ranks,
        aggregate_required_bytes,
        minimum_rank_headroom_bytes,
        admitted_local_committed_slots,
    })
}

fn bytes_for(slots: u64, groups: u64, bytes: u64) -> Result<u64, MemoryPlanError> {
    slots
        .checked_mul(groups)
        .and_then(|value| value.checked_mul(bytes))
        .ok_or(MemoryPlanError::Overflow)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemoryPlanError {
    Rank,
    RankCount,
    RankSet,
    ProfileMismatch,
    ServingFloor,
    DraftFloor,
    UnexpectedDraft,
    Overflow,
    DoesNotFit {
        rank: u8,
        required: u64,
        available: u64,
    },
}

impl fmt::Display for MemoryPlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for MemoryPlanError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(rank: u8) -> RankMemoryInput {
        RankMemoryInput {
            rank,
            profile: ProfileClass::HybridServe,
            mtp_enabled: true,
            measured_usable_hbm_bytes: 95 * GIB,
            weight_bytes: 82 * GIB,
            module_and_context_bytes: GIB,
            graph_resident_bytes: 256 << 20,
            maximum_prefill_workspace_bytes: 512 << 20,
            maximum_verifier_workspace_bytes: 128 << 20,
            collective_bytes: 256 << 20,
            staging_bytes: 256 << 20,
            model_metadata_bytes: 64 << 20,
            page_table_bytes: 64 << 20,
            allocator_padding_bytes: 256 << 20,
            escrow_bytes: GIB,
            target_committed_slots: MIN_LOCAL_CAPACITY_TOKENS,
            target_slack_slots: 0,
            draft_committed_slots: MIN_LOCAL_CAPACITY_TOKENS,
            draft_tentative_slots: 448,
        }
    }

    #[test]
    fn one_million_mtp_cache_terms_match_the_frozen_arithmetic() {
        let plan = RankMemoryPlan::build(input(0)).unwrap();
        assert_eq!(plan.terms.target_kv, 7_524_581_376);
        assert_eq!(plan.terms.target_indexer_keys, 726_663_168);
        assert_eq!(plan.terms.draft_kv, 96_633_856);
        assert_eq!(plan.terms.draft_indexer_keys, 34_662_144);
    }

    #[test]
    fn workspace_is_the_mutually_exclusive_maximum() {
        let plan = RankMemoryPlan::build(input(0)).unwrap();
        assert_eq!(plan.terms.maximum_workspace, 512 << 20);
    }

    #[test]
    fn every_rank_must_fit_independently() {
        let mut too_small = input(2);
        too_small.measured_usable_hbm_bytes = 90 * GIB;
        let result = plan_system_memory(vec![input(0), input(1), too_small, input(3)]);
        assert!(matches!(
            result,
            Err(MemoryPlanError::DoesNotFit { rank: 2, .. })
        ));
    }

    #[test]
    fn serving_capacity_and_escrow_floors_fail_closed() {
        let mut insufficient = input(0);
        insufficient.target_committed_slots -= 1;
        assert_eq!(
            RankMemoryPlan::build(insufficient),
            Err(MemoryPlanError::ServingFloor)
        );

        let mut no_draft = input(0);
        no_draft.draft_committed_slots = 0;
        assert_eq!(
            RankMemoryPlan::build(no_draft),
            Err(MemoryPlanError::DraftFloor)
        );
    }

    #[test]
    fn system_plan_is_rank_order_independent_and_rejects_mismatch() {
        let plan = plan_system_memory(vec![input(3), input(1), input(0), input(2)]).unwrap();
        assert_eq!(
            plan.ranks.iter().map(|rank| rank.rank).collect::<Vec<_>>(),
            [0, 1, 2, 3]
        );

        let mut mismatch = input(3);
        mismatch.mtp_enabled = false;
        mismatch.draft_committed_slots = 0;
        mismatch.draft_tentative_slots = 0;
        assert_eq!(
            plan_system_memory(vec![input(0), input(1), input(2), mismatch]),
            Err(MemoryPlanError::ProfileMismatch)
        );
    }

    #[test]
    fn overflow_and_unexpected_draft_are_rejected() {
        let mut overflow = input(0);
        overflow.target_slack_slots = u64::MAX;
        assert_eq!(
            RankMemoryPlan::build(overflow),
            Err(MemoryPlanError::Overflow)
        );

        let mut draft = input(0);
        draft.profile = ProfileClass::Nvfp4Laboratory;
        draft.mtp_enabled = false;
        assert_eq!(
            RankMemoryPlan::build(draft),
            Err(MemoryPlanError::UnexpectedDraft)
        );
    }
}
