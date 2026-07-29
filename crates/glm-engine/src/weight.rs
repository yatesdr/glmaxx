use std::{collections::BTreeSet, fmt};

use serde::Serialize;
use sha2::{Digest, Sha256};

const POLICY_HASH_DOMAIN: &[u8] = b"glmaxx.weight-policy.v1\0";
const PROTECTED_HASH_DOMAIN: &[u8] = b"glmaxx.protected-inventory.v1\0";
const TARGET_SPARSE_LAYERS: std::ops::Range<u16> = 3..78;
const DRAFT_LAYER: u16 = 78;
const ROUTED_EXPERTS: std::ops::Range<u16> = 0..256;

/// One EXL3 projection contains 1,179,648 trellis bytes, 13,312 rotation
/// bytes, a four-byte MCG marker, and the deterministic 96-byte native record.
pub const EXL3_PROJECTION_BYTES: u64 = 1_193_060;
/// One actual-shape rank-local NVFP4 projection contains 3,145,728 values:
/// one nibble/value, one scale byte/16 values, and 128 metadata bytes.
pub const NVFP4_PROJECTION_BYTES: u64 = 1_769_600;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[repr(u8)]
#[serde(rename_all = "kebab-case")]
pub enum WeightProfile {
    CapacityExl3 = 1,
    Nvfp4Laboratory = 2,
    HybridServe = 3,
}

