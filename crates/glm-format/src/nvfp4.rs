use std::fmt;

use crate::{crc32c, decode_e2m1, decode_e4m3, encode_e2m1, encode_e4m3};

pub const VALUE_LAYOUT_SM120_ROW_MAJOR: u16 = 0x1201;
pub const SCALE_LAYOUT_SM120_K_MAJOR: u16 = 0x1201;
pub const LAYOUT_SOURCE_SHA256: [u8; 32] = [
    0x59, 0x8e, 0x05, 0x4b, 0xef, 0x21, 0xed, 0xf9, 0x4b, 0x1f, 0xd6, 0xbb, 0x14, 0x47, 0xcf, 0xa9,
    0xcf, 0xcf, 0x5a, 0x59, 0x07, 0xab, 0x37, 0x01, 0x28, 0x10, 0x24, 0x48, 0xdb, 0xb6, 0xd5, 0x30,
];
pub const QUANT_POLICY_SHA256: [u8; 32] = [
    0xcd, 0x90, 0x95, 0x79, 0x33, 0x44, 0x05, 0xec, 0xd4, 0xcd, 0x8d, 0x9a, 0x6c, 0x2d, 0xfc, 0xba,
    0x7f, 0x01, 0x24, 0xc4, 0xc4, 0xba, 0x92, 0xbc, 0x40, 0xc9, 0x76, 0xd5, 0x74, 0xbe, 0x05, 0xa3,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u16)]
pub enum Codec {
    OneDimensional = 0x0100,
    TwoDimensional = 0x0101,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Nvfp4Metadata {
    pub codec: Codec,
    pub logical_n: u32,
    pub logical_k: u32,
    pub padded_n: u32,
    pub padded_k: u32,
    pub global_scale: f32,
    pub global_amax: f32,
    pub value_plane_bytes: u32,
    pub scale_plane_bytes: u32,
}

impl Nvfp4Metadata {
    pub const BYTES: usize = 128;

