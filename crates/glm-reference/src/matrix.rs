use glm_format::{decode_e2m1, decode_e4m3};
use serde::Serialize;

use crate::{Fc1Error, Route};

pub const DECODE_ROWS: [usize; 8] = [1, 2, 4, 8, 16, 32, 64, 128];
pub const PREFILL_ROWS: [usize; 1] = [256];

pub const ROUTING_CASES: [RoutingCase; 8] = [
    RoutingCase::OneHotExpert0,
    RoutingCase::OneHotExpert255,
    RoutingCase::UniformAllExperts,
    RoutingCase::ZipfSkew,
    RoutingCase::EmptyExperts,
    RoutingCase::DuplicateExpertRejected,
    RoutingCase::RouteSlotPermutation,
    RoutingCase::TailAssignmentCount,
];

pub const NUMERICAL_CASES: [NumericalCase; 8] = [
    NumericalCase::AllZero,
    NumericalCase::AllE2m1Codes,
    NumericalCase::AllE4m3FiniteScaleClasses,
    NumericalCase::Bf16Extrema,
    NumericalCase::SubnormalBlockScale,
    NumericalCase::SingleOutlierPerBlock,
    NumericalCase::AlternatingSign,
    NumericalCase::DeterministicRandom,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RoutingCase {
    OneHotExpert0,
    OneHotExpert255,
    UniformAllExperts,
    ZipfSkew,
    EmptyExperts,
    DuplicateExpertRejected,
    RouteSlotPermutation,
    TailAssignmentCount,
}

impl RoutingCase {
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::OneHotExpert0 => "one-hot-expert-0",
            Self::OneHotExpert255 => "one-hot-expert-255",
            Self::UniformAllExperts => "uniform-all-experts",
            Self::ZipfSkew => "zipf-skew",
            Self::EmptyExperts => "empty-experts",
            Self::DuplicateExpertRejected => "same-token-duplicate-expert-rejected",
            Self::RouteSlotPermutation => "route-slot-permutation",
            Self::TailAssignmentCount => "tail-assignment-count",
        }
    }

    #[must_use]
    pub const fn expects_rejection(self) -> bool {
        matches!(self, Self::DuplicateExpertRejected)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum NumericalCase {
    AllZero,
    AllE2m1Codes,
    AllE4m3FiniteScaleClasses,
    Bf16Extrema,
    SubnormalBlockScale,
    SingleOutlierPerBlock,
    AlternatingSign,
    DeterministicRandom,
}

impl NumericalCase {
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::AllZero => "all-zero",
            Self::AllE2m1Codes => "all-e2m1-codes",
            Self::AllE4m3FiniteScaleClasses => "all-e4m3-finite-scale-classes",
            Self::Bf16Extrema => "bf16-extrema",
            Self::SubnormalBlockScale => "subnormal-block-scale",
            Self::SingleOutlierPerBlock => "single-outlier-per-block",
            Self::AlternatingSign => "alternating-sign",
            Self::DeterministicRandom => "deterministic-random-v1",
        }
    }
}

#[derive(Clone, Debug)]
pub struct NumericalFixture {
    pub activations: Vec<f32>,
    pub weights: Vec<f32>,
}

