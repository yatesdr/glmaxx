use std::fmt;

use crate::{
    DRAFT_INDEXER_GROUPS, INDEXER_GROUPS, INDEXER_RECORD_BYTES, KV_RECORD_BYTES, MODEL_POSITIONS,
    TARGET_LAYERS,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CacheCapacity {
    pub target_kv_bytes: u64,
    pub draft_kv_bytes: u64,
    pub indexer_key_bytes: u64,
    pub draft_indexer_key_bytes: u64,
}

impl CacheCapacity {
    pub fn at_positions(positions: u64, mtp: bool) -> Result<Self, BudgetError> {
        if positions > MODEL_POSITIONS {
            return Err(BudgetError::Positions);
        }
        Ok(Self {
            target_kv_bytes: positions
                .checked_mul(TARGET_LAYERS)
                .and_then(|value| value.checked_mul(KV_RECORD_BYTES))
                .ok_or(BudgetError::Overflow)?,
            draft_kv_bytes: if mtp {
                positions
                    .checked_mul(KV_RECORD_BYTES)
                    .ok_or(BudgetError::Overflow)?
            } else {
                0
            },
            indexer_key_bytes: positions
                .checked_mul(INDEXER_GROUPS)
                .and_then(|value| value.checked_mul(INDEXER_RECORD_BYTES))
                .ok_or(BudgetError::Overflow)?,
            draft_indexer_key_bytes: if mtp {
                positions
                    .checked_mul(DRAFT_INDEXER_GROUPS)
                    .and_then(|value| value.checked_mul(INDEXER_RECORD_BYTES))
                    .ok_or(BudgetError::Overflow)?
            } else {
                0
            },
        })
    }

    pub fn total(self) -> Result<u64, BudgetError> {
        self.target_kv_bytes
            .checked_add(self.draft_kv_bytes)
            .and_then(|value| value.checked_add(self.indexer_key_bytes))
            .and_then(|value| value.checked_add(self.draft_indexer_key_bytes))
            .ok_or(BudgetError::Overflow)
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Budget {
    pub weights: u64,
    pub modules_and_contexts: u64,
    pub graphs: u64,
    pub workspace: u64,
    pub collectives_and_staging: u64,
    pub target_kv: u64,
    pub draft_kv: u64,
    pub indexer_keys: u64,
    pub draft_indexer_keys: u64,
    pub model_metadata: u64,
    pub page_tables: u64,
    pub allocator_padding: u64,
    pub escrow: u64,
}

impl Budget {
    pub fn required(self) -> Result<u64, BudgetError> {
        [
            self.weights,
            self.modules_and_contexts,
            self.graphs,
            self.workspace,
            self.collectives_and_staging,
            self.target_kv,
            self.draft_kv,
            self.indexer_keys,
            self.draft_indexer_keys,
            self.model_metadata,
            self.page_tables,
            self.allocator_padding,
            self.escrow,
        ]
        .into_iter()
        .try_fold(0_u64, |sum, value| {
            sum.checked_add(value).ok_or(BudgetError::Overflow)
        })
    }

    pub fn validate(self, measured_usable: u64) -> Result<u64, BudgetError> {
        let required = self.required()?;
        measured_usable
            .checked_sub(required)
            .ok_or(BudgetError::DoesNotFit {
                required,
                available: measured_usable,
            })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BudgetError {
    Positions,
    Overflow,
    DoesNotFit { required: u64, available: u64 },
}

impl fmt::Display for BudgetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for BudgetError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_million_cache_arithmetic_is_exact() {
        let capacity = CacheCapacity::at_positions(MODEL_POSITIONS, true).unwrap();
        assert_eq!(capacity.target_kv_bytes, 30_098_325_504);
        assert_eq!(capacity.draft_kv_bytes, 385_875_968);
        assert_eq!(capacity.indexer_key_bytes, 2_906_652_672);
        assert_eq!(capacity.draft_indexer_key_bytes, 138_412_032);
        assert_eq!(capacity.total().unwrap(), 33_529_266_176);
    }

    #[test]
    fn explicit_terms_cannot_hide_in_padding() {
        let budget = Budget {
            weights: 10,
            indexer_keys: 3,
            model_metadata: 2,
            page_tables: 1,
            escrow: 4,
            ..Budget::default()
        };
        assert_eq!(budget.required().unwrap(), 20);
        assert_eq!(budget.validate(21).unwrap(), 1);
        assert!(matches!(
            budget.validate(19),
            Err(BudgetError::DoesNotFit { .. })
        ));
    }
}
