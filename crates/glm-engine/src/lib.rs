//! CPU-testable execution contracts for the fixed GLM-5.2 TP4 engine.
//!
//! This crate deliberately contains no CUDA work. It defines the immutable
//! plan, graph-admission, and memory-accounting contracts that the coordinator
//! must prove before any of the four device workers may enter a step.

mod checkpoint_cuda;
mod checkpoint_load;
mod graph;
mod input;
mod memory;
mod output;
mod startup;
mod step;
mod weight;
mod worker;

pub use checkpoint_cuda::{CudaArenaVerificationEvidence, CudaQuarantinedArena, CudaWeightArena};
pub use checkpoint_load::{
    AdoptedRankSetReceipt, AdoptionAcknowledgement, AdoptionCommand, LOAD_PLAN_HEADER_BYTES,
    LoadPlanError, LoadProfile, LoadVerificationMode, PREPARED_RANK_RECEIPT_BYTES,
    PlannedRankTensorSink, PreparedRankReceipt, PreparedRankSet, QuarantinedArenaWriter,
    RANK_LOAD_ENTRY_BYTES, RANK_SET_SIZE, READER_CHUNK_BYTES, RankArenaLifecycle, RankArenaState,
    RankArenaUploadSummary, RankLoadEntry, RankSetAbortCommand, RankSetLoadAction,
    RankSetLoadCoordinator, RankSetLoadCoordinatorState, RankSetLoadEnvironment, RankSetLoadPlan,
    RankSetLoadPlanHeader, TENSOR_ARENA_ENTRY_BYTES, TensorArenaEntry, WeightArenaExecutionPermit,
    arena_layout_sha256, build_rank_set_load_plan,
};
pub use graph::{GraphEntry, GraphKey, GraphProfile, GraphProfileError};
pub use input::{
    STEP_INPUT_SCHEMA, SequenceStepInput, StepInput, StepInputError, StepSampling, StepSamplingKind,
};
pub use memory::{
    CAPACITY_EXL3_RANK_WEIGHT_BYTES, CacheArenaLayout, GIB, MAXIMUM_ACTIVE_SEQUENCES,
    MAXIMUM_VERIFIER_ROWS, MIN_MTP_TENTATIVE_SLOTS_PER_RANK, MIN_MTP0_TENTATIVE_SLOTS_PER_RANK,
    MIN_PAGE_SLACK_SLOTS_PER_RANK, MemoryPlanError, MemoryTerms, ProfileBudgetArtifact,
    ProfileBudgetError, ProfileBudgetGlobalCapacity, ProfileBudgetRank, ProfileBudgetSource,
    ProfileBudgetTerms, ProfileClass, RankMemoryInput, RankMemoryPlan, SystemMemoryPlan,
    plan_system_memory,
};
pub use output::{
    CommittedTokens, GLM_52_OUTPUT_VOCABULARY, MAX_COMMITTED_TOKENS_PER_SEQUENCE, OutputError,
    StepOutput,
};
pub use startup::{
    MockFault, RankStartupReport, StartupCoordinator, StartupError, StartupState, run_mock_startup,
};
pub use step::{
    AttentionTransport, CollectiveKind, CollectiveOp, CollectiveSchedule, MAX_ACTIVE_SEQUENCES,
    MAX_MTP_DEPTH, MAX_VERIFIER_ROWS, PlanError, STEP_PLAN_ABI, STEP_PLAN_HASH_INPUT_BYTES,
    STEP_PLAN_RECORD_BYTES, StepMode, StepPlan, StepPlanRequest, TP_RANK_MASK,
};
pub use weight::{
    EXL3_PROJECTION_BYTES, ExpertCodec, ExpertKey, ExpertTensorRole, NVFP4_PROJECTION_BYTES,
    ProtectedAllocation, ProtectedPrecision, WeightPolicy, WeightPolicyError, WeightProfile,
};
pub use worker::{
    MockWorkerFault, PageDeltaAck, RankExecutionError, RankExecutor, RankStepAck, StepHandle,
    StepOutcome, Tp4WorkerPool, WorkerError,
};