pub fn generate_routes(case: RoutingCase, token_count: usize) -> Result<Vec<Route>, Fc1Error> {
    if token_count == 0 || token_count > u32::MAX as usize {
        return Err(Fc1Error::Route);
    }
    let mut routes = Vec::with_capacity(token_count.checked_mul(8).ok_or(Fc1Error::Overflow)?);
    match case {
        RoutingCase::OneHotExpert0 | RoutingCase::OneHotExpert255 => {
            let expert = if case == RoutingCase::OneHotExpert0 {
                0
            } else {
                255
            };
            for token in 0..token_count {
                routes.push(route(token, expert, 0)?);
            }
        }
        RoutingCase::UniformAllExperts => {
            for token in 0..token_count {
                for slot in 0..8 {
                    routes.push(route(token, (token * 8 + slot) % 256, slot)?);
                }
            }
        }
        RoutingCase::ZipfSkew => {
            for token in 0..token_count {
                routes.push(route(token, 0, 0)?);
                for slot in 1..8 {
                    let expert = 1 + (token * 7 + (slot - 1) * 31) % 255;
                    routes.push(route(token, expert, slot)?);
                }
            }
        }
        RoutingCase::EmptyExperts => {
            const USED: [usize; 8] = [0, 17, 34, 68, 85, 119, 170, 255];
            for token in 0..token_count {
                for (slot, &expert) in USED.iter().enumerate() {
                    routes.push(route(token, expert, slot)?);
                }
            }
        }
        RoutingCase::DuplicateExpertRejected => {
            routes.push(route(0, 7, 0)?);
            routes.push(route(0, 7, 1)?);
        }
        RoutingCase::RouteSlotPermutation => {
            for token in 0..token_count {
                for slot in (0..8).rev() {
                    routes.push(route(token, (token * 8 + slot) % 256, slot)?);
                }
            }
            routes.reverse();
        }
        RoutingCase::TailAssignmentCount => {
            for token in 0..token_count {
                let slots = if token + 1 == token_count { 3 } else { 8 };
                for slot in 0..slots {
                    routes.push(route(token, (token * 11 + slot * 29) % 256, slot)?);
                }
            }
        }
    }
    Ok(routes)
}

pub fn generate_numerical_fixture(
    case: NumericalCase,
    rows: usize,
    n: usize,
    k: usize,
) -> Result<NumericalFixture, Fc1Error> {
    if rows == 0 || n == 0 || k == 0 {
        return Err(Fc1Error::Shape);
    }
    let activation_elements = rows.checked_mul(k).ok_or(Fc1Error::Overflow)?;
    let weight_elements = n.checked_mul(k).ok_or(Fc1Error::Overflow)?;
    let mut activations = vec![0.0_f32; activation_elements];
    let mut weights = vec![0.0_f32; weight_elements];

    match case {
        NumericalCase::AllZero => {}
        NumericalCase::AllE2m1Codes => {
            fill_with(&mut activations, |index| decode_e2m1((index % 16) as u8));
            fill_with(&mut weights, |index| decode_e2m1((index % 16) as u8));
        }
        NumericalCase::AllE4m3FiniteScaleClasses => {
            fill_with(
                &mut activations,
                |index| {
                    if index & 1 == 0 { 1.0 } else { -1.0 }
                },
            );
            for (group, block) in weights.chunks_mut(16).enumerate() {
                let code = (group % 127) as u8;
                block[0] = 6.0 * decode_e4m3(code);
            }
            weights[0] = 6.0 * 448.0;
        }
        NumericalCase::Bf16Extrema => {
            const BF16_MAX: f32 = f32::from_bits(0x7f7f_0000);
            const BF16_MIN_NORMAL: f32 = f32::from_bits(0x0080_0000);
            const VALUES: [f32; 6] = [
                BF16_MAX,
                -BF16_MAX,
                BF16_MIN_NORMAL,
                -BF16_MIN_NORMAL,
                1.0,
                -1.0,
            ];
            fill_with(&mut activations, |index| VALUES[index % VALUES.len()]);
            let tiny = 2.0_f32.powi(-120);
            fill_with(
                &mut weights,
                |index| if index & 1 == 0 { tiny } else { -tiny },
            );
        }
        NumericalCase::SubnormalBlockScale => {
            fill_with(
                &mut activations,
                |index| {
                    if index & 1 == 0 { 0.5 } else { -0.5 }
                },
            );
            for block in weights.chunks_mut(16) {
                block[0] = 6.0 * decode_e4m3(1);
            }
            weights[0] = 6.0 * 448.0;
        }
        NumericalCase::SingleOutlierPerBlock => {
            fill_with(&mut activations, |index| deterministic_value(index, 0x51));
            for (group, block) in weights.chunks_mut(16).enumerate() {
                for (lane, value) in block.iter_mut().enumerate() {
                    *value = deterministic_value(group * 16 + lane, 0xa7) * 0.125;
                }
                block[group % block.len()] = if group & 1 == 0 { 256.0 } else { -256.0 };
            }
        }
        NumericalCase::AlternatingSign => {
            fill_with(
                &mut activations,
                |index| {
                    if index & 1 == 0 { 1.0 } else { -1.0 }
                },
            );
            fill_with(
                &mut weights,
                |index| {
                    if index & 1 == 0 { -0.75 } else { 0.75 }
                },
            );
        }
        NumericalCase::DeterministicRandom => {
            fill_with(&mut activations, |index| deterministic_value(index, 0x23));
            fill_with(&mut weights, |index| deterministic_value(index, 0xd1));
        }
    }
    Ok(NumericalFixture {
        activations,
        weights,
    })
}

