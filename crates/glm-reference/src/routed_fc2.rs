use std::{collections::BTreeMap, fmt};

use glm_format::{Codec, PackedNvfp4};
use sha2::{Digest, Sha256};

use crate::{Route, compact_routes};

const LAYER_DESCRIPTOR_DOMAIN: &[u8] = b"glmaxx.sparse-layer-descriptor.v1\0";

#[derive(Clone, Debug, PartialEq)]
pub struct RoutedExpertWeights {
    pub expert: u16,
    /// Rank-local down projection `[hidden, local_intermediate]`.
    pub down: PackedNvfp4,
}

/// Executes rank-local FC2 with FP32 accumulation and applies the route
/// weight only after the down projection. `activated` is ordered exactly like
/// `compact_routes(routes)`.
pub fn routed_fc2_oracle(
    activated: &[f32],
    routes: &[Route],
    token_count: usize,
    local_intermediate: usize,
    hidden: usize,
    weights: &[RoutedExpertWeights],
) -> Result<Vec<f32>, Fc2Error> {
    let compacted = compact_routes(routes, token_count).map_err(|_| Fc2Error::Route)?;
    if activated.len()
        != compacted
            .len()
            .checked_mul(local_intermediate)
            .ok_or(Fc2Error::Overflow)?
        || hidden == 0
        || local_intermediate == 0
    {
        return Err(Fc2Error::Shape);
    }
    let route_weights: BTreeMap<_, _> = routes
        .iter()
        .map(|route| ((route.token, route.expert, route.slot), route.weight))
        .collect();
    if route_weights.len() != routes.len() {
        return Err(Fc2Error::Route);
    }

    let mut dequant = BTreeMap::new();
    for expert in weights {
        if expert.expert >= 256
            || expert.down.metadata.codec != Codec::OneDimensional
            || expert.down.metadata.logical_n as usize != hidden
            || expert.down.metadata.logical_k as usize != local_intermediate
            || dequant.contains_key(&expert.expert)
        {
            return Err(Fc2Error::Weights);
        }
        dequant.insert(
            expert.expert,
            expert.down.dequantize().map_err(Fc2Error::Nvfp4)?,
        );
    }
    let mut output = vec![0.0_f32; token_count.checked_mul(hidden).ok_or(Fc2Error::Overflow)?];
    for (assignment, compacted_route) in compacted.iter().enumerate() {
        let route_weight = *route_weights
            .get(&(
                compacted_route.token,
                compacted_route.expert,
                compacted_route.slot,
            ))
            .ok_or(Fc2Error::Route)?;
        let down = dequant
            .get(&compacted_route.expert)
            .ok_or(Fc2Error::Weights)?;
        let source =
            &activated[assignment * local_intermediate..(assignment + 1) * local_intermediate];
        let packed_source = PackedNvfp4::pack(source, 1, local_intermediate, Codec::OneDimensional)
            .map_err(Fc2Error::Nvfp4)?;
        let source = packed_source.dequantize().map_err(Fc2Error::Nvfp4)?;
        let token = compacted_route.token as usize;
        for column in 0..hidden {
            let mut accumulator = 0.0_f32;
            for inner in 0..local_intermediate {
                accumulator += source[inner] * down[column * local_intermediate + inner];
            }
            output[token * hidden + column] += route_weight * accumulator;
        }
    }
    Ok(output)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum LayerOperation {
    InputRmsNorm = 1,
    MlaProjection = 2,
    DcpQueryCandidateLse = 3,
    AttentionOutput = 4,
    AttentionTpReduce = 5,
    AttentionResidual = 6,
    PostAttentionRmsNorm = 7,
    ProtectedRouter = 8,
    StableRouteCompaction = 9,
    RoutedFc1 = 10,
    Swiglu = 11,
    RoutedFc2WeightedScatter = 12,
    SharedExpert = 13,
    MoeTpReduce = 14,
    MoeResidual = 15,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SparseLayerDescriptor {
    pub layer: u16,
    pub draft: bool,
    pub hidden: u32,
    pub attention_heads: u16,
    pub local_attention_heads: u16,
    pub routed_experts: u16,
    pub top_k: u8,
    pub expert_intermediate: u32,
    pub local_intermediate: u32,
    pub tp: u8,
    pub operations: Vec<LayerOperation>,
    pub descriptor_hash: [u8; 32],
}

impl SparseLayerDescriptor {
    pub fn glm52(layer: u16) -> Result<Self, Fc2Error> {
        if !(3..=78).contains(&layer) {
            return Err(Fc2Error::Layer);
        }
        let mut descriptor = Self {
            layer,
            draft: layer == 78,
            hidden: 6_144,
            attention_heads: 64,
            local_attention_heads: 16,
            routed_experts: 256,
            top_k: 8,
            expert_intermediate: 2_048,
            local_intermediate: 512,
            tp: 4,
            operations: vec![
                LayerOperation::InputRmsNorm,
                LayerOperation::MlaProjection,
                LayerOperation::DcpQueryCandidateLse,
                LayerOperation::AttentionOutput,
                LayerOperation::AttentionTpReduce,
                LayerOperation::AttentionResidual,
                LayerOperation::PostAttentionRmsNorm,
                LayerOperation::ProtectedRouter,
                LayerOperation::StableRouteCompaction,
                LayerOperation::RoutedFc1,
                LayerOperation::Swiglu,
                LayerOperation::RoutedFc2WeightedScatter,
                LayerOperation::SharedExpert,
                LayerOperation::MoeTpReduce,
                LayerOperation::MoeResidual,
            ],
            descriptor_hash: [0; 32],
        };
        descriptor.descriptor_hash = descriptor.compute_hash();
        Ok(descriptor)
    }

    pub fn verify(&self) -> Result<(), Fc2Error> {
        let canonical = Self::glm52(self.layer)?;
        if *self == canonical {
            Ok(())
        } else if self.compute_hash() != self.descriptor_hash {
            Err(Fc2Error::DescriptorHash)
        } else {
            Err(Fc2Error::Descriptor)
        }
    }

    fn compute_hash(&self) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(LAYER_DESCRIPTOR_DOMAIN);
        hasher.update(self.layer.to_le_bytes());
        hasher.update([u8::from(self.draft)]);
        hasher.update(self.hidden.to_le_bytes());
        hasher.update(self.attention_heads.to_le_bytes());
        hasher.update(self.local_attention_heads.to_le_bytes());
        hasher.update(self.routed_experts.to_le_bytes());
        hasher.update([self.top_k]);
        hasher.update(self.expert_intermediate.to_le_bytes());
        hasher.update(self.local_intermediate.to_le_bytes());
        hasher.update([self.tp]);
        hasher.update(
            u16::try_from(self.operations.len())
                .unwrap_or(u16::MAX)
                .to_le_bytes(),
        );
        for operation in &self.operations {
            hasher.update([*operation as u8]);
        }
        hasher.finalize().into()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct RankLayerPartial {
    pub rank: u8,
    pub attention_output: Vec<f32>,
    pub routed_output: Vec<f32>,
    pub shared_output: Vec<f32>,
}

/// CPU oracle for the two TP4 reduction/residual boundaries of a sparse
/// layer. The attention, routed-expert, and shared-expert kernels remain
/// independently testable producers; this function fixes their combination
/// order so a one-layer replay has an unambiguous target.
pub fn finish_sparse_layer_oracle(
    input_residual: &[f32],
    rank_partials: Vec<RankLayerPartial>,
    rows: usize,
    hidden: usize,
) -> Result<Vec<f32>, Fc2Error> {
    let elements = rows.checked_mul(hidden).ok_or(Fc2Error::Overflow)?;
    if input_residual.len() != elements || rank_partials.len() != 4 {
        return Err(Fc2Error::Shape);
    }
    let mut ranks = rank_partials;
    ranks.sort_by_key(|partial| partial.rank);
    if ranks.iter().enumerate().any(|(rank, partial)| {
        usize::from(partial.rank) != rank
            || partial.attention_output.len() != elements
            || partial.routed_output.len() != elements
            || partial.shared_output.len() != elements
            || partial
                .attention_output
                .iter()
                .chain(&partial.routed_output)
                .chain(&partial.shared_output)
                .any(|value| !value.is_finite())
    }) || input_residual.iter().any(|value| !value.is_finite())
    {
        return Err(Fc2Error::Shape);
    }
    let mut post_attention = input_residual.to_vec();
    for rank in &ranks {
        for (output, partial) in post_attention.iter_mut().zip(&rank.attention_output) {
            *output += partial;
        }
    }
    let mut output = post_attention;
    for rank in &ranks {
        for ((output, routed), shared) in output
            .iter_mut()
            .zip(&rank.routed_output)
            .zip(&rank.shared_output)
        {
            *output += routed + shared;
        }
    }
    Ok(output)
}

#[derive(Debug, PartialEq)]
pub enum Fc2Error {
    Shape,
    Route,
    Weights,
    Layer,
    Descriptor,
    DescriptorHash,
    Overflow,
    Nvfp4(glm_format::Nvfp4Error),
}

impl fmt::Display for Fc2Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for Fc2Error {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn route_weight_is_applied_after_down_projection_and_scattered() {
        let routes = [
            Route {
                token: 0,
                expert: 1,
                slot: 0,
                weight: 0.25,
            },
            Route {
                token: 0,
                expert: 2,
                slot: 1,
                weight: 0.75,
            },
        ];
        let weight1 =
            PackedNvfp4::pack(&[1.0, 0.0, 0.0, 1.0], 2, 2, Codec::OneDimensional).unwrap();
        let weight2 =
            PackedNvfp4::pack(&[2.0, 0.0, 0.0, 2.0], 2, 2, Codec::OneDimensional).unwrap();
        let weights = [
            RoutedExpertWeights {
                expert: 1,
                down: weight1,
            },
            RoutedExpertWeights {
                expert: 2,
                down: weight2,
            },
        ];
        let activated = [1.0, 2.0, 1.0, 2.0];
        let output = routed_fc2_oracle(&activated, &routes, 1, 2, 2, &weights).unwrap();
        assert!((output[0] - 1.75).abs() < 0.2);
        assert!((output[1] - 3.5).abs() < 0.4);
    }

    #[test]
    fn actual_glm52_target_and_draft_descriptors_are_frozen() {
        for layer in 3..=78 {
            let descriptor = SparseLayerDescriptor::glm52(layer).unwrap();
            descriptor.verify().unwrap();
            assert_eq!(descriptor.local_intermediate, 512);
            assert_eq!(descriptor.local_attention_heads, 16);
            assert_eq!(descriptor.draft, layer == 78);
        }
        assert_eq!(SparseLayerDescriptor::glm52(2), Err(Fc2Error::Layer));
        assert_eq!(SparseLayerDescriptor::glm52(79), Err(Fc2Error::Layer));
    }

    #[test]
    fn actual_tp4_fc2_shape_executes() {
        let hidden = 6_144;
        let local_intermediate = 512;
        let values: Vec<f32> = (0..hidden * local_intermediate)
            .map(|index| ((index * 23 % 251) as f32 - 125.0) / 128.0)
            .collect();
        let down =
            PackedNvfp4::pack(&values, hidden, local_intermediate, Codec::OneDimensional).unwrap();
        let activated: Vec<f32> = (0..local_intermediate)
            .map(|index| ((index * 11 % 127) as f32 - 63.0) / 64.0)
            .collect();
        let output = routed_fc2_oracle(
            &activated,
            &[Route {
                token: 0,
                expert: 255,
                slot: 7,
                weight: 0.625,
            }],
            1,
            local_intermediate,
            hidden,
            &[RoutedExpertWeights { expert: 255, down }],
        )
        .unwrap();
        assert_eq!(output.len(), hidden);
        assert!(output.iter().all(|value| value.is_finite()));
    }

    #[test]
    fn full_layer_boundary_oracle_reduces_in_rank_order() {
        let ranks = (0..4)
            .rev()
            .map(|rank| RankLayerPartial {
                rank,
                attention_output: vec![rank as f32 + 1.0; 4],
                routed_output: vec![10.0 * (rank as f32 + 1.0); 4],
                shared_output: vec![0.5 * (rank as f32 + 1.0); 4],
            })
            .collect();
        let output = finish_sparse_layer_oracle(&[1.0; 4], ranks, 1, 4).unwrap();
        assert_eq!(output, vec![116.0; 4]);
    }

    #[test]
    fn descriptor_tampering_fails_closed() {
        let mut descriptor = SparseLayerDescriptor::glm52(3).unwrap();
        descriptor.hidden = 4096;
        assert_eq!(descriptor.verify(), Err(Fc2Error::DescriptorHash));
    }

    #[test]
    fn compacted_route_type_remains_the_activation_order_contract() {
        let _: Option<crate::CompactedRoute> = None;
    }
}
