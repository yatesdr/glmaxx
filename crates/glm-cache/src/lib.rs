//! CPU proof of KV/indexer records, page ownership, tier transitions, MTP
//! transactions, and capacity accounting.

mod attention;
mod budget;
mod kv;
mod mtp;
mod page;
mod prefix;
mod tier;

pub use budget::{Budget, BudgetError, CacheCapacity};
pub use kv::{IndexerKeyRecord, KvError, KvRecord};
pub use mtp::{MtpError, SpeculativeTail, VerifyOutcome};
pub use page::{AttachmentError, PageAttachments, PageState, PageTransitionError, owner_rank};
pub use prefix::{
    NamespaceInputs, PrefixError, PrefixIndex, PrefixMatch, PrefixNamespace, PrefixPageKey,
};
pub use tier::{
    JournalEvent, Tier, TierError, TierJournal, TierPiece, TierPieceRecord, TierRecord,
};

pub const PAGE_TOKENS: u64 = 64;
pub const MODEL_POSITIONS: u64 = 1_048_576;
pub const TARGET_LAYERS: u64 = 78;
pub const INDEXER_GROUPS: u64 = 21;
pub const DRAFT_INDEXER_GROUPS: u64 = 1;
pub const KV_RECORD_BYTES: u64 = 368;
pub const INDEXER_RECORD_BYTES: u64 = 132;
pub const DRAFT_COMMITTED_RECORD_BYTES: u64 = KV_RECORD_BYTES + INDEXER_RECORD_BYTES;
pub use attention::{AttentionError, Candidate, LseState, deterministic_top_k, score_indexer_key};
