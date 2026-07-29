//! Deterministic CPU definition of the GLM-5.2 native packed formats.

mod container;
mod crc32c;
mod exl3;
mod float;
mod nvfp4;

pub use container::{
    HEADER_BYTES, RankFile, RankFileBuilder, RankFileError, TensorDescriptor, TensorRecord,
};
pub use crc32c::crc32c;
pub use exl3::{
    EXL3_CODEBOOK_MCG, EXL3_MCG_MULTIPLIER, EXL3_MODEL_REVISION, EXL3_SOURCE_REVISION,
    EXL3_SOURCE_VERSION, Exl3Error, Exl3Metadata, Exl3Projection, Exl3Trellis, f16_bits_to_f32,
    f32_to_f16_bits,
};
pub use float::{decode_e2m1, decode_e4m3, encode_e2m1, encode_e4m3};
pub use nvfp4::{
    Codec, LAYOUT_SOURCE_SHA256, Nvfp4Error, Nvfp4Metadata, PackedNvfp4, QUANT_POLICY_SHA256,
    SCALE_LAYOUT_SM120_K_MAJOR, VALUE_LAYOUT_SM120_ROW_MAJOR, scale_offset,
};

pub const CUTLASS_COMMIT: &str = "e05f953a5b3d38adc240df2ff928e0421c2abba3";
pub const KERNEL_ABI: &str = "glmaxx.sm120.nvfp4.routed_fc1.v1";
