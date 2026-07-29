//! CPU-testable execution contracts for the fixed GLM-5.2 TP4 engine.
//!
//! This crate deliberately contains no CUDA work. It defines the immutable
//! plan, graph-admission, and memory-accounting contracts that the coordinator
//! must prove before any of the four device workers may enter a step.

mod graph;
mod memory;
mod startup;
mod step;
mod weight;
mod worker;

pub use graph::{GraphEntry, GraphKey, GraphProfile, GraphProfileError};
pub use memory::{
    CAPACITY_EXL3_RANK_WEIGHT_BYTES, GIB, MemoryPlanError, MemoryTerms, ProfileBudgetArtifact,
    ProfileBudgetError, ProfileBudgetGlobalCapacity, ProfileBudgetRank, ProfileBudgetSource,
    ProfileBudgetTerms, ProfileClass, RankMemoryInput, RankMemoryPlan, SystemMemoryPlan,
    plan_system_memory,
};
pub use startup::{
    MockFault, RankStartupReport, StartupCoordinator, StartupError, StartupState, run_mock_startup,
};
pub use step::{
    AttentionTransport, CollectiveKind, CollectiveOp, CollectiveSchedule, PlanError, STEP_PLAN_ABI,
    STEP_PLAN_HASH_INPUT_BYTES, STEP_PLAN_RECORD_BYTES, StepMode, StepPlan, StepPlanRequest,
    TP_RANK_MASK,
};
pub use weight::{
    EXL3_PROJECTION_BYTES, ExpertCodec, ExpertKey, ExpertTensorRole, NVFP4_PROJECTION_BYTES,
    ProtectedAllocation, ProtectedPrecision, WeightPolicy, WeightPolicyError, WeightProfile,
};
pub use worker::{
    MockWorkerFault, RankExecutionError, RankExecutor, RankStepAck, StepHandle, StepOutcome,
    Tp4WorkerPool, WorkerError,
};
