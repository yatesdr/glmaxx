use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PageState {
    Free,
    HbmMutable,
    HbmTentative,
    HbmSealed,
    DramWriting,
    DramResident,
    NvmeWriting,
    NvmeResident,
    Restoring,
    Invalid,
}

impl PageState {
    pub fn transition(self, next: Self) -> Result<Self, PageTransitionError> {
        let legal = matches!(
            (self, next),
            (Self::Free, Self::HbmMutable)
                | (
                    Self::HbmMutable,
                    Self::HbmTentative | Self::HbmSealed | Self::Invalid
                )
                | (
                    Self::HbmTentative,
                    Self::HbmMutable | Self::HbmSealed | Self::Invalid
                )
                | (Self::HbmSealed, Self::DramWriting | Self::Invalid)
                | (Self::DramWriting, Self::DramResident | Self::Invalid)
                | (
                    Self::DramResident,
                    Self::NvmeWriting | Self::Restoring | Self::Invalid
                )
                | (Self::NvmeWriting, Self::NvmeResident | Self::Invalid)
                | (Self::NvmeResident, Self::Restoring | Self::Invalid)
                | (Self::Restoring, Self::HbmSealed | Self::Invalid)
                | (Self::Invalid, Self::Free)
        );
        if legal {
            Ok(next)
        } else {
            Err(PageTransitionError {
                from: self,
                to: next,
            })
        }
    }
}

#[must_use]
pub const fn owner_rank(page_ordinal: u64) -> u8 {
    (page_ordinal % 4) as u8
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PageAttachments {
    pub target_generation: u64,
    pub target_indexer_generation: u64,
    pub draft_kv_generation: Option<u64>,
    pub draft_indexer_generation: Option<u64>,
}

impl PageAttachments {
    pub fn validate(self, mtp: bool) -> Result<(), AttachmentError> {
        if self.target_generation != self.target_indexer_generation {
            return Err(AttachmentError::Generation);
        }
        match (self.draft_kv_generation, self.draft_indexer_generation) {
            (Some(kv), Some(indexer)) if kv == indexer && kv == self.target_generation => Ok(()),
            (None, None) if !mtp => Ok(()),
            (None, None) => Err(AttachmentError::MissingDraft),
            _ => Err(AttachmentError::Generation),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AttachmentError {
    Generation,
    MissingDraft,
}

impl fmt::Display for AttachmentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for AttachmentError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PageTransitionError {
    pub from: PageState,
    pub to: PageState,
}

impl fmt::Display for PageTransitionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "illegal page transition {:?} -> {:?}",
            self.from, self.to
        )
    }
}

impl std::error::Error for PageTransitionError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ownership_is_balanced_at_model_limit() {
        let mut pages = [0_u32; 4];
        for ordinal in 0..16_384 {
            pages[usize::from(owner_rank(ordinal))] += 1;
        }
        assert_eq!(pages, [4096; 4]);
    }

    #[test]
    fn every_state_pair_matches_the_frozen_transition_table() {
        let states = [
            PageState::Free,
            PageState::HbmMutable,
            PageState::HbmTentative,
            PageState::HbmSealed,
            PageState::DramWriting,
            PageState::DramResident,
            PageState::NvmeWriting,
            PageState::NvmeResident,
            PageState::Restoring,
            PageState::Invalid,
        ];
        let legal = [
            (PageState::Free, PageState::HbmMutable),
            (PageState::HbmMutable, PageState::HbmTentative),
            (PageState::HbmMutable, PageState::HbmSealed),
            (PageState::HbmMutable, PageState::Invalid),
            (PageState::HbmTentative, PageState::HbmMutable),
            (PageState::HbmTentative, PageState::HbmSealed),
            (PageState::HbmTentative, PageState::Invalid),
            (PageState::HbmSealed, PageState::DramWriting),
            (PageState::HbmSealed, PageState::Invalid),
            (PageState::DramWriting, PageState::DramResident),
            (PageState::DramWriting, PageState::Invalid),
            (PageState::DramResident, PageState::NvmeWriting),
            (PageState::DramResident, PageState::Restoring),
            (PageState::DramResident, PageState::Invalid),
            (PageState::NvmeWriting, PageState::NvmeResident),
            (PageState::NvmeWriting, PageState::Invalid),
            (PageState::NvmeResident, PageState::Restoring),
            (PageState::NvmeResident, PageState::Invalid),
            (PageState::Restoring, PageState::HbmSealed),
            (PageState::Restoring, PageState::Invalid),
            (PageState::Invalid, PageState::Free),
        ];
        for &from in &states {
            for &to in &states {
                assert_eq!(
                    from.transition(to).is_ok(),
                    legal.contains(&(from, to)),
                    "{from:?} -> {to:?}"
                );
            }
        }
    }

    #[test]
    fn target_and_mtp_attachments_are_generation_atomic() {
        let target_only = PageAttachments {
            target_generation: 7,
            target_indexer_generation: 7,
            draft_kv_generation: None,
            draft_indexer_generation: None,
        };
        assert_eq!(target_only.validate(false), Ok(()));
        assert_eq!(
            target_only.validate(true),
            Err(AttachmentError::MissingDraft)
        );

        let mtp = PageAttachments {
            draft_kv_generation: Some(7),
            draft_indexer_generation: Some(7),
            ..target_only
        };
        assert_eq!(mtp.validate(true), Ok(()));
        assert_eq!(
            PageAttachments {
                draft_indexer_generation: Some(8),
                ..mtp
            }
            .validate(true),
            Err(AttachmentError::Generation)
        );
    }
}
