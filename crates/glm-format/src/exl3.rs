use std::fmt;

use crate::crc32c;

pub const EXL3_MCG_MULTIPLIER: u32 = 0xCBAC_1FED;
pub const EXL3_CODEBOOK_MCG: u16 = 1;
pub const EXL3_SOURCE_VERSION: &str = "exllamav3-v0.0.43";
pub const EXL3_SOURCE_REVISION: &str = "c5d9c657966ffeeaa9353f0cc899f18629da4a13";
pub const EXL3_MODEL_REVISION: &str = "9297b9f1d53af5c67cffa01e30cc071a1ff7144b";

const METADATA_MAGIC: [u8; 8] = *b"GLX3TR3\0";
const METADATA_VERSION: u16 = 1;
const LOP3_MASK: u32 = 0x8FFF_8FFF;
const LOP3_OR: u32 = 0x3B60_3B60;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum Exl3Projection {
    Gate = 1,
    Up = 2,
    Down = 3,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Exl3Metadata {
    pub projection: Exl3Projection,
    pub layer: u16,
    pub expert: u16,
    pub rank: u8,
    pub bits: u8,
    pub logical_k: u32,
    pub logical_n: u32,
    pub trellis_words: u64,
    pub rotation_words: u64,
}

impl Exl3Metadata {
    pub const BYTES: usize = 96;

    pub fn new(
        projection: Exl3Projection,
        layer: u16,
        expert: u16,
        rank: u8,
        bits: u8,
        logical_k: u32,
        logical_n: u32,
    ) -> Result<Self, Exl3Error> {
        let trellis_words = trellis_word_count(logical_k, logical_n, bits)?;
        let rotation_words = u64::from(logical_k)
            .checked_add(u64::from(logical_n))
            .ok_or(Exl3Error::Overflow)?;
        let metadata = Self {
            projection,
            layer,
            expert,
            rank,
            bits,
            logical_k,
            logical_n,
            trellis_words,
            rotation_words,
        };
        metadata.validate()?;
        Ok(metadata)
    }

    pub fn validate(&self) -> Result<(), Exl3Error> {
        if self.layer < 3
            || self.layer > 78
            || self.expert >= 256
            || self.rank >= 4
            || self.bits != 3
            || self.logical_k == 0
            || self.logical_n == 0
            || !self.logical_k.is_multiple_of(128)
            || !self.logical_n.is_multiple_of(128)
        {
            return Err(Exl3Error::UnsupportedContract);
        }
        if self.trellis_words != trellis_word_count(self.logical_k, self.logical_n, self.bits)?
            || self.rotation_words
                != u64::from(self.logical_k)
                    .checked_add(u64::from(self.logical_n))
                    .ok_or(Exl3Error::Overflow)?
        {
            return Err(Exl3Error::ByteAccounting);
        }
        let expected_shape = match self.projection {
            Exl3Projection::Gate | Exl3Projection::Up => (6_144, 512),
            Exl3Projection::Down => (512, 6_144),
        };
        if self.layer <= 77 && (self.logical_k, self.logical_n) != expected_shape {
            return Err(Exl3Error::UnsupportedContract);
        }
        Ok(())
    }

    #[must_use]
    pub fn encode(&self) -> [u8; Self::BYTES] {
        let mut output = [0_u8; Self::BYTES];
        output[..8].copy_from_slice(&METADATA_MAGIC);
        output[8..10].copy_from_slice(&METADATA_VERSION.to_le_bytes());
        output[10..12].copy_from_slice(&EXL3_CODEBOOK_MCG.to_le_bytes());
        output[12] = self.projection as u8;
        output[13] = self.bits;
        output[14] = self.rank;
        output[16..18].copy_from_slice(&self.layer.to_le_bytes());
        output[18..20].copy_from_slice(&self.expert.to_le_bytes());
        output[20..24].copy_from_slice(&self.logical_k.to_le_bytes());
        output[24..28].copy_from_slice(&self.logical_n.to_le_bytes());
        output[28..32].copy_from_slice(&EXL3_MCG_MULTIPLIER.to_le_bytes());
        output[32..40].copy_from_slice(&self.trellis_words.to_le_bytes());
        output[40..48].copy_from_slice(&self.rotation_words.to_le_bytes());
        let crc = crc32c(&output);
        output[88..92].copy_from_slice(&crc.to_le_bytes());
        output
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, Exl3Error> {
        if bytes.len() != Self::BYTES {
            return Err(Exl3Error::MetadataLength(bytes.len()));
        }
        let mut checked = [0_u8; Self::BYTES];
        checked.copy_from_slice(bytes);
        let expected_crc = get_u32(bytes, 88);
        checked[88..92].fill(0);
        if crc32c(&checked) != expected_crc {
            return Err(Exl3Error::MetadataCrc);
        }
        if bytes[..8] != METADATA_MAGIC
            || get_u16(bytes, 8) != METADATA_VERSION
            || get_u16(bytes, 10) != EXL3_CODEBOOK_MCG
            || get_u32(bytes, 28) != EXL3_MCG_MULTIPLIER
            || bytes[15] != 0
            || bytes[48..88].iter().any(|&byte| byte != 0)
            || bytes[92..].iter().any(|&byte| byte != 0)
        {
            return Err(Exl3Error::UnsupportedContract);
        }
        let projection = match bytes[12] {
            1 => Exl3Projection::Gate,
            2 => Exl3Projection::Up,
            3 => Exl3Projection::Down,
            _ => return Err(Exl3Error::UnsupportedContract),
        };
        let metadata = Self {
            projection,
            bits: bytes[13],
            rank: bytes[14],
            layer: get_u16(bytes, 16),
            expert: get_u16(bytes, 18),
            logical_k: get_u32(bytes, 20),
            logical_n: get_u32(bytes, 24),
            trellis_words: get_u64(bytes, 32),
            rotation_words: get_u64(bytes, 40),
        };
        metadata.validate()?;
        Ok(metadata)
    }

    pub fn payload_bytes(&self) -> Result<u64, Exl3Error> {
        self.trellis_words
            .checked_add(self.rotation_words)
            .and_then(|words| words.checked_mul(2))
            .and_then(|bytes| bytes.checked_add(4))
            .ok_or(Exl3Error::Overflow)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Exl3Trellis {
    pub metadata: Exl3Metadata,
    /// Native ExLlamaV3 little-endian int16 view:
    /// `[K/16, N/16, 16*bits]`.
    pub trellis: Vec<u16>,
    /// FP16 input-side H128 scale/sign vector, length K.
    pub suh: Vec<u16>,
    /// FP16 output-side H128 scale/sign vector, length N.
    pub svh: Vec<u16>,
    pub mcg_marker: u32,
}

impl Exl3Trellis {
    /// Imports the concatenated source components in pinned safetensors
    /// offset order: `mcg`, `suh`, `svh`, `trellis`. All scalar and int16
    /// values are little-endian.
    pub fn from_source_payload(metadata: Exl3Metadata, payload: &[u8]) -> Result<Self, Exl3Error> {
        metadata.validate()?;
        let expected =
            usize::try_from(metadata.payload_bytes()?).map_err(|_| Exl3Error::Overflow)?;
        if payload.len() != expected {
            return Err(Exl3Error::ByteAccounting);
        }
        let k = metadata.logical_k as usize;
        let n = metadata.logical_n as usize;
        let mcg_marker = get_u32(payload, 0);
        let mut cursor = 4;
        let suh = read_u16s(payload, &mut cursor, k)?;
        let svh = read_u16s(payload, &mut cursor, n)?;
        let trellis = read_u16s(
            payload,
            &mut cursor,
            usize::try_from(metadata.trellis_words).map_err(|_| Exl3Error::Overflow)?,
        )?;
        if cursor != payload.len() {
            return Err(Exl3Error::ByteAccounting);
        }
        let tensor = Self {
            metadata,
            trellis,
            suh,
            svh,
            mcg_marker,
        };
        tensor.validate()?;
        Ok(tensor)
    }

    /// Imports the deterministic native-container split without concatenating
    /// or reordering the pinned source components.
    ///
    /// The primary plane is the little-endian I16 trellis in source order.
    /// The aux plane is `mcg`, `suh`, then `svh`, all little-endian.
    pub fn from_container_planes(
        metadata: Exl3Metadata,
        primary: &[u8],
        aux: &[u8],
    ) -> Result<Self, Exl3Error> {
        metadata.validate()?;
        let expected_primary = usize::try_from(
            metadata
                .trellis_words
                .checked_mul(2)
                .ok_or(Exl3Error::Overflow)?,
        )
        .map_err(|_| Exl3Error::Overflow)?;
        let expected_aux = usize::try_from(
            metadata
                .rotation_words
                .checked_mul(2)
                .and_then(|bytes| bytes.checked_add(4))
                .ok_or(Exl3Error::Overflow)?,
        )
        .map_err(|_| Exl3Error::Overflow)?;
        if primary.len() != expected_primary || aux.len() != expected_aux {
            return Err(Exl3Error::ByteAccounting);
        }

        let k = usize::try_from(metadata.logical_k).map_err(|_| Exl3Error::Overflow)?;
        let n = usize::try_from(metadata.logical_n).map_err(|_| Exl3Error::Overflow)?;
        let mut primary_cursor = 0;
        let trellis = read_u16s(
            primary,
            &mut primary_cursor,
            usize::try_from(metadata.trellis_words).map_err(|_| Exl3Error::Overflow)?,
        )?;
        let mcg_marker = get_u32(aux, 0);
        let mut aux_cursor = 4;
        let suh = read_u16s(aux, &mut aux_cursor, k)?;
        let svh = read_u16s(aux, &mut aux_cursor, n)?;
        if primary_cursor != primary.len() || aux_cursor != aux.len() {
            return Err(Exl3Error::ByteAccounting);
        }

        let tensor = Self {
            metadata,
            trellis,
            suh,
            svh,
            mcg_marker,
        };
        tensor.validate()?;
        Ok(tensor)
    }

    /// Serializes the source-order trellis used as the container primary
    /// plane. No serving layout or GPU-health claim is implied.
    pub fn primary_plane(&self) -> Result<Vec<u8>, Exl3Error> {
        self.validate()?;
        let mut output = Vec::with_capacity(
            self.trellis
                .len()
                .checked_mul(2)
                .ok_or(Exl3Error::Overflow)?,
        );
        for &word in &self.trellis {
            output.extend_from_slice(&word.to_le_bytes());
        }
        Ok(output)
    }

    /// Serializes `mcg + suh + svh` as the container aux plane.
    pub fn aux_plane(&self) -> Result<Vec<u8>, Exl3Error> {
        self.validate()?;
        let rotation_words = self
            .suh
            .len()
            .checked_add(self.svh.len())
            .ok_or(Exl3Error::Overflow)?;
        let capacity = rotation_words
            .checked_mul(2)
            .and_then(|bytes| bytes.checked_add(4))
            .ok_or(Exl3Error::Overflow)?;
        let mut output = Vec::with_capacity(capacity);
        output.extend_from_slice(&self.mcg_marker.to_le_bytes());
        for &word in self.suh.iter().chain(&self.svh) {
            output.extend_from_slice(&word.to_le_bytes());
        }
        Ok(output)
    }

    pub fn validate(&self) -> Result<(), Exl3Error> {
        self.metadata.validate()?;
        let trellis_words =
            usize::try_from(self.metadata.trellis_words).map_err(|_| Exl3Error::Overflow)?;
        let k = usize::try_from(self.metadata.logical_k).map_err(|_| Exl3Error::Overflow)?;
        let n = usize::try_from(self.metadata.logical_n).map_err(|_| Exl3Error::Overflow)?;
        if self.trellis.len() != trellis_words || self.suh.len() != k || self.svh.len() != n {
            return Err(Exl3Error::ByteAccounting);
        }
        if self.mcg_marker != EXL3_MCG_MULTIPLIER {
            return Err(Exl3Error::CodebookMarker);
        }
        if self
            .suh
            .iter()
            .chain(&self.svh)
            .any(|&bits| !f16_bits_to_f32(bits).is_finite())
        {
            return Err(Exl3Error::NonFiniteRotation);
        }
        Ok(())
    }

    /// Reconstructs the native K-major matrix before H128 rotations.
    pub fn reconstruct_native_f16(&self) -> Result<Vec<u16>, Exl3Error> {
        self.validate()?;
        let k = self.metadata.logical_k as usize;
        let n = self.metadata.logical_n as usize;
        let bits = usize::from(self.metadata.bits);
        let tile_words = 8 * bits;
        let tile_halves = 16 * bits;
        let mut output = vec![0_u16; k.checked_mul(n).ok_or(Exl3Error::Overflow)?];

        for k_tile in 0..k / 16 {
            for n_tile in 0..n / 16 {
                let tile_base = (k_tile * (n / 16) + n_tile) * tile_halves;
                let mut words = vec![0_u32; tile_words];
                for (word, halves) in words
                    .iter_mut()
                    .zip(self.trellis[tile_base..tile_base + tile_halves].chunks_exact(2))
                {
                    *word = u32::from(halves[0]) | (u32::from(halves[1]) << 16);
                }
                for lane in 0..32 {
                    let row0 = (lane % 4) * 2;
                    let rows = [row0, row0 + 1, row0 + 8, row0 + 9];
                    let col0 = lane / 8;
                    let col1 = col0 + 4;
                    let parity = (lane >> 2) & 1;
                    for weight in 0..8 {
                        let end_bit = (lane * 8 + weight + 257) * bits;
                        let start_bit = end_bit - 16;
                        let first_word = start_bit / 32;
                        let last_word = (end_bit - 1) / 32;
                        let shift = (last_word + 1) * 32 - end_bit;
                        let merged = (u64::from(words[first_word % tile_words]) << 32)
                            | u64::from(words[last_word % tile_words]);
                        let window = ((merged >> shift) & 0xffff) as u16;
                        let value = decode_3inst_f16(window);
                        let row = k_tile * 16 + rows[weight % 4];
                        let column =
                            n_tile * 16 + 2 * (if weight < 4 { col0 } else { col1 }) + parity;
                        output[row * n + column] = value;
                    }
                }
            }
        }
        Ok(output)
    }

    /// Materializes the unrounded effective matrix
    /// `diag(suh) * H * W_native * H * diag(svh)`.
    ///
    /// This is useful for inspection only. Serving and exact numerical tests
    /// must use [`Self::matmul_reference_f16`], because source execution has
    /// activation-dependent FP16 rounding boundaries.
    pub fn reconstruct_unrounded_effective_f32(&self) -> Result<Vec<f32>, Exl3Error> {
        let native = self.reconstruct_native_f16()?;
        let k = self.metadata.logical_k as usize;
        let n = self.metadata.logical_n as usize;
        let mut work: Vec<f32> = native.into_iter().map(f16_bits_to_f32).collect();

        // x * diag(suh) * H * W: transform W's row axis first, then scale
        // each resulting logical input row by suh.
        for column in 0..n {
            for block in (0..k).step_by(128) {
                let mut input = [0.0_f32; 128];
                for offset in 0..128 {
                    input[offset] = work[(block + offset) * n + column];
                }
                let transformed = hadamard_128(&input);
                for offset in 0..128 {
                    work[(block + offset) * n + column] =
                        transformed[offset] * f16_bits_to_f32(self.suh[block + offset]);
                }
            }
        }
        // Applying H and SVH to each activation result is equivalent to a
        // column-axis transform of W followed by the output-side scale.
        for row in 0..k {
            for block in (0..n).step_by(128) {
                let mut input = [0.0_f32; 128];
                input.copy_from_slice(&work[row * n + block..row * n + block + 128]);
                let transformed = hadamard_128(&input);
                for offset in 0..128 {
                    work[row * n + block + offset] =
                        transformed[offset] * f16_bits_to_f32(self.svh[block + offset]);
                }
            }
        }
        Ok(work)
    }

    /// Exact scalar CPU execution order for the pinned projection.
    ///
    /// Input and output are FP16 bit patterns. H128 and matrix products use
    /// fixed ascending-index FP32 accumulation; each source FP16 store
    /// boundary is reproduced explicitly.
    pub fn matmul_reference_f16(
        &self,
        activations: &[u16],
        rows: usize,
    ) -> Result<Vec<u16>, Exl3Error> {
        self.validate()?;
        let k = self.metadata.logical_k as usize;
        let n = self.metadata.logical_n as usize;
        if activations.len() != rows.checked_mul(k).ok_or(Exl3Error::Overflow)? {
            return Err(Exl3Error::InputLength);
        }
        let native = self.reconstruct_native_f16()?;
        let mut output = vec![0_u16; rows.checked_mul(n).ok_or(Exl3Error::Overflow)?];
        for row in 0..rows {
            let mut source = vec![0.0_f32; k];
            for index in 0..k {
                let scaled = f16_bits_to_f32(activations[row * k + index])
                    * f16_bits_to_f32(self.suh[index]);
                source[index] = f16_bits_to_f32(f32_to_f16_bits(scaled));
            }
            for block in source.chunks_exact_mut(128) {
                let input: [f32; 128] = block.try_into().expect("exact H128 block");
                let transformed = hadamard_128(&input);
                for (value, transformed) in block.iter_mut().zip(transformed) {
                    *value = f16_bits_to_f32(f32_to_f16_bits(transformed));
                }
            }
            let mut projected = vec![0.0_f32; n];
            for column in 0..n {
                let mut accumulator = 0.0_f32;
                for inner in 0..k {
                    accumulator += source[inner] * f16_bits_to_f32(native[inner * n + column]);
                }
                projected[column] = f16_bits_to_f32(f32_to_f16_bits(accumulator));
            }
            for (block_index, block) in projected.chunks_exact_mut(128).enumerate() {
                let input: [f32; 128] = block.try_into().expect("exact H128 block");
                let transformed = hadamard_128(&input);
                for offset in 0..128 {
                    let scaled =
                        transformed[offset] * f16_bits_to_f32(self.svh[block_index * 128 + offset]);
                    block[offset] = f16_bits_to_f32(f32_to_f16_bits(scaled));
                }
            }
            for (target, value) in output[row * n..(row + 1) * n].iter_mut().zip(projected) {
                *target = f32_to_f16_bits(value);
            }
        }
        Ok(output)
    }

    #[must_use]
    pub fn physical_bytes(&self) -> usize {
        Exl3Metadata::BYTES + self.trellis.len() * 2 + self.suh.len() * 2 + self.svh.len() * 2 + 4
    }
}

fn trellis_word_count(k: u32, n: u32, bits: u8) -> Result<u64, Exl3Error> {
    if k == 0 || n == 0 || !k.is_multiple_of(16) || !n.is_multiple_of(16) || bits == 0 {
        return Err(Exl3Error::UnsupportedContract);
    }
    u64::from(k / 16)
        .checked_mul(u64::from(n / 16))
        .and_then(|tiles| tiles.checked_mul(u64::from(16 * bits)))
        .ok_or(Exl3Error::Overflow)
}

fn decode_3inst_f16(window: u16) -> u16 {
    let value = u32::from(window).wrapping_mul(EXL3_MCG_MULTIPLIER);
    let packed = (value & LOP3_MASK) ^ LOP3_OR;
    let low = packed as u16;
    let high = (packed >> 16) as u16;
    f32_to_f16_bits(f16_bits_to_f32(low) + f16_bits_to_f32(high))
}

fn hadamard_128(input: &[f32; 128]) -> [f32; 128] {
    let mut output = [0.0_f32; 128];
    let normalization = 1.0_f32 / 128.0_f32.sqrt();
    for (row, value) in output.iter_mut().enumerate() {
        let mut sum = 0.0_f32;
        for (column, &input_value) in input.iter().enumerate() {
            let sign = if (row & column).count_ones() & 1 == 0 {
                1.0
            } else {
                -1.0
            };
            sum += sign * input_value;
        }
        *value = sum * normalization;
    }
    output
}

#[must_use]
pub fn f16_bits_to_f32(bits: u16) -> f32 {
    let sign = u32::from(bits & 0x8000) << 16;
    let exponent = (bits >> 10) & 0x1f;
    let fraction = u32::from(bits & 0x03ff);
    let output = match exponent {
        0 if fraction == 0 => sign,
        0 => {
            let mut significand = fraction;
            let mut exponent32 = 113_u32;
            while significand & 0x0400 == 0 {
                significand <<= 1;
                exponent32 -= 1;
            }
            sign | (exponent32 << 23) | ((significand & 0x03ff) << 13)
        }
        31 => sign | 0x7f80_0000 | (fraction << 13),
        _ => sign | ((u32::from(exponent) + 112) << 23) | (fraction << 13),
    };
    f32::from_bits(output)
}

#[must_use]
pub fn f32_to_f16_bits(value: f32) -> u16 {
    let bits = value.to_bits();
    let sign = ((bits >> 16) & 0x8000) as u16;
    let exponent = ((bits >> 23) & 0xff) as i32;
    let fraction = bits & 0x007f_ffff;
    if exponent == 255 {
        return sign
            | if fraction == 0 {
                0x7c00
            } else {
                0x7e00 | ((fraction >> 13) as u16 & 0x01ff)
            };
    }
    let half_exponent = exponent - 127 + 15;
    if half_exponent >= 31 {
        return sign | 0x7c00;
    }
    if half_exponent <= 0 {
        if half_exponent < -10 {
            return sign;
        }
        let significand = fraction | 0x0080_0000;
        let shift = u32::try_from(14 - half_exponent).expect("bounded shift");
        let mut rounded = significand >> shift;
        let remainder_mask = (1_u32 << shift) - 1;
        let remainder = significand & remainder_mask;
        let halfway = 1_u32 << (shift - 1);
        if remainder > halfway || (remainder == halfway && rounded & 1 != 0) {
            rounded += 1;
        }
        return sign | rounded as u16;
    }
    let mut rounded_fraction = fraction >> 13;
    let remainder = fraction & 0x1fff;
    if remainder > 0x1000 || (remainder == 0x1000 && rounded_fraction & 1 != 0) {
        rounded_fraction += 1;
        if rounded_fraction == 0x400 {
            let next_exponent = half_exponent + 1;
            if next_exponent >= 31 {
                return sign | 0x7c00;
            }
            return sign | ((next_exponent as u16) << 10);
        }
    }
    sign | ((half_exponent as u16) << 10) | rounded_fraction as u16
}

fn get_u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(
        bytes[offset..offset + 2]
            .try_into()
            .expect("bounded metadata"),
    )
}

fn get_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(
        bytes[offset..offset + 4]
            .try_into()
            .expect("bounded metadata"),
    )
}

fn get_u64(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(
        bytes[offset..offset + 8]
            .try_into()
            .expect("bounded metadata"),
    )
}

fn read_u16s(bytes: &[u8], cursor: &mut usize, count: usize) -> Result<Vec<u16>, Exl3Error> {
    let byte_count = count.checked_mul(2).ok_or(Exl3Error::Overflow)?;
    let end = cursor.checked_add(byte_count).ok_or(Exl3Error::Overflow)?;
    let source = bytes.get(*cursor..end).ok_or(Exl3Error::ByteAccounting)?;
    let output = source
        .chunks_exact(2)
        .map(|word| u16::from_le_bytes([word[0], word[1]]))
        .collect();
    *cursor = end;
    Ok(output)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Exl3Error {
    MetadataLength(usize),
    MetadataCrc,
    UnsupportedContract,
    CodebookMarker,
    ByteAccounting,
    NonFiniteRotation,
    Overflow,
    InputLength,
}

impl fmt::Display for Exl3Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for Exl3Error {}

#[cfg(test)]
mod tests {
    use sha2::{Digest, Sha256};

    use super::*;

    fn fixture() -> Exl3Trellis {
        let metadata = Exl3Metadata::new(Exl3Projection::Gate, 3, 0, 0, 3, 6_144, 512).unwrap();
        let mut state = 0x0002_c026_0721_u64;
        let mut trellis = Vec::with_capacity(metadata.trellis_words as usize);
        for _ in 0..metadata.trellis_words {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            trellis.push(state as u16);
        }
        Exl3Trellis {
            metadata,
            trellis,
            suh: vec![0x3c00; 6_144],
            svh: vec![0x3c00; 512],
            mcg_marker: EXL3_MCG_MULTIPLIER,
        }
    }

    #[test]
    fn metadata_is_deterministic_and_corruption_detected() {
        let metadata = fixture().metadata;
        let first = metadata.encode();
        let second = metadata.encode();
        assert_eq!(first, second);
        assert_eq!(Exl3Metadata::decode(&first).unwrap(), metadata);
        let mut corrupt = first;
        corrupt[20] ^= 1;
        assert_eq!(Exl3Metadata::decode(&corrupt), Err(Exl3Error::MetadataCrc));
    }

    #[test]
    fn glm_rank_slab_byte_arithmetic_is_exact() {
        let gate = Exl3Metadata::new(Exl3Projection::Gate, 3, 0, 0, 3, 6_144, 512).unwrap();
        let down = Exl3Metadata::new(Exl3Projection::Down, 3, 0, 0, 3, 512, 6_144).unwrap();
        assert_eq!(gate.trellis_words * 2, 1_179_648);
        assert_eq!(down.trellis_words * 2, 1_179_648);
        assert_eq!(gate.payload_bytes().unwrap(), 1_192_964);
        assert_eq!(down.payload_bytes().unwrap(), 1_192_964);
        assert_eq!(3 * gate.payload_bytes().unwrap(), 3_578_892);
    }

    #[test]
    fn native_decode_has_stable_content_digest() {
        let decoded = fixture().reconstruct_native_f16().unwrap();
        let mut hasher = Sha256::new();
        for bits in decoded {
            hasher.update(bits.to_le_bytes());
        }
        let digest: [u8; 32] = hasher.finalize().into();
        assert_eq!(
            digest,
            [
                0x72, 0xfd, 0x64, 0x9c, 0x52, 0x26, 0x34, 0x30, 0xab, 0xb7, 0x4e, 0x90, 0x36, 0x42,
                0x87, 0x8d, 0x7b, 0x28, 0x15, 0x03, 0xe1, 0x89, 0xfc, 0xe7, 0xe8, 0x06, 0x0b, 0xe2,
                0xbc, 0x5b, 0x4e, 0x15,
            ]
        );
    }

    #[test]
    fn rotations_fail_closed_on_non_finite_values() {
        let mut tensor = fixture();
        tensor.suh[7] = 0x7c00;
        assert_eq!(tensor.validate(), Err(Exl3Error::NonFiniteRotation));
    }

    #[test]
    fn source_component_order_round_trips() {
        let tensor = fixture();
        let mut payload = Vec::new();
        payload.extend_from_slice(&tensor.mcg_marker.to_le_bytes());
        for word in tensor.suh.iter().chain(&tensor.svh).chain(&tensor.trellis) {
            payload.extend_from_slice(&word.to_le_bytes());
        }
        let imported = Exl3Trellis::from_source_payload(tensor.metadata.clone(), &payload).unwrap();
        assert_eq!(imported, tensor);
    }

    #[test]
    fn native_container_planes_round_trip_without_repacking() {
        let tensor = fixture();
        let primary = tensor.primary_plane().unwrap();
        let aux = tensor.aux_plane().unwrap();
        assert_eq!(primary.len(), 1_179_648);
        assert_eq!(aux.len(), 13_316);
        let imported =
            Exl3Trellis::from_container_planes(tensor.metadata.clone(), &primary, &aux).unwrap();
        assert_eq!(imported, tensor);
    }

    #[test]
    fn fp16_conversions_cover_boundaries_and_ties() {
        for bits in 0_u16..=u16::MAX {
            let exponent = bits & 0x7c00;
            if exponent != 0x7c00 {
                assert_eq!(f32_to_f16_bits(f16_bits_to_f32(bits)), bits);
            }
        }
        assert_eq!(f32_to_f16_bits(1.0), 0x3c00);
        assert_eq!(f16_bits_to_f32(0x0001), 2.0_f32.powi(-24));
    }

    #[test]
    fn h128_and_matmul_rounding_path_is_finite_and_deterministic() {
        let metadata = Exl3Metadata::new(Exl3Projection::Gate, 78, 0, 0, 3, 128, 128).unwrap();
        let tensor = Exl3Trellis {
            trellis: (0..metadata.trellis_words)
                .map(|index| (index as u16).wrapping_mul(40503))
                .collect(),
            suh: vec![0x3c00; 128],
            svh: vec![0x3c00; 128],
            mcg_marker: EXL3_MCG_MULTIPLIER,
            metadata,
        };
        let input: Vec<_> = (0..128)
            .map(|index| f32_to_f16_bits((index as f32 - 63.0) / 64.0))
            .collect();
        let first = tensor.matmul_reference_f16(&input, 1).unwrap();
        let second = tensor.matmul_reference_f16(&input, 1).unwrap();
        assert_eq!(first, second);
        assert!(first.iter().all(|&bits| f16_bits_to_f32(bits).is_finite()));
    }
}
