use std::collections::BTreeSet;
use std::fmt;

use crate::{IndexerKeyRecord, KvError};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Candidate {
    pub position: u64,
    pub score: f32,
}

pub fn score_indexer_key(
    query: &[f32; 128],
    position: u64,
    key: &IndexerKeyRecord,
) -> Result<Candidate, AttentionError> {
    if query.iter().any(|value| !value.is_finite()) {
        return Err(AttentionError::NonFinite);
    }
    let decoded = key.decode().map_err(AttentionError::Kv)?;
    let score = query
        .iter()
        .zip(decoded)
        .fold(0.0_f32, |sum, (&left, right)| left.mul_add(right, sum));
    if !score.is_finite() {
        return Err(AttentionError::NonFinite);
    }
    Ok(Candidate { position, score })
}

/// Selects candidates by score descending, then logical position ascending.
///
/// Each owner may select its local top-k independently. Applying this function
/// again to the union of those owner results produces the exact global top-k.
pub fn deterministic_top_k(
    mut candidates: Vec<Candidate>,
    k: usize,
) -> Result<Vec<Candidate>, AttentionError> {
    if k == 0 {
        return Err(AttentionError::TopK);
    }
    let mut positions = BTreeSet::new();
    for candidate in &candidates {
        if !candidate.score.is_finite() {
            return Err(AttentionError::NonFinite);
        }
        if !positions.insert(candidate.position) {
            return Err(AttentionError::DuplicatePosition);
        }
    }
    candidates.sort_by(|left, right| {
        right
            .score
            .total_cmp(&left.score)
            .then_with(|| left.position.cmp(&right.position))
    });
    candidates.truncate(k);
    Ok(candidates)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LseState<const D: usize> {
    maximum: f32,
    denominator: f32,
    numerator: [f32; D],
    samples: u64,
}

impl<const D: usize> LseState<D> {
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            maximum: f32::NEG_INFINITY,
            denominator: 0.0,
            numerator: [0.0; D],
            samples: 0,
        }
    }

    pub fn from_samples(samples: &[(f32, [f32; D])]) -> Result<Self, AttentionError> {
        let mut state = Self::empty();
        for &(score, value) in samples {
            state.push(score, value)?;
        }
        Ok(state)
    }

    pub fn push(&mut self, score: f32, value: [f32; D]) -> Result<(), AttentionError> {
        if !score.is_finite() || value.iter().any(|element| !element.is_finite()) {
            return Err(AttentionError::NonFinite);
        }
        if self.samples == 0 {
            self.maximum = score;
            self.denominator = 1.0;
            self.numerator = value;
            self.samples = 1;
            return Ok(());
        }
        let next_maximum = self.maximum.max(score);
        let old_scale = (self.maximum - next_maximum).exp();
        let new_scale = (score - next_maximum).exp();
        self.denominator = self.denominator.mul_add(old_scale, new_scale);
        for (sum, element) in self.numerator.iter_mut().zip(value) {
            *sum = sum.mul_add(old_scale, element * new_scale);
        }
        self.maximum = next_maximum;
        self.samples = self
            .samples
            .checked_add(1)
            .ok_or(AttentionError::Overflow)?;
        Ok(())
    }

    pub fn merge(self, other: Self) -> Result<Self, AttentionError> {
        if self.samples == 0 {
            return Ok(other);
        }
        if other.samples == 0 {
            return Ok(self);
        }
        let maximum = self.maximum.max(other.maximum);
        let left_scale = (self.maximum - maximum).exp();
        let right_scale = (other.maximum - maximum).exp();
        let denominator = self
            .denominator
            .mul_add(left_scale, other.denominator * right_scale);
        let numerator = std::array::from_fn(|index| {
            self.numerator[index].mul_add(left_scale, other.numerator[index] * right_scale)
        });
        let samples = self
            .samples
            .checked_add(other.samples)
            .ok_or(AttentionError::Overflow)?;
        if !denominator.is_finite()
            || denominator <= 0.0
            || numerator.iter().any(|value| !value.is_finite())
        {
            return Err(AttentionError::NonFinite);
        }
        Ok(Self {
            maximum,
            denominator,
            numerator,
            samples,
        })
    }

    pub fn finish(self) -> Result<[f32; D], AttentionError> {
        if self.samples == 0 || !self.denominator.is_finite() || self.denominator <= 0.0 {
            return Err(AttentionError::Empty);
        }
        let result = self.numerator.map(|value| value / self.denominator);
        if result.iter().any(|value| !value.is_finite()) {
            return Err(AttentionError::NonFinite);
        }
        Ok(result)
    }

    #[must_use]
    pub const fn sample_count(&self) -> u64 {
        self.samples
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AttentionError {
    TopK,
    DuplicatePosition,
    NonFinite,
    Empty,
    Overflow,
    Kv(KvError),
}

impl fmt::Display for AttentionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for AttentionError {}

#[cfg(test)]
mod tests {
    use crate::{KvRecord, owner_rank};

    use super::*;

    #[test]
    fn top_k_ties_are_position_stable_and_duplicates_fail_closed() {
        let selected = deterministic_top_k(
            vec![
                Candidate {
                    position: 9,
                    score: 3.0,
                },
                Candidate {
                    position: 2,
                    score: 3.0,
                },
                Candidate {
                    position: 4,
                    score: 7.0,
                },
            ],
            2,
        )
        .unwrap();
        assert_eq!(selected[0].position, 4);
        assert_eq!(selected[1].position, 2);
        assert_eq!(
            deterministic_top_k(
                vec![
                    Candidate {
                        position: 1,
                        score: 0.0,
                    },
                    Candidate {
                        position: 1,
                        score: 1.0,
                    },
                ],
                1
            ),
            Err(AttentionError::DuplicatePosition)
        );
    }

    #[test]
    fn packed_indexer_and_kv_records_survive_distributed_sparse_attention() {
        const POSITIONS: usize = 97;
        const WINNERS: usize = 13;
        const VALUE_DIM: usize = 8;
        let index_query = std::array::from_fn(|lane| ((lane * 17 % 31) as f32 - 15.0) / 19.0);
        let attention_query: [f32; 512] =
            std::array::from_fn(|lane| ((lane * 11 % 43) as f32 - 21.0) / 23.0);
        let rope_query: [f32; 64] =
            std::array::from_fn(|lane| ((lane * 7 % 29) as f32 - 14.0) / 17.0);

        let mut keys = Vec::with_capacity(POSITIONS);
        let mut records = Vec::with_capacity(POSITIONS);
        for position in 0..POSITIONS {
            let key = std::array::from_fn(|lane| {
                (((position + 3) * (lane + 5) * 13 % 257) as f32 - 128.0) / 37.0
            });
            keys.push(IndexerKeyRecord::encode(&key).unwrap());
            let nope = std::array::from_fn(|lane| {
                (((position + 1) * (lane + 7) * 19 % 509) as f32 - 254.0) / 41.0
            });
            let rope = std::array::from_fn(|lane| {
                (((position + 11) * (lane + 2) * 5 % 127) as f32 - 63.0) / 29.0
            });
            records.push(KvRecord::encode(&nope, &rope).unwrap());
        }

        let mut owner_candidates: [Vec<Candidate>; 4] = std::array::from_fn(|_| Vec::new());
        for (position, key) in keys.iter().enumerate() {
            let candidate = score_indexer_key(&index_query, position as u64, key).unwrap();
            owner_candidates[usize::from(owner_rank(position as u64 / 64))].push(candidate);
        }
        let mut exchanged = Vec::new();
        for candidates in owner_candidates {
            exchanged.extend(deterministic_top_k(candidates, WINNERS).unwrap());
        }
        let winners = deterministic_top_k(exchanged, WINNERS).unwrap();
        assert_eq!(winners.len(), WINNERS);

        let mut owner_states: [LseState<VALUE_DIM>; 4] = std::array::from_fn(|_| LseState::empty());
        let mut direct_samples = Vec::with_capacity(WINNERS);
        for candidate in winners {
            let position = candidate.position as usize;
            let (nope, rope) = records[position].decode().unwrap();
            let score = attention_query
                .iter()
                .zip(nope)
                .fold(0.0_f32, |sum, (&query, key)| query.mul_add(key, sum))
                + rope_query
                    .iter()
                    .zip(rope)
                    .fold(0.0_f32, |sum, (&query, key)| query.mul_add(key, sum));
            let value = std::array::from_fn(|lane| nope[lane]);
            owner_states[usize::from(owner_rank(candidate.position / 64))]
                .push(score, value)
                .unwrap();
            direct_samples.push((score, value));
        }
        let merged = owner_states
            .into_iter()
            .try_fold(LseState::empty(), LseState::merge)
            .unwrap();
        assert_eq!(merged.sample_count(), WINNERS as u64);
        let distributed = merged.finish().unwrap();
        let direct = LseState::from_samples(&direct_samples)
            .unwrap()
            .finish()
            .unwrap();
        for (left, right) in distributed.into_iter().zip(direct) {
            assert!((left - right).abs() <= 2.0e-5, "{left} != {right}");
        }
    }

    #[test]
    fn lse_merge_accepts_empty_owners_and_rejects_invalid_samples() {
        let populated = LseState::<2>::from_samples(&[(1.0, [2.0, 4.0])]).unwrap();
        assert_eq!(
            LseState::empty().merge(populated).unwrap().finish(),
            Ok([2.0, 4.0])
        );
        assert_eq!(
            LseState::<2>::from_samples(&[(f32::NAN, [0.0, 0.0])]),
            Err(AttentionError::NonFinite)
        );
        assert_eq!(LseState::<2>::empty().finish(), Err(AttentionError::Empty));
    }
}