fn route(token: usize, expert: usize, slot: usize) -> Result<Route, Fc1Error> {
    Ok(Route {
        token: u32::try_from(token).map_err(|_| Fc1Error::Route)?,
        expert: u16::try_from(expert).map_err(|_| Fc1Error::Route)?,
        slot: u8::try_from(slot).map_err(|_| Fc1Error::Route)?,
        weight: 1.0,
    })
}

fn fill_with(values: &mut [f32], mut generator: impl FnMut(usize) -> f32) {
    for (index, value) in values.iter_mut().enumerate() {
        *value = generator(index);
    }
}

fn deterministic_value(index: usize, domain: u64) -> f32 {
    let mixed = (index as u64 ^ domain.wrapping_mul(0x9e37_79b9_7f4a_7c15))
        .wrapping_mul(0xbf58_476d_1ce4_e5b9)
        .rotate_left(29);
    let signed = ((mixed >> 40) as i32) - (1 << 23);
    signed as f32 / (1 << 21) as f32
}

#[cfg(test)]
mod tests {
    use glm_format::{Codec, PackedNvfp4};

    use super::*;
    use crate::compact_routes;

    #[test]
    fn every_routing_case_is_deterministic_and_has_expected_outcome() {
        for rows in DECODE_ROWS.into_iter().chain(PREFILL_ROWS) {
            for case in ROUTING_CASES {
                let first = generate_routes(case, rows).unwrap();
                let second = generate_routes(case, rows).unwrap();
                assert_eq!(first, second);
                assert_eq!(
                    compact_routes(&first, rows).is_err(),
                    case.expects_rejection(),
                    "{} M={rows}",
                    case.id()
                );
            }
        }
    }

    #[test]
    fn every_numerical_case_packs_and_reconstructs_finitely() {
        for case in NUMERICAL_CASES {
            let fixture = generate_numerical_fixture(case, 2, 128, 2048).unwrap();
            let packed =
                PackedNvfp4::pack(&fixture.weights, 128, 2048, Codec::OneDimensional).unwrap();
            assert!(
                packed
                    .dequantize()
                    .unwrap()
                    .iter()
                    .all(|value| value.is_finite()),
                "{}",
                case.id()
            );
            assert!(fixture.activations.iter().all(|value| value.is_finite()));
        }
    }

    #[test]
    fn e4m3_case_materializes_every_positive_finite_scale_code() {
        let fixture =
            generate_numerical_fixture(NumericalCase::AllE4m3FiniteScaleClasses, 1, 128, 2048)
                .unwrap();
        let packed = PackedNvfp4::pack(&fixture.weights, 128, 2048, Codec::OneDimensional).unwrap();
        let mut seen = [false; 127];
        for &code in &packed.scales {
            seen[usize::from(code)] = true;
        }
        assert!(seen.into_iter().all(|value| value));
    }
}
