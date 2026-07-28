//! GLM-5.2-only operation manifest and routed-expert CPU oracle.

mod manifest;
mod matrix;
mod routed_fc1;

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
