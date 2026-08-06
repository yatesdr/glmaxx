use std::fmt;

use sha2::{Digest, Sha256};

use crate::{
    Exl3Metadata, Exl3Trellis, Nvfp4Metadata, PackedNvfp4, crc32c, nvfp4::validate_nvfp4_planes,
};

pub const HEADER_BYTES: usize = 4096;
pub(crate) const DESCRIPTOR_BYTES: usize = 256;
pub(crate) const ALIGNMENT: usize = 4096;
pub const NATIVE_PAYLOAD_ALIGNMENT: u32 = 256;
pub(crate) const PAYLOAD_ALIGNMENT: usize = NATIVE_PAYLOAD_ALIGNMENT as usize;
pub(crate) const CODEC_BF16_ROW_MAJOR: u16 = 0x0001;
pub(crate) const CODEC_FP16_ROW_MAJOR: u16 = 0x0002;
pub(crate) const CODEC_FP32_ROW_MAJOR: u16 = 0x0003;
pub(crate) const CODEC_NVFP4_1D: u16 = 0x0100;
pub(crate) const CODEC_NVFP4_2D: u16 = 0x0101;
pub const CODEC_EXL3_SOURCE: u16 = 0x0200;
const HEADER_FLAG_DIRECT_KERNEL: u32 = 1 << 0;
const HEADER_FLAG_NVFP4: u32 = 1 << 1;
const HEADER_FLAG_EXL3: u32 = 1 << 2;
const HEADER_FLAG_HYBRID: u32 = 1 << 4;
pub(crate) const DESCRIPTOR_FLAG_AUX_REQUIRED: u8 = 1 << 7;
pub(crate) const DTYPE_BF16: u16 = 1;
pub(crate) const DTYPE_FP16: u16 = 2;
pub(crate) const DTYPE_PACKED_E2M1X2: u16 = 6;
pub(crate) const DTYPE_I16: u16 = 10;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u16)]
pub enum PlainDtype {
    Bf16 = CODEC_BF16_ROW_MAJOR,
    Fp16 = CODEC_FP16_ROW_MAJOR,
    Fp32 = CODEC_FP32_ROW_MAJOR,
}

impl PlainDtype {
    #[must_use]
    pub const fn element_bytes(self) -> u64 {
        match self {
            Self::Bf16 | Self::Fp16 => 2,
            Self::Fp32 => 4,
        }
    }

