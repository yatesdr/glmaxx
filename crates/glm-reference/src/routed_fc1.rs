use std::fmt;

use glm_format::{Codec, PackedNvfp4};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Route {
    pub token: u32,
    pub expert: u16,
    pub slot: u8,
    pub weight: f32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompactedRoute {
    pub token: u32,
    pub expert: u16,
    pub slot: u8,
}

pub fn compact_routes(
    routes: &[Route],
    token_count: usize,
) -> Result<Vec<CompactedRoute>, Fc1Error> {
    let mut slot_counts = vec![0_u8; token_count];
    let mut output = Vec::with_capacity(routes.len());
    for route in routes {
        let token = usize::try_from(route.token).map_err(|_| Fc1Error::Route)?;
        if token >= token_count
            || route.expert >= 256
            || route.slot >= 8
            || !route.weight.is_finite()
        {
            return Err(Fc1Error::Route);
        }
        slot_counts[token] = slot_counts[token].checked_add(1).ok_or(Fc1Error::Route)?;
        output.push(CompactedRoute {
            token: route.token,
            expert: route.expert,
            slot: route.slot,
        });
    }
    if slot_counts.iter().any(|&count| count > 8) {
        return Err(Fc1Error::Route);
    }
    output.sort_by_key(|route| (route.expert, route.token, route.slot));
    Ok(output)
}

/// CPU definition of the first rank-local routed FC1.
///
/// `weights` is the direct-packed rank-local `[gate=512; up=512, K=6144]`
/// tensor. Activations are rounded to BF16, dynamically quantized per row,
/// reconstructed, multiplied with FP32 accumulation, and the SwiGLU result is
/// rounded to BF16.
pub fn routed_fc1_oracle(
    activations: &[f32],
    rows: usize,
    k: usize,
    weights: &PackedNvfp4,
) -> Result<Vec<f32>, Fc1Error> {
    let expected = rows.checked_mul(k).ok_or(Fc1Error::Overflow)?;
    if activations.len() != expected
        || weights.metadata.logical_k as usize != k
        || !weights.metadata.logical_n.is_multiple_of(2)
        || weights.metadata.codec != Codec::OneDimensional
    {
        return Err(Fc1Error::Shape);
    }
    let n = weights.metadata.logical_n as usize;
    let local_intermediate = n / 2;
    let dequant_weights = weights.dequantize().map_err(Fc1Error::Nvfp4)?;
    let mut output = vec![0.0_f32; rows * local_intermediate];
    for row in 0..rows {
        let bf16_row: Vec<f32> = activations[row * k..(row + 1) * k]
            .iter()
            .copied()
            .map(bf16_round)
            .collect();
        let packed_activation =
            PackedNvfp4::pack(&bf16_row, 1, k, Codec::OneDimensional).map_err(Fc1Error::Nvfp4)?;
        let activation = packed_activation.dequantize().map_err(Fc1Error::Nvfp4)?;
        for column in 0..local_intermediate {
            let mut gate = 0.0_f32;
            let mut up = 0.0_f32;
            for inner in 0..k {
                gate += activation[inner] * dequant_weights[column * k + inner];
                up +=
                    activation[inner] * dequant_weights[(local_intermediate + column) * k + inner];
            }
            output[row * local_intermediate + column] = bf16_round(silu(gate) * up);
        }
    }
    Ok(output)
}

#[must_use]
pub fn bf16_round(value: f32) -> f32 {
    if !value.is_finite() {
        return value;
    }
    let bits = value.to_bits();
    let bias = 0x7fff + ((bits >> 16) & 1);
    f32::from_bits(bits.wrapping_add(bias) & 0xffff_0000)
}

fn silu(value: f32) -> f32 {
    value / (1.0 + (-value).exp())
}

#[derive(Debug)]
pub enum Fc1Error {
    Shape,
    Route,
    Overflow,
    Nvfp4(glm_format::Nvfp4Error),
}

impl fmt::Display for Fc1Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for Fc1Error {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn route_compaction_is_stable_and_handles_empty_experts() {
        let routes = [
            Route {
                token: 1,
                expert: 255,
                slot: 0,
                weight: 1.0,
            },
            Route {
                token: 0,
                expert: 3,
                slot: 7,
                weight: 1.0,
            },
            Route {
                token: 0,
                expert: 3,
                slot: 0,
                weight: 1.0,
            },
        ];
        let compacted = compact_routes(&routes, 2).unwrap();
        assert_eq!(
            compacted,
            [
                CompactedRoute {
                    token: 0,
                    expert: 3,
                    slot: 0
                },
                CompactedRoute {
                    token: 0,
                    expert: 3,
                    slot: 7
                },
                CompactedRoute {
                    token: 1,
                    expert: 255,
                    slot: 0
                },
            ]
        );
    }

    #[test]
    fn actual_tp4_fc1_shape_executes() {
        let k = 6144;
        let n = 1024;
        let weights: Vec<f32> = (0..n * k)
            .map(|index| ((index * 17 % 257) as f32 - 128.0) / 128.0)
            .collect();
        let packed = PackedNvfp4::pack(&weights, n, k, Codec::OneDimensional).unwrap();
        let activation: Vec<f32> = (0..k)
            .map(|index| ((index * 13 % 127) as f32 - 63.0) / 64.0)
            .collect();
        let output = routed_fc1_oracle(&activation, 1, k, &packed).unwrap();
        assert_eq!(output.len(), 512);
        assert!(output.iter().all(|value| value.is_finite()));
    }
}
