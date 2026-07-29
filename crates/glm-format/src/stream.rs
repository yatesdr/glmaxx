use std::{
    fmt,
    fs::{File, OpenOptions},
    io::{self, Read},
    os::unix::fs::{FileExt, MetadataExt},
    path::{Path, PathBuf},
};

use sha2::{Digest, Sha256};

use crate::{
    Exl3Metadata, Exl3Trellis, Nvfp4Metadata, PlainDtype, RankFileError, TensorDescriptor,
    container::{
        ALIGNMENT, CODEC_BF16_ROW_MAJOR, CODEC_EXL3_SOURCE, CODEC_FP16_ROW_MAJOR,
        CODEC_FP32_ROW_MAJOR, DESCRIPTOR_BYTES, DESCRIPTOR_FLAG_AUX_REQUIRED, DTYPE_BF16,
        DTYPE_FP16, DTYPE_I16, DTYPE_PACKED_E2M1X2, PAYLOAD_ALIGNMENT, RankHeaderFields, align_up,
        derive_header_flags, encode_rank_header, first_16, sha256, validate_plain_geometry,
        validate_plain_padding,
    },
    nvfp4::validate_scale_plane,
};

const STREAM_BUFFER_BYTES: usize = 8 * 1024 * 1024;

#[derive(Clone, Debug)]
pub struct StreamingTensorIdentity {
    pub tensor_id: u32,
    pub name: String,
    pub role_id: u16,
    pub layer_id: i16,
    pub expert_id: i16,
    pub tp_shard_axis: i8,
    pub flags: u8,
}

#[derive(Clone, Debug)]
pub struct StreamingTensorSpec {
    pub tensor_id: u32,
    pub name: String,
    pub role_id: u16,
    pub layer_id: i16,
    pub expert_id: i16,
    pub tp_shard_axis: i8,
    pub flags: u8,
    codec_id: u16,
    logical_dtype: u16,
    stored_dtype: u16,
    logical_shape: [u32; 4],
    padded_shape: [u32; 4],
    quant_group_elements: u32,
    ndim: u8,
    aux_required: bool,
    metadata: Vec<u8>,
    primary_bytes: u64,
    aux_bytes: u64,
}

impl StreamingTensorSpec {
    pub fn plain(
        identity: StreamingTensorIdentity,
        dtype: PlainDtype,
        ndim: u8,
        logical_shape: [u32; 4],
        padded_shape: [u32; 4],
    ) -> Result<Self, StreamRankError> {
        let primary_bytes = padded_shape[..usize::from(ndim.min(4))].iter().try_fold(
            dtype.element_bytes(),
            |bytes, &extent| {
                bytes
                    .checked_mul(u64::from(extent))
                    .ok_or(StreamRankError::Overflow)
            },
        )?;
        validate_plain_geometry(dtype, ndim, logical_shape, padded_shape, primary_bytes)
            .map_err(StreamRankError::RankFile)?;
        let dtype_id = match dtype {
            PlainDtype::Bf16 => DTYPE_BF16,
            PlainDtype::Fp16 => DTYPE_FP16,
            PlainDtype::Fp32 => 3,
        };
        Ok(Self {
            tensor_id: identity.tensor_id,
            name: identity.name,
            role_id: identity.role_id,
            layer_id: identity.layer_id,
            expert_id: identity.expert_id,
            tp_shard_axis: identity.tp_shard_axis,
            flags: identity.flags,
            codec_id: dtype as u16,
            logical_dtype: dtype_id,
            stored_dtype: dtype_id,
            logical_shape,
            padded_shape,
            quant_group_elements: 0,
            ndim,
            aux_required: false,
            metadata: Vec::new(),
            primary_bytes,
            aux_bytes: 0,
        })
    }

    pub fn nvfp4(
        identity: StreamingTensorIdentity,
        metadata: Nvfp4Metadata,
    ) -> Result<Self, StreamRankError> {
        metadata.validate().map_err(StreamRankError::Nvfp4)?;
        Ok(Self {
            tensor_id: identity.tensor_id,
            name: identity.name,
            role_id: identity.role_id,
            layer_id: identity.layer_id,
            expert_id: identity.expert_id,
            tp_shard_axis: identity.tp_shard_axis,
            flags: identity.flags,
            codec_id: metadata.codec as u16,
            logical_dtype: DTYPE_BF16,
            stored_dtype: DTYPE_PACKED_E2M1X2,
            logical_shape: [metadata.logical_n, metadata.logical_k, 1, 1],
            padded_shape: [metadata.padded_n, metadata.padded_k, 1, 1],
            quant_group_elements: 16,
            ndim: 2,
            aux_required: true,
            primary_bytes: u64::from(metadata.value_plane_bytes),
            aux_bytes: u64::from(metadata.scale_plane_bytes),
            metadata: metadata.encode().to_vec(),
        })
    }

