use std::{
    fmt,
    fs::{File, OpenOptions},
    io,
    os::unix::fs::{FileExt, MetadataExt, OpenOptionsExt},
    path::{Path, PathBuf},
    sync::Arc,
};

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::{
    Exl3Metadata, Exl3Trellis, Nvfp4Metadata, PlainDtype, RankFileError, TensorDescriptor,
    ValidatedRankManifest,
    container::{
        ALIGNMENT, CODEC_BF16_ROW_MAJOR, CODEC_EXL3_SOURCE, CODEC_FP16_ROW_MAJOR,
        CODEC_FP32_ROW_MAJOR, CODEC_NVFP4_1D, CODEC_NVFP4_2D, DESCRIPTOR_BYTES,
        DESCRIPTOR_FLAG_AUX_REQUIRED, DTYPE_BF16, DTYPE_FP16, DTYPE_I16, DTYPE_PACKED_E2M1X2,
        PAYLOAD_ALIGNMENT, align_up, derive_header_flags, validate_plain_geometry,
        validate_plain_padding_chunk,
    },
    crc32c,
    nvfp4::Nvfp4PlaneValidator,
    rank_manifest::{RankManifestContext, RankManifestError, validate_rank_manifest},
};

const STREAM_BUFFER_BYTES: usize = 8 * 1024 * 1024;
const CONTROL_REGION_MAX_BYTES: u64 = 1 << 30;
const CONTROL_REGIONS_TOTAL_MAX_BYTES: u64 = 512 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FileFingerprint {
    device: u64,
    inode: u64,
    bytes: u64,
    modified_seconds: i64,
    modified_nanoseconds: i64,
    changed_seconds: i64,
    changed_nanoseconds: i64,
}

