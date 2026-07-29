use std::collections::BTreeSet;
use std::fmt;

use glm_cache::{
    DRAFT_INDEXER_GROUPS, INDEXER_GROUPS, INDEXER_RECORD_BYTES, KV_RECORD_BYTES, TARGET_LAYERS,
};
use serde::{Deserialize, Serialize};

pub const GIB: u64 = 1 << 30;
pub const MIN_LOCAL_CAPACITY_TOKENS: u64 = 262_144;
pub const MIN_ESCROW_BYTES: u64 = GIB;
pub const CAPACITY_EXL3_RANK_WEIGHT_BYTES: u64 = 81_590_319_104;

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

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProfileBudgetGlobalCapacity {
    pub admitted_target_tokens: u64,
    pub dcp_degree: u8,
    pub local_target_committed_slots_per_rank: u64,
    pub mtp_depth_max: u8,
    pub tp_degree: u8,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProfileBudgetTerms {
    pub allocator_padding_bytes: u64,
    pub collective_bytes: u64,
    pub draft_indexer_key_committed_and_slack_bytes: u64,
    pub draft_kv_committed_and_slack_bytes: u64,
    pub emergency_escrow_bytes: u64,
    pub graph_resident_bytes: u64,
    pub maximum_workspace_bytes: u64,
    pub model_metadata_bytes: u64,
    pub module_and_context_bytes: u64,
    pub staging_bytes: u64,
    pub target_draft_indexer_page_table_bytes: u64,
    pub target_indexer_key_committed_and_slack_bytes: u64,
    pub target_kv_committed_and_slack_bytes: u64,
    pub weight_bytes: u64,
}

impl ProfileBudgetTerms {
    fn required(self) -> Result<u64, ProfileBudgetError> {
        [
            self.allocator_padding_bytes,
            self.collective_bytes,
            self.draft_indexer_key_committed_and_slack_bytes,
            self.draft_kv_committed_and_slack_bytes,
            self.emergency_escrow_bytes,
            self.graph_resident_bytes,
            self.maximum_workspace_bytes,
            self.model_metadata_bytes,
            self.module_and_context_bytes,
            self.staging_bytes,
            self.target_draft_indexer_page_table_bytes,
            self.target_indexer_key_committed_and_slack_bytes,
            self.target_kv_committed_and_slack_bytes,
            self.weight_bytes,
        ]
        .into_iter()
        .try_fold(0_u64, |sum, term| {
            sum.checked_add(term).ok_or(ProfileBudgetError::Overflow)
        })
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProfileBudgetRank {
    pub headroom_against_pre_context_observation_bytes: u64,
    pub measured_post_context_usable_bytes: Option<u64>,
    pub observed_pre_context_free_bytes: u64,
    pub planned_usable_hbm_floor_bytes: u64,
    pub rank: u8,
    pub required_bytes: u64,
    pub terms: ProfileBudgetTerms,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProfileBudgetSource {
    pub hardware: String,
    pub inventory_command: String,
    pub inventory_date: String,
    pub inventory_host: String,
    pub weight_contract: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProfileBudgetArtifact {
    pub conversion_allowed: bool,
    pub global_capacity: ProfileBudgetGlobalCapacity,
    pub measurement_status: String,
    pub profile: String,
    pub ranks: Vec<ProfileBudgetRank>,
    pub schema: String,
    pub source: ProfileBudgetSource,
    pub unmeasured_blockers: Vec<String>,
}

impl ProfileBudgetArtifact {
    pub fn validate(&self) -> Result<(), ProfileBudgetError> {
        if self.schema != "glmaxx.profile-budget.v0"
            || self.profile != "capacity-exl3"
            || self.global_capacity.admitted_target_tokens != 1_048_576
            || self.global_capacity.dcp_degree != 4
            || self.global_capacity.local_target_committed_slots_per_rank != 262_144
            || self.global_capacity.mtp_depth_max != 6
            || self.global_capacity.tp_degree != 4
        {
            return Err(ProfileBudgetError::Identity);
        }
        if self.ranks.len() != 4 {
            return Err(ProfileBudgetError::RankSet);
        }
        let mut ranks = self.ranks.iter().collect::<Vec<_>>();
        ranks.sort_by_key(|rank| rank.rank);
        if ranks
            .iter()
            .enumerate()
            .any(|(expected, rank)| usize::from(rank.rank) != expected)
        {
            return Err(ProfileBudgetError::RankSet);
        }
        let mut minimum_floor = u64::MAX;
        for rank in ranks {
            let terms = rank.terms;
            if terms.weight_bytes != CAPACITY_EXL3_RANK_WEIGHT_BYTES
                || terms.emergency_escrow_bytes < MIN_ESCROW_BYTES
                || terms.target_kv_committed_and_slack_bytes != 7_524_581_376
                || terms.target_indexer_key_committed_and_slack_bytes != 726_663_168
                || terms.draft_kv_committed_and_slack_bytes != 96_633_856
                || terms.draft_indexer_key_committed_and_slack_bytes != 34_662_144
                || terms.required()? != rank.required_bytes
                || rank
                    .observed_pre_context_free_bytes
                    .checked_sub(rank.required_bytes)
                    != Some(rank.headroom_against_pre_context_observation_bytes)
                || rank.planned_usable_hbm_floor_bytes > rank.observed_pre_context_free_bytes
            {
                return Err(ProfileBudgetError::Arithmetic(rank.rank));
            }
            minimum_floor = minimum_floor.min(rank.planned_usable_hbm_floor_bytes);
        }
        if self
            .ranks
            .iter()
            .any(|rank| rank.planned_usable_hbm_floor_bytes != minimum_floor)
        {
            return Err(ProfileBudgetError::Floor);
        }

        match self.measurement_status.as_str() {
            "pending-reviewed-sm120-post-context-measurements" => {
                if self.conversion_allowed
                    || self.unmeasured_blockers.is_empty()
                    || self
                        .ranks
                        .iter()
                        .any(|rank| rank.measured_post_context_usable_bytes.is_some())
                {
                    return Err(ProfileBudgetError::Status);
                }
            }
            "complete" => {
                if !self.conversion_allowed || !self.unmeasured_blockers.is_empty() {
                    return Err(ProfileBudgetError::Status);
                }
                let measured_floor = self
                    .ranks
                    .iter()
                    .map(|rank| {
                        rank.measured_post_context_usable_bytes
                            .ok_or(ProfileBudgetError::Status)
                    })
                    .collect::<Result<Vec<_>, _>>()?
                    .into_iter()
                    .min()
                    .ok_or(ProfileBudgetError::RankSet)?;
                if measured_floor != minimum_floor
                    || self.ranks.iter().any(|rank| {
                        rank.measured_post_context_usable_bytes
                            .is_none_or(|available| rank.required_bytes > available)
                    })
                {
                    return Err(ProfileBudgetError::DoesNotFit);
                }
            }
            _ => return Err(ProfileBudgetError::Status),
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProfileBudgetError {
    Identity,
    RankSet,
    Arithmetic(u8),
    Floor,
    Status,
    DoesNotFit,
    Overflow,
}

impl fmt::Display for ProfileBudgetError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for ProfileBudgetError {}

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

    #[test]
    fn checked_in_profile_budget_is_arithmetically_valid_but_blocks_conversion() {
        let artifact: ProfileBudgetArtifact =
            serde_json::from_str(include_str!("../../../profiles/profile-budget-v0.json")).unwrap();
        artifact.validate().unwrap();
        assert!(!artifact.conversion_allowed);
        assert_eq!(
            artifact.measurement_status,
            "pending-reviewed-sm120-post-context-measurements"
        );
    }

    #[test]
    fn profile_budget_rejects_hidden_terms_and_false_completion() {
        let mut artifact: ProfileBudgetArtifact =
            serde_json::from_str(include_str!("../../../profiles/profile-budget-v0.json")).unwrap();
        artifact.ranks[2].terms.staging_bytes += 1;
        assert_eq!(artifact.validate(), Err(ProfileBudgetError::Arithmetic(2)));

        let mut artifact: ProfileBudgetArtifact =
            serde_json::from_str(include_str!("../../../profiles/profile-budget-v0.json")).unwrap();
        artifact.measurement_status = "complete".into();
        artifact.conversion_allowed = true;
        assert_eq!(artifact.validate(), Err(ProfileBudgetError::Status));
    }

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
