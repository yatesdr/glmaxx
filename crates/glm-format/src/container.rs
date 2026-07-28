use std::fmt;

use sha2::{Digest, Sha256};

use crate::{Nvfp4Metadata, PackedNvfp4, crc32c, nvfp4::validate_scale_plane};

pub const HEADER_BYTES: usize = 4096;
const DESCRIPTOR_BYTES: usize = 256;
const ALIGNMENT: usize = 4096;
const PAYLOAD_ALIGNMENT: usize = 256;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TensorDescriptor {
    pub tensor_id: u32,
    pub name_offset: u32,
    pub name_bytes: u16,
    pub role_id: u16,
    pub layer_id: i16,
    pub expert_id: i16,
    pub codec_id: u16,
    pub tp_shard_axis: i8,
    pub flags: u8,
    pub logical_shape: [u32; 4],
    pub padded_shape: [u32; 4],
    pub payload_offset: u64,
    pub payload_bytes: u64,
    pub aux_offset: u64,
    pub aux_bytes: u64,
    pub codec_metadata_offset: u64,
    pub codec_metadata_bytes: u64,
    pub payload_sha256: [u8; 32],
    pub aux_sha256: [u8; 32],
    pub codec_metadata_sha256: [u8; 32],
}

impl TensorDescriptor {
    fn encode(&self) -> [u8; DESCRIPTOR_BYTES] {
        let mut out = [0_u8; DESCRIPTOR_BYTES];
        put_u32(&mut out, 0, self.tensor_id);
        put_u32(&mut out, 4, self.name_offset);
        put_u16(&mut out, 8, self.name_bytes);
        put_u16(&mut out, 10, self.role_id);
        put_i16(&mut out, 12, self.layer_id);
        put_i16(&mut out, 14, self.expert_id);
        put_u16(&mut out, 16, self.codec_id);
        put_u16(&mut out, 18, 1);
        put_u16(&mut out, 20, 6);
        out[22] = self.tp_shard_axis.to_le_bytes()[0];
        out[23] = 2;
        out[24] = self.flags | 0x80;
        for (index, &dimension) in self.logical_shape.iter().enumerate() {
            put_u32(&mut out, 28 + index * 4, dimension);
        }
        for (index, &dimension) in self.padded_shape.iter().enumerate() {
            put_u32(&mut out, 44 + index * 4, dimension);
        }
        let logical_elements = u64::from(self.logical_shape[0])
            .checked_mul(u64::from(self.logical_shape[1]))
            .unwrap();
        put_u64(&mut out, 64, logical_elements);
        put_u64(&mut out, 72, self.payload_offset);
        put_u64(&mut out, 80, self.payload_bytes);
        put_u64(&mut out, 88, self.aux_offset);
        put_u64(&mut out, 96, self.aux_bytes);
        put_u64(&mut out, 104, self.codec_metadata_offset);
        put_u64(&mut out, 112, self.codec_metadata_bytes);
        put_u32(&mut out, 120, PAYLOAD_ALIGNMENT as u32);
        put_u32(&mut out, 124, 16);
        out[128..160].copy_from_slice(&self.payload_sha256);
        out[160..192].copy_from_slice(&self.aux_sha256);
        out[192..224].copy_from_slice(&self.codec_metadata_sha256);
        out
    }

