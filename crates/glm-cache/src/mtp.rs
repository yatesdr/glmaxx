use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VerifyOutcome {
    pub accepted_drafts: u8,
    pub emitted_tokens: u8,
    pub committed_position: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SpeculativeTail {
    committed_position: u64,
    depth: u8,
    target_tentative: u8,
    draft_tentative: u8,
    draft_indexer_tentative: u8,
    max_positions: u64,
}

impl SpeculativeTail {
    pub fn begin(committed_position: u64, depth: u8, max_positions: u64) -> Result<Self, MtpError> {
        if depth > 6 || committed_position >= max_positions {
            return Err(MtpError::Bounds);
        }
        let available = max_positions - committed_position;
        let clamped_depth = depth.min(u8::try_from(available.saturating_sub(1)).unwrap_or(u8::MAX));
        Ok(Self {
            committed_position,
            depth: clamped_depth,
            target_tentative: clamped_depth.saturating_add(1),
            draft_tentative: clamped_depth,
            draft_indexer_tentative: clamped_depth,
            max_positions,
        })
    }

    pub fn verify_greedy(self, accepted_drafts: u8) -> Result<VerifyOutcome, MtpError> {
        if accepted_drafts > self.depth {
            return Err(MtpError::Accepted);
        }
        let emitted = if self.depth == 0 || accepted_drafts < self.depth {
            accepted_drafts.saturating_add(1)
        } else {
            self.depth.saturating_add(1)
        };
        let committed_position = self
            .committed_position
            .checked_add(u64::from(emitted))
            .ok_or(MtpError::Bounds)?;
        if committed_position > self.max_positions
            || self.target_tentative != self.depth + 1
            || self.draft_tentative != self.depth
            || self.draft_indexer_tentative != self.depth
        {
            return Err(MtpError::Bounds);
        }
        Ok(VerifyOutcome {
            accepted_drafts,
            emitted_tokens: emitted,
            committed_position,
        })
    }

    #[must_use]
    pub fn depth(&self) -> u8 {
        self.depth
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MtpError {
    Bounds,
    Accepted,
}

impl fmt::Display for MtpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for MtpError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mtp_zero_through_six_commit_and_rollback() {
        for depth in 0..=6 {
            let transaction = SpeculativeTail::begin(100, depth, 1_048_576).unwrap();
            assert_eq!(transaction.depth(), depth);
            for accepted in 0..=depth {
                let outcome = transaction.clone().verify_greedy(accepted).unwrap();
                assert_eq!(
                    outcome.committed_position,
                    100 + u64::from(outcome.emitted_tokens)
                );
                assert!(outcome.emitted_tokens >= 1);
            }
        }
    }

    #[test]
    fn depth_clamps_at_context_limit() {
        let transaction = SpeculativeTail::begin(1_048_574, 6, 1_048_576).unwrap();
        assert_eq!(transaction.depth(), 1);
        assert_eq!(
            transaction.verify_greedy(1).unwrap().committed_position,
            1_048_576
        );
    }
}