impl FileFingerprint {
    fn from_metadata(metadata: &std::fs::Metadata) -> Self {
        Self {
            device: metadata.dev(),
            inode: metadata.ino(),
            bytes: metadata.len(),
            modified_seconds: metadata.mtime(),
            modified_nanoseconds: metadata.mtime_nsec(),
            changed_seconds: metadata.ctime(),
            changed_nanoseconds: metadata.ctime_nsec(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FileRegion {
    start: u64,
    end: u64,
}

impl FileRegion {
    fn from_header(
        header: &[u8],
        offset_field: usize,
        length_field: usize,
        file_bytes: u64,
        alignment: u64,
    ) -> Result<Self, NativeRankReaderError> {
        let start = get_u64(header, offset_field);
        let bytes = get_u64(header, length_field);
        let end = start.checked_add(bytes).ok_or(RankFileError::Overflow)?;
        if !start.is_multiple_of(alignment) || start > end || end > file_bytes {
            return Err(RankFileError::Region.into());
        }
        Ok(Self { start, end })
    }

    fn len(self) -> u64 {
        self.end - self.start
    }
}

/// A bounded-memory view of one immutable native rank image.
///
/// Opening validates the header, all control regions, descriptor geometry,
/// canonical offsets, names, metadata, UUID, and non-payload hashes. Payload
/// bytes are verified and delivered in one sequential pass by
/// [`Self::verify_and_stream`]. A sink must treat all delivered bytes as
/// tentative until that method returns `Ok`.
#[derive(Debug)]
pub struct NativeRankReader {
    path: PathBuf,
    file: File,
    fingerprint: FileFingerprint,
    pub rank: u32,
    pub conversion_uuid: [u8; 16],
    pub file_uuid: [u8; 16],
    pub header_flags: u32,
    pub model_config_sha256: [u8; 32],
    pub tokenizer_bundle_sha256: [u8; 32],
    pub chat_template_sha256: [u8; 32],
    pub weight_policy_sha256: [u8; 32],
    pub kernel_abi_sha256: [u8; 32],
    pub manifest_sha256: [u8; 32],
    pub descriptor_sha256: [u8; 32],
    pub payload_sha256: [u8; 32],
    pub descriptors: Vec<TensorDescriptor>,
    names: Vec<String>,
    validated_manifest: Option<ValidatedRankManifest>,
    metadata: Arc<[u8]>,
    metadata_region: FileRegion,
    payload_region: FileRegion,
}

impl NativeRankReader {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, NativeRankReaderError> {
        let path = path.as_ref().to_owned();
        let path_metadata = path.symlink_metadata()?;
        if !path_metadata.file_type().is_file()
            || path_metadata.file_type().is_symlink()
            || path_metadata.nlink() != 1
        {
            return Err(NativeRankReaderError::UnsafeFile(path));
        }
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_CLOEXEC | libc::O_NOFOLLOW)
            .open(&path)?;
        let metadata = file.metadata()?;
        if !metadata.file_type().is_file() || metadata.nlink() != 1 {
            return Err(NativeRankReaderError::UnsafeFile(path));
        }
        let fingerprint = FileFingerprint::from_metadata(&metadata);
        if fingerprint != FileFingerprint::from_metadata(&path_metadata) {
            return Err(NativeRankReaderError::Changed(path));
        }
        let file_bytes = metadata.len();
        if file_bytes < crate::HEADER_BYTES as u64 {
            return Err(RankFileError::Truncated.into());
        }

        let mut header = [0_u8; crate::HEADER_BYTES];
        read_exact_at(&file, &mut header, 0)?;
        let header_flags = get_u32(&header, 20);
        if &header[0..8] != b"GLM5NAT0"
            || get_u16(&header, 8) != 0
            || get_u16(&header, 10) != 2
            || get_u32(&header, 12) != crate::HEADER_BYTES as u32
            || get_u32(&header, 16) != 0x0102_0304
            || header_flags & !0b1_1111 != 0
            || get_u32(&header, 28) != 4
            || header[36..40].iter().any(|&value| value != 0)
            || get_u64(&header, 408) != 0
            || header[484..].iter().any(|&value| value != 0)
        {
            return Err(RankFileError::Header.into());
        }
        let expected_crc = get_u32(&header, 416);
        let mut checked_header = header;
        checked_header[416..420].fill(0);
        if crc32c(&checked_header) != expected_crc {
            return Err(RankFileError::HeaderCrc.into());
        }

        let rank = get_u32(&header, 24);
        if rank > 3 {
            return Err(RankFileError::Rank.into());
        }
        let tensor_count =
            usize::try_from(get_u32(&header, 32)).map_err(|_| RankFileError::Overflow)?;
        if tensor_count == 0 {
            return Err(RankFileError::TensorCount.into());
        }

        let manifest = FileRegion::from_header(&header, 40, 48, file_bytes, ALIGNMENT as u64)?;
        let descriptor = FileRegion::from_header(&header, 56, 64, file_bytes, ALIGNMENT as u64)?;
        let strings = FileRegion::from_header(&header, 72, 80, file_bytes, ALIGNMENT as u64)?;
        let metadata_region =
            FileRegion::from_header(&header, 88, 96, file_bytes, ALIGNMENT as u64)?;
        let payload_region =
            FileRegion::from_header(&header, 104, 112, file_bytes, ALIGNMENT as u64)?;
        let control_bytes = manifest
            .len()
            .checked_add(descriptor.len())
            .and_then(|bytes| bytes.checked_add(strings.len()))
            .and_then(|bytes| bytes.checked_add(metadata_region.len()))
            .ok_or(RankFileError::Overflow)?;
        let expected_descriptor_bytes = tensor_count
            .checked_mul(DESCRIPTOR_BYTES)
            .ok_or(RankFileError::Overflow)?;
        if descriptor.len()
            != u64::try_from(expected_descriptor_bytes).map_err(|_| RankFileError::Overflow)?
            || control_bytes > CONTROL_REGIONS_TOTAL_MAX_BYTES
            || manifest.start != ALIGNMENT as u64
            || descriptor.start != aligned_end(manifest.end, ALIGNMENT)?
            || strings.start != aligned_end(descriptor.end, ALIGNMENT)?
            || metadata_region.start != aligned_end(strings.end, ALIGNMENT)?
            || payload_region.start != aligned_end(metadata_region.end, ALIGNMENT)?
            || payload_region.end != file_bytes
        {
            return Err(RankFileError::Region.into());
        }

        let manifest_bytes = read_control_region(&file, manifest)?;
        let descriptor_bytes = read_control_region(&file, descriptor)?;
        let string_bytes = read_control_region(&file, strings)?;
        let metadata_bytes = read_control_region(&file, metadata_region)?;
        verify_zero_gap(&file, manifest.end, descriptor.start)?;
        verify_zero_gap(&file, descriptor.end, strings.start)?;
        verify_zero_gap(&file, strings.end, metadata_region.start)?;
        verify_zero_gap(&file, metadata_region.end, payload_region.start)?;

        let manifest_sha256 = array32(&header[280..312]);
        let descriptor_sha256 = array32(&header[312..344]);
        let payload_sha256 = array32(&header[344..376]);
        if hash(&manifest_bytes) != manifest_sha256
            || hash(&descriptor_bytes) != descriptor_sha256
            || hash(&string_bytes) != array32(&header[420..452])
            || hash(&metadata_bytes) != array32(&header[452..484])
        {
            return Err(RankFileError::StrongHash.into());
        }
        let mut descriptors = Vec::with_capacity(tensor_count);
        let mut names = Vec::with_capacity(tensor_count);
        let mut name_cursor = 0_usize;
        for index in 0..tensor_count {
            let start = index
                .checked_mul(DESCRIPTOR_BYTES)
                .ok_or(RankFileError::Overflow)?;
            let decoded =
                TensorDescriptor::decode(&descriptor_bytes[start..start + DESCRIPTOR_BYTES])?;
            if decoded.tensor_id != u32::try_from(index).map_err(|_| RankFileError::Overflow)?
                || decoded.name_bytes == 0
                || usize::try_from(decoded.name_offset).map_err(|_| RankFileError::Overflow)?
                    != name_cursor
            {
                return Err(RankFileError::TensorId.into());
            }
            let name_end = name_cursor
                .checked_add(usize::from(decoded.name_bytes))
                .ok_or(RankFileError::Overflow)?;
            let name = std::str::from_utf8(
                string_bytes
                    .get(name_cursor..name_end)
                    .ok_or(RankFileError::StringTable)?,
            )
            .map_err(|_| RankFileError::StringTable)?;
            if name
                .bytes()
                .any(|byte| byte == 0 || byte.is_ascii_control())
            {
                return Err(RankFileError::StringTable.into());
            }
            names.push(name.to_owned());
            name_cursor = name_end;
            descriptors.push(decoded);
        }
        if name_cursor != string_bytes.len() {
            return Err(RankFileError::NonCanonicalLayout.into());
        }

        validate_descriptors(
            &descriptors,
            &metadata_bytes,
            metadata_region,
            payload_region,
            file_bytes,
            rank,
        )?;
        if derive_header_flags(&descriptors)? != header_flags {
            return Err(RankFileError::HeaderFlags.into());
        }
        let model_config_sha256 = array32(&header[120..152]);
        let tokenizer_bundle_sha256 = array32(&header[152..184]);
        let chat_template_sha256 = array32(&header[184..216]);
        let weight_policy_sha256 = array32(&header[216..248]);
        let kernel_abi_sha256 = array32(&header[248..280]);
        let validated_manifest = validate_rank_manifest(
            &manifest_bytes,
            RankManifestContext {
                rank,
                descriptors: &descriptors,
                names: &names,
                model_config_sha256,
                tokenizer_bundle_sha256,
                chat_template_sha256,
                weight_policy_sha256,
                kernel_abi_sha256,
                payload_sha256,
            },
        )?;

        let file_uuid = array16(&header[376..392]);
        let conversion_uuid = array16(&header[392..408]);
        let mut uuid_hasher = Sha256::new();
        uuid_hasher.update(b"g5n-file-v0\0");
        uuid_hasher.update(conversion_uuid);
        uuid_hasher.update(rank.to_le_bytes());
        uuid_hasher.update(manifest_sha256);
        uuid_hasher.update(descriptor_sha256);
        uuid_hasher.update(payload_sha256);
        if first_16(uuid_hasher.finalize().into()) != file_uuid {
            return Err(RankFileError::FileUuid.into());
        }

        let reader = Self {
            path,
            file,
            fingerprint,
            rank,
            conversion_uuid,
            file_uuid,
            header_flags,
            model_config_sha256,
            tokenizer_bundle_sha256,
            chat_template_sha256,
            weight_policy_sha256,
            kernel_abi_sha256,
            manifest_sha256,
            descriptor_sha256,
            payload_sha256,
            descriptors,
            names,
            validated_manifest,
            metadata: metadata_bytes.into(),
            metadata_region,
            payload_region,
        };
        reader.ensure_unchanged()?;
        Ok(reader)
    }

    #[must_use]
    pub fn tensor_count(&self) -> usize {
        self.descriptors.len()
    }

    pub fn tensor_name(&self, index: usize) -> Result<&str, NativeRankReaderError> {
        self.names
            .get(index)
            .map(String::as_str)
            .ok_or_else(|| RankFileError::TensorId.into())
    }

    pub fn tensor_codec_metadata(&self, index: usize) -> Result<&[u8], NativeRankReaderError> {
        let descriptor = self.descriptors.get(index).ok_or(RankFileError::TensorId)?;
        let start = descriptor
            .codec_metadata_offset
            .checked_sub(self.metadata_region.start)
            .ok_or(RankFileError::Region)?;
        let end = start
            .checked_add(descriptor.codec_metadata_bytes)
            .ok_or(RankFileError::Overflow)?;
        self.metadata
            .get(
                usize::try_from(start).map_err(|_| RankFileError::Overflow)?
                    ..usize::try_from(end).map_err(|_| RankFileError::Overflow)?,
            )
            .ok_or_else(|| RankFileError::Region.into())
    }

    #[must_use]
    pub const fn validated_manifest(&self) -> Option<&ValidatedRankManifest> {
        self.validated_manifest.as_ref()
    }

    /// Verifies the payload and streams it to `sink` in canonical tensor
    /// order. The streaming buffer is 8 MiB. EXL3 validation additionally
    /// retains at most one projection's primary and auxiliary planes.
    pub fn verify_and_stream(
        &self,
        sink: &mut impl RankTensorSink,
    ) -> Result<RankPayloadProof, NativeRankReaderError> {
        self.ensure_unchanged()?;
        let mut buffer = vec![0_u8; STREAM_BUFFER_BYTES];
        let mut whole = Sha256::new();
        let mut cursor = self.payload_region.start;
        let mut stream_chunks = 0_u64;
        let mut maximum_reader_scratch_bytes = buffer.len();

        for (index, descriptor) in self.descriptors.iter().enumerate() {
            let name = &self.names[index];
            let metadata = self.tensor_codec_metadata(index)?;
            let mut nvfp4_validator =
                if matches!(descriptor.codec_id, CODEC_NVFP4_1D | CODEC_NVFP4_2D) {
                    let metadata = Nvfp4Metadata::decode(metadata).map_err(RankFileError::Nvfp4)?;
                    Some(Nvfp4PlaneValidator::new(&metadata)?)
                } else {
                    None
                };
            sink.begin_tensor(self.rank, index, name, descriptor, metadata)
                .map_err(NativeRankReaderError::Sink)?;

            stream_padding(
                &self.file,
                &mut buffer,
                cursor,
                descriptor.payload_offset,
                &mut whole,
                &mut stream_chunks,
            )?;

            let collect_exl3 = descriptor.codec_id == CODEC_EXL3_SOURCE;
            let mut exl3_primary = if collect_exl3 {
                Some(Vec::with_capacity(
                    usize::try_from(descriptor.payload_bytes)
                        .map_err(|_| RankFileError::Overflow)?,
                ))
            } else {
                None
            };
            let primary_hash = stream_plane(
                &self.file,
                &mut buffer,
                descriptor.payload_offset,
                descriptor.payload_bytes,
                &mut whole,
                &mut stream_chunks,
                |chunk, offset| {
                    validate_plain_padding_stream_chunk(descriptor, chunk, offset)?;
                    if let Some(validator) = &mut nvfp4_validator {
                        validator.value_chunk(chunk, offset)?;
                    }
                    if let Some(bytes) = &mut exl3_primary {
                        bytes.extend_from_slice(chunk);
                    }
                    sink.primary_chunk(chunk)
                        .map_err(NativeRankReaderError::Sink)
                },
            )?;
            if primary_hash != descriptor.payload_sha256 {
                return Err(RankFileError::TensorRegion.into());
            }

            let primary_end = descriptor
                .payload_offset
                .checked_add(descriptor.payload_bytes)
                .ok_or(RankFileError::Overflow)?;
            stream_padding(
                &self.file,
                &mut buffer,
                primary_end,
                descriptor.aux_offset,
                &mut whole,
                &mut stream_chunks,
            )?;

            let mut exl3_aux = if collect_exl3 {
                Some(Vec::with_capacity(
                    usize::try_from(descriptor.aux_bytes).map_err(|_| RankFileError::Overflow)?,
                ))
            } else {
                None
            };
            maximum_reader_scratch_bytes = maximum_reader_scratch_bytes.max(
                buffer
                    .len()
                    .checked_add(
                        exl3_primary
                            .as_ref()
                            .map_or(0, Vec::capacity)
                            .checked_add(exl3_aux.as_ref().map_or(0, Vec::capacity))
                            .ok_or(RankFileError::Overflow)?,
                    )
                    .ok_or(RankFileError::Overflow)?,
            );
            maximum_reader_scratch_bytes = maximum_reader_scratch_bytes.max(
                buffer
                    .len()
                    .checked_add(
                        nvfp4_validator
                            .as_ref()
                            .map_or(0, Nvfp4PlaneValidator::scratch_bytes),
                    )
                    .ok_or(RankFileError::Overflow)?,
            );
            let aux_hash = stream_plane(
                &self.file,
                &mut buffer,
                descriptor.aux_offset,
                descriptor.aux_bytes,
                &mut whole,
                &mut stream_chunks,
                |chunk, offset| {
                    if let Some(validator) = &mut nvfp4_validator {
                        validator.scale_chunk(chunk, offset)?;
                    }
                    if let Some(bytes) = &mut exl3_aux {
                        bytes.extend_from_slice(chunk);
                    }
                    sink.aux_chunk(chunk).map_err(NativeRankReaderError::Sink)
                },
            )?;
            if aux_hash != descriptor.aux_sha256 {
                return Err(RankFileError::TensorRegion.into());
            }

            if let (Some(primary), Some(aux)) = (exl3_primary, exl3_aux) {
                let exl3_metadata = Exl3Metadata::decode(metadata).map_err(RankFileError::Exl3)?;
                Exl3Trellis::from_container_planes(exl3_metadata, &primary, &aux)
                    .map_err(RankFileError::Exl3)?;
            }
            if let Some(validator) = nvfp4_validator {
                validator.finish()?;
            }
            sink.finish_tensor().map_err(NativeRankReaderError::Sink)?;
            cursor = descriptor
                .aux_offset
                .checked_add(descriptor.aux_bytes)
                .ok_or(RankFileError::Overflow)?;
        }
        if cursor != self.payload_region.end {
            return Err(RankFileError::NonCanonicalLayout.into());
        }
        let payload_sha256: [u8; 32] = whole.finalize().into();
        if payload_sha256 != self.payload_sha256 {
            return Err(RankFileError::StrongHash.into());
        }
        self.ensure_unchanged()?;
        Ok(RankPayloadProof {
            rank: self.rank,
            tensor_count: self.descriptors.len(),
            payload_bytes: self.payload_region.len(),
            payload_sha256,
            stream_chunks,
            maximum_reader_scratch_bytes,
        })
    }

    pub fn verify(&self) -> Result<RankPayloadProof, NativeRankReaderError> {
        self.verify_and_stream(&mut NullRankTensorSink)
    }

    pub fn validate_rank_set(files: [&Self; 4]) -> Result<(), NativeRankReaderError> {
        let conversion_uuid = files[0].conversion_uuid;
        let model_config = files[0].model_config_sha256;
        let tokenizer = files[0].tokenizer_bundle_sha256;
        let chat_template = files[0].chat_template_sha256;
        let weight_policy = files[0].weight_policy_sha256;
        let kernel_abi = files[0].kernel_abi_sha256;
        let header_flags = files[0].header_flags;
        let tensor_count = files[0].descriptors.len();
        let validated_manifest = files[0].validated_manifest.as_ref();
        let mut hasher = Sha256::new();
        hasher.update(b"g5n-conversion-v0\0");
        for (expected_rank, file) in files.into_iter().enumerate() {
            file.ensure_unchanged()?;
            if file.rank != u32::try_from(expected_rank).map_err(|_| RankFileError::Overflow)?
                || file.conversion_uuid != conversion_uuid
                || file.model_config_sha256 != model_config
                || file.tokenizer_bundle_sha256 != tokenizer
                || file.chat_template_sha256 != chat_template
                || file.weight_policy_sha256 != weight_policy
                || file.kernel_abi_sha256 != kernel_abi
                || file.header_flags != header_flags
                || file.descriptors.len() != tensor_count
                || !manifest_consensus(validated_manifest, file.validated_manifest.as_ref())
            {
                return Err(RankFileError::RankSet.into());
            }
            for index in 0..tensor_count {
                if !descriptor_semantics_match(
                    &file.descriptors[index],
                    &files[0].descriptors[index],
                ) || file.names[index] != files[0].names[index]
                    || !codec_semantics_match(file, files[0], index)?
                {
                    return Err(RankFileError::RankSet.into());
                }
            }
            hasher.update(file.manifest_sha256);
            hasher.update(file.descriptor_sha256);
            hasher.update(file.payload_sha256);
        }
        if first_16(hasher.finalize().into()) != conversion_uuid {
            return Err(RankFileError::RankSet.into());
        }
        Ok(())
    }

    fn ensure_unchanged(&self) -> Result<(), NativeRankReaderError> {
        let open = FileFingerprint::from_metadata(&self.file.metadata()?);
        let path = self.path.symlink_metadata()?;
        if path.file_type().is_symlink()
            || !path.file_type().is_file()
            || path.nlink() != 1
            || open != self.fingerprint
            || FileFingerprint::from_metadata(&path) != self.fingerprint
        {
            return Err(NativeRankReaderError::Changed(self.path.clone()));
        }
        Ok(())
    }
}

/// A direct-upload sink. Implementations must keep received bytes tentative
/// and unreachable by execution until `verify_and_stream` succeeds.
pub trait RankTensorSink {
    fn begin_tensor(
        &mut self,
        rank: u32,
        index: usize,
        name: &str,
        descriptor: &TensorDescriptor,
        codec_metadata: &[u8],
    ) -> Result<(), String>;

    fn primary_chunk(&mut self, bytes: &[u8]) -> Result<(), String>;

    fn aux_chunk(&mut self, bytes: &[u8]) -> Result<(), String>;

    fn finish_tensor(&mut self) -> Result<(), String>;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct NullRankTensorSink;

impl RankTensorSink for NullRankTensorSink {
    fn begin_tensor(
        &mut self,
        _rank: u32,
        _index: usize,
        _name: &str,
        _descriptor: &TensorDescriptor,
        _codec_metadata: &[u8],
    ) -> Result<(), String> {
        Ok(())
    }

    fn primary_chunk(&mut self, _bytes: &[u8]) -> Result<(), String> {
        Ok(())
    }

    fn aux_chunk(&mut self, _bytes: &[u8]) -> Result<(), String> {
        Ok(())
    }

    fn finish_tensor(&mut self) -> Result<(), String> {
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct RankPayloadProof {
    pub rank: u32,
    pub tensor_count: usize,
    pub payload_bytes: u64,
    pub payload_sha256: [u8; 32],
    pub stream_chunks: u64,
    pub maximum_reader_scratch_bytes: usize,
}

#[derive(Debug)]
pub enum NativeRankReaderError {
    Io(io::Error),
    Format(RankFileError),
    Manifest(RankManifestError),
    UnsafeFile(PathBuf),
    Changed(PathBuf),
    Sink(String),
}

impl fmt::Display for NativeRankReaderError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "{error}"),
            Self::Format(error) => write!(formatter, "native rank format error: {error}"),
            Self::Manifest(error) => write!(formatter, "native rank manifest error: {error}"),
            Self::UnsafeFile(path) => {
                write!(
                    formatter,
                    "native rank path is not an exclusive regular file: {}",
                    path.display()
                )
            }
            Self::Changed(path) => {
                write!(
                    formatter,
                    "native rank file changed while open: {}",
                    path.display()
                )
            }
            Self::Sink(error) => write!(formatter, "native rank sink failed: {error}"),
        }
    }
}

impl std::error::Error for NativeRankReaderError {}

impl From<io::Error> for NativeRankReaderError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<RankFileError> for NativeRankReaderError {
    fn from(value: RankFileError) -> Self {
        Self::Format(value)
    }
}

impl From<crate::Nvfp4Error> for NativeRankReaderError {
    fn from(value: crate::Nvfp4Error) -> Self {
        Self::Format(RankFileError::Nvfp4(value))
    }
}

impl From<RankManifestError> for NativeRankReaderError {
    fn from(value: RankManifestError) -> Self {
        Self::Manifest(value)
    }
}

fn validate_descriptors(
    descriptors: &[TensorDescriptor],
    metadata: &[u8],
    metadata_region: FileRegion,
    payload_region: FileRegion,
    file_bytes: u64,
    rank: u32,
) -> Result<(), NativeRankReaderError> {
    let mut metadata_cursor = metadata_region.start;
    let mut payload_cursor = payload_region.start;
    for descriptor in descriptors {
        let metadata_range = checked_subregion(
            descriptor.codec_metadata_offset,
            descriptor.codec_metadata_bytes,
            metadata_region,
            file_bytes,
        )?;
        if metadata_range.start != metadata_cursor {
            return Err(RankFileError::NonCanonicalLayout.into());
        }
        metadata_cursor = metadata_range.end;
        let local_start = usize::try_from(metadata_range.start - metadata_region.start)
            .map_err(|_| RankFileError::Overflow)?;
        let local_end = usize::try_from(metadata_range.end - metadata_region.start)
            .map_err(|_| RankFileError::Overflow)?;
        let codec_metadata = &metadata[local_start..local_end];
        if hash(codec_metadata) != descriptor.codec_metadata_sha256 {
            return Err(RankFileError::TensorRegion.into());
        }

        let primary = checked_subregion(
            descriptor.payload_offset,
            descriptor.payload_bytes,
            payload_region,
            file_bytes,
        )?;
        let expected_primary = aligned_end(payload_cursor, PAYLOAD_ALIGNMENT)?;
        let aux = checked_subregion(
            descriptor.aux_offset,
            descriptor.aux_bytes,
            payload_region,
            file_bytes,
        )?;
        let expected_aux = aligned_end(primary.end, PAYLOAD_ALIGNMENT)?;
        if primary.start != expected_primary || aux.start != expected_aux {
            return Err(RankFileError::NonCanonicalLayout.into());
        }
        payload_cursor = aux.end;

        match descriptor.codec_id {
            CODEC_BF16_ROW_MAJOR | CODEC_FP16_ROW_MAJOR | CODEC_FP32_ROW_MAJOR => {
                let (dtype, dtype_id) = match descriptor.codec_id {
                    CODEC_BF16_ROW_MAJOR => (PlainDtype::Bf16, DTYPE_BF16),
                    CODEC_FP16_ROW_MAJOR => (PlainDtype::Fp16, DTYPE_FP16),
                    CODEC_FP32_ROW_MAJOR => (PlainDtype::Fp32, 3),
                    _ => unreachable!(),
                };
                if descriptor.logical_dtype != dtype_id
                    || descriptor.stored_dtype != dtype_id
                    || descriptor.flags & DESCRIPTOR_FLAG_AUX_REQUIRED != 0
                    || descriptor.quant_group_elements != 0
                    || descriptor.aux_bytes != 0
                    || descriptor.codec_metadata_bytes != 0
                {
                    return Err(RankFileError::Descriptor.into());
                }
                validate_plain_geometry(
                    dtype,
                    descriptor.ndim,
                    descriptor.logical_shape,
                    descriptor.padded_shape,
                    descriptor.payload_bytes,
                )?;
            }
            CODEC_NVFP4_1D | CODEC_NVFP4_2D => {
                let decoded =
                    Nvfp4Metadata::decode(codec_metadata).map_err(RankFileError::Nvfp4)?;
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
                    return Err(RankFileError::Descriptor.into());
                }
            }
            CODEC_EXL3_SOURCE => {
                let decoded = Exl3Metadata::decode(codec_metadata).map_err(RankFileError::Exl3)?;
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
                    return Err(RankFileError::Descriptor.into());
                }
            }
            codec => return Err(RankFileError::UnsupportedCodec(codec).into()),
        }
    }
    if metadata_cursor != metadata_region.end || payload_cursor != payload_region.end {
        return Err(RankFileError::NonCanonicalLayout.into());
    }
    Ok(())
}

