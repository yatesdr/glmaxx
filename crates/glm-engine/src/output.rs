use sha2::{Digest, Sha256};

use crate::{MAX_ACTIVE_SEQUENCES, MAX_MTP_DEPTH, StepMode, StepPlan};

/// GLM-5.2 has 154,856 tokenizer-defined token IDs. The checkpoint pads the
/// language-model head to 154,880 rows; those final 24 rows must be masked
/// before distributed sampling and may never cross the rank-result boundary.
pub const GLM_52_OUTPUT_VOCABULARY: u32 = 154_856;
pub const MAX_COMMITTED_TOKENS_PER_SEQUENCE: usize = MAX_MTP_DEPTH as usize + 1;

const OUTPUT_HASH_DOMAIN: &[u8] = b"glmaxx.step-output.v1\0";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CommittedTokens {
    count: u8,
    accepted_draft_count: u8,
    target_token_present: bool,
    token_ids: [u32; MAX_COMMITTED_TOKENS_PER_SEQUENCE],
}

impl CommittedTokens {
    const EMPTY: Self = Self {
        count: 0,
        accepted_draft_count: 0,
        target_token_present: false,
        token_ids: [0; MAX_COMMITTED_TOKENS_PER_SEQUENCE],
    };

    pub fn target(token_id: u32) -> Result<Self, OutputError> {
        Self::from_parts(&[], Some(token_id))
    }

    /// Constructs a verifier result from accepted draft tokens followed by an
    /// optional target residual/bonus token. A missing target is legal only
    /// when serving subsequently proves that the final accepted draft token
    /// is EOS.
    pub fn verify(
        accepted_draft_token_ids: &[u32],
        target_token_id: Option<u32>,
    ) -> Result<Self, OutputError> {
        Self::from_parts(accepted_draft_token_ids, target_token_id)
    }

    fn from_parts(
        accepted_draft_token_ids: &[u32],
        target_token_id: Option<u32>,
    ) -> Result<Self, OutputError> {
        let count = accepted_draft_token_ids.len() + usize::from(target_token_id.is_some());
        if count == 0 || count > MAX_COMMITTED_TOKENS_PER_SEQUENCE {
            return Err(OutputError::CommitCount);
        }
        if accepted_draft_token_ids
            .iter()
            .copied()
            .chain(target_token_id)
            .any(|token_id| token_id >= GLM_52_OUTPUT_VOCABULARY)
        {
            return Err(OutputError::TokenId);
        }
        let mut output = Self::EMPTY;
        output.count = u8::try_from(count).map_err(|_| OutputError::CommitCount)?;
        output.accepted_draft_count =
            u8::try_from(accepted_draft_token_ids.len()).map_err(|_| OutputError::CommitCount)?;
        output.target_token_present = target_token_id.is_some();
        output.token_ids[..accepted_draft_token_ids.len()]
            .copy_from_slice(accepted_draft_token_ids);
        if let Some(target_token_id) = target_token_id {
            output.token_ids[accepted_draft_token_ids.len()] = target_token_id;
        }
        Ok(output)
    }

    #[must_use]
    pub const fn count(self) -> u8 {
        self.count
    }

    #[must_use]
    pub const fn accepted_draft_count(self) -> u8 {
        self.accepted_draft_count
    }

    #[must_use]
    pub const fn target_token_present(self) -> bool {
        self.target_token_present
    }

    #[must_use]
    pub fn token_ids(&self) -> &[u32] {
        &self.token_ids[..usize::from(self.count)]
    }
}

/// Bounded, allocation-free output of one TP4 execution step.
///
/// Rows are in the exact sequence-table order represented by the `StepPlan`.
/// Every rank returns the same logical record after distributed sampling; the
/// worker pool compares its canonical digest and the record itself before the
/// serving coordinator is allowed to consume it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StepOutput {
    sequence_count: u16,
    sequences: [CommittedTokens; MAX_ACTIVE_SEQUENCES as usize],
}