impl WeightProfile {
    const fn serving(self) -> bool {
        matches!(self, Self::CapacityExl3 | Self::HybridServe)
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[repr(u8)]
#[serde(rename_all = "kebab-case")]
pub enum ExpertCodec {
    Exl3 = 1,
    Nvfp4 = 2,
}

impl ExpertCodec {
    #[must_use]
    pub const fn projection_bytes(self) -> u64 {
        match self {
            Self::Exl3 => EXL3_PROJECTION_BYTES,
            Self::Nvfp4 => NVFP4_PROJECTION_BYTES,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[repr(u8)]
#[serde(rename_all = "kebab-case")]
pub enum ExpertTensorRole {
    Gate = 1,
    Up = 2,
    Down = 3,
}

const EXPERT_TENSOR_ROLES: [ExpertTensorRole; 3] = [
    ExpertTensorRole::Gate,
    ExpertTensorRole::Up,
    ExpertTensorRole::Down,
];

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct ExpertKey {
    pub layer: u16,
    pub expert: u16,
    pub role: ExpertTensorRole,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct ExpertAssignment {
    pub key: ExpertKey,
    pub codec: ExpertCodec,
    pub rank_physical_bytes: u64,
    pub quality_evidence_sha256: [u8; 32],
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[repr(u8)]
#[serde(rename_all = "kebab-case")]
pub enum ProtectedPrecision {
    Bf16 = 1,
    Fp8E4m3 = 2,
    Fp32 = 3,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct ProtectedAllocation {
    pub tensor_id: u64,
    pub role_id: u16,
    pub precision: ProtectedPrecision,
    pub rank_physical_bytes: u64,
    pub payload_sha256: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct WeightPolicy {
    pub schema: &'static str,
    pub profile: WeightProfile,
    pub assignments: Vec<ExpertAssignment>,
    pub mtp_enabled: bool,
    pub protected_allocations: Vec<ProtectedAllocation>,
    pub protected_rank_bytes: u64,
    pub protected_inventory_sha256: [u8; 32],
    pub rank_weight_bytes: u64,
    pub rank_weight_budget_bytes: u64,
    pub policy_hash: [u8; 32],
}

impl WeightPolicy {
    /// Builds the immutable full-target policy keyed by
    /// `(layer, expert, tensor_role)`. A selection can never change per
    /// request or step.
    pub fn build_full(
        profile: WeightProfile,
        mut codecs: Vec<(ExpertKey, ExpertCodec, [u8; 32])>,
        mtp_enabled: bool,
        mut protected_allocations: Vec<ProtectedAllocation>,
        rank_weight_budget_bytes: u64,
    ) -> Result<Self, WeightPolicyError> {
        if !profile.serving() {
            return Err(WeightPolicyError::Profile);
        }
        codecs.sort_by_key(|entry| entry.0);
        let layer_count = TARGET_SPARSE_LAYERS.len() + usize::from(mtp_enabled);
        let expected_count = layer_count * ROUTED_EXPERTS.len() * EXPERT_TENSOR_ROLES.len();
        if codecs.len() != expected_count {
            return Err(WeightPolicyError::Inventory);
        }
        let mut seen = BTreeSet::new();
        let mut assignments = Vec::with_capacity(expected_count);
        let mut expert_bytes = 0_u64;
        let mut saw_exl3 = false;
        let mut saw_nvfp4 = false;
        for (key, codec, evidence) in codecs {
            if !(TARGET_SPARSE_LAYERS.contains(&key.layer)
                || (mtp_enabled && key.layer == DRAFT_LAYER))
                || !ROUTED_EXPERTS.contains(&key.expert)
                || !seen.insert(key)
                || evidence == [0; 32]
            {
                return Err(WeightPolicyError::Inventory);
            }
            saw_exl3 |= codec == ExpertCodec::Exl3;
            saw_nvfp4 |= codec == ExpertCodec::Nvfp4;
            let rank_physical_bytes = codec.projection_bytes();
            expert_bytes = expert_bytes
                .checked_add(rank_physical_bytes)
                .ok_or(WeightPolicyError::Overflow)?;
            assignments.push(ExpertAssignment {
                key,
                codec,
                rank_physical_bytes,
                quality_evidence_sha256: evidence,
            });
        }
        match profile {
            WeightProfile::CapacityExl3 if saw_nvfp4 => return Err(WeightPolicyError::Profile),
            WeightProfile::HybridServe if !saw_exl3 || !saw_nvfp4 => {
                return Err(WeightPolicyError::Profile);
            }
            WeightProfile::Nvfp4Laboratory => return Err(WeightPolicyError::Profile),
            _ => {}
        }
        protected_allocations.sort_by_key(|allocation| allocation.tensor_id);
        if protected_allocations.is_empty()
            || protected_allocations.iter().any(|allocation| {
                allocation.tensor_id == 0
                    || allocation.role_id == 0
                    || allocation.rank_physical_bytes == 0
                    || allocation.payload_sha256 == [0; 32]
            })
            || protected_allocations
                .windows(2)
                .any(|window| window[0].tensor_id == window[1].tensor_id)
        {
            return Err(WeightPolicyError::ProtectedInventory);
        }
        let protected_rank_bytes =
            protected_allocations
                .iter()
                .try_fold(0_u64, |sum, allocation| {
                    sum.checked_add(allocation.rank_physical_bytes)
                        .ok_or(WeightPolicyError::Overflow)
                })?;
        let protected_inventory_sha256 = hash_protected(&protected_allocations);
        let rank_weight_bytes = expert_bytes
            .checked_add(protected_rank_bytes)
            .ok_or(WeightPolicyError::Overflow)?;
        if rank_weight_bytes > rank_weight_budget_bytes {
            return Err(WeightPolicyError::DoesNotFit {
                required: rank_weight_bytes,
                budget: rank_weight_budget_bytes,
            });
        }
        let mut policy = Self {
            schema: "glmaxx.weight-policy.v1",
            profile,
            assignments,
            mtp_enabled,
            protected_allocations,
            protected_rank_bytes,
            protected_inventory_sha256,
            rank_weight_bytes,
            rank_weight_budget_bytes,
            policy_hash: [0; 32],
        };
        policy.policy_hash = policy.compute_hash();
        Ok(policy)
    }

    #[must_use]
    pub fn codec_for(&self, key: ExpertKey) -> Option<ExpertCodec> {
        self.assignments
            .binary_search_by_key(&key, |assignment| assignment.key)
            .ok()
            .map(|index| self.assignments[index].codec)
    }

    pub fn verify(&self) -> Result<(), WeightPolicyError> {
        if self.compute_hash() != self.policy_hash {
            return Err(WeightPolicyError::Hash);
        }
        let records = self
            .assignments
            .iter()
            .map(|assignment| {
                (
                    assignment.key,
                    assignment.codec,
                    assignment.quality_evidence_sha256,
                )
            })
            .collect();
        Self::build_full(
            self.profile,
            records,
            self.mtp_enabled,
            self.protected_allocations.clone(),
            self.rank_weight_budget_bytes,
        )
        .map(|_| ())
    }

    fn compute_hash(&self) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(POLICY_HASH_DOMAIN);
        hasher.update([self.profile as u8]);
        hasher.update([u8::from(self.mtp_enabled)]);
        hasher.update(self.protected_rank_bytes.to_le_bytes());
        hasher.update(self.protected_inventory_sha256);
        hasher.update(self.rank_weight_bytes.to_le_bytes());
        hasher.update(self.rank_weight_budget_bytes.to_le_bytes());
        for assignment in &self.assignments {
            hasher.update(assignment.key.layer.to_le_bytes());
            hasher.update(assignment.key.expert.to_le_bytes());
            hasher.update([assignment.key.role as u8]);
            hasher.update([assignment.codec as u8]);
            hasher.update(assignment.rank_physical_bytes.to_le_bytes());
            hasher.update(assignment.quality_evidence_sha256);
        }
        hasher.finalize().into()
    }
}

fn hash_protected(allocations: &[ProtectedAllocation]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(PROTECTED_HASH_DOMAIN);
    hasher.update(
        u32::try_from(allocations.len())
            .unwrap_or(u32::MAX)
            .to_le_bytes(),
    );
    for allocation in allocations {
        hasher.update(allocation.tensor_id.to_le_bytes());
        hasher.update(allocation.role_id.to_le_bytes());
        hasher.update([allocation.precision as u8]);
        hasher.update(allocation.rank_physical_bytes.to_le_bytes());
        hasher.update(allocation.payload_sha256);
    }
    hasher.finalize().into()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WeightPolicyError {
    Profile,
    Inventory,
    ProtectedInventory,
    Overflow,
    DoesNotFit { required: u64, budget: u64 },
    Hash,
}

impl fmt::Display for WeightPolicyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for WeightPolicyError {}

#[cfg(test)]
mod tests {
    use super::*;

    const GIB: u64 = 1 << 30;

    fn protected() -> Vec<ProtectedAllocation> {
        vec![
            ProtectedAllocation {
                tensor_id: 1,
                role_id: 0x0001,
                precision: ProtectedPrecision::Bf16,
                rank_physical_bytes: 6 * GIB,
                payload_sha256: [0xa5; 32],
            },
            ProtectedAllocation {
                tensor_id: 2,
                role_id: 0x0301,
                precision: ProtectedPrecision::Fp32,
                rank_physical_bytes: GIB,
                payload_sha256: [0xb6; 32],
            },
        ]
    }

    fn records(nvfp4_tensors: usize, mtp_enabled: bool) -> Vec<(ExpertKey, ExpertCodec, [u8; 32])> {
        let mut output = Vec::new();
        let mut ordinal = 0;
        for layer in 3..if mtp_enabled { 79 } else { 78 } {
            for expert in ROUTED_EXPERTS {
                for role in EXPERT_TENSOR_ROLES {
                    let codec = if ordinal < nvfp4_tensors {
                        ExpertCodec::Nvfp4
                    } else {
                        ExpertCodec::Exl3
                    };
                    let mut evidence = [0x51; 32];
                    evidence[..8].copy_from_slice(&(ordinal as u64).to_le_bytes());
                    output.push((
                        ExpertKey {
                            layer,
                            expert,
                            role,
                        },
                        codec,
                        evidence,
                    ));
                    ordinal += 1;
                }
            }
        }
        output
    }

    #[test]
    fn capacity_policy_is_complete_hashed_and_order_independent() {
        let forward = records(0, true);
        let mut reverse = forward.clone();
        reverse.reverse();
        let first = WeightPolicy::build_full(
            WeightProfile::CapacityExl3,
            forward,
            true,
            protected(),
            80 * GIB,
        )
        .unwrap();
        let second = WeightPolicy::build_full(
            WeightProfile::CapacityExl3,
            reverse,
            true,
            protected().into_iter().rev().collect(),
            80 * GIB,
        )
        .unwrap();
        assert_eq!(first.policy_hash, second.policy_hash);
        assert_eq!(first.assignments.len(), 76 * 256 * 3);
        assert_eq!(
            first.codec_for(ExpertKey {
                layer: 77,
                expert: 255,
                role: ExpertTensorRole::Down,
            }),
            Some(ExpertCodec::Exl3)
        );
        first.verify().unwrap();
    }

    #[test]
    fn hybrid_is_immutable_and_fails_closed_on_budget() {
        let policy = WeightPolicy::build_full(
            WeightProfile::HybridServe,
            records(128, true),
            true,
            protected(),
            80 * GIB,
        )
        .unwrap();
        assert_eq!(
            policy.codec_for(ExpertKey {
                layer: 3,
                expert: 0,
                role: ExpertTensorRole::Gate,
            }),
            Some(ExpertCodec::Nvfp4)
        );
        assert_eq!(
            policy.codec_for(ExpertKey {
                layer: 77,
                expert: 255,
                role: ExpertTensorRole::Down,
            }),
            Some(ExpertCodec::Exl3)
        );
        assert!(matches!(
            WeightPolicy::build_full(
                WeightProfile::HybridServe,
                records(76 * 256 * 3 - 1, true),
                true,
                protected(),
                90 * GIB,
            ),
            Err(WeightPolicyError::DoesNotFit { .. })
        ));
    }

    #[test]
    fn serving_profiles_cannot_masquerade_as_all_nvfp4() {
        assert_eq!(
            WeightPolicy::build_full(
                WeightProfile::HybridServe,
                records(75 * 256 * 3, false),
                false,
                protected(),
                200 * GIB,
            ),
            Err(WeightPolicyError::Profile)
        );
        assert_eq!(
            WeightPolicy::build_full(
                WeightProfile::CapacityExl3,
                records(1, false),
                false,
                protected(),
                200 * GIB,
            ),
            Err(WeightPolicyError::Profile)
        );
    }
}