fn descriptor_semantics_match(left: &TensorDescriptor, right: &TensorDescriptor) -> bool {
    left.tensor_id == right.tensor_id
        && left.role_id == right.role_id
        && left.layer_id == right.layer_id
        && left.expert_id == right.expert_id
        && left.codec_id == right.codec_id
        && left.logical_dtype == right.logical_dtype
        && left.stored_dtype == right.stored_dtype
        && left.tp_shard_axis == right.tp_shard_axis
        && left.ndim == right.ndim
        && left.flags == right.flags
        && left.logical_shape == right.logical_shape
        && left.padded_shape == right.padded_shape
        && left.payload_bytes == right.payload_bytes
        && left.aux_bytes == right.aux_bytes
        && left.codec_metadata_bytes == right.codec_metadata_bytes
        && left.payload_alignment == right.payload_alignment
        && left.quant_group_elements == right.quant_group_elements
}

fn manifest_consensus(
    left: Option<&ValidatedRankManifest>,
    right: Option<&ValidatedRankManifest>,
) -> bool {
    match (left, right) {
        (None, None) => true,
        (Some(left), Some(right)) => {
            left.profile == right.profile
                && left.conversion_commit == right.conversion_commit
                && left.operation_manifest_sha256 == right.operation_manifest_sha256
                && left.profile_budget_sha256 == right.profile_budget_sha256
                && left.review_artifact_sha256 == right.review_artifact_sha256
                && left.format_spec_sha256 == right.format_spec_sha256
                && left.engine_spec_sha256 == right.engine_spec_sha256
                && left.tensor_source_payload_bytes == right.tensor_source_payload_bytes
                && left.source_verified_file_bytes == right.source_verified_file_bytes
        }
        _ => false,
    }
}

