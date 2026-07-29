//! GLM-5.2-only operation manifest and routed-expert CPU oracle.

mod manifest;
mod matrix;
mod routed_fc1;
mod routed_fc2;
mod sampling;

pub use manifest::{
    FULL_INDEXER_LAYERS, ModelConstants, OperationManifest, operation_manifest,
    operation_manifest_json,
};
pub use matrix::{
    DECODE_ROWS, NUMERICAL_CASES, NumericalCase, NumericalFixture, PREFILL_ROWS, ROUTING_CASES,
    RoutingCase, generate_numerical_fixture, generate_routes,
};
pub use routed_fc1::{
    CompactedRoute, Fc1Error, Route, bf16_round, compact_routes, routed_fc1_oracle,
};
pub use routed_fc2::{
    Fc2Error, LayerOperation, RankLayerPartial, RoutedExpertWeights, SparseLayerDescriptor,
    finish_sparse_layer_oracle, routed_fc2_oracle,
};
pub use sampling::{
    CounterTicket, LogitShard, ProbabilityShard, SamplePurpose, SampleResult, SamplingCounter,
    SamplingError, SamplingParams, distributed_greedy, distributed_residual_sample,
    distributed_sample,
};
