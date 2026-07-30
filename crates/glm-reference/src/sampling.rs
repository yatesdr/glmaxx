use std::{cmp::Ordering, fmt};

#[derive(Clone, Debug, PartialEq)]
pub struct LogitShard {
    pub rank: u8,
    pub first_token: u32,
    pub logits: Vec<f32>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProbabilityShard {
    pub rank: u8,
    pub first_token: u32,
    pub target: Vec<f32>,
    pub draft: Vec<f32>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SamplingParams {
    pub temperature: f32,
    pub top_k: u16,
    pub top_p: f64,
    pub seed: u64,
    pub counter: u64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SampleResult {
    pub token: u32,
    pub probability: f64,
    pub exchanged_candidates: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum SamplePurpose {
    Target = 1,
    Draft = 2,
    Acceptance = 3,
    Residual = 4,
    Bonus = 5,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CounterTicket {
    pub request_id: u64,
    pub position: u64,
    pub draft_step: u8,
    pub purpose: SamplePurpose,
    pub counter: u64,
    pub final_counter: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SamplingCounter {
    request_id: u64,
    next: u64,
}

impl SamplingCounter {
    pub fn new(request_id: u64, initial_counter: u64) -> Result<Self, SamplingError> {
        if request_id == 0 {
            return Err(SamplingError::Counter);
        }
        Ok(Self {
            request_id,
            next: initial_counter,
        })
    }

    pub fn allocate(
        &mut self,
        position: u64,
        draft_step: u8,
        purpose: SamplePurpose,
    ) -> Result<CounterTicket, SamplingError> {
        if draft_step > 6 || (matches!(purpose, SamplePurpose::Target) && draft_step != 0) {
            return Err(SamplingError::Counter);
        }
        let counter = self.next;
        self.next = self.next.checked_add(1).ok_or(SamplingError::Overflow)?;
        Ok(CounterTicket {
            request_id: self.request_id,
            position,
            draft_step,
            purpose,
            counter,
            final_counter: self.next,
        })
    }

    #[must_use]
    pub const fn final_counter(self) -> u64 {
        self.next
    }
}

#[derive(Clone, Copy, Debug)]
struct Candidate {
    token: u32,
    logit: f32,
}

pub fn distributed_greedy(shards: &[LogitShard]) -> Result<u32, SamplingError> {
    validate_logit_shards(shards)?;
    let mut best: Option<Candidate> = None;
    for shard in shards {
        let local = shard
            .logits
            .iter()
            .enumerate()
            .map(|(offset, &logit)| Candidate {
                token: shard.first_token + offset as u32,
                logit,
            })
            .max_by(candidate_order)
            .ok_or(SamplingError::Empty)?;
        if best.is_none_or(|current| candidate_order(&local, &current) == Ordering::Greater) {
            best = Some(local);
        }
    }
    let best = best.ok_or(SamplingError::Empty)?;
    if !best.logit.is_finite() {
        return Err(SamplingError::Logit);
    }
    Ok(best.token)
}

/// Exact top-k then top-p sampling without a full-vocabulary logits gather.
///
/// Each rank contributes at most `top_k` `(token, logit)` candidates. The
/// coordinator merges at most `4*top_k` records, truncates to global top-k,
/// applies temperature and top-p, and samples with a counter-based RNG.
pub fn distributed_sample(
    shards: &[LogitShard],
    params: SamplingParams,
) -> Result<SampleResult, SamplingError> {
    validate_logit_shards(shards)?;
    validate_params(params)?;
    if params.top_k == 0 {
        return distributed_unbounded_sample(shards, params);
    }
    let top_k = usize::from(params.top_k);
    let mut exchanged = Vec::with_capacity(shards.len() * top_k);
    for shard in shards {
        let mut local: Vec<_> = shard
            .logits
            .iter()
            .enumerate()
            .map(|(offset, &logit)| Candidate {
                token: shard.first_token + offset as u32,
                logit,
            })
            .collect();
        local.sort_by(|left, right| candidate_order(right, left));
        local.truncate(top_k);
        exchanged.extend(local);
    }
    let exchanged_candidates =
        u16::try_from(exchanged.len()).map_err(|_| SamplingError::Overflow)?;
    exchanged.sort_by(|left, right| candidate_order(right, left));
    exchanged.truncate(top_k);
    sample_candidates(&exchanged, params, exchanged_candidates)
}

/// Samples the exact speculative residual distribution
/// `max(P_target - P_draft, 0)`, falling back to the target distribution when
/// the residual mass is zero. Probability shards use the same TP4 vocabulary
/// partition as normal sampling.
pub fn distributed_residual_sample(
    shards: &[ProbabilityShard],
    seed: u64,
    counter: u64,
) -> Result<SampleResult, SamplingError> {
    validate_probability_shards(shards)?;
    let mut rank_masses = Vec::with_capacity(4);
    let mut residual_mass = 0.0_f32;
    let mut target_mass = 0.0_f32;
    for shard in shards {
        let mut local_residual = 0.0_f32;
        let mut local_target = 0.0_f32;
        for offset in 0..shard.target.len() {
            let target = shard.target[offset];
            let residual = (target - shard.draft[offset]).max(0.0);
            local_residual += residual;
            local_target += target;
        }
        residual_mass += local_residual;
        target_mass += local_target;
        rank_masses.push((local_target, local_residual));
    }
    let use_residual = residual_mass > 0.0;
    let mass = if use_residual {
        residual_mass
    } else {
        target_mass
    };
    if !mass.is_finite() || mass <= 0.0 {
        return Err(SamplingError::Probability);
    }
    let draw = bounded_draw(seed, counter, mass);
    let mut rank_start = 0.0_f32;
    for (shard, &(target_rank_mass, residual_rank_mass)) in shards.iter().zip(&rank_masses) {
        let rank_mass = if use_residual {
            residual_rank_mass
        } else {
            target_rank_mass
        };
        if draw < rank_start + rank_mass {
            let local_draw = draw - rank_start;
            let mut local_cdf = 0.0_f32;
            for offset in 0..shard.target.len() {
                let probability = if use_residual {
                    (shard.target[offset] - shard.draft[offset]).max(0.0)
                } else {
                    shard.target[offset]
                };
                local_cdf += probability;
                if local_draw < local_cdf {
                    return Ok(SampleResult {
                        token: shard.first_token + offset as u32,
                        probability: f64::from(probability / mass),
                        exchanged_candidates: 0,
                    });
                }
            }
        }
        rank_start += rank_mass;
    }
    let shard = shards.last().ok_or(SamplingError::Empty)?;
    let offset = shard.target.len() - 1;
    let probability = if use_residual {
        (shard.target[offset] - shard.draft[offset]).max(0.0)
    } else {
        shard.target[offset]
    };
    Ok(SampleResult {
        token: shard.first_token + offset as u32,
        probability: f64::from(probability / mass),
        exchanged_candidates: 0,
    })
}

fn distributed_unbounded_sample(
    shards: &[LogitShard],
    params: SamplingParams,
) -> Result<SampleResult, SamplingError> {
    let mut maximum = f32::NEG_INFINITY;
    for shard in shards {
        for &logit in &shard.logits {
            maximum = maximum.max(logit);
        }
    }
    if !maximum.is_finite() {
        return Err(SamplingError::Logit);
    }
    let mut rank_masses = Vec::with_capacity(4);
    let mut total_mass = 0.0_f32;
    for shard in shards {
        let mut local_mass = 0.0_f32;
        for &logit in &shard.logits {
            local_mass += ((logit - maximum) / params.temperature).exp();
        }
        rank_masses.push(local_mass);
        total_mass += local_mass;
    }
    if !total_mass.is_finite() || total_mass <= 0.0 {
        return Err(SamplingError::Probability);
    }
    let draw = bounded_draw(params.seed, params.counter, total_mass);
    let mut rank_start = 0.0_f32;
    for (shard, &rank_mass) in shards.iter().zip(&rank_masses) {
        if draw < rank_start + rank_mass {
            let local_draw = draw - rank_start;
            let mut local_cdf = 0.0_f32;
            for (offset, &logit) in shard.logits.iter().enumerate() {
                let probability = ((logit - maximum) / params.temperature).exp();
                local_cdf += probability;
                if local_draw < local_cdf {
                    return Ok(SampleResult {
                        token: shard.first_token + offset as u32,
                        probability: f64::from(probability / total_mass),
                        exchanged_candidates: 0,
                    });
                }
            }
        }
        rank_start += rank_mass;
    }
    Err(SamplingError::Probability)
}

fn sample_candidates(
    candidates: &[Candidate],
    params: SamplingParams,
    exchanged_candidates: u16,
) -> Result<SampleResult, SamplingError> {
    let maximum = candidates.first().ok_or(SamplingError::Empty)?.logit;
    let temperature = params.temperature;
    let mut weighted = Vec::with_capacity(candidates.len());
    let mut total = 0.0_f32;
    for candidate in candidates {
        let weight = ((candidate.logit - maximum) / temperature).exp();
        if !weight.is_finite() {
            return Err(SamplingError::Logit);
        }
        weighted.push((candidate.token, weight));
        total += weight;
    }
    if total <= 0.0 || !total.is_finite() {
        return Err(SamplingError::Probability);
    }
    let nucleus_target = params.top_p as f32 * total;
    let mut kept = 0;
    let mut kept_mass = 0.0_f32;
    while kept < weighted.len() {
        kept_mass += weighted[kept].1;
        kept += 1;
        if kept_mass >= nucleus_target {
            break;
        }
    }
    let draw = bounded_draw(params.seed, params.counter, kept_mass);
    let mut cumulative = 0.0_f32;
    for &(token, weight) in &weighted[..kept] {
        cumulative += weight;
        if draw < cumulative {
            return Ok(SampleResult {
                token,
                probability: f64::from(weight / kept_mass),
                exchanged_candidates,
            });
        }
    }
    let (token, weight) = weighted[kept - 1];
    Ok(SampleResult {
        token,
        probability: f64::from(weight / kept_mass),
        exchanged_candidates,
    })
}

fn validate_logit_shards(shards: &[LogitShard]) -> Result<(), SamplingError> {
    if shards.len() != 4 {
        return Err(SamplingError::RankSet);
    }
    let mut expected_token = 0_u32;
    for (expected_rank, shard) in shards.iter().enumerate() {
        if usize::from(shard.rank) != expected_rank
            || shard.first_token != expected_token
            || shard.logits.is_empty()
            || shard
                .logits
                .iter()
                .any(|value| value.is_nan() || *value == f32::INFINITY)
        {
            return Err(SamplingError::Shard);
        }
        expected_token = expected_token
            .checked_add(u32::try_from(shard.logits.len()).map_err(|_| SamplingError::Overflow)?)
            .ok_or(SamplingError::Overflow)?;
    }
    Ok(())
}

fn validate_probability_shards(shards: &[ProbabilityShard]) -> Result<(), SamplingError> {
    if shards.len() != 4 {
        return Err(SamplingError::RankSet);
    }
    let mut expected_token = 0_u32;
    for (expected_rank, shard) in shards.iter().enumerate() {
        if usize::from(shard.rank) != expected_rank
            || shard.first_token != expected_token
            || shard.target.is_empty()
            || shard.target.len() != shard.draft.len()
            || shard
                .target
                .iter()
                .chain(&shard.draft)
                .any(|value| !value.is_finite() || *value < 0.0)
        {
            return Err(SamplingError::Shard);
        }
        expected_token = expected_token
            .checked_add(u32::try_from(shard.target.len()).map_err(|_| SamplingError::Overflow)?)
            .ok_or(SamplingError::Overflow)?;
    }
    Ok(())
}

fn validate_params(params: SamplingParams) -> Result<(), SamplingError> {
    if !params.temperature.is_finite()
        || params.temperature <= 0.0
        || params.top_k > 256
        || !params.top_p.is_finite()
        || params.top_p <= 0.0
        || params.top_p > 1.0
        || (params.top_k == 0 && params.top_p != 1.0)
    {
        return Err(SamplingError::Parameters);
    }
    Ok(())
}

fn candidate_order(left: &Candidate, right: &Candidate) -> Ordering {
    left.logit
        .total_cmp(&right.logit)
        .then_with(|| right.token.cmp(&left.token))
}

fn counter_uniform(seed: u64, counter: u64) -> f64 {
    let mut value = seed
        .wrapping_add(counter.wrapping_mul(0x9e37_79b9_7f4a_7c15))
        .wrapping_add(0x9e37_79b9_7f4a_7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^= value >> 31;
    (value >> 11) as f64 * (1.0 / ((1_u64 << 53) as f64))
}

fn bounded_draw(seed: u64, counter: u64, mass: f32) -> f32 {
    let draw = (counter_uniform(seed, counter) * f64::from(mass)) as f32;
    if draw < mass {
        draw
    } else {
        f32::from_bits(mass.to_bits() - 1)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SamplingError {
    RankSet,
    Shard,
    Empty,
    Logit,
    Probability,
    Parameters,
    Counter,
    Overflow,
}

impl fmt::Display for SamplingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for SamplingError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn shards(logits: &[f32]) -> Vec<LogitShard> {
        assert!(logits.len().is_multiple_of(4));
        let width = logits.len() / 4;
        (0..4)
            .map(|rank| LogitShard {
                rank: rank as u8,
                first_token: (rank * width) as u32,
                logits: logits[rank * width..(rank + 1) * width].to_vec(),
            })
            .collect()
    }

    fn gathered_control(logits: &[f32], params: SamplingParams) -> SampleResult {
        let candidates: Vec<_> = logits
            .iter()
            .enumerate()
            .map(|(token, &logit)| Candidate {
                token: token as u32,
                logit,
            })
            .collect();
        let mut candidates = candidates;
        candidates.sort_by(|left, right| candidate_order(right, left));
        candidates.truncate(usize::from(params.top_k));
        sample_candidates(&candidates, params, 0).unwrap()
    }

    #[test]
    fn greedy_ties_choose_the_lowest_global_token() {
        let mut logits = vec![0.0; 32];
        logits[7] = 9.0;
        logits[24] = 9.0;
        assert_eq!(distributed_greedy(&shards(&logits)).unwrap(), 7);
    }

    #[test]
    fn greedy_rejects_an_all_masked_row_but_accepts_masked_ranks() {
        let mut logits = vec![f32::NEG_INFINITY; 32];
        logits[17] = -3.0;
        assert_eq!(distributed_greedy(&shards(&logits)).unwrap(), 17);

        logits[17] = f32::NEG_INFINITY;
        assert_eq!(
            distributed_greedy(&shards(&logits)),
            Err(SamplingError::Logit)
        );
    }

    #[test]
    fn sharded_top_k_top_p_matches_gathered_control() {
        let logits: Vec<_> = (0..128)
            .map(|token| ((token * 37 % 101) as f32 - 50.0) / 7.0)
            .collect();
        for counter in 0..32 {
            let params = SamplingParams {
                temperature: 0.7,
                top_k: 31,
                top_p: 0.91,
                seed: 0x2026_0729,
                counter,
            };
            let sharded = distributed_sample(&shards(&logits), params).unwrap();
            let gathered = gathered_control(&logits, params);
            assert_eq!(sharded.token, gathered.token);
            assert_eq!(sharded.probability, gathered.probability);
            assert!(sharded.exchanged_candidates <= 4 * params.top_k);
        }
    }

    #[test]
    fn residual_sampling_matches_known_distribution() {
        let target = [0.1_f32, 0.2, 0.3, 0.4, 0.0, 0.0, 0.0, 0.0];
        let draft = [0.2_f32, 0.1, 0.1, 0.6, 0.0, 0.0, 0.0, 0.0];
        let shards: Vec<_> = (0..4)
            .map(|rank| ProbabilityShard {
                rank: rank as u8,
                first_token: (rank * 2) as u32,
                target: target[rank * 2..rank * 2 + 2].to_vec(),
                draft: draft[rank * 2..rank * 2 + 2].to_vec(),
            })
            .collect();
        for counter in 0..16 {
            let result = distributed_residual_sample(&shards, 7, counter).unwrap();
            assert!(matches!(result.token, 1 | 2));
            assert!(
                (result.probability
                    - if result.token == 1 {
                        f64::from(0.1_f32 / 0.3_f32)
                    } else {
                        f64::from(0.2_f32 / 0.3_f32)
                    })
                .abs()
                    < 1e-6
            );
        }
    }

    #[test]
    fn malformed_rank_partitions_fail_closed() {
        let mut invalid = shards(&[0.0; 16]);
        invalid[2].first_token += 1;
        assert_eq!(distributed_greedy(&invalid), Err(SamplingError::Shard));
    }

    #[test]
    fn unbounded_sampling_uses_distributed_mass_without_candidates() {
        let logits: Vec<_> = (0..64)
            .map(|token| ((token * 19 % 47) as f32 - 20.0) / 5.0)
            .collect();
        let params = SamplingParams {
            temperature: 0.9,
            top_k: 0,
            top_p: 1.0,
            seed: 123,
            counter: 7,
        };
        let result = distributed_sample(&shards(&logits), params).unwrap();
        assert!(result.token < 64);
        assert_eq!(result.exchanged_candidates, 0);
        assert_eq!(
            distributed_sample(
                &shards(&logits),
                SamplingParams {
                    top_p: 0.9,
                    ..params
                }
            ),
            Err(SamplingError::Parameters)
        );
    }

    #[test]
    fn counter_allocation_is_explicit_for_draft_accept_residual_and_bonus() {
        let mut state = SamplingCounter::new(42, 100).unwrap();
        let draft = state.allocate(1_000, 3, SamplePurpose::Draft).unwrap();
        let accept = state.allocate(1_000, 3, SamplePurpose::Acceptance).unwrap();
        let residual = state.allocate(1_000, 3, SamplePurpose::Residual).unwrap();
        let bonus = state.allocate(1_000, 3, SamplePurpose::Bonus).unwrap();
        assert_eq!(
            [
                draft.counter,
                accept.counter,
                residual.counter,
                bonus.counter
            ],
            [100, 101, 102, 103]
        );
        assert_eq!(state.final_counter(), 104);
        assert_eq!(SamplingCounter::new(0, 0), Err(SamplingError::Counter));
    }
}