impl StepOutput {
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            sequence_count: 0,
            sequences: [CommittedTokens::EMPTY; MAX_ACTIVE_SEQUENCES as usize],
        }
    }

    pub fn new(sequences: &[CommittedTokens]) -> Result<Self, OutputError> {
        if sequences.is_empty() || sequences.len() > usize::from(MAX_ACTIVE_SEQUENCES) {
            return Err(OutputError::SequenceCount);
        }
        let mut output = Self::empty();
        output.sequence_count =
            u16::try_from(sequences.len()).map_err(|_| OutputError::SequenceCount)?;
        output.sequences[..sequences.len()].copy_from_slice(sequences);
        Ok(output)
    }

    #[must_use]
    pub const fn sequence_count(&self) -> u16 {
        self.sequence_count
    }

    #[must_use]
    pub fn sequences(&self) -> &[CommittedTokens] {
        &self.sequences[..usize::from(self.sequence_count)]
    }

    #[must_use]
    pub fn canonical_digest(&self) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(OUTPUT_HASH_DOMAIN);
        hasher.update(self.sequence_count.to_le_bytes());
        for sequence in self.sequences() {
            hasher.update([sequence.count]);
            hasher.update([sequence.accepted_draft_count]);
            hasher.update([u8::from(sequence.target_token_present)]);
            for token_id in sequence.token_ids() {
                hasher.update(token_id.to_le_bytes());
            }
        }
        hasher.finalize().into()
    }

    pub fn validate(&self, plan: &StepPlan) -> Result<(), OutputError> {
        let expected_sequences = match plan.mode {
            StepMode::Prefill | StepMode::CacheOnly => 0,
            StepMode::Decode | StepMode::Verify => plan.active_sequences,
            StepMode::Mixed => return Err(OutputError::Mode),
        };
        if self.sequence_count != expected_sequences {
            return Err(OutputError::SequenceCount);
        }
        for sequence in self.sequences() {
            let valid_count = match plan.mode {
                StepMode::Decode => {
                    sequence.count == 1
                        && sequence.accepted_draft_count == 0
                        && sequence.target_token_present
                }
                StepMode::Verify => {
                    sequence.count != 0
                        && sequence.count <= plan.mtp_depth.saturating_add(1)
                        && sequence.accepted_draft_count <= plan.mtp_depth
                        && sequence.count
                            == sequence
                                .accepted_draft_count
                                .saturating_add(u8::from(sequence.target_token_present))
                }
                StepMode::Prefill | StepMode::CacheOnly | StepMode::Mixed => false,
            };
            if !valid_count {
                return Err(OutputError::CommitCount);
            }
            if sequence
                .token_ids()
                .iter()
                .any(|&token_id| token_id >= GLM_52_OUTPUT_VOCABULARY)
            {
                return Err(OutputError::TokenId);
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OutputError {
    Mode,
    SequenceCount,
    CommitCount,
    TokenId,
}

#[cfg(test)]
mod tests {
    use crate::{
        AttentionTransport, CollectiveKind, CollectiveOp, CollectiveSchedule, StepPlanRequest,
        TP_RANK_MASK,
    };

    use super::*;

    fn plan(mode: StepMode, active_sequences: u16, depth: u8) -> StepPlan {
        let query_rows = match mode {
            StepMode::Prefill => 64,
            StepMode::Decode => u32::from(active_sequences),
            StepMode::Verify => u32::from(active_sequences) * (u32::from(depth) + 1),
            StepMode::CacheOnly | StepMode::Mixed => unreachable!(),
        };
        let schedule = CollectiveSchedule::new(vec![CollectiveOp {
            ordinal: 0,
            kind: CollectiveKind::TpReduce,
            route_id: 1,
            payload_bytes: 16,
            participant_mask: TP_RANK_MASK,
        }])
        .unwrap();
        StepPlan::build(
            StepPlanRequest {
                epoch: 1,
                step_id: 1,
                mode,
                active_sequences,
                sequence_bucket: active_sequences,
                scheduled_prompt_tokens: if mode == StepMode::Prefill { 64 } else { 0 },
                query_rows,
                verifier_row_bucket: if mode == StepMode::Prefill {
                    0
                } else {
                    query_rows
                },
                mtp_depth: depth,
                graph_id: 1,
                tp_route_id: 1,
                dcp_route_id: 1,
                attention_transport: if mode == StepMode::Prefill {
                    AttentionTransport::PrefillQuery
                } else {
                    AttentionTransport::DecodeQueryLse
                },
                sampling_route_id: if mode == StepMode::Prefill { 0 } else { 1 },
                sequence_table_generation: 1,
            },
            &schedule,
        )
        .unwrap()
    }

    #[test]
    fn mode_shape_and_vocabulary_are_fail_closed() {
        let decode = plan(StepMode::Decode, 2, 0);
        let valid = StepOutput::new(&[
            CommittedTokens::target(0).unwrap(),
            CommittedTokens::target(GLM_52_OUTPUT_VOCABULARY - 1).unwrap(),
        ])
        .unwrap();
        assert_eq!(valid.validate(&decode), Ok(()));
        assert_eq!(
            StepOutput::empty().validate(&decode),
            Err(OutputError::SequenceCount)
        );
        assert_eq!(
            CommittedTokens::target(GLM_52_OUTPUT_VOCABULARY),
            Err(OutputError::TokenId)
        );
    }

    #[test]
    fn verify_accepts_one_through_depth_plus_one_tokens() {
        let verify = plan(StepMode::Verify, 2, 6);
        let valid = StepOutput::new(&[
            CommittedTokens::verify(&[], Some(1)).unwrap(),
            CommittedTokens::verify(&[2, 3, 4, 5, 6, 7], Some(8)).unwrap(),
        ])
        .unwrap();
        assert_eq!(valid.validate(&verify), Ok(()));
        assert_ne!(
            valid.canonical_digest(),
            StepOutput::empty().canonical_digest()
        );
    }

    #[test]
    fn prefill_requires_an_empty_output_record() {
        let prefill = plan(StepMode::Prefill, 1, 0);
        assert_eq!(StepOutput::empty().validate(&prefill), Ok(()));
        let token = StepOutput::new(&[CommittedTokens::target(1).unwrap()]).unwrap();
        assert_eq!(token.validate(&prefill), Err(OutputError::SequenceCount));
    }

    #[test]
    fn verifier_can_distinguish_accepted_eos_from_a_target_token() {
        let verify = plan(StepMode::Verify, 1, 6);
        let accepted_only =
            StepOutput::new(&[CommittedTokens::verify(&[10, 11], None).unwrap()]).unwrap();
        assert_eq!(accepted_only.validate(&verify), Ok(()));
        let sequence = accepted_only.sequences()[0];
        assert_eq!(sequence.accepted_draft_count(), 2);
        assert!(!sequence.target_token_present());
    }
}