fn codec_semantics_match(
    left_file: &NativeRankReader,
    right_file: &NativeRankReader,
    index: usize,
) -> Result<bool, NativeRankReaderError> {
    let left_descriptor = &left_file.descriptors[index];
    let left = left_file.tensor_codec_metadata(index)?;
    let right = right_file.tensor_codec_metadata(index)?;
    match left_descriptor.codec_id {
        CODEC_BF16_ROW_MAJOR | CODEC_FP16_ROW_MAJOR | CODEC_FP32_ROW_MAJOR => Ok(true),
        CODEC_NVFP4_1D | CODEC_NVFP4_2D => {
            let left = Nvfp4Metadata::decode(left).map_err(RankFileError::Nvfp4)?;
            let right = Nvfp4Metadata::decode(right).map_err(RankFileError::Nvfp4)?;
            Ok(left.codec == right.codec
                && left.logical_n == right.logical_n
                && left.logical_k == right.logical_k
                && left.padded_n == right.padded_n
                && left.padded_k == right.padded_k
                && left.value_plane_bytes == right.value_plane_bytes
                && left.scale_plane_bytes == right.scale_plane_bytes)
        }
        CODEC_EXL3_SOURCE => {
            let left = Exl3Metadata::decode(left).map_err(RankFileError::Exl3)?;
            let right = Exl3Metadata::decode(right).map_err(RankFileError::Exl3)?;
            Ok(left.projection == right.projection
                && left.bits == right.bits
                && left.layer == right.layer
                && left.expert == right.expert
                && left.logical_k == right.logical_k
                && left.logical_n == right.logical_n
                && left.trellis_words == right.trellis_words
                && left.rotation_words == right.rotation_words)
        }
        codec => Err(RankFileError::UnsupportedCodec(codec).into()),
    }
}

