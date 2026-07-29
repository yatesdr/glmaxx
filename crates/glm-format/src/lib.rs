//! Deterministic CPU definition of the GLM-5.2 native packed formats.

mod checkpoint;
mod container;
mod crc32c;
mod exl3;
mod float;
mod native_reader;
mod nvfp4;
mod rank_manifest;
mod safetensors;
mod stream;

pub use checkpoint::{
    CheckpointConversionError, CheckpointError, CheckpointInventoryReport,
    CheckpointTensorContract, Exl3Component, Exl3ComponentContract, PINNED_EXL3_COMPONENT_COUNT,
    PINNED_EXL3_INDEX_SHA256, PINNED_EXL3_PAYLOAD_BYTES, PINNED_EXL3_REPOSITORY,
    PINNED_EXL3_SHARD_COUNT, PINNED_EXL3_TENSOR_COUNT, PINNED_PROTECTED_TENSOR_COUNT,
    PINNED_RANK_SOURCE_PAYLOAD_BYTES, PINNED_RANK_TENSOR_COUNT, PINNED_SOURCE_FILE_COUNT,
    PINNED_SOURCE_FILE_MAP_SHA256, PINNED_SOURCE_MANIFEST_SHA256, PinnedConversionProgress,
    PinnedManifestException, PinnedRankManifestTensor, PinnedRankPlan, PinnedSourceBinding,
    PinnedSourceError, PinnedSourceVerification, ProtectedTensorContract, TP_DEGREE,
    parse_exl3_component, pinned_exl3_rank_plan, pinned_exl3_weight_policy_sha256,
    protected_tensor_contracts, validate_pinned_exl3_checkpoint, verify_pinned_source_files,
};
pub use container::{
    HEADER_BYTES, PlainDtype, PlainTensor, RankFile, RankFileBuilder, RankFileError,
    TensorDescriptor, TensorPayload, TensorRecord,
};
pub use crc32c::crc32c;
pub use exl3::{
    EXL3_CODEBOOK_MCG, EXL3_MCG_MULTIPLIER, EXL3_MODEL_REVISION, EXL3_SOURCE_REVISION,
    EXL3_SOURCE_VERSION, Exl3Error, Exl3Metadata, Exl3Projection, Exl3Trellis, f16_bits_to_f32,
    f32_to_f16_bits,
};
pub use float::{decode_e2m1, decode_e4m3, encode_e2m1, encode_e4m3};
pub use native_reader::{
    NativeRankReader, NativeRankReaderError, NullRankTensorSink, RankPayloadProof, RankTensorSink,
};
pub use nvfp4::{
    Codec, LAYOUT_SOURCE_SHA256, Nvfp4Error, Nvfp4Metadata, PackedNvfp4, QUANT_POLICY_SHA256,
    SCALE_LAYOUT_SM120_K_MAJOR, VALUE_LAYOUT_SM120_ROW_MAJOR, scale_offset,
};
pub use rank_manifest::{
    PRODUCTION_RANK_MANIFEST_SCHEMA, RankManifestError, RankWeightProfile, ValidatedRankManifest,
};
pub use safetensors::{
    SafeDtype, SafeTensorDescriptor, SafeTensorError, SafeTensorFile, SafeTensorReader,
    ShardedSafetensors, ShardedTensorReader, TensorShardReader, load_exl3_projection,
    load_exl3_projection_sharded,
};
pub use stream::{
    StreamRankError, StreamingRankConfig, StreamingRankSet, StreamingRankSummary,
    StreamingRankWriter, StreamingTensorIdentity, StreamingTensorSpec,
};

pub const CUTLASS_COMMIT: &str = "e05f953a5b3d38adc240df2ff928e0421c2abba3";
pub const KERNEL_ABI: &str = "glmaxx.sm120.nvfp4.routed_moe.v2";