    pub fn exl3_source(
        identity: StreamingTensorIdentity,
        metadata: Exl3Metadata,
    ) -> Result<Self, StreamRankError> {
        metadata.validate().map_err(StreamRankError::Exl3)?;
        let layer = u16::try_from(identity.layer_id).map_err(|_| StreamRankError::Spec)?;
        let expert = u16::try_from(identity.expert_id).map_err(|_| StreamRankError::Spec)?;
        if metadata.layer != layer || metadata.expert != expert {
            return Err(StreamRankError::Spec);
        }
        let primary_bytes = metadata
            .trellis_words
            .checked_mul(2)
            .ok_or(StreamRankError::Overflow)?;
        let aux_bytes = metadata
            .rotation_words
            .checked_mul(2)
            .and_then(|bytes| bytes.checked_add(4))
            .ok_or(StreamRankError::Overflow)?;
        let logical_shape = [metadata.logical_n, metadata.logical_k, 1, 1];
        Ok(Self {
            tensor_id: identity.tensor_id,
            name: identity.name,
            role_id: identity.role_id,
            layer_id: identity.layer_id,
            expert_id: identity.expert_id,
            tp_shard_axis: identity.tp_shard_axis,
            flags: identity.flags,
            codec_id: CODEC_EXL3_SOURCE,
            logical_dtype: DTYPE_FP16,
            stored_dtype: DTYPE_I16,
            logical_shape,
            padded_shape: logical_shape,
            quant_group_elements: 0,
            ndim: 2,
            aux_required: true,
            metadata: metadata.encode().to_vec(),
            primary_bytes,
            aux_bytes,
        })
    }
}