fn validate_plain_padding_stream_chunk(
    descriptor: &TensorDescriptor,
    bytes: &[u8],
    plane_offset: u64,
) -> Result<(), NativeRankReaderError> {
    if !matches!(
        descriptor.codec_id,
        CODEC_BF16_ROW_MAJOR | CODEC_FP16_ROW_MAJOR | CODEC_FP32_ROW_MAJOR
    ) || descriptor.logical_shape == descriptor.padded_shape
    {
        return Ok(());
    }
    let dtype = if descriptor.codec_id == CODEC_FP32_ROW_MAJOR {
        PlainDtype::Fp32
    } else if descriptor.codec_id == CODEC_FP16_ROW_MAJOR {
        PlainDtype::Fp16
    } else {
        PlainDtype::Bf16
    };
    validate_plain_padding_chunk(
        bytes,
        plane_offset,
        dtype,
        descriptor.ndim,
        descriptor.logical_shape,
        descriptor.padded_shape,
    )
    .map_err(Into::into)
}

fn stream_plane(
    file: &File,
    buffer: &mut [u8],
    start: u64,
    bytes: u64,
    whole: &mut Sha256,
    stream_chunks: &mut u64,
    mut visitor: impl FnMut(&[u8], u64) -> Result<(), NativeRankReaderError>,
) -> Result<[u8; 32], NativeRankReaderError> {
    let mut plane = Sha256::new();
    let mut consumed = 0_u64;
    while consumed < bytes {
        let chunk = usize::try_from((bytes - consumed).min(buffer.len() as u64))
            .map_err(|_| RankFileError::Overflow)?;
        read_exact_at(file, &mut buffer[..chunk], start + consumed)?;
        plane.update(&buffer[..chunk]);
        whole.update(&buffer[..chunk]);
        visitor(&buffer[..chunk], consumed)?;
        consumed = consumed
            .checked_add(u64::try_from(chunk).map_err(|_| RankFileError::Overflow)?)
            .ok_or(RankFileError::Overflow)?;
        *stream_chunks = stream_chunks
            .checked_add(1)
            .ok_or(RankFileError::Overflow)?;
    }
    Ok(plane.finalize().into())
}