    #[must_use]
    pub fn encode(&self) -> [u8; Self::BYTES] {
        let mut out = [0_u8; Self::BYTES];
        put_u16(&mut out, 0, 0);
        put_u16(&mut out, 2, 1);
        put_u16(&mut out, 4, self.codec as u16);
        out[6] = match self.codec {
            Codec::OneDimensional => 1,
            Codec::TwoDimensional => 2,
        };
        out[7] = 1;
        put_u32(&mut out, 8, self.logical_n);
        put_u32(&mut out, 12, self.logical_k);
        put_u32(&mut out, 16, self.padded_n);
        put_u32(&mut out, 20, self.padded_k);
        put_u16(&mut out, 24, 16);
        put_u16(
            &mut out,
            26,
            match self.codec {
                Codec::OneDimensional => 1,
                Codec::TwoDimensional => 16,
            },
        );
        put_u16(&mut out, 28, VALUE_LAYOUT_SM120_ROW_MAJOR);
        put_u16(&mut out, 30, SCALE_LAYOUT_SM120_K_MAJOR);
        out[32] = 1;
        out[33] = 1;
        out[34] = 1;
        out[35] = 1;
        put_f32(&mut out, 36, self.global_scale);
        put_f32(&mut out, 40, self.global_amax);
        put_u32(&mut out, 44, self.value_plane_bytes);
        put_u32(&mut out, 48, self.scale_plane_bytes);
        out[56..88].copy_from_slice(&LAYOUT_SOURCE_SHA256);
        out[88..120].copy_from_slice(&QUANT_POLICY_SHA256);
        let crc = crc32c(&out);
        put_u32(&mut out, 120, crc);
        out
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, Nvfp4Error> {
        if bytes.len() != Self::BYTES {
            return Err(Nvfp4Error::MetadataLength(bytes.len()));
        }
        let mut checked = [0_u8; Self::BYTES];
        checked.copy_from_slice(bytes);
        let expected_crc = get_u32(bytes, 120);
        checked[120..124].fill(0);
        if crc32c(&checked) != expected_crc {
            return Err(Nvfp4Error::MetadataCrc);
        }
        if get_u16(bytes, 0) != 0
            || get_u16(bytes, 2) != 1
            || bytes[7] != 1
            || get_u16(bytes, 24) != 16
            || get_u16(bytes, 28) != VALUE_LAYOUT_SM120_ROW_MAJOR
            || get_u16(bytes, 30) != SCALE_LAYOUT_SM120_K_MAJOR
            || bytes[32] != 1
            || bytes[56..88] != LAYOUT_SOURCE_SHA256
            || bytes[88..120] != QUANT_POLICY_SHA256
        {
            return Err(Nvfp4Error::UnsupportedMetadata);
        }
        let codec = match get_u16(bytes, 4) {
            0x0100 if bytes[6] == 1 && get_u16(bytes, 26) == 1 => Codec::OneDimensional,
            0x0101 if bytes[6] == 2 && get_u16(bytes, 26) == 16 => Codec::TwoDimensional,
            _ => return Err(Nvfp4Error::UnsupportedMetadata),
        };
        let metadata = Self {
            codec,
            logical_n: get_u32(bytes, 8),
            logical_k: get_u32(bytes, 12),
            padded_n: get_u32(bytes, 16),
            padded_k: get_u32(bytes, 20),
            global_scale: get_f32(bytes, 36),
            global_amax: get_f32(bytes, 40),
            value_plane_bytes: get_u32(bytes, 44),
            scale_plane_bytes: get_u32(bytes, 48),
        };
        metadata.validate()?;
        Ok(metadata)
    }

    pub fn validate(&self) -> Result<(), Nvfp4Error> {
        if self.logical_n == 0
            || self.logical_k == 0
            || self.padded_n < self.logical_n
            || self.padded_k < self.logical_k
            || !self.padded_n.is_multiple_of(128)
            || !self.padded_k.is_multiple_of(64)
            || !self.global_scale.is_finite()
            || self.global_scale <= 0.0
            || !self.global_amax.is_finite()
            || self.global_amax < 0.0
        {
            return Err(Nvfp4Error::InvalidShapeOrScale);
        }
        let elements = u64::from(self.padded_n)
            .checked_mul(u64::from(self.padded_k))
            .ok_or(Nvfp4Error::Overflow)?;
        if u64::from(self.value_plane_bytes) != elements / 2
            || u64::from(self.scale_plane_bytes) != elements / 16
        {
            return Err(Nvfp4Error::ByteAccounting);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PackedNvfp4 {
    pub metadata: Nvfp4Metadata,
    pub values: Vec<u8>,
    pub scales: Vec<u8>,
}

impl PackedNvfp4 {
    pub fn pack(
        input: &[f32],
        logical_n: usize,
        logical_k: usize,
        codec: Codec,
    ) -> Result<Self, Nvfp4Error> {
        let logical_elements = logical_n
            .checked_mul(logical_k)
            .ok_or(Nvfp4Error::Overflow)?;
        if logical_n == 0 || logical_k == 0 || input.len() != logical_elements {
            return Err(Nvfp4Error::InputLength {
                expected: logical_elements,
                actual: input.len(),
            });
        }
        let padded_n = align_up(logical_n, 128)?;
        let padded_k = align_up(logical_k, 64)?;
        let padded_elements = padded_n.checked_mul(padded_k).ok_or(Nvfp4Error::Overflow)?;
        let value_bytes = padded_elements / 2;
        let scale_bytes = padded_elements / 16;
        let global_amax = input.iter().try_fold(0.0_f32, |amax, &value| {
            if value.is_finite() {
                Ok(amax.max(value.abs()))
            } else {
                Err(Nvfp4Error::NonFinite)
            }
        })?;
        let global_scale = if global_amax == 0.0 {
            1.0
        } else {
            global_amax / (448.0 * 6.0)
        };
        let mut values = vec![0_u8; value_bytes];
        let mut scales = vec![0_u8; scale_bytes];
        let groups_k = padded_k / 16;

        match codec {
            Codec::OneDimensional => {
                for n in 0..padded_n {
                    for group in 0..groups_k {
                        let block_scale =
                            block_scale_1d(input, logical_n, logical_k, n, group, global_scale)?;
                        scales[scale_offset(n, group, padded_n, padded_k)?] = block_scale;
                        encode_block(
                            input,
                            logical_n,
                            logical_k,
                            n,
                            group,
                            padded_k,
                            block_scale,
                            global_scale,
                            &mut values,
                        )?;
                    }
                }
            }
            Codec::TwoDimensional => {
                for n_tile in (0..padded_n).step_by(16) {
                    for group in 0..groups_k {
                        let block_scale = block_scale_2d(
                            input,
                            logical_n,
                            logical_k,
                            n_tile,
                            group,
                            global_scale,
                        )?;
                        for row in n_tile..n_tile + 16 {
                            scales[scale_offset(row, group, padded_n, padded_k)?] = block_scale;
                            encode_block(
                                input,
                                logical_n,
                                logical_k,
                                row,
                                group,
                                padded_k,
                                block_scale,
                                global_scale,
                                &mut values,
                            )?;
                        }
                    }
                }
            }
        }
        let metadata = Nvfp4Metadata {
            codec,
            logical_n: u32::try_from(logical_n).map_err(|_| Nvfp4Error::Overflow)?,
            logical_k: u32::try_from(logical_k).map_err(|_| Nvfp4Error::Overflow)?,
            padded_n: u32::try_from(padded_n).map_err(|_| Nvfp4Error::Overflow)?,
            padded_k: u32::try_from(padded_k).map_err(|_| Nvfp4Error::Overflow)?,
            global_scale,
            global_amax,
            value_plane_bytes: u32::try_from(value_bytes).map_err(|_| Nvfp4Error::Overflow)?,
            scale_plane_bytes: u32::try_from(scale_bytes).map_err(|_| Nvfp4Error::Overflow)?,
        };
        metadata.validate()?;
        Ok(Self {
            metadata,
            values,
            scales,
        })
    }

    pub fn validate(&self) -> Result<(), Nvfp4Error> {
        self.metadata.validate()?;
        if self.values.len()
            != usize::try_from(self.metadata.value_plane_bytes).map_err(|_| Nvfp4Error::Overflow)?
            || self.scales.len()
                != usize::try_from(self.metadata.scale_plane_bytes)
                    .map_err(|_| Nvfp4Error::Overflow)?
        {
            return Err(Nvfp4Error::ByteAccounting);
        }
        Ok(())
    }

    pub fn dequantize(&self) -> Result<Vec<f32>, Nvfp4Error> {
        self.validate()?;
        let n = usize::try_from(self.metadata.logical_n).map_err(|_| Nvfp4Error::Overflow)?;
        let k = usize::try_from(self.metadata.logical_k).map_err(|_| Nvfp4Error::Overflow)?;
        let padded_n = usize::try_from(self.metadata.padded_n).map_err(|_| Nvfp4Error::Overflow)?;
        let padded_k = usize::try_from(self.metadata.padded_k).map_err(|_| Nvfp4Error::Overflow)?;
        let mut output = vec![0.0_f32; n.checked_mul(k).ok_or(Nvfp4Error::Overflow)?];
        for row in 0..n {
            for col in 0..k {
                let linear = row * padded_k + col;
                let byte = self.values[linear / 2];
                let code = if linear & 1 == 0 {
                    byte & 0x0f
                } else {
                    byte >> 4
                };
                let scale = self.scales[scale_offset(row, col / 16, padded_n, padded_k)?];
                output[row * k + col] =
                    decode_e2m1(code) * decode_e4m3(scale) * self.metadata.global_scale;
            }
        }
        Ok(output)
    }

    #[must_use]
    pub fn physical_bytes(&self) -> usize {
        self.values.len() + self.scales.len() + Nvfp4Metadata::BYTES
    }
}

pub fn scale_offset(
    n: usize,
    group: usize,
    padded_n: usize,
    padded_k: usize,
) -> Result<usize, Nvfp4Error> {
    if !padded_n.is_multiple_of(128)
        || !padded_k.is_multiple_of(64)
        || n >= padded_n
        || group >= padded_k / 16
    {
        return Err(Nvfp4Error::ScaleIndex);
    }
    let n_block = n / 128;
    let n0 = n % 32;
    let n1 = (n % 128) / 32;
    let k_block = group / 4;
    let group_in = group % 4;
    let k_blocks = padded_k / 64;
    512_usize
        .checked_mul(
            n_block
                .checked_mul(k_blocks)
                .and_then(|value| value.checked_add(k_block))
                .ok_or(Nvfp4Error::Overflow)?,
        )
        .and_then(|value| value.checked_add(16 * n0 + 4 * n1 + group_in))
        .ok_or(Nvfp4Error::Overflow)
}

fn block_scale_1d(
    input: &[f32],
    logical_n: usize,
    logical_k: usize,
    n: usize,
    group: usize,
    global_scale: f32,
) -> Result<u8, Nvfp4Error> {
    if n >= logical_n {
        return Ok(0);
    }
    let start = group * 16;
    let end = (start + 16).min(logical_k);
    if start >= end {
        return Ok(0);
    }
    let mut amax = 0.0_f32;
    for &value in &input[n * logical_k + start..n * logical_k + end] {
        amax = amax.max(value.abs());
    }
    quantize_scale(amax, global_scale)
}

fn block_scale_2d(
    input: &[f32],
    logical_n: usize,
    logical_k: usize,
    n_tile: usize,
    group: usize,
    global_scale: f32,
) -> Result<u8, Nvfp4Error> {
    let start = group * 16;
    if start >= logical_k || n_tile >= logical_n {
        return Ok(0);
    }
    let mut amax = 0.0_f32;
    for n in n_tile..(n_tile + 16).min(logical_n) {
        for &value in &input[n * logical_k + start..n * logical_k + (start + 16).min(logical_k)] {
            amax = amax.max(value.abs());
        }
    }
    quantize_scale(amax, global_scale)
}

fn quantize_scale(block_amax: f32, global_scale: f32) -> Result<u8, Nvfp4Error> {
    if block_amax == 0.0 {
        Ok(0)
    } else {
        encode_e4m3((block_amax / 6.0) / global_scale)
    }
}

#[allow(clippy::too_many_arguments)]
fn encode_block(
    input: &[f32],
    logical_n: usize,
    logical_k: usize,
    n: usize,
    group: usize,
    padded_k: usize,
    block_scale_code: u8,
    global_scale: f32,
    output: &mut [u8],
) -> Result<(), Nvfp4Error> {
    let decoded_scale = decode_e4m3(block_scale_code) * global_scale;
    for lane in 0..16 {
        let k = group * 16 + lane;
        let value = if n < logical_n && k < logical_k {
            input[n * logical_k + k]
        } else {
            0.0
        };
        let code = if value == 0.0 || block_scale_code == 0 {
            0
        } else {
            encode_e2m1(value / decoded_scale)?
        };
        let linear = n * padded_k + k;
        if linear & 1 == 0 {
            output[linear / 2] = code;
        } else {
            output[linear / 2] |= code << 4;
        }
    }
    Ok(())
}

fn align_up(value: usize, alignment: usize) -> Result<usize, Nvfp4Error> {
    value
        .checked_add(alignment - 1)
        .map(|sum| sum / alignment * alignment)
        .ok_or(Nvfp4Error::Overflow)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Nvfp4Error {
    NonFinite,
    Overflow,
    InputLength { expected: usize, actual: usize },
    InvalidShapeOrScale,
    ByteAccounting,
    ScaleIndex,
    MetadataLength(usize),
    MetadataCrc,
    UnsupportedMetadata,
}

impl fmt::Display for Nvfp4Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for Nvfp4Error {}

fn put_u16(out: &mut [u8], offset: usize, value: u16) {
    out[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn put_u32(out: &mut [u8], offset: usize, value: u32) {
    out[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn put_f32(out: &mut [u8], offset: usize, value: f32) {
    put_u32(out, offset, value.to_bits());
}

fn get_u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(bytes[offset..offset + 2].try_into().unwrap())
}

fn get_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}

fn get_f32(bytes: &[u8], offset: usize) -> f32 {
    f32::from_bits(get_u32(bytes, offset))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scale_layout_is_bijective_for_fc1_shape() {
        let n = 1024;
        let k = 6144;
        let mut seen = vec![false; n * k / 16];
        for row in 0..n {
            for group in 0..k / 16 {
                let offset = scale_offset(row, group, n, k).unwrap();
                assert!(!seen[offset], "duplicate offset {offset}");
                seen[offset] = true;
            }
        }
        assert!(seen.into_iter().all(|value| value));
    }

    #[test]
    fn zero_tensor_is_canonical() {
        let packed =
            PackedNvfp4::pack(&vec![0.0; 129 * 65], 129, 65, Codec::OneDimensional).unwrap();
        assert_eq!(packed.metadata.global_scale, 1.0);
        assert!(packed.values.iter().all(|&value| value == 0));
        assert!(packed.scales.iter().all(|&value| value == 0));
        assert!(
            packed
                .dequantize()
                .unwrap()
                .iter()
                .all(|&value| value == 0.0)
        );
    }

    #[test]
    fn nibble_order_is_low_then_high() {
        let mut input = vec![0.0_f32; 128 * 64];
        input[0] = 6.0;
        input[1] = -6.0;
        let packed = PackedNvfp4::pack(&input, 128, 64, Codec::OneDimensional).unwrap();
        assert_eq!(packed.values[0] & 0x0f, 0x07);
        assert_eq!(packed.values[0] >> 4, 0x0f);
    }

    #[test]
    fn metadata_corruption_is_rejected() {
        let packed =
            PackedNvfp4::pack(&vec![1.0; 128 * 64], 128, 64, Codec::OneDimensional).unwrap();
        let mut metadata = packed.metadata.encode();
        metadata[44] ^= 1;
        assert_eq!(
            Nvfp4Metadata::decode(&metadata).unwrap_err(),
            Nvfp4Error::MetadataCrc
        );
    }
}
