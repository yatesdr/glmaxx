//! CPU proof of KV/indexer records, page ownership, tier transitions, MTP
//! transactions, and capacity accounting.

mod attention;
mod budget;
mod delta;
mod direct;
mod direct_restore;
mod direct_schedule;
mod direct_state;
mod kv;
mod mtp;
mod page;
mod prefix;
mod residency;
mod sequence;
mod store;
mod tier;

pub use budget::{Budget, BudgetError, CacheCapacity};
pub use delta::{
    MAXIMUM_DELTA_SEQUENCES, PAGE_TABLE_DELTA_SCHEMA, PageTableDelta, PageTableDeltaError,
    PageTableMirror, RankPageEntry, SequencePageUpdate,
};
pub use direct::{
    DIRECT_IO_ALIGNMENT, DIRECT_TIER_FORMAT_VERSION, DRAFT_SIDECAR_EXTENT_LENGTH,
    DRAFT_SIDECAR_EXTENT_OFFSET, DirectExtentBuffer, DirectExtentError, DirectExtentRecord,
    DirectExtentView, DirectPagePieces, DirectPieceRecord, DirectTierCapability, MTP_LOGICAL_BYTES,
    MTP_PHYSICAL_BYTES, TARGET_INDEXER_EXTENT_LENGTH, TARGET_INDEXER_EXTENT_OFFSET,
    TARGET_KV_EXTENT_LENGTH, TARGET_KV_EXTENT_OFFSET, TARGET_ONLY_LOGICAL_BYTES,
    TARGET_ONLY_PHYSICAL_BYTES, decode_direct_extent, encode_direct_extent,
    validate_direct_io_span,
};
pub use direct_restore::{
    DirectCancellation, DirectCatalogBinding, DirectCqKind, DirectCqTracker, DirectHashJob,
    DirectHashResult, DirectReadCompletion, DirectRestoreAdmission, DirectRestoreConfig,
    DirectRestoreError, DirectRestoreRequest, DirectRestoreState, DirectRestoreTable,
    DirectRestoreTicketId,
};
pub use direct_schedule::{
    DIRECT_PUBLICATION_CQ_RESERVATION, DirectIoClass, DirectIoCommand, DirectIoDecision,
    DirectIoOrderKey, DirectIoResources, DirectIoScheduleError, DirectIoScheduler,
    DirectIoSchedulerConfig, DirectIoSchedulerStats, MAX_DIRECT_IO_QUEUED_COMMANDS,
};
pub use direct_state::{
    DirectBufferId, DirectBufferPool, DirectBufferState, DirectBufferStateError, DirectBufferUse,
    DirectCompletionKind, DirectCompletionToken, DirectDescriptorBinding, DirectDescriptorError,
    DirectDescriptorTable, DirectOperationKind,
};
pub use kv::{IndexerKeyRecord, KvError, KvRecord};
pub use mtp::{MtpError, SpeculativeTail, VerifyOutcome};
pub use page::{AttachmentError, PageAttachments, PageState, PageTransitionError, owner_rank};
pub use prefix::{
    NamespaceInputs, PrefixError, PrefixIndex, PrefixMatch, PrefixNamespace, PrefixPageKey,
};
pub use residency::{
    NvmeRegistrationPlan, Residency, ResidencyConfig, ResidencyError, ResidencyManager,
    RestoreError, RestoreHandle, RestoreRequest, RestoreResult, RestoreService,
};
pub use sequence::{
    MAXIMUM_CONTEXT_TOKENS, MAXIMUM_PHYSICAL_PAGES_PER_RANK, PageReuseQuarantineStats,
    PageTableConfig, PageTableStats, PhysicalPageId, PrefixPageAttachment, SequencePageError,
    SequencePageSnapshot, SequencePageTable, SequencePageView,
};
pub use store::{
    DurablePageRequest, FileTierReader, FileTierStore, PagePieceBytes, RestoredPage, StoreError,
};
pub use tier::{
    DRAFT_INDEXER_PAGE_BYTES, DRAFT_KV_PAGE_BYTES, DRAFT_SIDECAR_PAGE_BYTES, JournalEvent, Tier,
    TierError, TierJournal, TierPiece, TierPieceRecord, TierRecord, TierRecordRelation,
    decode_draft_sidecar_payload, encode_draft_sidecar_payload,
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