#[derive(Clone, Debug)]
pub struct StreamingRankConfig {
    pub rank: u32,
    pub manifest: Vec<u8>,
    pub model_config_sha256: [u8; 32],
    pub tokenizer_bundle_sha256: [u8; 32],
    pub chat_template_sha256: [u8; 32],
    pub weight_policy_sha256: [u8; 32],
    pub kernel_abi_sha256: [u8; 32],
    pub tensors: Vec<StreamingTensorSpec>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StreamingRankSummary {
    pub rank: u32,
    pub tensor_count: usize,
    pub total_file_bytes: u64,
    pub manifest_sha256: [u8; 32],
    pub descriptor_sha256: [u8; 32],
    pub payload_sha256: [u8; 32],
    pub string_sha256: [u8; 32],
    pub metadata_sha256: [u8; 32],
}

impl StreamingRankSummary {
    pub fn derive_conversion_uuid(
        summaries: &[StreamingRankSummary; 4],
    ) -> Result<[u8; 16], StreamRankError> {
        let mut hasher = Sha256::new();
        hasher.update(b"g5n-conversion-v0\0");
        for (expected_rank, summary) in summaries.iter().enumerate() {
            if summary.rank
                != u32::try_from(expected_rank).map_err(|_| StreamRankError::Overflow)?
            {
                return Err(StreamRankError::RankSet);
            }
            hasher.update(summary.manifest_sha256);
            hasher.update(summary.descriptor_sha256);
            hasher.update(summary.payload_sha256);
        }
        Ok(first_16(hasher.finalize().into()))
    }
}

#[derive(Debug)]
struct StreamLayout {
    manifest_offset: usize,
    descriptor_offset: usize,
    string_offset: usize,
    metadata_offset: usize,
    payload_offset: usize,
    total_bytes: usize,
    strings: Vec<u8>,
    metadata: Vec<u8>,
    descriptors: Vec<TensorDescriptor>,
}

impl StreamLayout {
    fn new(config: &StreamingRankConfig) -> Result<Self, StreamRankError> {
        if config.rank > 3 || config.tensors.is_empty() {
            return Err(StreamRankError::Spec);
        }
        let manifest_offset = ALIGNMENT;
        let descriptor_offset = align_up(
            manifest_offset
                .checked_add(config.manifest.len())
                .ok_or(StreamRankError::Overflow)?,
            ALIGNMENT,
        )
        .map_err(StreamRankError::RankFile)?;
        let descriptor_bytes = config
            .tensors
            .len()
            .checked_mul(DESCRIPTOR_BYTES)
            .ok_or(StreamRankError::Overflow)?;
        let string_offset = align_up(
            descriptor_offset
                .checked_add(descriptor_bytes)
                .ok_or(StreamRankError::Overflow)?,
            ALIGNMENT,
        )
        .map_err(StreamRankError::RankFile)?;

        let mut strings = Vec::new();
        let mut name_offsets = Vec::with_capacity(config.tensors.len());
        for (index, tensor) in config.tensors.iter().enumerate() {
            if tensor.tensor_id != u32::try_from(index).map_err(|_| StreamRankError::Overflow)?
                || tensor.name.is_empty()
                || tensor
                    .name
                    .bytes()
                    .any(|byte| byte == 0 || byte.is_ascii_control())
                || tensor.flags & DESCRIPTOR_FLAG_AUX_REQUIRED != 0
            {
                return Err(StreamRankError::Spec);
            }
            name_offsets.push(strings.len());
            strings.extend_from_slice(tensor.name.as_bytes());
        }
        let metadata_offset = align_up(
            string_offset
                .checked_add(strings.len())
                .ok_or(StreamRankError::Overflow)?,
            ALIGNMENT,
        )
        .map_err(StreamRankError::RankFile)?;
        let mut metadata = Vec::new();
        let mut metadata_locals = Vec::with_capacity(config.tensors.len());
        for tensor in &config.tensors {
            metadata_locals.push(metadata.len());
            metadata.extend_from_slice(&tensor.metadata);
        }
        let payload_offset = align_up(
            metadata_offset
                .checked_add(metadata.len())
                .ok_or(StreamRankError::Overflow)?,
            ALIGNMENT,
        )
        .map_err(StreamRankError::RankFile)?;

        let mut cursor = payload_offset;
        let mut descriptors = Vec::with_capacity(config.tensors.len());
        for (index, tensor) in config.tensors.iter().enumerate() {
            let primary_offset =
                align_up(cursor, PAYLOAD_ALIGNMENT).map_err(StreamRankError::RankFile)?;
            let primary_bytes =
                usize::try_from(tensor.primary_bytes).map_err(|_| StreamRankError::Overflow)?;
            let primary_end = primary_offset
                .checked_add(primary_bytes)
                .ok_or(StreamRankError::Overflow)?;
            let aux_offset =
                align_up(primary_end, PAYLOAD_ALIGNMENT).map_err(StreamRankError::RankFile)?;
            let aux_bytes =
                usize::try_from(tensor.aux_bytes).map_err(|_| StreamRankError::Overflow)?;
            cursor = aux_offset
                .checked_add(aux_bytes)
                .ok_or(StreamRankError::Overflow)?;
            descriptors.push(TensorDescriptor {
                tensor_id: tensor.tensor_id,
                name_offset: u32::try_from(name_offsets[index])
                    .map_err(|_| StreamRankError::Overflow)?,
                name_bytes: u16::try_from(tensor.name.len())
                    .map_err(|_| StreamRankError::Overflow)?,
                role_id: tensor.role_id,
                layer_id: tensor.layer_id,
                expert_id: tensor.expert_id,
                codec_id: tensor.codec_id,
                logical_dtype: tensor.logical_dtype,
                stored_dtype: tensor.stored_dtype,
                tp_shard_axis: tensor.tp_shard_axis,
                ndim: tensor.ndim,
                flags: tensor.flags
                    | if tensor.aux_required {
                        DESCRIPTOR_FLAG_AUX_REQUIRED
                    } else {
                        0
                    },
                logical_shape: tensor.logical_shape,
                padded_shape: tensor.padded_shape,
                payload_offset: u64::try_from(primary_offset)
                    .map_err(|_| StreamRankError::Overflow)?,
                payload_bytes: tensor.primary_bytes,
                aux_offset: u64::try_from(aux_offset).map_err(|_| StreamRankError::Overflow)?,
                aux_bytes: tensor.aux_bytes,
                codec_metadata_offset: u64::try_from(
                    metadata_offset
                        .checked_add(metadata_locals[index])
                        .ok_or(StreamRankError::Overflow)?,
                )
                .map_err(|_| StreamRankError::Overflow)?,
                codec_metadata_bytes: tensor.metadata.len() as u64,
                payload_alignment: PAYLOAD_ALIGNMENT as u32,
                quant_group_elements: tensor.quant_group_elements,
                payload_sha256: [0; 32],
                aux_sha256: [0; 32],
                codec_metadata_sha256: sha256(&tensor.metadata),
            });
        }
        derive_header_flags(&descriptors).map_err(StreamRankError::RankFile)?;
        Ok(Self {
            manifest_offset,
            descriptor_offset,
            string_offset,
            metadata_offset,
            payload_offset,
            total_bytes: cursor,
            strings,
            metadata,
            descriptors,
        })
    }
}

#[derive(Debug)]
pub struct StreamingRankWriter {
    path: PathBuf,
    file: File,
    config: StreamingRankConfig,
    layout: StreamLayout,
    completed: Vec<bool>,
}

impl StreamingRankWriter {
    pub fn create_or_resume(
        path: impl AsRef<Path>,
        config: StreamingRankConfig,
    ) -> Result<Self, StreamRankError> {
        let path = path.as_ref().to_owned();
        let layout = StreamLayout::new(&config)?;
        let created = match OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(file) => {
                file.set_len(
                    u64::try_from(layout.total_bytes).map_err(|_| StreamRankError::Overflow)?,
                )
                .map_err(StreamRankError::Io)?;
                write_all_at(&file, &config.manifest, layout.manifest_offset as u64)?;
                write_all_at(&file, &layout.strings, layout.string_offset as u64)?;
                write_all_at(&file, &layout.metadata, layout.metadata_offset as u64)?;
                file.sync_all().map_err(StreamRankError::Io)?;
                Some(file)
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => None,
            Err(error) => return Err(StreamRankError::Io(error)),
        };
        let file = match created {
            Some(file) => file,
            None => {
                let metadata = path.symlink_metadata().map_err(StreamRankError::Io)?;
                if !metadata.file_type().is_file() || metadata.nlink() != 1 {
                    return Err(StreamRankError::UnsafePath);
                }
                OpenOptions::new()
                    .read(true)
                    .write(true)
                    .open(&path)
                    .map_err(StreamRankError::Io)?
            }
        };
        let mut writer = Self {
            path,
            file,
            completed: vec![false; config.tensors.len()],
            config,
            layout,
        };
        writer.verify_staging()?;
        Ok(writer)
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    #[must_use]
    pub fn completed_tensors(&self) -> usize {
        self.completed
            .iter()
            .filter(|&&completed| completed)
            .count()
    }

    pub fn write_tensor(
        &mut self,
        index: usize,
        primary: &mut impl Read,
        aux: &mut impl Read,
    ) -> Result<(), StreamRankError> {
        let expected = self
            .layout
            .descriptors
            .get(index)
            .ok_or(StreamRankError::TensorIndex)?
            .clone();
        if self.completed[index] {
            return Err(StreamRankError::AlreadyComplete(index));
        }
        let primary_hash = copy_exact_at(
            &self.file,
            primary,
            expected.payload_offset,
            expected.payload_bytes,
        )?;
        let aux_hash = copy_exact_at(&self.file, aux, expected.aux_offset, expected.aux_bytes)?;
        self.validate_planes(index)?;
        self.file.sync_data().map_err(StreamRankError::Io)?;

        let mut completed = expected;
        completed.payload_sha256 = primary_hash;
        completed.aux_sha256 = aux_hash;
        let descriptor_offset = self
            .layout
            .descriptor_offset
            .checked_add(
                index
                    .checked_mul(DESCRIPTOR_BYTES)
                    .ok_or(StreamRankError::Overflow)?,
            )
            .ok_or(StreamRankError::Overflow)?;
        write_all_at(
            &self.file,
            &completed.encode(),
            u64::try_from(descriptor_offset).map_err(|_| StreamRankError::Overflow)?,
        )?;
        self.file.sync_data().map_err(StreamRankError::Io)?;
        self.completed[index] = true;
        Ok(())
    }

    pub fn prepare(&self) -> Result<StreamingRankSummary, StreamRankError> {
        if self.completed.iter().any(|&completed| !completed) {
            return Err(StreamRankError::Incomplete);
        }
        let descriptors = self.read_completed_descriptors()?;
        let descriptor_sha256 = hash_range(
            &self.file,
            self.layout.descriptor_offset as u64,
            descriptors
                .len()
                .checked_mul(DESCRIPTOR_BYTES)
                .ok_or(StreamRankError::Overflow)? as u64,
        )?;
        let payload_sha256 = self.audit_payload(&descriptors)?;
        Ok(StreamingRankSummary {
            rank: self.config.rank,
            tensor_count: descriptors.len(),
            total_file_bytes: self.layout.total_bytes as u64,
            manifest_sha256: sha256(&self.config.manifest),
            descriptor_sha256,
            payload_sha256,
            string_sha256: sha256(&self.layout.strings),
            metadata_sha256: sha256(&self.layout.metadata),
        })
    }

    pub fn finalize(
        &mut self,
        expected: &StreamingRankSummary,
        conversion_uuid: [u8; 16],
    ) -> Result<(), StreamRankError> {
        let observed = self.prepare()?;
        if &observed != expected {
            return Err(StreamRankError::SourceChanged);
        }
        let descriptors = self.read_completed_descriptors()?;
        let header_flags = derive_header_flags(&descriptors).map_err(StreamRankError::RankFile)?;
        let header = encode_rank_header(
            &RankHeaderFields {
                rank: self.config.rank,
                tensor_count: descriptors.len(),
                header_flags,
                manifest_offset: self.layout.manifest_offset,
                manifest_bytes: self.config.manifest.len(),
                descriptor_offset: self.layout.descriptor_offset,
                descriptor_bytes: descriptors
                    .len()
                    .checked_mul(DESCRIPTOR_BYTES)
                    .ok_or(StreamRankError::Overflow)?,
                string_offset: self.layout.string_offset,
                string_bytes: self.layout.strings.len(),
                metadata_offset: self.layout.metadata_offset,
                metadata_bytes: self.layout.metadata.len(),
                payload_offset: self.layout.payload_offset,
                payload_bytes: self
                    .layout
                    .total_bytes
                    .checked_sub(self.layout.payload_offset)
                    .ok_or(StreamRankError::Overflow)?,
                model_config_sha256: self.config.model_config_sha256,
                tokenizer_bundle_sha256: self.config.tokenizer_bundle_sha256,
                chat_template_sha256: self.config.chat_template_sha256,
                weight_policy_sha256: self.config.weight_policy_sha256,
                kernel_abi_sha256: self.config.kernel_abi_sha256,
                manifest_sha256: observed.manifest_sha256,
                descriptor_sha256: observed.descriptor_sha256,
                payload_sha256: observed.payload_sha256,
                string_sha256: observed.string_sha256,
                metadata_sha256: observed.metadata_sha256,
            },
            conversion_uuid,
        )
        .map_err(StreamRankError::RankFile)?;
        write_all_at(&self.file, &header, 0)?;
        self.file.sync_all().map_err(StreamRankError::Io)
    }

    fn verify_staging(&mut self) -> Result<(), StreamRankError> {
        let length = self.file.metadata().map_err(StreamRankError::Io)?.len();
        if length != self.layout.total_bytes as u64 {
            return Err(StreamRankError::Layout);
        }
        if !range_is_zero(&self.file, 0, crate::HEADER_BYTES as u64)? {
            return Err(StreamRankError::Finalized);
        }
        verify_range(
            &self.file,
            self.layout.manifest_offset as u64,
            &self.config.manifest,
        )?;
        verify_range(
            &self.file,
            self.layout.string_offset as u64,
            &self.layout.strings,
        )?;
        verify_range(
            &self.file,
            self.layout.metadata_offset as u64,
            &self.layout.metadata,
        )?;
        self.verify_fixed_padding()?;

        for index in 0..self.layout.descriptors.len() {
            let offset = self.layout.descriptor_offset + index * DESCRIPTOR_BYTES;
            let mut bytes = [0_u8; DESCRIPTOR_BYTES];
            read_exact_at(&self.file, &mut bytes, offset as u64)?;
            if bytes.iter().all(|&byte| byte == 0) {
                continue;
            }
            let observed = TensorDescriptor::decode(&bytes).map_err(StreamRankError::RankFile)?;
            let mut expected = self.layout.descriptors[index].clone();
            expected.payload_sha256 = observed.payload_sha256;
            expected.aux_sha256 = observed.aux_sha256;
            if observed != expected
                || hash_range(&self.file, observed.payload_offset, observed.payload_bytes)?
                    != observed.payload_sha256
                || hash_range(&self.file, observed.aux_offset, observed.aux_bytes)?
                    != observed.aux_sha256
            {
                return Err(StreamRankError::ResumeCorruption(index));
            }
            self.completed[index] = true;
            self.validate_planes(index)?;
        }
        Ok(())
    }

    fn verify_fixed_padding(&self) -> Result<(), StreamRankError> {
        let descriptor_bytes = self.layout.descriptors.len() * DESCRIPTOR_BYTES;
        for (start, end) in [
            (crate::HEADER_BYTES, self.layout.manifest_offset),
            (
                self.layout.manifest_offset + self.config.manifest.len(),
                self.layout.descriptor_offset,
            ),
            (
                self.layout.descriptor_offset + descriptor_bytes,
                self.layout.string_offset,
            ),
            (
                self.layout.string_offset + self.layout.strings.len(),
                self.layout.metadata_offset,
            ),
            (
                self.layout.metadata_offset + self.layout.metadata.len(),
                self.layout.payload_offset,
            ),
        ] {
            if start < end && !range_is_zero(&self.file, start as u64, (end - start) as u64)? {
                return Err(StreamRankError::Layout);
            }
        }
        let mut cursor = self.layout.payload_offset;
        for descriptor in &self.layout.descriptors {
            let primary = descriptor.payload_offset as usize;
            let aux = descriptor.aux_offset as usize;
            if cursor < primary
                && !range_is_zero(&self.file, cursor as u64, (primary - cursor) as u64)?
            {
                return Err(StreamRankError::Layout);
            }
            let primary_end = primary + descriptor.payload_bytes as usize;
            if primary_end < aux
                && !range_is_zero(&self.file, primary_end as u64, (aux - primary_end) as u64)?
            {
                return Err(StreamRankError::Layout);
            }
            cursor = aux + descriptor.aux_bytes as usize;
        }
        Ok(())
    }

    fn validate_planes(&self, index: usize) -> Result<(), StreamRankError> {
        let descriptor = &self.layout.descriptors[index];
        match descriptor.codec_id {
            CODEC_BF16_ROW_MAJOR | CODEC_FP16_ROW_MAJOR | CODEC_FP32_ROW_MAJOR => {
                let dtype = match descriptor.codec_id {
                    CODEC_BF16_ROW_MAJOR => PlainDtype::Bf16,
                    CODEC_FP16_ROW_MAJOR => PlainDtype::Fp16,
                    CODEC_FP32_ROW_MAJOR => PlainDtype::Fp32,
                    _ => unreachable!(),
                };
                if descriptor.logical_shape == descriptor.padded_shape {
                    return Ok(());
                }
                let primary = read_range_vec(
                    &self.file,
                    descriptor.payload_offset,
                    descriptor.payload_bytes,
                )?;
                validate_plain_padding(
                    &primary,
                    dtype,
                    descriptor.ndim,
                    descriptor.logical_shape,
                    descriptor.padded_shape,
                )
                .map_err(StreamRankError::RankFile)
            }
            0x0100 | 0x0101 => {
                let aux = read_range_vec(&self.file, descriptor.aux_offset, descriptor.aux_bytes)?;
                validate_scale_plane(&aux).map_err(StreamRankError::Nvfp4)
            }
            CODEC_EXL3_SOURCE => {
                let metadata = Exl3Metadata::decode(&self.config.tensors[index].metadata)
                    .map_err(StreamRankError::Exl3)?;
                let primary = read_range_vec(
                    &self.file,
                    descriptor.payload_offset,
                    descriptor.payload_bytes,
                )?;
                let aux = read_range_vec(&self.file, descriptor.aux_offset, descriptor.aux_bytes)?;
                Exl3Trellis::from_container_planes(metadata, &primary, &aux)
                    .map(|_| ())
                    .map_err(StreamRankError::Exl3)
            }
            _ => Err(StreamRankError::Spec),
        }
    }

    fn read_completed_descriptors(&self) -> Result<Vec<TensorDescriptor>, StreamRankError> {
        let mut descriptors = Vec::with_capacity(self.layout.descriptors.len());
        for index in 0..self.layout.descriptors.len() {
            let mut bytes = [0_u8; DESCRIPTOR_BYTES];
            let offset = self.layout.descriptor_offset + index * DESCRIPTOR_BYTES;
            read_exact_at(&self.file, &mut bytes, offset as u64)?;
            let descriptor = TensorDescriptor::decode(&bytes).map_err(StreamRankError::RankFile)?;
            descriptors.push(descriptor);
        }
        Ok(descriptors)
    }

    fn audit_payload(&self, descriptors: &[TensorDescriptor]) -> Result<[u8; 32], StreamRankError> {
        let mut global = Sha256::new();
        let mut cursor = self.layout.payload_offset as u64;
        for descriptor in descriptors {
            audit_zero_into(
                &self.file,
                cursor,
                descriptor
                    .payload_offset
                    .checked_sub(cursor)
                    .ok_or(StreamRankError::Layout)?,
                &mut global,
            )?;
            let primary = audit_data_into(
                &self.file,
                descriptor.payload_offset,
                descriptor.payload_bytes,
                &mut global,
            )?;
            if primary != descriptor.payload_sha256 {
                return Err(StreamRankError::SourceChanged);
            }
            let primary_end = descriptor
                .payload_offset
                .checked_add(descriptor.payload_bytes)
                .ok_or(StreamRankError::Overflow)?;
            audit_zero_into(
                &self.file,
                primary_end,
                descriptor
                    .aux_offset
                    .checked_sub(primary_end)
                    .ok_or(StreamRankError::Layout)?,
                &mut global,
            )?;
            let aux = audit_data_into(
                &self.file,
                descriptor.aux_offset,
                descriptor.aux_bytes,
                &mut global,
            )?;
            if aux != descriptor.aux_sha256 {
                return Err(StreamRankError::SourceChanged);
            }
            cursor = descriptor
                .aux_offset
                .checked_add(descriptor.aux_bytes)
                .ok_or(StreamRankError::Overflow)?;
        }
        if cursor != self.layout.total_bytes as u64 {
            return Err(StreamRankError::Layout);
        }
        Ok(global.finalize().into())
    }
}

fn copy_exact_at(
    file: &File,
    input: &mut impl Read,
    offset: u64,
    bytes: u64,
) -> Result<[u8; 32], StreamRankError> {
    let buffer_bytes = usize::try_from(bytes.clamp(1, STREAM_BUFFER_BYTES as u64))
        .map_err(|_| StreamRankError::Overflow)?;
    let mut buffer = vec![0_u8; buffer_bytes];
    let mut hasher = Sha256::new();
    let mut consumed = 0_u64;
    while consumed < bytes {
        let chunk = usize::try_from((bytes - consumed).min(buffer.len() as u64))
            .map_err(|_| StreamRankError::Overflow)?;
        input
            .read_exact(&mut buffer[..chunk])
            .map_err(StreamRankError::Io)?;
        write_all_at(
            file,
            &buffer[..chunk],
            offset
                .checked_add(consumed)
                .ok_or(StreamRankError::Overflow)?,
        )?;
        hasher.update(&buffer[..chunk]);
        consumed += chunk as u64;
    }
    if input.read(&mut buffer[..1]).map_err(StreamRankError::Io)? != 0 {
        return Err(StreamRankError::TrailingSource);
    }
    Ok(hasher.finalize().into())
}

fn hash_range(file: &File, offset: u64, bytes: u64) -> Result<[u8; 32], StreamRankError> {
    let mut sink = Sha256::new();
    audit_data_into(file, offset, bytes, &mut sink)
}

fn audit_data_into(
    file: &File,
    offset: u64,
    bytes: u64,
    global: &mut Sha256,
) -> Result<[u8; 32], StreamRankError> {
    let mut local = Sha256::new();
    read_chunks(file, offset, bytes, |chunk| {
        local.update(chunk);
        global.update(chunk);
        Ok(())
    })?;
    Ok(local.finalize().into())
}

fn audit_zero_into(
    file: &File,
    offset: u64,
    bytes: u64,
    global: &mut Sha256,
) -> Result<(), StreamRankError> {
    read_chunks(file, offset, bytes, |chunk| {
        if chunk.iter().any(|&byte| byte != 0) {
            return Err(StreamRankError::Layout);
        }
        global.update(chunk);
        Ok(())
    })
}

fn read_chunks(
    file: &File,
    offset: u64,
    bytes: u64,
    mut consume: impl FnMut(&[u8]) -> Result<(), StreamRankError>,
) -> Result<(), StreamRankError> {
    let buffer_bytes = usize::try_from(bytes.clamp(1, STREAM_BUFFER_BYTES as u64))
        .map_err(|_| StreamRankError::Overflow)?;
    let mut buffer = vec![0_u8; buffer_bytes];
    let mut consumed = 0_u64;
    while consumed < bytes {
        let chunk = usize::try_from((bytes - consumed).min(buffer.len() as u64))
            .map_err(|_| StreamRankError::Overflow)?;
        read_exact_at(
            file,
            &mut buffer[..chunk],
            offset
                .checked_add(consumed)
                .ok_or(StreamRankError::Overflow)?,
        )?;
        consume(&buffer[..chunk])?;
        consumed += chunk as u64;
    }
    Ok(())
}

fn range_is_zero(file: &File, offset: u64, bytes: u64) -> Result<bool, StreamRankError> {
    let mut zero = true;
    read_chunks(file, offset, bytes, |chunk| {
        zero &= chunk.iter().all(|&byte| byte == 0);
        Ok(())
    })?;
    Ok(zero)
}

fn verify_range(file: &File, offset: u64, expected: &[u8]) -> Result<(), StreamRankError> {
    let mut observed = vec![0_u8; expected.len()];
    read_exact_at(file, &mut observed, offset)?;
    if observed != expected {
        return Err(StreamRankError::Layout);
    }
    Ok(())
}

fn read_range_vec(file: &File, offset: u64, bytes: u64) -> Result<Vec<u8>, StreamRankError> {
    let mut output = vec![0_u8; usize::try_from(bytes).map_err(|_| StreamRankError::Overflow)?];
    read_exact_at(file, &mut output, offset)?;
    Ok(output)
}

fn read_exact_at(
    file: &File,
    mut output: &mut [u8],
    mut offset: u64,
) -> Result<(), StreamRankError> {
    while !output.is_empty() {
        let read = file.read_at(output, offset).map_err(StreamRankError::Io)?;
        if read == 0 {
            return Err(StreamRankError::Io(io::Error::from(
                io::ErrorKind::UnexpectedEof,
            )));
        }
        offset = offset
            .checked_add(read as u64)
            .ok_or(StreamRankError::Overflow)?;
        output = &mut output[read..];
    }
    Ok(())
}

fn write_all_at(file: &File, mut input: &[u8], mut offset: u64) -> Result<(), StreamRankError> {
    while !input.is_empty() {
        let written = file.write_at(input, offset).map_err(StreamRankError::Io)?;
        if written == 0 {
            return Err(StreamRankError::Io(io::Error::from(
                io::ErrorKind::WriteZero,
            )));
        }
        offset = offset
            .checked_add(written as u64)
            .ok_or(StreamRankError::Overflow)?;
        input = &input[written..];
    }
    Ok(())
}

#[derive(Debug)]
pub enum StreamRankError {
    Io(io::Error),
    RankFile(RankFileError),
    Nvfp4(crate::Nvfp4Error),
    Exl3(crate::Exl3Error),
    Spec,
    Layout,
    Overflow,
    TensorIndex,
    AlreadyComplete(usize),
    Incomplete,
    TrailingSource,
    ResumeCorruption(usize),
    SourceChanged,
    Finalized,
    RankSet,
    UnsafePath,
}

impl fmt::Display for StreamRankError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for StreamRankError {}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        sync::atomic::{AtomicU64, Ordering},
    };