fn stream_padding(
    file: &File,
    buffer: &mut [u8],
    start: u64,
    end: u64,
    whole: &mut Sha256,
    stream_chunks: &mut u64,
) -> Result<(), NativeRankReaderError> {
    if start > end {
        return Err(RankFileError::NonCanonicalLayout.into());
    }
    let hash = stream_plane(
        file,
        buffer,
        start,
        end - start,
        whole,
        stream_chunks,
        |chunk, _| {
            if chunk.iter().any(|&byte| byte != 0) {
                return Err(RankFileError::NonCanonicalLayout.into());
            }
            Ok(())
        },
    )?;
    let _ = hash;
    Ok(())
}

fn read_control_region(file: &File, region: FileRegion) -> Result<Vec<u8>, NativeRankReaderError> {
    if region.len() > CONTROL_REGION_MAX_BYTES {
        return Err(RankFileError::Region.into());
    }
    let mut bytes = vec![0_u8; usize::try_from(region.len()).map_err(|_| RankFileError::Overflow)?];
    read_exact_at(file, &mut bytes, region.start)?;
    Ok(bytes)
}

fn verify_zero_gap(file: &File, start: u64, end: u64) -> Result<(), NativeRankReaderError> {
    if start > end || end - start >= ALIGNMENT as u64 {
        return Err(RankFileError::NonCanonicalLayout.into());
    }
    let mut bytes = vec![0_u8; usize::try_from(end - start).map_err(|_| RankFileError::Overflow)?];
    read_exact_at(file, &mut bytes, start)?;
    if bytes.iter().any(|&byte| byte != 0) {
        return Err(RankFileError::NonCanonicalLayout.into());
    }
    Ok(())
}

fn checked_subregion(
    start: u64,
    bytes: u64,
    parent: FileRegion,
    file_bytes: u64,
) -> Result<FileRegion, NativeRankReaderError> {
    let end = start.checked_add(bytes).ok_or(RankFileError::Overflow)?;
    if start > end || end > file_bytes || start < parent.start || end > parent.end {
        return Err(RankFileError::Region.into());
    }
    Ok(FileRegion { start, end })
}

fn aligned_end(value: u64, alignment: usize) -> Result<u64, NativeRankReaderError> {
    let value = usize::try_from(value).map_err(|_| RankFileError::Overflow)?;
    u64::try_from(align_up(value, alignment)?).map_err(|_| RankFileError::Overflow.into())
}