    fn decode(bytes: &[u8]) -> Result<Self, RankFileError> {
        if bytes.len() != DESCRIPTOR_BYTES
            || bytes[25..28].iter().any(|&value| value != 0)
            || bytes[224..].iter().any(|&value| value != 0)
            || get_u16(bytes, 18) != 1
            || get_u16(bytes, 20) != 6
            || bytes[23] != 2
            || bytes[24] & 0x80 == 0
            || get_u32(bytes, 120) < PAYLOAD_ALIGNMENT as u32
            || get_u32(bytes, 124) != 16
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
        let logical_elements = u64::from(logical_shape[0])
            .checked_mul(u64::from(logical_shape[1]))
            .ok_or(RankFileError::Overflow)?;
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
            tp_shard_axis: i8::from_le_bytes([bytes[22]]),
            flags: bytes[24] & 0x7f,
            logical_shape,
            padded_shape,
            payload_offset: get_u64(bytes, 72),
            payload_bytes: get_u64(bytes, 80),
            aux_offset: get_u64(bytes, 88),
            aux_bytes: get_u64(bytes, 96),
            codec_metadata_offset: get_u64(bytes, 104),
            codec_metadata_bytes: get_u64(bytes, 112),
            payload_sha256,
            aux_sha256,
            codec_metadata_sha256,
        })
    }
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
    pub packed: PackedNvfp4,
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

        let mut uuid_hasher = Sha256::new();
        uuid_hasher.update(b"g5n-file-v0\0");
        uuid_hasher.update(conversion_uuid);
        uuid_hasher.update(self.rank.to_le_bytes());
        uuid_hasher.update(prepared.manifest_hash);
        uuid_hasher.update(prepared.descriptor_hash);
        uuid_hasher.update(prepared.payload_hash);
        let file_uuid = first_16(uuid_hasher.finalize().into());

        let header = &mut file[..HEADER_BYTES];
        header[0..8].copy_from_slice(b"GLM5NAT0");
        put_u16(header, 8, 0);
        put_u16(header, 10, 2);
        put_u32(header, 12, HEADER_BYTES as u32);
        put_u32(header, 16, 0x0102_0304);
        put_u32(header, 20, 0b11);
        put_u32(header, 24, self.rank);
        put_u32(header, 28, 4);
        put_u32(
            header,
            32,
            u32::try_from(self.tensors.len()).map_err(|_| RankFileError::Overflow)?,
        );
        put_u64(header, 40, prepared.manifest_offset as u64);
        put_u64(header, 48, self.manifest.len() as u64);
        put_u64(header, 56, prepared.descriptor_offset as u64);
        put_u64(header, 64, prepared.descriptor_bytes.len() as u64);
        put_u64(header, 72, prepared.string_offset as u64);
        put_u64(header, 80, prepared.strings.len() as u64);
        put_u64(header, 88, prepared.metadata_offset as u64);
        put_u64(header, 96, prepared.metadata.len() as u64);
        put_u64(header, 104, prepared.payload_offset as u64);
        put_u64(header, 112, prepared.payload.len() as u64);
        header[120..152].copy_from_slice(&self.model_config_sha256);
        header[152..184].copy_from_slice(&self.tokenizer_bundle_sha256);
        header[184..216].copy_from_slice(&self.chat_template_sha256);
        header[216..248].copy_from_slice(&self.weight_policy_sha256);
        header[248..280].copy_from_slice(&self.kernel_abi_sha256);
        header[280..312].copy_from_slice(&prepared.manifest_hash);
        header[312..344].copy_from_slice(&prepared.descriptor_hash);
        header[344..376].copy_from_slice(&prepared.payload_hash);
        header[376..392].copy_from_slice(&file_uuid);
        header[392..408].copy_from_slice(&conversion_uuid);
        put_u64(header, 408, 0);
        put_u32(header, 416, 0);
        header[420..452].copy_from_slice(&prepared.string_hash);
        header[452..484].copy_from_slice(&prepared.metadata_hash);
        let crc = crc32c(header);
        put_u32(header, 416, crc);
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
        let metadata_len = self
            .tensors
            .len()
            .checked_mul(Nvfp4Metadata::BYTES)
            .ok_or(RankFileError::Overflow)?;
        let payload_offset = align_up(
            metadata_offset
                .checked_add(metadata_len)
                .ok_or(RankFileError::Overflow)?,
            ALIGNMENT,
        )?;
        let mut metadata = vec![0_u8; metadata_len];
        let mut payload = Vec::new();
        let mut descriptors = Vec::with_capacity(self.tensors.len());
        for (index, tensor) in self.tensors.iter().enumerate() {
            tensor.packed.validate().map_err(RankFileError::Nvfp4)?;
            if tensor.tensor_id != u32::try_from(index).unwrap() {
                return Err(RankFileError::TensorId);
            }
            let encoded_metadata = tensor.packed.metadata.encode();
            let metadata_local = index * Nvfp4Metadata::BYTES;
            metadata[metadata_local..metadata_local + Nvfp4Metadata::BYTES]
                .copy_from_slice(&encoded_metadata);
            let value_local = align_up(payload.len(), PAYLOAD_ALIGNMENT)?;
            payload.resize(value_local, 0);
            payload.extend_from_slice(&tensor.packed.values);
            let scale_local = align_up(payload.len(), PAYLOAD_ALIGNMENT)?;
            payload.resize(scale_local, 0);
            payload.extend_from_slice(&tensor.packed.scales);
            let logical_n = tensor.packed.metadata.logical_n;
            let logical_k = tensor.packed.metadata.logical_k;
            let padded_n = tensor.packed.metadata.padded_n;
            let padded_k = tensor.packed.metadata.padded_k;
            descriptors.push(TensorDescriptor {
                tensor_id: tensor.tensor_id,
                name_offset: u32::try_from(name_offsets[index])
                    .map_err(|_| RankFileError::Overflow)?,
                name_bytes: u16::try_from(tensor.name.len())
                    .map_err(|_| RankFileError::Overflow)?,
                role_id: tensor.role_id,
                layer_id: tensor.layer_id,
                expert_id: tensor.expert_id,
                codec_id: tensor.packed.metadata.codec as u16,
                tp_shard_axis: tensor.tp_shard_axis,
                flags: tensor.flags,
                logical_shape: [logical_n, logical_k, 1, 1],
                padded_shape: [padded_n, padded_k, 1, 1],
                payload_offset: u64::try_from(payload_offset + value_local)
                    .map_err(|_| RankFileError::Overflow)?,
                payload_bytes: tensor.packed.values.len() as u64,
                aux_offset: u64::try_from(payload_offset + scale_local)
                    .map_err(|_| RankFileError::Overflow)?,
                aux_bytes: tensor.packed.scales.len() as u64,
                codec_metadata_offset: u64::try_from(metadata_offset + metadata_local)
                    .map_err(|_| RankFileError::Overflow)?,
                codec_metadata_bytes: Nvfp4Metadata::BYTES as u64,
                payload_sha256: sha256(&tensor.packed.values),
                aux_sha256: sha256(&tensor.packed.scales),
                codec_metadata_sha256: sha256(&encoded_metadata),
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
        if &header[0..8] != b"GLM5NAT0"
            || get_u16(header, 8) != 0
            || get_u16(header, 10) != 2
            || get_u32(header, 12) != HEADER_BYTES as u32
            || get_u32(header, 16) != 0x0102_0304
            || get_u32(header, 20) & !0b1_1111 != 0
            || get_u32(header, 28) != 4
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
            validate_tensor_regions(&bytes, &descriptor, &metadata, &payload)?;
            descriptors.push(descriptor);
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
}

fn validate_tensor_regions(
    bytes: &[u8],
    descriptor: &TensorDescriptor,
    metadata_region: &std::ops::Range<usize>,
    payload_region: &std::ops::Range<usize>,
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
        || sha256(&bytes[value]) != descriptor.payload_sha256
        || sha256(&bytes[aux.clone()]) != descriptor.aux_sha256
        || sha256(&bytes[metadata.clone()]) != descriptor.codec_metadata_sha256
    {
        return Err(RankFileError::TensorRegion);
    }
    validate_scale_plane(&bytes[aux]).map_err(RankFileError::Nvfp4)?;
    let decoded = Nvfp4Metadata::decode(&bytes[metadata]).map_err(RankFileError::Nvfp4)?;
    if decoded.logical_n != descriptor.logical_shape[0]
        || decoded.logical_k != descriptor.logical_shape[1]
        || decoded.padded_n != descriptor.padded_shape[0]
        || decoded.padded_k != descriptor.padded_shape[1]
        || decoded.codec as u16 != descriptor.codec_id
        || u64::from(decoded.value_plane_bytes) != descriptor.payload_bytes
        || u64::from(decoded.scale_plane_bytes) != descriptor.aux_bytes
    {
        return Err(RankFileError::Descriptor);
    }
    Ok(())
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

fn align_up(value: usize, alignment: usize) -> Result<usize, RankFileError> {
    value
        .checked_add(alignment - 1)
        .map(|sum| sum / alignment * alignment)
        .ok_or(RankFileError::Overflow)
}

#[must_use]
pub fn sha256(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn first_16(hash: [u8; 32]) -> [u8; 16] {
    hash[..16].try_into().unwrap()
}

#[derive(Debug)]
pub enum RankFileError {
    Truncated,
    Header,
    HeaderCrc,
    Rank,
    TensorCount,
    TensorId,
    Descriptor,
    Region,
    TensorRegion,
    StringTable,
    StrongHash,
    FileUuid,
    RankSet,
    Overflow,
    Nvfp4(crate::Nvfp4Error),
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
    use crate::{Codec, KERNEL_ABI};

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
                packed,
            }],
        }
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
}
