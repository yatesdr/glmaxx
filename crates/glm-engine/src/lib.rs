//! CPU-testable execution contracts for the fixed GLM-5.2 TP4 engine.
//!
//! This crate deliberately contains no CUDA work. It defines the immutable
//! plan, graph-admission, and memory-accounting contracts that the coordinator
//! must prove before any of the four device workers may enter a step.

mod graph;
mod memory;
mod step;

pub use graph::{GraphEntry, GraphKey, GraphProfile, GraphProfileError};
pub use memory::{
    GIB, MemoryPlanError, MemoryTerms, ProfileClass, RankMemoryInput, RankMemoryPlan,
    SystemMemoryPlan, plan_system_memory,
};
pub use step::{
    AttentionTransport, CollectiveKind, CollectiveOp, CollectiveSchedule, PlanError, STEP_PLAN_ABI,
    STEP_PLAN_HASH_INPUT_BYTES, STEP_PLAN_RECORD_BYTES, StepMode, StepPlan, StepPlanRequest,
    TP_RANK_MASK,
};