    use crate::{
        Codec, EXL3_MCG_MULTIPLIER, Exl3Projection, PackedNvfp4, PlainTensor, RankFile,
        RankFileBuilder, TensorPayload, TensorRecord,
    };

    use super::*;

    static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

    struct TempPath(PathBuf);

    impl TempPath {
        fn new() -> Self {
            let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            Self(std::env::temp_dir().join(format!(
                "glmaxx-stream-rank-{}-{sequence}.partial",
                std::process::id()
            )))
        }
    }

    impl Drop for TempPath {
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.0);
        }
    }

    fn fixtures(rank: u32) -> (PackedNvfp4, Exl3Trellis) {
        let values: Vec<f32> = (0..128 * 128)
            .map(|index| ((index % 29) as f32 - 14.0) / 8.0)
            .collect();
        let nvfp4 = PackedNvfp4::pack(&values, 128, 128, Codec::OneDimensional).unwrap();
        let metadata =
            Exl3Metadata::new(Exl3Projection::Gate, 78, 0, rank as u8, 3, 128, 128).unwrap();
        let exl3 = Exl3Trellis {
            trellis: (0..metadata.trellis_words)
                .map(|index| (index as u16).wrapping_mul(40_503))
                .collect(),
            suh: vec![0x3c00; 128],
            svh: vec![0x3c00; 128],
            mcg_marker: EXL3_MCG_MULTIPLIER,
            metadata,
        };
        (nvfp4, exl3)
    }

    fn configs(rank: u32) -> (StreamingRankConfig, RankFileBuilder, Vec<Vec<u8>>) {
        let (nvfp4, exl3) = fixtures(rank);
        let records = vec![
            TensorRecord {
                tensor_id: 0,
                name: "nvfp4".into(),
                role_id: 1,
                layer_id: 3,
                expert_id: 0,
                tp_shard_axis: 0,
                flags: 0,
                payload: TensorPayload::Nvfp4(nvfp4.clone()),
            },
            TensorRecord {
                tensor_id: 1,
                name: "exl3".into(),
                role_id: 2,
                layer_id: 78,
                expert_id: 0,
                tp_shard_axis: 1,
                flags: 0,
                payload: TensorPayload::Exl3Source(exl3.clone()),
            },
            TensorRecord {
                tensor_id: 2,
                name: "plain".into(),
                role_id: 3,
                layer_id: -1,
                expert_id: -1,
                tp_shard_axis: -1,
                flags: 1,
                payload: TensorPayload::Plain(PlainTensor {
                    dtype: PlainDtype::Bf16,
                    ndim: 1,
                    logical_shape: [3, 1, 1, 1],
                    padded_shape: [4, 1, 1, 1],
                    bytes: vec![1, 0, 2, 0, 3, 0, 0, 0],
                }),
            },
        ];
        let tensors = vec![
            StreamingTensorSpec::nvfp4(
                StreamingTensorIdentity {
                    tensor_id: 0,
                    name: "nvfp4".into(),
                    role_id: 1,
                    layer_id: 3,
                    expert_id: 0,
                    tp_shard_axis: 0,
                    flags: 0,
                },
                nvfp4.metadata.clone(),
            )
            .unwrap(),
            StreamingTensorSpec::exl3_source(
                StreamingTensorIdentity {
                    tensor_id: 1,
                    name: "exl3".into(),
                    role_id: 2,
                    layer_id: 78,
                    expert_id: 0,
                    tp_shard_axis: 1,
                    flags: 0,
                },
                exl3.metadata.clone(),
            )
            .unwrap(),
            StreamingTensorSpec::plain(
                StreamingTensorIdentity {
                    tensor_id: 2,
                    name: "plain".into(),
                    role_id: 3,
                    layer_id: -1,
                    expert_id: -1,
                    tp_shard_axis: -1,
                    flags: 1,
                },
                PlainDtype::Bf16,
                1,
                [3, 1, 1, 1],
                [4, 1, 1, 1],
            )
            .unwrap(),
        ];
        let config = StreamingRankConfig {
            rank,
            manifest: b"manifest".to_vec(),
            model_config_sha256: [1; 32],
            tokenizer_bundle_sha256: [2; 32],
            chat_template_sha256: [3; 32],
            weight_policy_sha256: [4; 32],
            kernel_abi_sha256: [5; 32],
            tensors,
        };
        let builder = RankFileBuilder {
            rank,
            manifest: config.manifest.clone(),
            model_config_sha256: config.model_config_sha256,
            tokenizer_bundle_sha256: config.tokenizer_bundle_sha256,
            chat_template_sha256: config.chat_template_sha256,
            weight_policy_sha256: config.weight_policy_sha256,
            kernel_abi_sha256: config.kernel_abi_sha256,
            tensors: records,
        };
        let planes = vec![
            nvfp4.values,
            nvfp4.scales,
            exl3.primary_plane().unwrap(),
            exl3.aux_plane().unwrap(),
            vec![1, 0, 2, 0, 3, 0, 0, 0],
            Vec::new(),
        ];
        (config, builder, planes)
    }

    #[test]
    fn streaming_bytes_match_in_memory_builder_and_resume() {
        let path = TempPath::new();
        let (config, builder, planes) = configs(0);
        {
            let mut writer =
                StreamingRankWriter::create_or_resume(&path.0, config.clone()).unwrap();
            writer
                .write_tensor(0, &mut &planes[0][..], &mut &planes[1][..])
                .unwrap();
            assert_eq!(writer.completed_tensors(), 1);
        }
        let mut writer = StreamingRankWriter::create_or_resume(&path.0, config).unwrap();
        assert_eq!(writer.completed_tensors(), 1);
        writer
            .write_tensor(1, &mut &planes[2][..], &mut &planes[3][..])
            .unwrap();
        writer
            .write_tensor(2, &mut &planes[4][..], &mut &planes[5][..])
            .unwrap();
        let summary = writer.prepare().unwrap();
        let conversion_uuid = [9; 16];
        writer.finalize(&summary, conversion_uuid).unwrap();
        drop(writer);

        let observed = fs::read(&path.0).unwrap();
        let expected = builder.build(conversion_uuid).unwrap();
        assert_eq!(observed.len(), expected.len());
        let difference = observed
            .iter()
            .zip(&expected)
            .position(|(observed, expected)| observed != expected);
        let body_difference = observed[crate::HEADER_BYTES..]
            .iter()
            .zip(&expected[crate::HEADER_BYTES..])
            .position(|(observed, expected)| observed != expected)
            .map(|offset| offset + crate::HEADER_BYTES);
        assert!(
            difference.is_none(),
            "first streaming/reference difference at {difference:?}; body {body_difference:?}"
        );
        RankFile::read(observed).unwrap();
    }

    #[test]
    fn truncated_trailing_and_corrupt_resume_fail_closed() {
        let path = TempPath::new();
        let (config, _, mut planes) = configs(0);
        let mut writer = StreamingRankWriter::create_or_resume(&path.0, config.clone()).unwrap();
        let mut short = &planes[0][..planes[0].len() - 1];
        assert!(matches!(
            writer.write_tensor(0, &mut short, &mut &planes[1][..]),
            Err(StreamRankError::Io(_))
        ));
        planes[0].push(0);
        assert!(matches!(
            writer.write_tensor(0, &mut &planes[0][..], &mut &planes[1][..]),
            Err(StreamRankError::TrailingSource)
        ));
        planes[0].pop();
        writer
            .write_tensor(0, &mut &planes[0][..], &mut &planes[1][..])
            .unwrap();
        let descriptor = &writer.layout.descriptors[0];
        let mut corrupt = [0_u8; 1];
        writer
            .file
            .read_at(&mut corrupt, descriptor.payload_offset)
            .unwrap();
        corrupt[0] ^= 1;
        writer
            .file
            .write_at(&corrupt, descriptor.payload_offset)
            .unwrap();
        writer.file.sync_all().unwrap();
        drop(writer);
        let error = StreamingRankWriter::create_or_resume(&path.0, config).unwrap_err();
        assert!(
            matches!(error, StreamRankError::ResumeCorruption(0)),
            "unexpected error: {error:?}"
        );
    }

    #[test]
    fn conversion_uuid_requires_rank_order() {
        let summaries: [StreamingRankSummary; 4] =
            std::array::from_fn(|rank| StreamingRankSummary {
                rank: rank as u32,
                tensor_count: 1,
                total_file_bytes: 1,
                manifest_sha256: [rank as u8; 32],
                descriptor_sha256: [rank as u8 + 1; 32],
                payload_sha256: [rank as u8 + 2; 32],
                string_sha256: [0; 32],
                metadata_sha256: [0; 32],
            });
        assert_ne!(
            StreamingRankSummary::derive_conversion_uuid(&summaries).unwrap(),
            [0; 16]
        );
        let mut wrong = summaries;
        wrong.swap(0, 1);
        assert!(matches!(
            StreamingRankSummary::derive_conversion_uuid(&wrong),
            Err(StreamRankError::RankSet)
        ));
    }
}
