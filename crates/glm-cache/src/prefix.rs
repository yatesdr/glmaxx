use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};

use sha2::{Digest, Sha256};

use crate::{PAGE_TOKENS, Tier, TierPiece, TierRecord};

const NAMESPACE_DOMAIN: &[u8] = b"glmaxx.prefix-namespace.v1\0";
const PAGE_DOMAIN: &[u8] = b"glmaxx.prefix-page.v1\0";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NamespaceInputs {
    pub model_revision_sha256: [u8; 32],
    pub tokenizer_sha256: [u8; 32],
    pub chat_template_sha256: [u8; 32],
    pub weight_policy_hash: [u8; 32],
    pub target_kv_abi_sha256: [u8; 32],
    pub draft_kv_abi_sha256: [u8; 32],
    pub rope_parameters_sha256: [u8; 32],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PrefixNamespace(pub [u8; 32]);

impl PrefixNamespace {
    pub fn new(inputs: NamespaceInputs) -> Result<Self, PrefixError> {
        if [
            inputs.model_revision_sha256,
            inputs.tokenizer_sha256,
            inputs.chat_template_sha256,
            inputs.weight_policy_hash,
            inputs.target_kv_abi_sha256,
            inputs.draft_kv_abi_sha256,
            inputs.rope_parameters_sha256,
        ]
        .contains(&[0; 32])
        {
            return Err(PrefixError::Namespace);
        }
        let mut hasher = Sha256::new();
        hasher.update(NAMESPACE_DOMAIN);
        hasher.update(inputs.model_revision_sha256);
        hasher.update(inputs.tokenizer_sha256);
        hasher.update(inputs.chat_template_sha256);
        hasher.update(inputs.weight_policy_hash);
        hasher.update(inputs.target_kv_abi_sha256);
        hasher.update(inputs.draft_kv_abi_sha256);
        hasher.update(inputs.rope_parameters_sha256);
        Ok(Self(hasher.finalize().into()))
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct PrefixPageKey(pub [u8; 32]);

impl PrefixPageKey {
    fn derive(namespace: PrefixNamespace, parent: Option<Self>, tokens: &[u32]) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(PAGE_DOMAIN);
        hasher.update(namespace.0);
        hasher.update(parent.map_or([0; 32], |key| key.0));
        hasher.update((tokens.len() as u16).to_le_bytes());
        for token in tokens {
            hasher.update(token.to_le_bytes());
        }
        Self(hasher.finalize().into())
    }
}

#[derive(Clone, Debug)]
struct PrefixPage {
    parent: Option<PrefixPageKey>,
    record: TierRecord,
    references: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PrefixMatch {
    pub page_keys: Vec<PrefixPageKey>,
    pub matched_tokens: u64,
    pub terminal_tier: Tier,
    pub terminal_generation: u64,
}

#[derive(Clone, Debug)]
pub struct PrefixIndex {
    namespace: PrefixNamespace,
    pages: BTreeMap<PrefixPageKey, PrefixPage>,
}

impl PrefixIndex {
    #[must_use]
    pub const fn new(namespace: PrefixNamespace) -> Self {
        Self {
            namespace,
            pages: BTreeMap::new(),
        }
    }

    /// Inserts one or more fully sealed 64-token pages. Partial tails are
    /// intentionally not shareable and must remain request-owned.
    pub fn insert(
        &mut self,
        tokens: &[u32],
        records: Vec<TierRecord>,
    ) -> Result<Vec<PrefixPageKey>, PrefixError> {
        if tokens.is_empty() || !tokens.len().is_multiple_of(PAGE_TOKENS as usize) {
            return Err(PrefixError::PartialPage);
        }
        if records.len() != tokens.len() / PAGE_TOKENS as usize {
            return Err(PrefixError::RecordCount);
        }
        let mut parent = None;
        let mut pending = Vec::with_capacity(records.len());
        let mut pending_keys = BTreeSet::new();
        for (page_tokens, record) in tokens
            .chunks_exact(PAGE_TOKENS as usize)
            .zip(records.into_iter())
        {
            record.validate().map_err(|_| PrefixError::TierRecord)?;
            if record.namespace != self.namespace.0 {
                return Err(PrefixError::Namespace);
            }
            let key = PrefixPageKey::derive(self.namespace, parent, page_tokens);
            if record.page_key != key.0 {
                return Err(PrefixError::PageKey);
            }
            if !pending_keys.insert(key) {
                return Err(PrefixError::Collision);
            }
            if let Some(existing) = self.pages.get(&key) {
                if existing.parent != parent
                    || !records_are_logically_compatible(&existing.record, &record)
                {
                    return Err(PrefixError::Collision);
                }
                existing
                    .references
                    .checked_add(1)
                    .ok_or(PrefixError::Overflow)?;
            }
            pending.push((key, parent, record));
            parent = Some(key);
        }
        let keys = pending.iter().map(|entry| entry.0).collect();
        for (key, parent, record) in pending {
            match self.pages.get_mut(&key) {
                Some(existing) => {
                    if record.generation > existing.record.generation
                        && (!existing.record.mtp || record.mtp)
                    {
                        existing.record = record;
                    }
                    existing.references += 1;
                }
                None => {
                    self.pages.insert(
                        key,
                        PrefixPage {
                            parent,
                            record,
                            references: 1,
                        },
                    );
                }
            }
        }
        Ok(keys)
    }

    #[must_use]
    pub fn longest_match(&self, tokens: &[u32]) -> Option<PrefixMatch> {
        self.longest_match_with_capability(tokens, false)
    }

    #[must_use]
    pub fn longest_match_with_capability(
        &self,
        tokens: &[u32],
        require_draft: bool,
    ) -> Option<PrefixMatch> {
        let mut parent = None;
        let mut keys = Vec::new();
        let mut terminal = None;
        for page_tokens in tokens.chunks_exact(PAGE_TOKENS as usize) {
            let key = PrefixPageKey::derive(self.namespace, parent, page_tokens);
            let Some(page) = self.pages.get(&key) else {
                break;
            };
            if page.parent != parent || (require_draft && !page.record.mtp) {
                break;
            }
            keys.push(key);
            terminal = Some(&page.record);
            parent = Some(key);
        }
        terminal.map(|record| PrefixMatch {
            matched_tokens: keys.len() as u64 * PAGE_TOKENS,
            page_keys: keys,
            terminal_tier: record.tier,
            terminal_generation: record.generation,
        })
    }

    #[must_use]
    pub fn references(&self, key: PrefixPageKey) -> Option<u64> {
        self.pages.get(&key).map(|page| page.references)
    }

    #[must_use]
    pub fn record(&self, key: PrefixPageKey) -> Option<&TierRecord> {
        self.pages.get(&key).map(|page| &page.record)
    }

    #[must_use]
    pub fn derive_keys(&self, tokens: &[u32]) -> Vec<PrefixPageKey> {
        let mut parent = None;
        tokens
            .chunks_exact(PAGE_TOKENS as usize)
            .map(|page_tokens| {
                let key = PrefixPageKey::derive(self.namespace, parent, page_tokens);
                parent = Some(key);
                key
            })
            .collect()
    }
}

fn records_are_logically_compatible(first: &TierRecord, second: &TierRecord) -> bool {
    [TierPiece::TargetKv, TierPiece::TargetIndexer]
        .into_iter()
        .all(|piece| logical_piece_identity(first, piece) == logical_piece_identity(second, piece))
        && (!first.mtp
            || !second.mtp
            || logical_piece_identity(first, TierPiece::DraftSidecar)
                == logical_piece_identity(second, TierPiece::DraftSidecar))
}

fn logical_piece_identity(record: &TierRecord, piece: TierPiece) -> Option<(u64, [u8; 32])> {
    record
        .pieces
        .iter()
        .find(|candidate| candidate.piece == piece)
        .map(|candidate| (candidate.byte_length, candidate.sha256))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PrefixError {
    Namespace,
    PartialPage,
    RecordCount,
    TierRecord,
    PageKey,
    Collision,
    Overflow,
}

impl fmt::Display for PrefixError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for PrefixError {}

#[cfg(test)]
mod tests {
    use crate::{TierPiece, TierPieceRecord};

    use super::*;

    fn namespace() -> PrefixNamespace {
        PrefixNamespace::new(NamespaceInputs {
            model_revision_sha256: [1; 32],
            tokenizer_sha256: [2; 32],
            chat_template_sha256: [3; 32],
            weight_policy_hash: [4; 32],
            target_kv_abi_sha256: [5; 32],
            draft_kv_abi_sha256: [6; 32],
            rope_parameters_sha256: [7; 32],
        })
        .unwrap()
    }

    fn record(namespace: PrefixNamespace, key: PrefixPageKey, generation: u64) -> TierRecord {
        TierRecord {
            namespace: namespace.0,
            page_key: key.0,
            generation,
            tier: Tier::Nvme,
            mtp: false,
            pieces: [TierPiece::TargetKv, TierPiece::TargetIndexer]
                .iter()
                .enumerate()
                .map(|(ordinal, &piece)| TierPieceRecord {
                    piece,
                    byte_length: piece.expected_bytes(),
                    storage_offset: ordinal as u64 * 2 * 1024 * 1024,
                    sha256: [ordinal as u8 + 1; 32],
                })
                .collect(),
        }
    }

    fn with_draft(mut record: TierRecord, digest: [u8; 32]) -> TierRecord {
        record.mtp = true;
        record.pieces.push(TierPieceRecord {
            piece: TierPiece::DraftSidecar,
            byte_length: TierPiece::DraftSidecar.expected_bytes(),
            storage_offset: 4 * 1024 * 1024,
            sha256: digest,
        });
        record
    }

    #[test]
    fn chained_keys_find_the_longest_full_page_prefix() {
        let namespace = namespace();
        let mut index = PrefixIndex::new(namespace);
        let tokens: Vec<u32> = (0..128).collect();
        let keys = index.derive_keys(&tokens);
        index
            .insert(
                &tokens,
                vec![record(namespace, keys[0], 1), record(namespace, keys[1], 2)],
            )
            .unwrap();
        let mut query = tokens.clone();
        query.extend(128..170);
        let matched = index.longest_match(&query).unwrap();
        assert_eq!(matched.matched_tokens, 128);
        assert_eq!(matched.page_keys, keys);
        assert_eq!(matched.terminal_generation, 2);
    }

    #[test]
    fn draft_capability_stops_at_the_first_target_only_page() {
        let namespace = namespace();
        let mut index = PrefixIndex::new(namespace);
        let tokens: Vec<u32> = (0..128).collect();
        let keys = index.derive_keys(&tokens);
        let mut first = record(namespace, keys[0], 1);
        first.mtp = true;
        first.pieces.push(TierPieceRecord {
            piece: TierPiece::DraftSidecar,
            byte_length: TierPiece::DraftSidecar.expected_bytes(),
            storage_offset: 4 * 1024 * 1024,
            sha256: [3; 32],
        });
        index
            .insert(&tokens, vec![first, record(namespace, keys[1], 1)])
            .unwrap();
        assert_eq!(
            index
                .longest_match_with_capability(&tokens, true)
                .unwrap()
                .page_keys,
            [keys[0]]
        );
        assert_eq!(index.longest_match(&tokens).unwrap().page_keys, keys);
    }

    #[test]
    fn repeated_prefixes_share_pages_and_partial_tails_do_not_publish() {
        let namespace = namespace();
        let mut index = PrefixIndex::new(namespace);
        let tokens: Vec<u32> = (0..64).collect();
        let key = index.derive_keys(&tokens)[0];
        let record = record(namespace, key, 1);
        index.insert(&tokens, vec![record.clone()]).unwrap();
        index.insert(&tokens, vec![record]).unwrap();
        assert_eq!(index.references(key), Some(2));
        assert_eq!(
            index.insert(&(0..63).collect::<Vec<_>>(), vec![]),
            Err(PrefixError::PartialPage)
        );
    }

    #[test]
    fn namespace_is_dcp_posture_neutral_but_policy_sensitive() {
        let first = namespace();
        // There is intentionally no DCP field in NamespaceInputs.
        assert_eq!(first, namespace());
        let mut inputs = NamespaceInputs {
            model_revision_sha256: [1; 32],
            tokenizer_sha256: [2; 32],
            chat_template_sha256: [3; 32],
            weight_policy_hash: [4; 32],
            target_kv_abi_sha256: [5; 32],
            draft_kv_abi_sha256: [6; 32],
            rope_parameters_sha256: [7; 32],
        };
        inputs.weight_policy_hash[0] ^= 1;
        assert_ne!(PrefixNamespace::new(inputs).unwrap(), first);
    }

    #[test]
    fn multi_page_insert_is_atomic_on_late_validation_failure() {
        let namespace = namespace();
        let mut index = PrefixIndex::new(namespace);
        let tokens: Vec<u32> = (0..128).collect();
        let keys = index.derive_keys(&tokens);
        let first = record(namespace, keys[0], 1);
        let mut invalid_second = record(namespace, keys[1], 1);
        invalid_second.page_key[0] ^= 1;
        assert_eq!(
            index.insert(&tokens, vec![first, invalid_second]),
            Err(PrefixError::PageKey)
        );
        assert!(index.longest_match(&tokens).is_none());
        assert_eq!(index.references(keys[0]), None);
    }

    #[test]
    fn same_key_generations_require_identical_bytes_and_never_downgrade_mtp() {
        let namespace = namespace();
        let mut index = PrefixIndex::new(namespace);
        let tokens: Vec<u32> = (0..64).collect();
        let key = index.derive_keys(&tokens)[0];

        index
            .insert(&tokens, vec![record(namespace, key, 1)])
            .unwrap();
        let upgrade = with_draft(record(namespace, key, 2), [3; 32]);
        index.insert(&tokens, vec![upgrade.clone()]).unwrap();
        assert_eq!(index.references(key), Some(2));
        assert!(index.record(key).unwrap().mtp);
        assert_eq!(index.record(key).unwrap().generation, 2);

        let downgrade = record(namespace, key, 3);
        index.insert(&tokens, vec![downgrade]).unwrap();
        assert_eq!(index.references(key), Some(3));
        assert!(index.record(key).unwrap().mtp);
        assert_eq!(index.record(key).unwrap().generation, 2);

        let mut conflicting_target = record(namespace, key, 4);
        conflicting_target.pieces[0].sha256[0] ^= 1;
        assert_eq!(
            index.insert(&tokens, vec![conflicting_target]),
            Err(PrefixError::Collision)
        );
        assert_eq!(index.references(key), Some(3));
        assert_eq!(index.record(key).unwrap(), &upgrade);

        let mut conflicting_draft = with_draft(record(namespace, key, 4), [3; 32]);
        conflicting_draft.pieces[2].sha256[0] ^= 1;
        assert_eq!(
            index.insert(&tokens, vec![conflicting_draft]),
            Err(PrefixError::Collision)
        );
        assert_eq!(index.references(key), Some(3));
        assert_eq!(index.record(key).unwrap(), &upgrade);

        let refresh = with_draft(record(namespace, key, 5), [3; 32]);
        index.insert(&tokens, vec![refresh.clone()]).unwrap();
        assert_eq!(index.references(key), Some(4));
        assert_eq!(index.record(key).unwrap(), &refresh);
    }
}