fn read_exact_at(file: &File, mut output: &mut [u8], mut offset: u64) -> io::Result<()> {
    while !output.is_empty() {
        match file.read_at(output, offset) {
            Ok(0) => return Err(io::Error::from(io::ErrorKind::UnexpectedEof)),
            Ok(bytes) => {
                offset = offset
                    .checked_add(bytes as u64)
                    .ok_or_else(|| io::Error::from(io::ErrorKind::InvalidData))?;
                output = &mut output[bytes..];
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

fn hash(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn first_16(hash: [u8; 32]) -> [u8; 16] {
    hash[..16].try_into().unwrap()
}

fn array16(bytes: &[u8]) -> [u8; 16] {
    bytes.try_into().unwrap()
}

fn array32(bytes: &[u8]) -> [u8; 32] {
    bytes.try_into().unwrap()
}

fn get_u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(bytes[offset..offset + 2].try_into().unwrap())
}

fn get_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}

fn get_u64(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap())
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        sync::atomic::{AtomicU64, Ordering},
    };

    use super::*;
    use crate::{
        Codec, EXL3_MCG_MULTIPLIER, Exl3Projection, PackedNvfp4, PlainTensor, RankFileBuilder,
        TensorPayload, TensorRecord,
    };

    static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

    #[derive(Default)]
    struct CollectingSink {
        names: Vec<String>,
        primary: Vec<Vec<u8>>,
        aux: Vec<Vec<u8>>,
        maximum_chunk: usize,
    }

    impl RankTensorSink for CollectingSink {
        fn begin_tensor(
            &mut self,
            _rank: u32,
            _index: usize,
            name: &str,
            _descriptor: &TensorDescriptor,
            _codec_metadata: &[u8],
        ) -> Result<(), String> {
            self.names.push(name.to_owned());
            self.primary.push(Vec::new());
            self.aux.push(Vec::new());
            Ok(())
        }

        fn primary_chunk(&mut self, bytes: &[u8]) -> Result<(), String> {
            self.maximum_chunk = self.maximum_chunk.max(bytes.len());
            self.primary.last_mut().unwrap().extend_from_slice(bytes);
            Ok(())
        }

        fn aux_chunk(&mut self, bytes: &[u8]) -> Result<(), String> {
            self.maximum_chunk = self.maximum_chunk.max(bytes.len());
            self.aux.last_mut().unwrap().extend_from_slice(bytes);
            Ok(())
        }

        fn finish_tensor(&mut self) -> Result<(), String> {
            Ok(())
        }
    }

    fn temp_directory(test: &str) -> PathBuf {
        let nonce = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "glmaxx-native-reader-{test}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir(&path).unwrap();
        path
    }

    fn rank_builders() -> [RankFileBuilder; 4] {
        std::array::from_fn(|rank| {
            let plain_values = [
                u16::try_from(rank * 100 + 1).unwrap(),
                u16::try_from(rank * 100 + 2).unwrap(),
                u16::try_from(rank * 100 + 3).unwrap(),
                0,
                u16::try_from(rank * 100 + 4).unwrap(),
                u16::try_from(rank * 100 + 5).unwrap(),
                u16::try_from(rank * 100 + 6).unwrap(),
                0,
            ];
            let payload: Vec<u8> = plain_values
                .into_iter()
                .flat_map(u16::to_le_bytes)
                .collect();
            let nvfp4_values: Vec<f32> = (0..129 * 65)
                .map(|index| ((index * 17 + rank * 3) % 127) as f32 / 31.0 - 2.0)
                .collect();
            let nvfp4 = PackedNvfp4::pack(&nvfp4_values, 129, 65, Codec::OneDimensional).unwrap();
            let exl3_metadata =
                Exl3Metadata::new(Exl3Projection::Gate, 78, 0, rank as u8, 3, 128, 128).unwrap();
            let exl3 = Exl3Trellis {
                trellis: (0..exl3_metadata.trellis_words)
                    .map(|index| (index as u16).wrapping_mul(40_503))
                    .collect(),
                suh: vec![0x3c00; 128],
                svh: vec![0x3c00; 128],
                mcg_marker: EXL3_MCG_MULTIPLIER,
                metadata: exl3_metadata,
            };
            RankFileBuilder {
                rank: rank as u32,
                manifest: format!(r#"{{"rank":{rank},"schema":"reader-test"}}"#).into_bytes(),
                model_config_sha256: [1; 32],
                tokenizer_bundle_sha256: [2; 32],
                chat_template_sha256: [3; 32],
                weight_policy_sha256: [4; 32],
                kernel_abi_sha256: [5; 32],
                tensors: vec![
                    TensorRecord {
                        tensor_id: 0,
                        name: "model.test.plain.weight".to_owned(),
                        role_id: 1,
                        layer_id: -1,
                        expert_id: -1,
                        tp_shard_axis: -1,
                        flags: 1,
                        payload: TensorPayload::Plain(PlainTensor {
                            dtype: PlainDtype::Bf16,
                            ndim: 2,
                            logical_shape: [2, 3, 1, 1],
                            padded_shape: [2, 4, 1, 1],
                            bytes: payload,
                        }),
                    },
                    TensorRecord {
                        tensor_id: 1,
                        name: "model.test.nvfp4.weight".to_owned(),
                        role_id: 0x0501,
                        layer_id: 3,
                        expert_id: 0,
                        tp_shard_axis: 0,
                        flags: 0b0000_1010,
                        payload: TensorPayload::Nvfp4(nvfp4),
                    },
                    TensorRecord {
                        tensor_id: 2,
                        name: "model.test.exl3.weight".to_owned(),
                        role_id: 0x0501,
                        layer_id: 78,
                        expert_id: 0,
                        tp_shard_axis: 0,
                        flags: 0b0000_1010,
                        payload: TensorPayload::Exl3Source(exl3),
                    },
                ],
            }
        })
    }

    fn resign_header(bytes: &mut [u8]) {
        let manifest_start = usize::try_from(get_u64(bytes, 40)).unwrap();
        let manifest_bytes = usize::try_from(get_u64(bytes, 48)).unwrap();
        let descriptor_start = usize::try_from(get_u64(bytes, 56)).unwrap();
        let descriptor_bytes = usize::try_from(get_u64(bytes, 64)).unwrap();
        let payload_start = usize::try_from(get_u64(bytes, 104)).unwrap();
        let payload_bytes = usize::try_from(get_u64(bytes, 112)).unwrap();
        let manifest_sha256 = hash(&bytes[manifest_start..manifest_start + manifest_bytes]);
        let descriptor_sha256 = hash(&bytes[descriptor_start..descriptor_start + descriptor_bytes]);
        let payload_sha256 = hash(&bytes[payload_start..payload_start + payload_bytes]);
        bytes[280..312].copy_from_slice(&manifest_sha256);
        bytes[312..344].copy_from_slice(&descriptor_sha256);
        bytes[344..376].copy_from_slice(&payload_sha256);
        let mut uuid = Sha256::new();
        uuid.update(b"g5n-file-v0\0");
        uuid.update(&bytes[392..408]);
        uuid.update(get_u32(bytes, 24).to_le_bytes());
        uuid.update(&bytes[280..312]);
        uuid.update(descriptor_sha256);
        uuid.update(payload_sha256);
        bytes[376..392].copy_from_slice(&first_16(uuid.finalize().into()));
        bytes[416..420].fill(0);
        let header_crc = crc32c(&bytes[..crate::HEADER_BYTES]);
        bytes[416..420].copy_from_slice(&header_crc.to_le_bytes());
    }

    #[test]
    fn four_rank_images_stream_once_with_bounded_scratch() {
        let directory = temp_directory("rank-set");
        let builders = rank_builders();
        let conversion = RankFileBuilder::derive_conversion_uuid(&builders).unwrap();
        let mut paths = Vec::new();
        for builder in &builders {
            let path = directory.join(format!("rank-{}.g5n", builder.rank));
            fs::write(&path, builder.build(conversion).unwrap()).unwrap();
            paths.push(path);
        }
        let readers: Vec<_> = paths
            .iter()
            .map(|path| NativeRankReader::open(path).unwrap())
            .collect();
        NativeRankReader::validate_rank_set([&readers[0], &readers[1], &readers[2], &readers[3]])
            .unwrap();
        for reader in &readers {
            let mut sink = CollectingSink::default();
            let proof = reader.verify_and_stream(&mut sink).unwrap();
            assert_eq!(proof.tensor_count, 3);
            assert_eq!(proof.payload_sha256, reader.payload_sha256);
            assert!(proof.maximum_reader_scratch_bytes >= STREAM_BUFFER_BYTES);
            assert!(sink.maximum_chunk <= STREAM_BUFFER_BYTES);
            assert_eq!(
                sink.names,
                [
                    "model.test.plain.weight",
                    "model.test.nvfp4.weight",
                    "model.test.exl3.weight",
                ]
            );
            assert_eq!(sink.primary[0].len(), 16);
            assert!(sink.aux[0].is_empty());
            assert!(!sink.primary[1].is_empty());
            assert!(!sink.aux[1].is_empty());
            assert!(!sink.primary[2].is_empty());
            assert!(!sink.aux[2].is_empty());
        }
        drop(readers);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn payload_corruption_and_trailing_bytes_fail_closed() {
        let directory = temp_directory("corruption");
        let builders = rank_builders();
        let conversion = RankFileBuilder::derive_conversion_uuid(&builders).unwrap();
        let mut bytes = builders[0].build(conversion).unwrap();
        let clean_path = directory.join("clean.g5n");
        fs::write(&clean_path, &bytes).unwrap();
        let reader = NativeRankReader::open(&clean_path).unwrap();
        let offset = usize::try_from(reader.descriptors[0].payload_offset).unwrap();
        drop(reader);
        bytes[offset] ^= 1;
        let corrupt_path = directory.join("corrupt.g5n");
        fs::write(&corrupt_path, &bytes).unwrap();
        let corrupt = NativeRankReader::open(&corrupt_path).unwrap();
        assert!(matches!(
            corrupt.verify(),
            Err(NativeRankReaderError::Format(
                RankFileError::TensorRegion | RankFileError::StrongHash
            ))
        ));
        bytes.push(0);
        let trailing_path = directory.join("trailing.g5n");
        fs::write(&trailing_path, &bytes).unwrap();
        assert!(matches!(
            NativeRankReader::open(&trailing_path),
            Err(NativeRankReaderError::Format(RankFileError::Region))
        ));
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn resigned_noncanonical_padding_and_rank_divergence_fail_closed() {
        let directory = temp_directory("canonical");
        let mut builders = rank_builders();
        let conversion = RankFileBuilder::derive_conversion_uuid(&builders).unwrap();
        let mut noncanonical_manifest = builders[0].build(conversion).unwrap();
        let manifest_start = usize::try_from(get_u64(&noncanonical_manifest, 40)).unwrap();
        let manifest_bytes = usize::try_from(get_u64(&noncanonical_manifest, 48)).unwrap();
        let replacement = br#"{"schema":"reader-test","rank":0}"#;
        assert_eq!(replacement.len(), manifest_bytes);
        noncanonical_manifest[manifest_start..manifest_start + manifest_bytes]
            .copy_from_slice(replacement);
        resign_header(&mut noncanonical_manifest);
        let manifest_path = directory.join("manifest.g5n");
        fs::write(&manifest_path, noncanonical_manifest).unwrap();
        assert!(matches!(
            NativeRankReader::open(&manifest_path),
            Err(NativeRankReaderError::Manifest(
                RankManifestError::NonCanonical
            ))
        ));

        let mut bytes = builders[0].build(conversion).unwrap();
        let descriptor_start = usize::try_from(get_u64(&bytes, 56)).unwrap();
        let payload_offset = usize::try_from(get_u64(&bytes, descriptor_start + 72)).unwrap();
        bytes[payload_offset + 6] = 1;
        let payload_bytes = usize::try_from(get_u64(&bytes, descriptor_start + 80)).unwrap();
        let payload_sha256 = hash(&bytes[payload_offset..payload_offset + payload_bytes]);
        bytes[descriptor_start + 128..descriptor_start + 160].copy_from_slice(&payload_sha256);
        resign_header(&mut bytes);
        let padding_path = directory.join("padding.g5n");
        fs::write(&padding_path, bytes).unwrap();
        let reader = NativeRankReader::open(&padding_path).unwrap();
        assert!(matches!(
            reader.verify(),
            Err(NativeRankReaderError::Format(
                RankFileError::NonCanonicalLayout
            ))
        ));
        drop(reader);

        let mut bytes = builders[0].build(conversion).unwrap();
        let descriptor_start = usize::try_from(get_u64(&bytes, 56)).unwrap() + DESCRIPTOR_BYTES;
        let payload_offset = usize::try_from(get_u64(&bytes, descriptor_start + 72)).unwrap();
        let payload_bytes = usize::try_from(get_u64(&bytes, descriptor_start + 80)).unwrap();
        let padded_value = 65_usize;
        bytes[payload_offset + padded_value / 2] |= 0x10;
        let payload_sha256 = hash(&bytes[payload_offset..payload_offset + payload_bytes]);
        bytes[descriptor_start + 128..descriptor_start + 160].copy_from_slice(&payload_sha256);
        resign_header(&mut bytes);
        let padding_path = directory.join("nvfp4-value-padding.g5n");
        fs::write(&padding_path, bytes).unwrap();
        let reader = NativeRankReader::open(&padding_path).unwrap();
        assert!(matches!(
            reader.verify(),
            Err(NativeRankReaderError::Format(RankFileError::Nvfp4(
                crate::Nvfp4Error::NonCanonicalPadding
            )))
        ));
        drop(reader);

        let mut bytes = builders[0].build(conversion).unwrap();
        let descriptor_start = usize::try_from(get_u64(&bytes, 56)).unwrap() + DESCRIPTOR_BYTES;
        let aux_offset = usize::try_from(get_u64(&bytes, descriptor_start + 88)).unwrap();
        let aux_bytes = usize::try_from(get_u64(&bytes, descriptor_start + 96)).unwrap();
        assert_ne!(bytes[aux_offset], 0);
        bytes[aux_offset] = 0;
        let aux_sha256 = hash(&bytes[aux_offset..aux_offset + aux_bytes]);
        bytes[descriptor_start + 160..descriptor_start + 192].copy_from_slice(&aux_sha256);
        resign_header(&mut bytes);
        let zero_scale_path = directory.join("nvfp4-zero-scale.g5n");
        fs::write(&zero_scale_path, bytes).unwrap();
        let reader = NativeRankReader::open(&zero_scale_path).unwrap();
        assert!(matches!(
            reader.verify(),
            Err(NativeRankReaderError::Format(RankFileError::Nvfp4(
                crate::Nvfp4Error::ZeroScaleValue
            )))
        ));
        drop(reader);

        builders[3].tensors[0].name = "model.test.divergent.weight".to_owned();
        let divergent_conversion = RankFileBuilder::derive_conversion_uuid(&builders).unwrap();
        let mut paths = Vec::new();
        for builder in &builders {
            let path = directory.join(format!("divergent-rank-{}.g5n", builder.rank));
            fs::write(&path, builder.build(divergent_conversion).unwrap()).unwrap();
            paths.push(path);
        }
        let readers: Vec<_> = paths
            .iter()
            .map(|path| NativeRankReader::open(path).unwrap())
            .collect();
        assert!(matches!(
            NativeRankReader::validate_rank_set([
                &readers[0],
                &readers[1],
                &readers[2],
                &readers[3],
            ]),
            Err(NativeRankReaderError::Format(RankFileError::RankSet))
        ));
        drop(readers);

        let mut builders = rank_builders();
        let TensorPayload::Exl3Source(exl3) = &mut builders[3].tensors[2].payload else {
            panic!("fixture tensor is not EXL3");
        };
        exl3.metadata = Exl3Metadata::new(Exl3Projection::Up, 78, 0, 3, 3, 128, 128).unwrap();
        let divergent_conversion = RankFileBuilder::derive_conversion_uuid(&builders).unwrap();
        let mut paths = Vec::new();
        for builder in &builders {
            let path = directory.join(format!("projection-rank-{}.g5n", builder.rank));
            fs::write(&path, builder.build(divergent_conversion).unwrap()).unwrap();
            paths.push(path);
        }
        let readers: Vec<_> = paths
            .iter()
            .map(|path| NativeRankReader::open(path).unwrap())
            .collect();
        assert!(matches!(
            NativeRankReader::validate_rank_set([
                &readers[0],
                &readers[1],
                &readers[2],
                &readers[3],
            ]),
            Err(NativeRankReaderError::Format(RankFileError::RankSet))
        ));
        drop(readers);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn hard_links_and_sink_failures_are_rejected() {
        let directory = temp_directory("unsafe");
        let builders = rank_builders();
        let conversion = RankFileBuilder::derive_conversion_uuid(&builders).unwrap();
        let path = directory.join("rank.g5n");
        fs::write(&path, builders[0].build(conversion).unwrap()).unwrap();
        let link = directory.join("rank-link.g5n");
        fs::hard_link(&path, &link).unwrap();
        assert!(matches!(
            NativeRankReader::open(&path),
            Err(NativeRankReaderError::UnsafeFile(_))
        ));
        fs::remove_file(link).unwrap();
        let reader = NativeRankReader::open(&path).unwrap();
        struct FailingSink;
        impl RankTensorSink for FailingSink {
            fn begin_tensor(
                &mut self,
                _rank: u32,
                _index: usize,
                _name: &str,
                _descriptor: &TensorDescriptor,
                _codec_metadata: &[u8],
            ) -> Result<(), String> {
                Ok(())
            }
            fn primary_chunk(&mut self, _bytes: &[u8]) -> Result<(), String> {
                Err("injected upload failure".to_owned())
            }
            fn aux_chunk(&mut self, _bytes: &[u8]) -> Result<(), String> {
                Ok(())
            }
            fn finish_tensor(&mut self) -> Result<(), String> {
                Ok(())
            }
        }
        assert!(matches!(
            reader.verify_and_stream(&mut FailingSink),
            Err(NativeRankReaderError::Sink(message))
                if message == "injected upload failure"
        ));
        drop(reader);
        fs::remove_dir_all(directory).unwrap();
    }
}