    const fn dtype_id(self) -> u16 {
        match self {
            Self::Bf16 => DTYPE_BF16,
            Self::Fp16 => DTYPE_FP16,
            Self::Fp32 => 3,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlainTensor {
    pub dtype: PlainDtype,
    pub ndim: u8,
    pub logical_shape: [u32; 4],
    pub padded_shape: [u32; 4],
    pub bytes: Vec<u8>,
}

impl PlainTensor {
    pub fn validate(&self) -> Result<(), RankFileError> {
        validate_plain_geometry(
            self.dtype,
            self.ndim,
            self.logical_shape,
            self.padded_shape,
            self.bytes.len() as u64,
        )?;
        validate_plain_padding(
            &self.bytes,
            self.dtype,
            self.ndim,
            self.logical_shape,
            self.padded_shape,
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TensorDescriptor {
    pub tensor_id: u32,
    pub name_offset: u32,
    pub name_bytes: u16,
    pub role_id: u16,
    pub layer_id: i16,
    pub expert_id: i16,
    pub codec_id: u16,
    pub logical_dtype: u16,
    pub stored_dtype: u16,
    pub tp_shard_axis: i8,
    pub ndim: u8,
    pub flags: u8,
    pub logical_shape: [u32; 4],
    pub padded_shape: [u32; 4],
    pub payload_offset: u64,
    pub payload_bytes: u64,
    pub aux_offset: u64,
    pub aux_bytes: u64,
    pub codec_metadata_offset: u64,
    pub codec_metadata_bytes: u64,
    pub payload_alignment: u32,
    pub quant_group_elements: u32,
    pub payload_sha256: [u8; 32],
    pub aux_sha256: [u8; 32],
    pub codec_metadata_sha256: [u8; 32],
}

impl TensorDescriptor {
    pub(crate) fn encode(&self) -> [u8; DESCRIPTOR_BYTES] {
        let mut out = [0_u8; DESCRIPTOR_BYTES];
        put_u32(&mut out, 0, self.tensor_id);
        put_u32(&mut out, 4, self.name_offset);
        put_u16(&mut out, 8, self.name_bytes);
        put_u16(&mut out, 10, self.role_id);
        put_i16(&mut out, 12, self.layer_id);
        put_i16(&mut out, 14, self.expert_id);
        put_u16(&mut out, 16, self.codec_id);
        put_u16(&mut out, 18, self.logical_dtype);
        put_u16(&mut out, 20, self.stored_dtype);
        out[22] = self.tp_shard_axis.to_le_bytes()[0];
        out[23] = self.ndim;
        out[24] = self.flags;
        for (index, &dimension) in self.logical_shape.iter().enumerate() {
            put_u32(&mut out, 28 + index * 4, dimension);
        }
        for (index, &dimension) in self.padded_shape.iter().enumerate() {
            put_u32(&mut out, 44 + index * 4, dimension);
        }
        let logical_elements = shape_elements(self.logical_shape, self.ndim).unwrap();
        put_u64(&mut out, 64, logical_elements);
        put_u64(&mut out, 72, self.payload_offset);
        put_u64(&mut out, 80, self.payload_bytes);
        put_u64(&mut out, 88, self.aux_offset);
        put_u64(&mut out, 96, self.aux_bytes);
        put_u64(&mut out, 104, self.codec_metadata_offset);
        put_u64(&mut out, 112, self.codec_metadata_bytes);
        put_u32(&mut out, 120, self.payload_alignment);
        put_u32(&mut out, 124, self.quant_group_elements);
        out[128..160].copy_from_slice(&self.payload_sha256);
        out[160..192].copy_from_slice(&self.aux_sha256);
        out[192..224].copy_from_slice(&self.codec_metadata_sha256);
        out
    }

    pub(crate) fn decode(bytes: &[u8]) -> Result<Self, RankFileError> {
        if bytes.len() != DESCRIPTOR_BYTES
            || bytes[25..28].iter().any(|&value| value != 0)
            || bytes[224..].iter().any(|&value| value != 0)
            || bytes[23] == 0
            || bytes[23] > 4
            || (bytes[24] & 0b0000_0111).count_ones() > 1
            || get_u32(bytes, 120) < PAYLOAD_ALIGNMENT as u32
            || !get_u32(bytes, 120).is_power_of_two()
        {
            return Err(RankFileError::Descriptor);
        }
        let mut payload_sha256 = [0_u8; 32];
        payload_sha256.copy_from_slice(&bytes[128..160]);
        let mut aux_sha256 = [0_u8; 32];
        aux_sha256.copy_from_slice(&bytes[160..192]);
        let mut codec_metadata_sha256 = [0_u8; 32];
        codec_metadata_sha256.copy_from_slice(&bytes[192..224]);
        let logical_shape = std::array::from_fn(|index| get_u32(bytes, 28 + index * 4));
        let padded_shape = std::array::from_fn(|index| get_u32(bytes, 44 + index * 4));
        let ndim = bytes[23];
        let logical_elements = shape_elements(logical_shape, ndim)?;
        if logical_shape[usize::from(ndim)..]
            .iter()
            .any(|&extent| extent != 1)
            || padded_shape[usize::from(ndim)..]
                .iter()
                .any(|&extent| extent != 1)
        {
            return Err(RankFileError::Descriptor);
        }
        if get_u64(bytes, 64) != logical_elements {
            return Err(RankFileError::Descriptor);
        }
        Ok(Self {
            tensor_id: get_u32(bytes, 0),
            name_offset: get_u32(bytes, 4),
            name_bytes: get_u16(bytes, 8),
            role_id: get_u16(bytes, 10),
            layer_id: get_i16(bytes, 12),
            expert_id: get_i16(bytes, 14),
            codec_id: get_u16(bytes, 16),
            logical_dtype: get_u16(bytes, 18),
            stored_dtype: get_u16(bytes, 20),
            tp_shard_axis: i8::from_le_bytes([bytes[22]]),
            ndim,
            flags: bytes[24],
            logical_shape,
            padded_shape,
            payload_offset: get_u64(bytes, 72),
            payload_bytes: get_u64(bytes, 80),
            aux_offset: get_u64(bytes, 88),
            aux_bytes: get_u64(bytes, 96),
            codec_metadata_offset: get_u64(bytes, 104),
            codec_metadata_bytes: get_u64(bytes, 112),
            payload_alignment: get_u32(bytes, 120),
            quant_group_elements: get_u32(bytes, 124),
            payload_sha256,
            aux_sha256,
            codec_metadata_sha256,
        })
    }
}

#[derive(Clone, Debug)]
pub enum TensorPayload {
    Plain(PlainTensor),
    Nvfp4(PackedNvfp4),
    /// Pinned EXL3 source components. This is an inspection/CPU-proof
    /// payload until a reviewed direct SM120 kernel qualifies codec 0x0200.
    Exl3Source(Exl3Trellis),
}

#[derive(Clone, Debug)]
pub struct TensorRecord {
    pub tensor_id: u32,
    pub name: String,
    pub role_id: u16,
    pub layer_id: i16,
    pub expert_id: i16,
    pub tp_shard_axis: i8,
    pub flags: u8,
    pub payload: TensorPayload,
}

#[derive(Clone, Debug)]
pub struct RankFileBuilder {
    pub rank: u32,
    pub manifest: Vec<u8>,
    pub model_config_sha256: [u8; 32],
    pub tokenizer_bundle_sha256: [u8; 32],
    pub chat_template_sha256: [u8; 32],
    pub weight_policy_sha256: [u8; 32],
    pub kernel_abi_sha256: [u8; 32],
    pub tensors: Vec<TensorRecord>,
}

impl RankFileBuilder {
    pub fn derive_conversion_uuid(builders: &[Self; 4]) -> Result<[u8; 16], RankFileError> {
        let mut hasher = Sha256::new();
        hasher.update(b"g5n-conversion-v0\0");
        for (expected_rank, builder) in builders.iter().enumerate() {
            if builder.rank != u32::try_from(expected_rank).unwrap() {
                return Err(RankFileError::Rank);
            }
            let prepared = builder.prepare()?;
            hasher.update(prepared.manifest_hash);
            hasher.update(prepared.descriptor_hash);
            hasher.update(prepared.payload_hash);
        }
        Ok(first_16(hasher.finalize().into()))
    }

    pub fn build(&self, conversion_uuid: [u8; 16]) -> Result<Vec<u8>, RankFileError> {
        if self.rank > 3 {
            return Err(RankFileError::Rank);
        }
        let prepared = self.prepare()?;
        let total = prepared
            .payload_offset
            .checked_add(prepared.payload.len())
            .ok_or(RankFileError::Overflow)?;
        let mut file = vec![0_u8; total];
        file[prepared.manifest_offset..prepared.manifest_offset + self.manifest.len()]
            .copy_from_slice(&self.manifest);
        file[prepared.descriptor_offset
            ..prepared.descriptor_offset + prepared.descriptor_bytes.len()]
            .copy_from_slice(&prepared.descriptor_bytes);
        file[prepared.string_offset..prepared.string_offset + prepared.strings.len()]
            .copy_from_slice(&prepared.strings);
        file[prepared.metadata_offset..prepared.metadata_offset + prepared.metadata.len()]
            .copy_from_slice(&prepared.metadata);
        file[prepared.payload_offset..prepared.payload_offset + prepared.payload.len()]
            .copy_from_slice(&prepared.payload);

        let header = encode_rank_header(
            &RankHeaderFields {
                rank: self.rank,
                tensor_count: self.tensors.len(),
                header_flags: prepared.header_flags,
                manifest_offset: prepared.manifest_offset,
                manifest_bytes: self.manifest.len(),
                descriptor_offset: prepared.descriptor_offset,
                descriptor_bytes: prepared.descriptor_bytes.len(),
                string_offset: prepared.string_offset,
                string_bytes: prepared.strings.len(),
                metadata_offset: prepared.metadata_offset,
                metadata_bytes: prepared.metadata.len(),
                payload_offset: prepared.payload_offset,
                payload_bytes: prepared.payload.len(),
                model_config_sha256: self.model_config_sha256,
                tokenizer_bundle_sha256: self.tokenizer_bundle_sha256,
                chat_template_sha256: self.chat_template_sha256,
                weight_policy_sha256: self.weight_policy_sha256,
                kernel_abi_sha256: self.kernel_abi_sha256,
                manifest_sha256: prepared.manifest_hash,
                descriptor_sha256: prepared.descriptor_hash,
                payload_sha256: prepared.payload_hash,
                string_sha256: prepared.string_hash,
                metadata_sha256: prepared.metadata_hash,
            },
            conversion_uuid,
        )?;
        file[..HEADER_BYTES].copy_from_slice(&header);
        Ok(file)
    }

    fn prepare(&self) -> Result<Prepared, RankFileError> {
        if self.tensors.is_empty() {
            return Err(RankFileError::TensorCount);
        }
        let manifest_offset = ALIGNMENT;
        let descriptor_offset = align_up(
            manifest_offset
                .checked_add(self.manifest.len())
                .ok_or(RankFileError::Overflow)?,
            ALIGNMENT,
        )?;
        let descriptor_len = self
            .tensors
            .len()
            .checked_mul(DESCRIPTOR_BYTES)
            .ok_or(RankFileError::Overflow)?;
        let string_offset = align_up(
            descriptor_offset
                .checked_add(descriptor_len)
                .ok_or(RankFileError::Overflow)?,
            ALIGNMENT,
        )?;
        let mut strings = Vec::new();
        let mut name_offsets = Vec::with_capacity(self.tensors.len());
        for tensor in &self.tensors {
            let offset = strings.len();
            strings.extend_from_slice(tensor.name.as_bytes());
            name_offsets.push(offset);
        }
        let metadata_offset = align_up(
            string_offset
                .checked_add(strings.len())
                .ok_or(RankFileError::Overflow)?,
            ALIGNMENT,
        )?;
        let mut metadata = Vec::new();
        let mut metadata_locals = Vec::with_capacity(self.tensors.len());
        let mut encoded_payloads = Vec::with_capacity(self.tensors.len());
        for (index, tensor) in self.tensors.iter().enumerate() {
            if tensor.tensor_id != u32::try_from(index).unwrap() {
                return Err(RankFileError::TensorId);
            }
            let encoded = EncodedTensor::from_record(tensor, self.rank)?;
            let metadata_local = metadata.len();
            metadata.extend_from_slice(&encoded.metadata);
            metadata_locals.push(metadata_local);
            encoded_payloads.push(encoded);
        }
        let payload_offset = align_up(
            metadata_offset
                .checked_add(metadata.len())
                .ok_or(RankFileError::Overflow)?,
            ALIGNMENT,
        )?;
        let mut payload = Vec::new();
        let mut descriptors = Vec::with_capacity(self.tensors.len());
        for (index, (tensor, encoded)) in self.tensors.iter().zip(&encoded_payloads).enumerate() {
            let metadata_local = metadata_locals[index];
            let value_local = align_up(payload.len(), PAYLOAD_ALIGNMENT)?;
            payload.resize(value_local, 0);
            payload.extend_from_slice(&encoded.primary);
            let scale_local = align_up(payload.len(), PAYLOAD_ALIGNMENT)?;
            payload.resize(scale_local, 0);
            payload.extend_from_slice(&encoded.aux);
            descriptors.push(TensorDescriptor {
                tensor_id: tensor.tensor_id,
                name_offset: u32::try_from(name_offsets[index])
                    .map_err(|_| RankFileError::Overflow)?,
                name_bytes: u16::try_from(tensor.name.len())
                    .map_err(|_| RankFileError::Overflow)?,
                role_id: tensor.role_id,
                layer_id: tensor.layer_id,
                expert_id: tensor.expert_id,
                codec_id: encoded.codec_id,
                logical_dtype: encoded.logical_dtype,
                stored_dtype: encoded.stored_dtype,
                tp_shard_axis: tensor.tp_shard_axis,
                ndim: encoded.ndim,
                flags: tensor.flags
                    | if encoded.aux_required {
                        DESCRIPTOR_FLAG_AUX_REQUIRED
                    } else {
                        0
                    },
                logical_shape: encoded.logical_shape,
                padded_shape: encoded.padded_shape,
                payload_offset: u64::try_from(
                    payload_offset
                        .checked_add(value_local)
                        .ok_or(RankFileError::Overflow)?,
                )
                .map_err(|_| RankFileError::Overflow)?,
                payload_bytes: encoded.primary.len() as u64,
                aux_offset: u64::try_from(
                    payload_offset
                        .checked_add(scale_local)
                        .ok_or(RankFileError::Overflow)?,
                )
                .map_err(|_| RankFileError::Overflow)?,
                aux_bytes: encoded.aux.len() as u64,
                codec_metadata_offset: u64::try_from(
                    metadata_offset
                        .checked_add(metadata_local)
                        .ok_or(RankFileError::Overflow)?,
                )
                .map_err(|_| RankFileError::Overflow)?,
                codec_metadata_bytes: encoded.metadata.len() as u64,
                payload_alignment: PAYLOAD_ALIGNMENT as u32,
                quant_group_elements: encoded.quant_group_elements,
                payload_sha256: sha256(&encoded.primary),
                aux_sha256: sha256(&encoded.aux),
                codec_metadata_sha256: sha256(&encoded.metadata),
            });
        }
        let descriptor_bytes: Vec<u8> = descriptors
            .iter()
            .flat_map(TensorDescriptor::encode)
            .collect();
        Ok(Prepared {
            manifest_offset,
            descriptor_offset,
            string_offset,
            metadata_offset,
            payload_offset,
            manifest_hash: sha256(&self.manifest),
            descriptor_hash: sha256(&descriptor_bytes),
            payload_hash: sha256(&payload),
            string_hash: sha256(&strings),
            metadata_hash: sha256(&metadata),
            header_flags: derive_header_flags(&descriptors)?,
            descriptor_bytes,
            strings,
            metadata,
            payload,
        })
    }
}

#[derive(Clone, Debug)]
struct Prepared {
    manifest_offset: usize,
    descriptor_offset: usize,
    string_offset: usize,
    metadata_offset: usize,
    payload_offset: usize,
    manifest_hash: [u8; 32],
    descriptor_hash: [u8; 32],
    payload_hash: [u8; 32],
    string_hash: [u8; 32],
    metadata_hash: [u8; 32],
    header_flags: u32,
    descriptor_bytes: Vec<u8>,
    strings: Vec<u8>,
    metadata: Vec<u8>,
    payload: Vec<u8>,
}

#[derive(Clone, Debug)]
pub struct RankFile {
    bytes: Vec<u8>,
    pub rank: u32,
    pub conversion_uuid: [u8; 16],
    pub file_uuid: [u8; 16],
    pub header_flags: u32,
    pub descriptors: Vec<TensorDescriptor>,
    manifest_hash: [u8; 32],
    descriptor_hash: [u8; 32],
    payload_hash: [u8; 32],
    string_range: std::ops::Range<usize>,
}

impl RankFile {
    pub fn read(bytes: Vec<u8>) -> Result<Self, RankFileError> {
        if bytes.len() < HEADER_BYTES {
            return Err(RankFileError::Truncated);
        }
        let header = &bytes[..HEADER_BYTES];
        let header_flags = get_u32(header, 20);
        if &header[0..8] != b"GLM5NAT0"
            || get_u16(header, 8) != 0
            || get_u16(header, 10) != 2
            || get_u32(header, 12) != HEADER_BYTES as u32
            || get_u32(header, 16) != 0x0102_0304
            || header_flags & !0b1_1111 != 0
            || get_u32(header, 28) != 4
            || header[36..40].iter().any(|&value| value != 0)
            || get_u64(header, 408) != 0
            || header[484..].iter().any(|&value| value != 0)
        {
            return Err(RankFileError::Header);
        }
        let mut checked_header = [0_u8; HEADER_BYTES];
        checked_header.copy_from_slice(header);
        let expected_crc = get_u32(header, 416);
        checked_header[416..420].fill(0);
        if crc32c(&checked_header) != expected_crc {
            return Err(RankFileError::HeaderCrc);
        }
        let rank = get_u32(header, 24);
        if rank > 3 {
            return Err(RankFileError::Rank);
        }
        let tensor_count =
            usize::try_from(get_u32(header, 32)).map_err(|_| RankFileError::Overflow)?;
        if tensor_count == 0 {
            return Err(RankFileError::TensorCount);
        }
        let manifest = checked_range(header, 40, 48, bytes.len(), ALIGNMENT)?;
        let descriptor = checked_range(header, 56, 64, bytes.len(), ALIGNMENT)?;
        let strings = checked_range(header, 72, 80, bytes.len(), ALIGNMENT)?;
        let metadata = checked_range(header, 88, 96, bytes.len(), ALIGNMENT)?;
        let payload = checked_range(header, 104, 112, bytes.len(), ALIGNMENT)?;
        if descriptor.len() != tensor_count * DESCRIPTOR_BYTES
            || manifest.start != ALIGNMENT
            || descriptor.start != align_up(manifest.end, ALIGNMENT)?
            || strings.start != align_up(descriptor.end, ALIGNMENT)?
            || metadata.start != align_up(strings.end, ALIGNMENT)?
            || payload.start != align_up(metadata.end, ALIGNMENT)?
            || payload.end != bytes.len()
            || !ordered_non_overlapping([
                0..HEADER_BYTES,
                manifest.clone(),
                descriptor.clone(),
                strings.clone(),
                metadata.clone(),
                payload.clone(),
            ])
        {
            return Err(RankFileError::Region);
        }
        if sha256(&bytes[manifest.clone()]) != header[280..312]
            || sha256(&bytes[descriptor.clone()]) != header[312..344]
            || sha256(&bytes[payload.clone()]) != header[344..376]
            || sha256(&bytes[strings.clone()]) != header[420..452]
            || sha256(&bytes[metadata.clone()]) != header[452..484]
        {
            return Err(RankFileError::StrongHash);
        }
        let mut manifest_hash = [0_u8; 32];
        manifest_hash.copy_from_slice(&header[280..312]);
        let mut descriptor_hash = [0_u8; 32];
        descriptor_hash.copy_from_slice(&header[312..344]);
        let mut payload_hash = [0_u8; 32];
        payload_hash.copy_from_slice(&header[344..376]);
        let mut descriptors = Vec::with_capacity(tensor_count);
        for index in 0..tensor_count {
            let start = descriptor.start + index * DESCRIPTOR_BYTES;
            let descriptor = TensorDescriptor::decode(&bytes[start..start + DESCRIPTOR_BYTES])?;
            if descriptor.tensor_id != u32::try_from(index).unwrap() {
                return Err(RankFileError::TensorId);
            }
            let name_start = strings
                .start
                .checked_add(
                    usize::try_from(descriptor.name_offset).map_err(|_| RankFileError::Overflow)?,
                )
                .ok_or(RankFileError::Overflow)?;
            let name_end = name_start
                .checked_add(usize::from(descriptor.name_bytes))
                .ok_or(RankFileError::Overflow)?;
            if name_end > strings.end || std::str::from_utf8(&bytes[name_start..name_end]).is_err()
            {
                return Err(RankFileError::StringTable);
            }
            validate_tensor_regions(&bytes, &descriptor, &metadata, &payload, rank)?;
            descriptors.push(descriptor);
        }
        validate_canonical_tensor_layout(&bytes, &descriptors, &metadata, &payload)?;
        if derive_header_flags(&descriptors)? != header_flags {
            return Err(RankFileError::HeaderFlags);
        }
        let mut file_uuid = [0_u8; 16];
        file_uuid.copy_from_slice(&header[376..392]);
        let mut conversion_uuid = [0_u8; 16];
        conversion_uuid.copy_from_slice(&header[392..408]);
        let mut uuid_hasher = Sha256::new();
        uuid_hasher.update(b"g5n-file-v0\0");
        uuid_hasher.update(conversion_uuid);
        uuid_hasher.update(rank.to_le_bytes());
        uuid_hasher.update(manifest_hash);
        uuid_hasher.update(descriptor_hash);
        uuid_hasher.update(payload_hash);
        if first_16(uuid_hasher.finalize().into()) != file_uuid {
            return Err(RankFileError::FileUuid);
        }
        Ok(Self {
            bytes,
            rank,
            conversion_uuid,
            file_uuid,
            header_flags,
            descriptors,
            manifest_hash,
            descriptor_hash,
            payload_hash,
            string_range: strings,
        })
    }

    pub fn validate_rank_set(files: &[Self; 4]) -> Result<(), RankFileError> {
        let conversion_uuid = files[0].conversion_uuid;
        let mut hasher = Sha256::new();
        hasher.update(b"g5n-conversion-v0\0");
        for (expected_rank, file) in files.iter().enumerate() {
            if file.rank != u32::try_from(expected_rank).map_err(|_| RankFileError::Overflow)?
                || file.conversion_uuid != conversion_uuid
            {
                return Err(RankFileError::RankSet);
            }
            hasher.update(file.manifest_hash);
            hasher.update(file.descriptor_hash);
            hasher.update(file.payload_hash);
        }
        if first_16(hasher.finalize().into()) != conversion_uuid {
            return Err(RankFileError::RankSet);
        }
        Ok(())
    }

    pub fn tensor_name(&self, index: usize) -> Result<&str, RankFileError> {
        let descriptor = self.descriptors.get(index).ok_or(RankFileError::TensorId)?;
        let start = self.string_range.start + descriptor.name_offset as usize;
        let end = start + usize::from(descriptor.name_bytes);
        std::str::from_utf8(&self.bytes[start..end]).map_err(|_| RankFileError::StringTable)
    }

    pub fn tensor_primary(&self, index: usize) -> Result<&[u8], RankFileError> {
        let descriptor = self.descriptors.get(index).ok_or(RankFileError::TensorId)?;
        let range = u64_range(
            descriptor.payload_offset,
            descriptor.payload_bytes,
            self.bytes.len(),
        )?;
        Ok(&self.bytes[range])
    }

    pub fn tensor_aux(&self, index: usize) -> Result<&[u8], RankFileError> {
        let descriptor = self.descriptors.get(index).ok_or(RankFileError::TensorId)?;
        let range = u64_range(
            descriptor.aux_offset,
            descriptor.aux_bytes,
            self.bytes.len(),
        )?;
        Ok(&self.bytes[range])
    }

    pub fn tensor_codec_metadata(&self, index: usize) -> Result<&[u8], RankFileError> {
        let descriptor = self.descriptors.get(index).ok_or(RankFileError::TensorId)?;
        let range = u64_range(
            descriptor.codec_metadata_offset,
            descriptor.codec_metadata_bytes,
            self.bytes.len(),
        )?;
        Ok(&self.bytes[range])
    }

    /// Decodes a source EXL3 payload for inspection or CPU proof. This API
    /// intentionally does not advertise GPU load support.
    pub fn decode_exl3_source(&self, index: usize) -> Result<Exl3Trellis, RankFileError> {
        let descriptor = self.descriptors.get(index).ok_or(RankFileError::TensorId)?;
        if descriptor.codec_id != CODEC_EXL3_SOURCE {
            return Err(RankFileError::CodecMismatch);
        }
        let metadata = Exl3Metadata::decode(self.tensor_codec_metadata(index)?)
            .map_err(RankFileError::Exl3)?;
        Exl3Trellis::from_container_planes(
            metadata,
            self.tensor_primary(index)?,
            self.tensor_aux(index)?,
        )
        .map_err(RankFileError::Exl3)
    }
}

fn validate_tensor_regions(
    bytes: &[u8],
    descriptor: &TensorDescriptor,
    metadata_region: &std::ops::Range<usize>,
    payload_region: &std::ops::Range<usize>,
    rank: u32,
) -> Result<(), RankFileError> {
    let value = u64_range(
        descriptor.payload_offset,
        descriptor.payload_bytes,
        bytes.len(),
    )?;
    let aux = u64_range(descriptor.aux_offset, descriptor.aux_bytes, bytes.len())?;
    let metadata = u64_range(
        descriptor.codec_metadata_offset,
        descriptor.codec_metadata_bytes,
        bytes.len(),
    )?;
    if value.start % PAYLOAD_ALIGNMENT != 0
        || aux.start % PAYLOAD_ALIGNMENT != 0
        || value.start < payload_region.start
        || value.end > payload_region.end
        || aux.start < payload_region.start
        || aux.end > payload_region.end
        || metadata.start < metadata_region.start
        || metadata.end > metadata_region.end
        || sha256(&bytes[value.clone()]) != descriptor.payload_sha256
        || sha256(&bytes[aux.clone()]) != descriptor.aux_sha256
        || sha256(&bytes[metadata.clone()]) != descriptor.codec_metadata_sha256
    {
        return Err(RankFileError::TensorRegion);
    }
    match descriptor.codec_id {
        CODEC_BF16_ROW_MAJOR | CODEC_FP16_ROW_MAJOR | CODEC_FP32_ROW_MAJOR => {
            let dtype = match descriptor.codec_id {
                CODEC_BF16_ROW_MAJOR => PlainDtype::Bf16,
                CODEC_FP16_ROW_MAJOR => PlainDtype::Fp16,
                CODEC_FP32_ROW_MAJOR => PlainDtype::Fp32,
                _ => unreachable!(),
            };
            if descriptor.logical_dtype != dtype.dtype_id()
                || descriptor.stored_dtype != dtype.dtype_id()
                || descriptor.flags & DESCRIPTOR_FLAG_AUX_REQUIRED != 0
                || descriptor.quant_group_elements != 0
                || descriptor.aux_bytes != 0
                || descriptor.codec_metadata_bytes != 0
            {
                return Err(RankFileError::Descriptor);
            }
            validate_plain_geometry(
                dtype,
                descriptor.ndim,
                descriptor.logical_shape,
                descriptor.padded_shape,
                descriptor.payload_bytes,
            )?;
            validate_plain_padding(
                &bytes[value],
                dtype,
                descriptor.ndim,
                descriptor.logical_shape,
                descriptor.padded_shape,
            )?;
        }
        CODEC_NVFP4_1D | CODEC_NVFP4_2D => {
            let decoded = Nvfp4Metadata::decode(&bytes[metadata]).map_err(RankFileError::Nvfp4)?;
            if descriptor.logical_dtype != DTYPE_BF16
                || descriptor.stored_dtype != DTYPE_PACKED_E2M1X2
                || descriptor.ndim != 2
                || descriptor.flags & DESCRIPTOR_FLAG_AUX_REQUIRED == 0
                || descriptor.payload_alignment != PAYLOAD_ALIGNMENT as u32
                || descriptor.quant_group_elements != 16
                || decoded.logical_n != descriptor.logical_shape[0]
                || decoded.logical_k != descriptor.logical_shape[1]
                || decoded.padded_n != descriptor.padded_shape[0]
                || decoded.padded_k != descriptor.padded_shape[1]
                || decoded.codec as u16 != descriptor.codec_id
                || u64::from(decoded.value_plane_bytes) != descriptor.payload_bytes
                || u64::from(decoded.scale_plane_bytes) != descriptor.aux_bytes
            {
                return Err(RankFileError::Descriptor);
            }
            validate_nvfp4_planes(&decoded, &bytes[value], &bytes[aux])
                .map_err(RankFileError::Nvfp4)?;
        }
        CODEC_EXL3_SOURCE => {
            let decoded = Exl3Metadata::decode(&bytes[metadata]).map_err(RankFileError::Exl3)?;
            let layer =
                u16::try_from(descriptor.layer_id).map_err(|_| RankFileError::Descriptor)?;
            let expert =
                u16::try_from(descriptor.expert_id).map_err(|_| RankFileError::Descriptor)?;
            let expected_primary = decoded
                .trellis_words
                .checked_mul(2)
                .ok_or(RankFileError::Overflow)?;
            let expected_aux = decoded
                .rotation_words
                .checked_mul(2)
                .and_then(|bytes| bytes.checked_add(4))
                .ok_or(RankFileError::Overflow)?;
            if descriptor.logical_dtype != DTYPE_FP16
                || descriptor.stored_dtype != DTYPE_I16
                || descriptor.ndim != 2
                || descriptor.flags & DESCRIPTOR_FLAG_AUX_REQUIRED == 0
                || descriptor.payload_alignment != PAYLOAD_ALIGNMENT as u32
                || descriptor.quant_group_elements != 0
                || decoded.rank != rank as u8
                || decoded.layer != layer
                || decoded.expert != expert
                || decoded.logical_n != descriptor.logical_shape[0]
                || decoded.logical_k != descriptor.logical_shape[1]
                || descriptor.logical_shape != descriptor.padded_shape
                || descriptor.payload_bytes != expected_primary
                || descriptor.aux_bytes != expected_aux
            {
                return Err(RankFileError::Descriptor);
            }
            Exl3Trellis::from_container_planes(decoded, &bytes[value], &bytes[aux])
                .map_err(RankFileError::Exl3)?;
        }
        codec => return Err(RankFileError::UnsupportedCodec(codec)),
    }
    Ok(())
}

pub(crate) fn derive_header_flags(descriptors: &[TensorDescriptor]) -> Result<u32, RankFileError> {
    let mut saw_nvfp4 = false;
    let mut saw_exl3 = false;
    let mut saw_plain = false;
    for descriptor in descriptors {
        match descriptor.codec_id {
            CODEC_BF16_ROW_MAJOR | CODEC_FP16_ROW_MAJOR | CODEC_FP32_ROW_MAJOR => {
                saw_plain = true;
            }
            CODEC_NVFP4_1D | CODEC_NVFP4_2D => saw_nvfp4 = true,
            CODEC_EXL3_SOURCE => saw_exl3 = true,
            codec => return Err(RankFileError::UnsupportedCodec(codec)),
        }
    }
    let mut flags = 0;
    if (saw_plain || saw_nvfp4) && !saw_exl3 {
        flags |= HEADER_FLAG_DIRECT_KERNEL;
    }
    if saw_nvfp4 {
        flags |= HEADER_FLAG_NVFP4;
    }
    if saw_exl3 {
        flags |= HEADER_FLAG_EXL3;
    }
    if saw_nvfp4 && saw_exl3 {
        flags |= HEADER_FLAG_HYBRID;
    }
    Ok(flags)
}

pub(crate) fn validate_plain_geometry(
    dtype: PlainDtype,
    ndim: u8,
    logical_shape: [u32; 4],
    padded_shape: [u32; 4],
    payload_bytes: u64,
) -> Result<(), RankFileError> {
    if !(1..=4).contains(&ndim)
        || logical_shape
            .iter()
            .zip(padded_shape)
            .take(usize::from(ndim))
            .any(|(&logical, padded)| logical == 0 || logical > padded)
        || logical_shape[usize::from(ndim)..]
            .iter()
            .any(|&extent| extent != 1)
        || padded_shape[usize::from(ndim)..]
            .iter()
            .any(|&extent| extent != 1)
    {
        return Err(RankFileError::Descriptor);
    }
    let padded_elements = shape_elements(padded_shape, ndim)?;
    if padded_elements
        .checked_mul(dtype.element_bytes())
        .ok_or(RankFileError::Overflow)?
        != payload_bytes
    {
        return Err(RankFileError::Descriptor);
    }
    Ok(())
}

pub(crate) fn validate_plain_padding(
    payload: &[u8],
    dtype: PlainDtype,
    ndim: u8,
    logical_shape: [u32; 4],
    padded_shape: [u32; 4],
) -> Result<(), RankFileError> {
    validate_plain_geometry(
        dtype,
        ndim,
        logical_shape,
        padded_shape,
        payload.len() as u64,
    )?;
    if logical_shape == padded_shape {
        return Ok(());
    }
    validate_plain_padding_chunk(payload, 0, dtype, ndim, logical_shape, padded_shape)
}

pub(crate) fn validate_plain_padding_chunk(
    bytes: &[u8],
    plane_offset: u64,
    dtype: PlainDtype,
    ndim: u8,
    logical_shape: [u32; 4],
    padded_shape: [u32; 4],
) -> Result<(), RankFileError> {
    let element_bytes = dtype.element_bytes();
    let total_bytes = shape_elements(padded_shape, ndim)?
        .checked_mul(element_bytes)
        .ok_or(RankFileError::Overflow)?;
    let bytes_u64 = u64::try_from(bytes.len()).map_err(|_| RankFileError::Overflow)?;
    let end = plane_offset
        .checked_add(bytes_u64)
        .ok_or(RankFileError::Overflow)?;
    if !plane_offset.is_multiple_of(element_bytes)
        || !bytes_u64.is_multiple_of(element_bytes)
        || end > total_bytes
    {
        return Err(RankFileError::Descriptor);
    }
    if logical_shape == padded_shape {
        return Ok(());
    }
    let first_element = plane_offset / element_bytes;
    let element_bytes = usize::try_from(element_bytes).map_err(|_| RankFileError::Overflow)?;
    for (local, element) in bytes.chunks_exact(element_bytes).enumerate() {
        let linear = first_element
            .checked_add(u64::try_from(local).map_err(|_| RankFileError::Overflow)?)
            .ok_or(RankFileError::Overflow)?;
        let mut remainder = linear;
        let mut padding = false;
        for axis in (0..usize::from(ndim)).rev() {
            let extent = u64::from(padded_shape[axis]);
            let coordinate = remainder % extent;
            remainder /= extent;
            padding |= coordinate >= u64::from(logical_shape[axis]);
        }
        if padding && element.iter().any(|&byte| byte != 0) {
            return Err(RankFileError::NonCanonicalLayout);
        }
    }
    Ok(())
}

fn shape_elements(shape: [u32; 4], ndim: u8) -> Result<u64, RankFileError> {
    if !(1..=4).contains(&ndim) {
        return Err(RankFileError::Descriptor);
    }
    shape[..usize::from(ndim)]
        .iter()
        .try_fold(1_u64, |product, &extent| {
            if extent == 0 {
                return Err(RankFileError::Descriptor);
            }
            product
                .checked_mul(u64::from(extent))
                .ok_or(RankFileError::Overflow)
        })
}

fn validate_canonical_tensor_layout(
    bytes: &[u8],
    descriptors: &[TensorDescriptor],
    metadata_region: &std::ops::Range<usize>,
    payload_region: &std::ops::Range<usize>,
) -> Result<(), RankFileError> {
    let mut metadata_cursor = metadata_region.start;
    let mut payload_cursor = payload_region.start;
    for descriptor in descriptors {
        let metadata = u64_range(
            descriptor.codec_metadata_offset,
            descriptor.codec_metadata_bytes,
            bytes.len(),
        )?;
        if metadata.start != metadata_cursor {
            return Err(RankFileError::NonCanonicalLayout);
        }
        metadata_cursor = metadata.end;

        let primary = u64_range(
            descriptor.payload_offset,
            descriptor.payload_bytes,
            bytes.len(),
        )?;
        let expected_primary = align_up(payload_cursor, PAYLOAD_ALIGNMENT)?;
        if primary.start != expected_primary
            || bytes[payload_cursor..expected_primary]
                .iter()
                .any(|&byte| byte != 0)
        {
            return Err(RankFileError::NonCanonicalLayout);
        }

        let aux = u64_range(descriptor.aux_offset, descriptor.aux_bytes, bytes.len())?;
        let expected_aux = align_up(primary.end, PAYLOAD_ALIGNMENT)?;
        if aux.start != expected_aux
            || bytes[primary.end..expected_aux]
                .iter()
                .any(|&byte| byte != 0)
        {
            return Err(RankFileError::NonCanonicalLayout);
        }
        payload_cursor = aux.end;
    }
    if metadata_cursor != metadata_region.end || payload_cursor != payload_region.end {
        return Err(RankFileError::NonCanonicalLayout);
    }
    Ok(())
}

pub(crate) struct RankHeaderFields {
    pub rank: u32,
    pub tensor_count: usize,
    pub header_flags: u32,
    pub manifest_offset: usize,
    pub manifest_bytes: usize,
    pub descriptor_offset: usize,
    pub descriptor_bytes: usize,
    pub string_offset: usize,
    pub string_bytes: usize,
    pub metadata_offset: usize,
    pub metadata_bytes: usize,
    pub payload_offset: usize,
    pub payload_bytes: usize,
    pub model_config_sha256: [u8; 32],
    pub tokenizer_bundle_sha256: [u8; 32],
    pub chat_template_sha256: [u8; 32],
    pub weight_policy_sha256: [u8; 32],
    pub kernel_abi_sha256: [u8; 32],
    pub manifest_sha256: [u8; 32],
    pub descriptor_sha256: [u8; 32],
    pub payload_sha256: [u8; 32],
    pub string_sha256: [u8; 32],
    pub metadata_sha256: [u8; 32],
}

pub(crate) fn encode_rank_header(
    fields: &RankHeaderFields,
    conversion_uuid: [u8; 16],
) -> Result<[u8; HEADER_BYTES], RankFileError> {
    if fields.rank > 3 || fields.tensor_count == 0 {
        return Err(RankFileError::Header);
    }
    let mut uuid_hasher = Sha256::new();
    uuid_hasher.update(b"g5n-file-v0\0");
    uuid_hasher.update(conversion_uuid);
    uuid_hasher.update(fields.rank.to_le_bytes());
    uuid_hasher.update(fields.manifest_sha256);
    uuid_hasher.update(fields.descriptor_sha256);
    uuid_hasher.update(fields.payload_sha256);
    let file_uuid = first_16(uuid_hasher.finalize().into());

    let mut header = [0_u8; HEADER_BYTES];
    header[0..8].copy_from_slice(b"GLM5NAT0");
    put_u16(&mut header, 8, 0);
    put_u16(&mut header, 10, 2);
    put_u32(&mut header, 12, HEADER_BYTES as u32);
    put_u32(&mut header, 16, 0x0102_0304);
    put_u32(&mut header, 20, fields.header_flags);
    put_u32(&mut header, 24, fields.rank);
    put_u32(&mut header, 28, 4);
    put_u32(
        &mut header,
        32,
        u32::try_from(fields.tensor_count).map_err(|_| RankFileError::Overflow)?,
    );
    put_u64(&mut header, 40, to_u64(fields.manifest_offset)?);
    put_u64(&mut header, 48, to_u64(fields.manifest_bytes)?);
    put_u64(&mut header, 56, to_u64(fields.descriptor_offset)?);
    put_u64(&mut header, 64, to_u64(fields.descriptor_bytes)?);
    put_u64(&mut header, 72, to_u64(fields.string_offset)?);
    put_u64(&mut header, 80, to_u64(fields.string_bytes)?);
    put_u64(&mut header, 88, to_u64(fields.metadata_offset)?);
    put_u64(&mut header, 96, to_u64(fields.metadata_bytes)?);
    put_u64(&mut header, 104, to_u64(fields.payload_offset)?);
    put_u64(&mut header, 112, to_u64(fields.payload_bytes)?);
    header[120..152].copy_from_slice(&fields.model_config_sha256);
    header[152..184].copy_from_slice(&fields.tokenizer_bundle_sha256);
    header[184..216].copy_from_slice(&fields.chat_template_sha256);
    header[216..248].copy_from_slice(&fields.weight_policy_sha256);
    header[248..280].copy_from_slice(&fields.kernel_abi_sha256);
    header[280..312].copy_from_slice(&fields.manifest_sha256);
    header[312..344].copy_from_slice(&fields.descriptor_sha256);
    header[344..376].copy_from_slice(&fields.payload_sha256);
    header[376..392].copy_from_slice(&file_uuid);
    header[392..408].copy_from_slice(&conversion_uuid);
    put_u64(&mut header, 408, 0);
    put_u32(&mut header, 416, 0);
    header[420..452].copy_from_slice(&fields.string_sha256);
    header[452..484].copy_from_slice(&fields.metadata_sha256);
    let crc = crc32c(&header);
    put_u32(&mut header, 416, crc);
    Ok(header)
}

fn to_u64(value: usize) -> Result<u64, RankFileError> {
    u64::try_from(value).map_err(|_| RankFileError::Overflow)
}

#[derive(Clone, Debug)]
struct EncodedTensor {
    codec_id: u16,
    logical_dtype: u16,
    stored_dtype: u16,
    logical_shape: [u32; 4],
    padded_shape: [u32; 4],
    quant_group_elements: u32,
    ndim: u8,
    aux_required: bool,
    metadata: Vec<u8>,
    primary: Vec<u8>,
    aux: Vec<u8>,
}

impl EncodedTensor {
    fn from_record(record: &TensorRecord, rank: u32) -> Result<Self, RankFileError> {
        match &record.payload {
            TensorPayload::Plain(plain) => {
                plain.validate()?;
                Ok(Self {
                    codec_id: plain.dtype as u16,
                    logical_dtype: plain.dtype.dtype_id(),
                    stored_dtype: plain.dtype.dtype_id(),
                    logical_shape: plain.logical_shape,
                    padded_shape: plain.padded_shape,
                    quant_group_elements: 0,
                    ndim: plain.ndim,
                    aux_required: false,
                    metadata: Vec::new(),
                    primary: plain.bytes.clone(),
                    aux: Vec::new(),
                })
            }
            TensorPayload::Nvfp4(packed) => {
                packed.validate().map_err(RankFileError::Nvfp4)?;
                Ok(Self {
                    codec_id: packed.metadata.codec as u16,
                    logical_dtype: DTYPE_BF16,
                    stored_dtype: DTYPE_PACKED_E2M1X2,
                    logical_shape: [packed.metadata.logical_n, packed.metadata.logical_k, 1, 1],
                    padded_shape: [packed.metadata.padded_n, packed.metadata.padded_k, 1, 1],
                    quant_group_elements: 16,
                    ndim: 2,
                    aux_required: true,
                    metadata: packed.metadata.encode().to_vec(),
                    primary: packed.values.clone(),
                    aux: packed.scales.clone(),
                })
            }
            TensorPayload::Exl3Source(source) => {
                source.validate().map_err(RankFileError::Exl3)?;
                let layer =
                    u16::try_from(record.layer_id).map_err(|_| RankFileError::Descriptor)?;
                let expert =
                    u16::try_from(record.expert_id).map_err(|_| RankFileError::Descriptor)?;
                if source.metadata.rank != rank as u8
                    || source.metadata.layer != layer
                    || source.metadata.expert != expert
                {
                    return Err(RankFileError::Descriptor);
                }
                let logical_shape = [source.metadata.logical_n, source.metadata.logical_k, 1, 1];
                Ok(Self {
                    codec_id: CODEC_EXL3_SOURCE,
                    logical_dtype: DTYPE_FP16,
                    stored_dtype: DTYPE_I16,
                    logical_shape,
                    padded_shape: logical_shape,
                    quant_group_elements: 0,
                    ndim: 2,
                    aux_required: true,
                    metadata: source.metadata.encode().to_vec(),
                    primary: source.primary_plane().map_err(RankFileError::Exl3)?,
                    aux: source.aux_plane().map_err(RankFileError::Exl3)?,
                })
            }
        }
    }
}

fn checked_range(
    header: &[u8],
    offset_field: usize,
    length_field: usize,
    file_len: usize,
    alignment: usize,
) -> Result<std::ops::Range<usize>, RankFileError> {
    let range = u64_range(
        get_u64(header, offset_field),
        get_u64(header, length_field),
        file_len,
    )?;
    if range.start % alignment != 0 {
        return Err(RankFileError::Region);
    }
    Ok(range)
}

fn u64_range(
    offset: u64,
    length: u64,
    file_len: usize,
) -> Result<std::ops::Range<usize>, RankFileError> {
    let end = offset.checked_add(length).ok_or(RankFileError::Overflow)?;
    let start = usize::try_from(offset).map_err(|_| RankFileError::Overflow)?;
    let end = usize::try_from(end).map_err(|_| RankFileError::Overflow)?;
    if end > file_len || start > end {
        return Err(RankFileError::Region);
    }
    Ok(start..end)
}

fn ordered_non_overlapping<const N: usize>(ranges: [std::ops::Range<usize>; N]) -> bool {
    ranges.windows(2).all(|pair| pair[0].end <= pair[1].start)
}

pub(crate) fn align_up(value: usize, alignment: usize) -> Result<usize, RankFileError> {
    value
        .checked_add(alignment - 1)
        .map(|sum| sum / alignment * alignment)
        .ok_or(RankFileError::Overflow)
}

#[must_use]
pub fn sha256(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

pub(crate) fn first_16(hash: [u8; 32]) -> [u8; 16] {
    hash[..16].try_into().unwrap()
}

#[derive(Debug)]
pub enum RankFileError {
    Truncated,
    Header,
    HeaderFlags,
    HeaderCrc,
    Rank,
    TensorCount,
    TensorId,
    Descriptor,
    Region,
    TensorRegion,
    NonCanonicalLayout,
    StringTable,
    StrongHash,
    FileUuid,
    RankSet,
    Overflow,
    UnsupportedCodec(u16),
    CodecMismatch,
    Nvfp4(crate::Nvfp4Error),
    Exl3(crate::Exl3Error),
}

impl fmt::Display for RankFileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for RankFileError {}

fn put_u16(out: &mut [u8], offset: usize, value: u16) {
    out[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn put_i16(out: &mut [u8], offset: usize, value: i16) {
    out[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn put_u32(out: &mut [u8], offset: usize, value: u32) {
    out[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn put_u64(out: &mut [u8], offset: usize, value: u64) {
    out[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

fn get_u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(bytes[offset..offset + 2].try_into().unwrap())
}

fn get_i16(bytes: &[u8], offset: usize) -> i16 {
    i16::from_le_bytes(bytes[offset..offset + 2].try_into().unwrap())
}

fn get_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}

fn get_u64(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Codec, EXL3_MCG_MULTIPLIER, Exl3Projection, KERNEL_ABI};

    fn builder(rank: u32) -> RankFileBuilder {
        let input: Vec<f32> = (0..128 * 64)
            .map(|index| (index as f32 % 31.0 - 15.0) / 7.0)
            .collect();
        let packed = PackedNvfp4::pack(&input, 128, 64, Codec::OneDimensional).unwrap();
        RankFileBuilder {
            rank,
            manifest: format!("{{\"rank\":{rank},\"schema\":\"test-v1\"}}").into_bytes(),
            model_config_sha256: sha256(b"config"),
            tokenizer_bundle_sha256: sha256(b"tokenizer"),
            chat_template_sha256: sha256(b"template"),
            weight_policy_sha256: sha256(b"policy"),
            kernel_abi_sha256: sha256(KERNEL_ABI.as_bytes()),
            tensors: vec![TensorRecord {
                tensor_id: 0,
                name: "model.layers.3.mlp.experts.0.gate_up_proj.weight".into(),
                role_id: 0x0501,
                layer_id: 3,
                expert_id: 0,
                tp_shard_axis: 0,
                flags: 0b0000_1010,
                payload: TensorPayload::Nvfp4(packed),
            }],
        }
    }

    fn exl3_fixture(rank: u8) -> Exl3Trellis {
        let metadata = Exl3Metadata::new(Exl3Projection::Gate, 78, 0, rank, 3, 128, 128).unwrap();
        Exl3Trellis {
            trellis: (0..metadata.trellis_words)
                .map(|index| (index as u16).wrapping_mul(40_503))
                .collect(),
            suh: vec![0x3c00; 128],
            svh: vec![0x3c00; 128],
            mcg_marker: EXL3_MCG_MULTIPLIER,
            metadata,
        }
    }

    fn exl3_builder(rank: u32) -> RankFileBuilder {
        let mut builder = builder(rank);
        builder.manifest =
            format!("{{\"rank\":{rank},\"schema\":\"exl3-source-test-v1\"}}").into_bytes();
        builder.tensors = vec![TensorRecord {
            tensor_id: 0,
            name: "model.layers.78.mlp.experts.0.gate_proj.weight".into(),
            role_id: 0x0501,
            layer_id: 78,
            expert_id: 0,
            tp_shard_axis: 0,
            flags: 0b0000_1010,
            payload: TensorPayload::Exl3Source(exl3_fixture(rank as u8)),
        }];
        builder
    }

    fn plain_builder(rank: u32) -> RankFileBuilder {
        let mut builder = builder(rank);
        builder.manifest = format!("{{\"rank\":{rank},\"schema\":\"plain-test-v1\"}}").into_bytes();
        builder.tensors = vec![TensorRecord {
            tensor_id: 0,
            name: "model.layers.3.mlp.gate.e_score_correction_bias".into(),
            role_id: 0x0302,
            layer_id: 3,
            expert_id: -1,
            tp_shard_axis: -1,
            flags: 1,
            payload: TensorPayload::Plain(PlainTensor {
                dtype: PlainDtype::Bf16,
                ndim: 2,
                logical_shape: [2, 3, 1, 1],
                padded_shape: [2, 4, 1, 1],
                bytes: vec![1, 0, 2, 0, 3, 0, 0, 0, 4, 0, 5, 0, 6, 0, 0, 0],
            }),
        }];
        builder
    }

    fn resign_header(bytes: &mut [u8]) {
        let descriptor_start = usize::try_from(get_u64(bytes, 56)).unwrap();
        let descriptor_len = usize::try_from(get_u64(bytes, 64)).unwrap();
        let descriptor_hash =
            sha256(&bytes[descriptor_start..descriptor_start.checked_add(descriptor_len).unwrap()]);
        bytes[312..344].copy_from_slice(&descriptor_hash);
        let payload_start = usize::try_from(get_u64(bytes, 104)).unwrap();
        let payload_len = usize::try_from(get_u64(bytes, 112)).unwrap();
        let payload_hash =
            sha256(&bytes[payload_start..payload_start.checked_add(payload_len).unwrap()]);
        bytes[344..376].copy_from_slice(&payload_hash);
        let mut uuid_hasher = Sha256::new();
        uuid_hasher.update(b"g5n-file-v0\0");
        uuid_hasher.update(&bytes[392..408]);
        uuid_hasher.update(get_u32(bytes, 24).to_le_bytes());
        uuid_hasher.update(&bytes[280..312]);
        uuid_hasher.update(descriptor_hash);
        uuid_hasher.update(&bytes[344..376]);
        bytes[376..392].copy_from_slice(&first_16(uuid_hasher.finalize().into()));
        put_u32(bytes, 416, 0);
        put_u32(bytes, 416, crc32c(&bytes[..HEADER_BYTES]));
    }

    fn resign_first_tensor_metadata(bytes: &mut [u8]) {
        let descriptor_start = usize::try_from(get_u64(bytes, 56)).unwrap();
        let metadata_start =
            usize::try_from(get_u64(bytes, descriptor_start.checked_add(104).unwrap())).unwrap();
        let metadata_bytes =
            usize::try_from(get_u64(bytes, descriptor_start.checked_add(112).unwrap())).unwrap();
        let metadata_end = metadata_start.checked_add(metadata_bytes).unwrap();
        let metadata_hash = sha256(&bytes[metadata_start..metadata_end]);
        bytes[descriptor_start + 192..descriptor_start + 224].copy_from_slice(&metadata_hash);
        let metadata_region_start = usize::try_from(get_u64(bytes, 88)).unwrap();
        let metadata_region_bytes = usize::try_from(get_u64(bytes, 96)).unwrap();
        let metadata_region_end = metadata_region_start
            .checked_add(metadata_region_bytes)
            .unwrap();
        let region_hash = sha256(&bytes[metadata_region_start..metadata_region_end]);
        bytes[452..484].copy_from_slice(&region_hash);
        resign_header(bytes);
    }

    #[test]
    fn four_rank_identity_is_deterministic() {
        let builders = [builder(0), builder(1), builder(2), builder(3)];
        let conversion = RankFileBuilder::derive_conversion_uuid(&builders).unwrap();
        let a = builders[0].build(conversion).unwrap();
        let b = builders[0].build(conversion).unwrap();
        assert_eq!(a, b);
        let parsed = RankFile::read(a).unwrap();
        assert_eq!(parsed.rank, 0);
        assert_eq!(parsed.conversion_uuid, conversion);
        assert_eq!(
            parsed.tensor_name(0).unwrap(),
            "model.layers.3.mlp.experts.0.gate_up_proj.weight"
        );
        let files = std::array::from_fn(|rank| {
            RankFile::read(builders[rank].build(conversion).unwrap()).unwrap()
        });
        RankFile::validate_rank_set(&files).unwrap();
    }

    #[test]
    fn exl3_source_container_is_deterministic_and_cpu_decodable() {
        let builders = [
            exl3_builder(0),
            exl3_builder(1),
            exl3_builder(2),
            exl3_builder(3),
        ];
        let conversion = RankFileBuilder::derive_conversion_uuid(&builders).unwrap();
        let first = builders[0].build(conversion).unwrap();
        assert_eq!(first, builders[0].build(conversion).unwrap());
        assert_eq!(get_u32(&first, 20), HEADER_FLAG_EXL3);
        let parsed = RankFile::read(first).unwrap();
        assert_eq!(parsed.header_flags, HEADER_FLAG_EXL3);
        assert_eq!(parsed.descriptors[0].codec_id, CODEC_EXL3_SOURCE);
        assert_eq!(parsed.descriptors[0].logical_dtype, DTYPE_FP16);
        assert_eq!(parsed.descriptors[0].stored_dtype, DTYPE_I16);
        assert_eq!(parsed.descriptors[0].quant_group_elements, 0);
        assert_eq!(parsed.decode_exl3_source(0).unwrap(), exl3_fixture(0));
        let files = std::array::from_fn(|rank| {
            RankFile::read(builders[rank].build(conversion).unwrap()).unwrap()
        });
        RankFile::validate_rank_set(&files).unwrap();
    }

    #[test]
    fn mixed_container_sets_hybrid_flags_without_direct_layout_claim() {
        let builders: [RankFileBuilder; 4] = std::array::from_fn(|rank| {
            let mut builder = builder(rank as u32);
            builder.tensors.push(TensorRecord {
                tensor_id: 1,
                name: "model.layers.78.mlp.experts.0.gate_proj.weight".into(),
                role_id: 0x0501,
                layer_id: 78,
                expert_id: 0,
                tp_shard_axis: 0,
                flags: 0b0000_1010,
                payload: TensorPayload::Exl3Source(exl3_fixture(rank as u8)),
            });
            builder
        });
        let conversion = RankFileBuilder::derive_conversion_uuid(&builders).unwrap();
        let bytes = builders[0].build(conversion).unwrap();
        let expected = HEADER_FLAG_NVFP4 | HEADER_FLAG_EXL3 | HEADER_FLAG_HYBRID;
        assert_eq!(get_u32(&bytes, 20), expected);
        assert_eq!(get_u32(&bytes, 20) & HEADER_FLAG_DIRECT_KERNEL, 0);
        let parsed = RankFile::read(bytes).unwrap();
        assert_eq!(parsed.header_flags, expected);
        assert!(matches!(
            parsed.decode_exl3_source(0),
            Err(RankFileError::CodecMismatch)
        ));
        assert_eq!(parsed.decode_exl3_source(1).unwrap(), exl3_fixture(0));
    }

    #[test]
    fn plain_protected_tensor_round_trips_and_padding_is_hard() {
        let builders = [
            plain_builder(0),
            plain_builder(1),
            plain_builder(2),
            plain_builder(3),
        ];
        let conversion = RankFileBuilder::derive_conversion_uuid(&builders).unwrap();
        let bytes = builders[0].build(conversion).unwrap();
        assert_eq!(get_u32(&bytes, 20), HEADER_FLAG_DIRECT_KERNEL);
        let parsed = RankFile::read(bytes).unwrap();
        let descriptor = &parsed.descriptors[0];
        assert_eq!(descriptor.codec_id, CODEC_BF16_ROW_MAJOR);
        assert_eq!(descriptor.ndim, 2);
        assert_eq!(descriptor.payload_bytes, 16);
        assert_eq!(descriptor.aux_bytes, 0);
        assert_eq!(descriptor.codec_metadata_bytes, 0);
        assert_eq!(parsed.tensor_primary(0).unwrap().len(), 16);

        let payload = parsed.tensor_primary(0).unwrap();
        for (start, end) in [(0, 4), (4, 12), (12, 16)] {
            validate_plain_padding_chunk(
                &payload[start..end],
                u64::try_from(start).unwrap(),
                PlainDtype::Bf16,
                2,
                [2, 3, 1, 1],
                [2, 4, 1, 1],
            )
            .unwrap();
        }
        assert!(matches!(
            validate_plain_padding_chunk(
                &payload[2..4],
                1,
                PlainDtype::Bf16,
                2,
                [2, 3, 1, 1],
                [2, 4, 1, 1],
            ),
            Err(RankFileError::Descriptor)
        ));

        let mut invalid = plain_builder(0);
        let TensorPayload::Plain(plain) = &mut invalid.tensors[0].payload else {
            unreachable!()
        };
        plain.bytes[6] = 1;
        assert!(matches!(
            invalid.build([0; 16]),
            Err(RankFileError::NonCanonicalLayout)
        ));
    }

    #[test]
    fn invalid_exl3_source_is_rejected_before_serialization() {
        let mut builder = exl3_builder(0);
        let TensorPayload::Exl3Source(source) = &mut builder.tensors[0].payload else {
            unreachable!()
        };
        source.mcg_marker ^= 1;
        assert!(matches!(
            builder.build([0; 16]),
            Err(RankFileError::Exl3(crate::Exl3Error::CodebookMarker))
        ));
    }

    #[test]
    fn unknown_codec_and_header_flag_lies_are_rejected() {
        let builders = [builder(0), builder(1), builder(2), builder(3)];
        let conversion = RankFileBuilder::derive_conversion_uuid(&builders).unwrap();
        let mut unknown = builders[0].build(conversion).unwrap();
        let descriptor_start = usize::try_from(get_u64(&unknown, 56)).unwrap();
        put_u16(&mut unknown, descriptor_start + 16, 0x7777);
        resign_header(&mut unknown);
        assert!(matches!(
            RankFile::read(unknown),
            Err(RankFileError::UnsupportedCodec(0x7777))
        ));

        let mut lying = builders[0].build(conversion).unwrap();
        put_u32(&mut lying, 20, HEADER_FLAG_EXL3);
        resign_header(&mut lying);
        assert!(matches!(
            RankFile::read(lying),
            Err(RankFileError::HeaderFlags)
        ));
    }

    #[test]
    fn nonzero_internal_padding_is_rejected_even_when_resigned() {
        let builders: [RankFileBuilder; 4] = std::array::from_fn(|rank| {
            let mut exl3 = exl3_builder(rank as u32);
            let mut nvfp4 = builder(rank as u32).tensors.remove(0);
            nvfp4.tensor_id = 1;
            exl3.tensors.push(nvfp4);
            exl3
        });
        let conversion = RankFileBuilder::derive_conversion_uuid(&builders).unwrap();
        let mut bytes = builders[0].build(conversion).unwrap();
        let descriptor_start = usize::try_from(get_u64(&bytes, 56)).unwrap();
        let aux_offset = usize::try_from(get_u64(&bytes, descriptor_start + 88)).unwrap();
        let aux_bytes = usize::try_from(get_u64(&bytes, descriptor_start + 96)).unwrap();
        let padding = aux_offset.checked_add(aux_bytes).unwrap();
        assert_eq!(bytes[padding], 0);
        bytes[padding] = 1;
        resign_header(&mut bytes);
        assert!(matches!(
            RankFile::read(bytes),
            Err(RankFileError::NonCanonicalLayout)
        ));
    }

    #[test]
    fn corruption_is_rejected_by_strong_hash() {
        let builders = [builder(0), builder(1), builder(2), builder(3)];
        let conversion = RankFileBuilder::derive_conversion_uuid(&builders).unwrap();
        let mut bytes = builders[0].build(conversion).unwrap();
        let last = bytes.len() - 1;
        bytes[last] ^= 1;
        assert!(matches!(
            RankFile::read(bytes),
            Err(RankFileError::StrongHash)
        ));
    }

    #[test]
    fn resigned_nvfp4_metadata_reserved_and_mode_lies_are_rejected() {
        let builders = [builder(0), builder(1), builder(2), builder(3)];
        let conversion = RankFileBuilder::derive_conversion_uuid(&builders).unwrap();
        for relative_offset in [33_usize, 52, 124] {
            let mut bytes = builders[0].build(conversion).unwrap();
            let descriptor_start = usize::try_from(get_u64(&bytes, 56)).unwrap();
            let metadata_start = usize::try_from(get_u64(&bytes, descriptor_start + 104)).unwrap();
            bytes[metadata_start + relative_offset] ^= 0x02;
            put_u32(&mut bytes, metadata_start + 120, 0);
            let metadata_crc =
                crc32c(&bytes[metadata_start..metadata_start + Nvfp4Metadata::BYTES]);
            put_u32(&mut bytes, metadata_start + 120, metadata_crc);
            resign_first_tensor_metadata(&mut bytes);
            assert!(matches!(
                RankFile::read(bytes),
                Err(RankFileError::Nvfp4(crate::Nvfp4Error::UnsupportedMetadata))
            ));
        }
    }

    #[test]
    fn reserved_header_bytes_and_trailing_file_bytes_are_rejected() {
        let builders = [builder(0), builder(1), builder(2), builder(3)];
        let conversion = RankFileBuilder::derive_conversion_uuid(&builders).unwrap();

        let mut reserved = builders[0].build(conversion).unwrap();
        reserved[36] = 1;
        put_u32(&mut reserved, 416, 0);
        let crc = crc32c(&reserved[..HEADER_BYTES]);
        put_u32(&mut reserved, 416, crc);
        assert!(matches!(
            RankFile::read(reserved),
            Err(RankFileError::Header)
        ));

        let mut trailing = builders[0].build(conversion).unwrap();
        trailing.push(0);
        assert!(matches!(
            RankFile::read(trailing),
            Err(RankFileError::Region)
        ));
    }
}
