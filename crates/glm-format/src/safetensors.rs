use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    fs::File,
    io::{self, Read, Write},
    os::unix::fs::{FileExt, MetadataExt},
    path::{Component, Path, PathBuf},
    thread,
};

use serde::{
    Deserialize, Deserializer,
    de::{MapAccess, Visitor},
};
use sha2::{Digest, Sha256};

use crate::{Exl3Error, Exl3Metadata, Exl3Trellis};

const MAX_HEADER_BYTES: u64 = 100_000_000;
const MAX_INDEX_BYTES: u64 = 100_000_000;
const HASH_CHUNK_BYTES: usize = 8 * 1024 * 1024;
const MAXIMUM_SHARD_OPEN_WORKERS: usize = 16;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SafeDtype {
    Bool,
    F4,
    F6E2m3,
    F6E3m2,
    U8,
    I8,
    U16,
    I16,
    U32,
    I32,
    U64,
    I64,
    F16,
    Bf16,
    F32,
    F64,
    F8E4m3,
    F8E5m2,
    F8E8m0,
    F8E4m3Fnuz,
    F8E5m2Fnuz,
    C64,
}

impl SafeDtype {
    fn parse(value: &str) -> Result<Self, SafeTensorError> {
        match value {
            "BOOL" => Ok(Self::Bool),
            "F4" => Ok(Self::F4),
            "F6_E2M3" => Ok(Self::F6E2m3),
            "F6_E3M2" => Ok(Self::F6E3m2),
            "U8" => Ok(Self::U8),
            "I8" => Ok(Self::I8),
            "U16" => Ok(Self::U16),
            "I16" => Ok(Self::I16),
            "U32" => Ok(Self::U32),
            "I32" => Ok(Self::I32),
            "U64" => Ok(Self::U64),
            "I64" => Ok(Self::I64),
            "F16" => Ok(Self::F16),
            "BF16" => Ok(Self::Bf16),
            "F32" => Ok(Self::F32),
            "F64" => Ok(Self::F64),
            "F8_E4M3" => Ok(Self::F8E4m3),
            "F8_E5M2" => Ok(Self::F8E5m2),
            "F8_E8M0" => Ok(Self::F8E8m0),
            "F8_E4M3FNUZ" => Ok(Self::F8E4m3Fnuz),
            "F8_E5M2FNUZ" => Ok(Self::F8E5m2Fnuz),
            "C64" => Ok(Self::C64),
            _ => Err(SafeTensorError::UnsupportedDtype(value.to_owned())),
        }
    }

    #[must_use]
    pub const fn bits(self) -> u64 {
        match self {
            Self::F4 => 4,
            Self::F6E2m3 | Self::F6E3m2 => 6,
            Self::Bool
            | Self::U8
            | Self::I8
            | Self::F8E4m3
            | Self::F8E5m2
            | Self::F8E8m0
            | Self::F8E4m3Fnuz
            | Self::F8E5m2Fnuz => 8,
            Self::U16 | Self::I16 | Self::F16 | Self::Bf16 => 16,
            Self::U32 | Self::I32 | Self::F32 => 32,
            Self::U64 | Self::I64 | Self::F64 | Self::C64 => 64,
        }
    }

    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Bool => "BOOL",
            Self::F4 => "F4",
            Self::F6E2m3 => "F6_E2M3",
            Self::F6E3m2 => "F6_E3M2",
            Self::U8 => "U8",
            Self::I8 => "I8",
            Self::U16 => "U16",
            Self::I16 => "I16",
            Self::U32 => "U32",
            Self::I32 => "I32",
            Self::U64 => "U64",
            Self::I64 => "I64",
            Self::F16 => "F16",
            Self::Bf16 => "BF16",
            Self::F32 => "F32",
            Self::F64 => "F64",
            Self::F8E4m3 => "F8_E4M3",
            Self::F8E5m2 => "F8_E5M2",
            Self::F8E8m0 => "F8_E8M0",
            Self::F8E4m3Fnuz => "F8_E4M3FNUZ",
            Self::F8E5m2Fnuz => "F8_E5M2FNUZ",
            Self::C64 => "C64",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SafeTensorDescriptor {
    pub dtype: SafeDtype,
    pub shape: Vec<u64>,
    /// Half-open offsets relative to the safetensors data region.
    pub data_offsets: [u64; 2],
    pub elements: u64,
    pub bytes: u64,
}

#[derive(Debug)]
pub struct SafeTensorFile {
    path: PathBuf,
    file: File,
    fingerprint: FileFingerprint,
    file_bytes: u64,
    data_offset: u64,
    header_sha256: [u8; 32],
    metadata: BTreeMap<String, String>,
    tensors: BTreeMap<String, SafeTensorDescriptor>,
}

impl SafeTensorFile {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, SafeTensorError> {
        let path = path.as_ref().to_owned();
        let (file, fingerprint) = open_retained_regular_file(&path)?;
        let file_bytes = fingerprint.bytes;
        if file_bytes < 10 {
            return Err(SafeTensorError::Truncated);
        }
        let mut length_bytes = [0_u8; 8];
        read_exact_at(&file, &mut length_bytes, 0)?;
        let header_bytes = u64::from_le_bytes(length_bytes);
        if !(2..=MAX_HEADER_BYTES).contains(&header_bytes) {
            return Err(SafeTensorError::HeaderLength(header_bytes));
        }
        let data_offset = 8_u64
            .checked_add(header_bytes)
            .ok_or(SafeTensorError::Overflow)?;
        if data_offset > file_bytes {
            return Err(SafeTensorError::Truncated);
        }
        if !data_offset.is_multiple_of(8) {
            return Err(SafeTensorError::Header);
        }
        let mut header =
            vec![0_u8; usize::try_from(header_bytes).map_err(|_| SafeTensorError::Overflow)?];
        read_exact_at(&file, &mut header, 8)?;
        if header.first() != Some(&b'{') {
            return Err(SafeTensorError::Header);
        }
        let raw: RawHeader = serde_json::from_slice(&header).map_err(SafeTensorError::Json)?;
        if raw.tensors.is_empty() {
            return Err(SafeTensorError::TensorCount);
        }

        let data_bytes = file_bytes
            .checked_sub(data_offset)
            .ok_or(SafeTensorError::Overflow)?;
        let mut tensors = BTreeMap::new();
        for (name, raw_tensor) in raw.tensors {
            validate_tensor_name(&name)?;
            let dtype = SafeDtype::parse(&raw_tensor.dtype)?;
            let elements = raw_tensor
                .shape
                .iter()
                .try_fold(1_u64, |product, &extent| {
                    product.checked_mul(extent).ok_or(SafeTensorError::Overflow)
                })?;
            let storage_bits = elements
                .checked_mul(dtype.bits())
                .ok_or(SafeTensorError::Overflow)?;
            let bytes = storage_bits.div_ceil(8);
            if raw_tensor.data_offsets[0] > raw_tensor.data_offsets[1]
                || raw_tensor.data_offsets[1] > data_bytes
                || raw_tensor.data_offsets[1] - raw_tensor.data_offsets[0] != bytes
            {
                return Err(SafeTensorError::ByteAccounting(name));
            }
            tensors.insert(
                name,
                SafeTensorDescriptor {
                    dtype,
                    shape: raw_tensor.shape,
                    data_offsets: raw_tensor.data_offsets,
                    elements,
                    bytes,
                },
            );
        }
        validate_contiguous_data(&tensors, data_bytes)?;
        validate_retained_regular_file(&path, &file, fingerprint)?;

        Ok(Self {
            path,
            file,
            fingerprint,
            file_bytes,
            data_offset,
            header_sha256: Sha256::digest(&header).into(),
            metadata: raw.metadata,
            tensors,
        })
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    #[must_use]
    pub const fn file_bytes(&self) -> u64 {
        self.file_bytes
    }

    #[must_use]
    pub const fn data_offset(&self) -> u64 {
        self.data_offset
    }

    #[must_use]
    pub const fn header_sha256(&self) -> [u8; 32] {
        self.header_sha256
    }

    #[must_use]
    pub fn metadata(&self) -> &BTreeMap<String, String> {
        &self.metadata
    }

    #[must_use]
    pub fn tensors(&self) -> &BTreeMap<String, SafeTensorDescriptor> {
        &self.tensors
    }

    #[must_use]
    pub fn tensor(&self, name: &str) -> Option<&SafeTensorDescriptor> {
        self.tensors.get(name)
    }

    pub fn read_tensor(&self, name: &str) -> Result<Vec<u8>, SafeTensorError> {
        self.revalidate()?;
        let descriptor = self
            .tensors
            .get(name)
            .ok_or_else(|| SafeTensorError::MissingTensor(name.to_owned()))?;
        let mut bytes =
            vec![0_u8; usize::try_from(descriptor.bytes).map_err(|_| SafeTensorError::Overflow)?];
        let absolute = self
            .data_offset
            .checked_add(descriptor.data_offsets[0])
            .ok_or(SafeTensorError::Overflow)?;
        read_exact_at(&self.file, &mut bytes, absolute)?;
        self.revalidate()?;
        Ok(bytes)
    }

    pub fn tensor_reader(&self, name: &str) -> Result<SafeTensorReader<'_>, SafeTensorError> {
        self.revalidate()?;
        let descriptor = self
            .tensors
            .get(name)
            .ok_or_else(|| SafeTensorError::MissingTensor(name.to_owned()))?;
        Ok(SafeTensorReader {
            file: &self.file,
            fingerprint: self.fingerprint,
            absolute_offset: self
                .data_offset
                .checked_add(descriptor.data_offsets[0])
                .ok_or(SafeTensorError::Overflow)?,
            bytes: descriptor.bytes,
            position: 0,
        })
    }

    pub fn tensor_shard_reader(
        &self,
        name: &str,
        axis: u8,
        part: u8,
        parts: u8,
    ) -> Result<TensorShardReader, SafeTensorError> {
        self.revalidate()?;
        let descriptor = self
            .tensor(name)
            .ok_or_else(|| SafeTensorError::MissingTensor(name.to_owned()))?;
        TensorShardReader::new(
            self.file.try_clone().map_err(SafeTensorError::Io)?,
            self.fingerprint,
            self.data_offset
                .checked_add(descriptor.data_offsets[0])
                .ok_or(SafeTensorError::Overflow)?,
            descriptor,
            axis,
            part,
            parts,
        )
    }

    pub fn read_tensor_range(
        &self,
        name: &str,
        relative_offset: u64,
        output: &mut [u8],
    ) -> Result<(), SafeTensorError> {
        self.revalidate()?;
        let descriptor = self
            .tensors
            .get(name)
            .ok_or_else(|| SafeTensorError::MissingTensor(name.to_owned()))?;
        let end = relative_offset
            .checked_add(output.len() as u64)
            .ok_or(SafeTensorError::Overflow)?;
        if end > descriptor.bytes {
            return Err(SafeTensorError::TensorRange(name.to_owned()));
        }
        let absolute = self
            .data_offset
            .checked_add(descriptor.data_offsets[0])
            .and_then(|offset| offset.checked_add(relative_offset))
            .ok_or(SafeTensorError::Overflow)?;
        read_exact_at(&self.file, output, absolute)?;
        self.revalidate()
    }

    pub fn copy_tensor(&self, name: &str, output: &mut impl Write) -> Result<(), SafeTensorError> {
        self.revalidate()?;
        let descriptor = self
            .tensors
            .get(name)
            .ok_or_else(|| SafeTensorError::MissingTensor(name.to_owned()))?;
        let absolute = self
            .data_offset
            .checked_add(descriptor.data_offsets[0])
            .ok_or(SafeTensorError::Overflow)?;
        let mut buffer = vec![
            0_u8;
            usize::try_from(descriptor.bytes.min(HASH_CHUNK_BYTES as u64))
                .map_err(|_| SafeTensorError::Overflow)?
        ];
        let mut consumed = 0_u64;
        while consumed < descriptor.bytes {
            let chunk = usize::try_from((descriptor.bytes - consumed).min(buffer.len() as u64))
                .map_err(|_| SafeTensorError::Overflow)?;
            read_exact_at(
                &self.file,
                &mut buffer[..chunk],
                absolute
                    .checked_add(consumed)
                    .ok_or(SafeTensorError::Overflow)?,
            )?;
            output
                .write_all(&buffer[..chunk])
                .map_err(SafeTensorError::Io)?;
            consumed += chunk as u64;
        }
        self.revalidate()
    }

    pub fn hash_tensor(&self, name: &str) -> Result<[u8; 32], SafeTensorError> {
        self.revalidate()?;
        let descriptor = self
            .tensors
            .get(name)
            .ok_or_else(|| SafeTensorError::MissingTensor(name.to_owned()))?;
        let absolute = self
            .data_offset
            .checked_add(descriptor.data_offsets[0])
            .ok_or(SafeTensorError::Overflow)?;
        let mut hasher = Sha256::new();
        let mut buffer = vec![
            0_u8;
            usize::try_from(descriptor.bytes.min(HASH_CHUNK_BYTES as u64))
                .map_err(|_| SafeTensorError::Overflow)?
        ];
        let mut consumed = 0_u64;
        while consumed < descriptor.bytes {
            let remaining = descriptor.bytes - consumed;
            let chunk = usize::try_from(remaining.min(buffer.len() as u64))
                .map_err(|_| SafeTensorError::Overflow)?;
            read_exact_at(
                &self.file,
                &mut buffer[..chunk],
                absolute
                    .checked_add(consumed)
                    .ok_or(SafeTensorError::Overflow)?,
            )?;
            hasher.update(&buffer[..chunk]);
            consumed += chunk as u64;
        }
        self.revalidate()?;
        Ok(hasher.finalize().into())
    }

    /// Hashes the complete source file through the already-open descriptor.
    ///
    /// Header hashes are useful for cheap structural identity, but conversion
    /// provenance must use this full-file digest (or per-tensor digests) so a
    /// same-shape data mutation cannot masquerade as the original checkpoint.
    pub fn hash_file(&self) -> Result<[u8; 32], SafeTensorError> {
        self.revalidate()?;
        let digest = hash_retained_file(&self.file, self.file_bytes)?;
        self.revalidate()?;
        Ok(digest)
    }

    /// Proves that the pathname and retained descriptor still have the exact
    /// identity opened before header parsing.
    pub fn revalidate(&self) -> Result<(), SafeTensorError> {
        validate_retained_regular_file(&self.path, &self.file, self.fingerprint)
    }
}

#[derive(Debug)]
pub struct SafeTensorReader<'a> {
    file: &'a File,
    fingerprint: FileFingerprint,
    absolute_offset: u64,
    bytes: u64,
    position: u64,
}

impl SafeTensorReader<'_> {
    #[must_use]
    pub const fn len(&self) -> u64 {
        self.bytes
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.bytes == 0
    }
}

impl Read for SafeTensorReader<'_> {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        read_retained_cursor(
            self.file,
            self.fingerprint,
            self.absolute_offset,
            self.bytes,
            &mut self.position,
            output,
        )
    }
}

#[derive(Clone, Debug)]
struct ShardedLocation {
    shard: PathBuf,
    descriptor: SafeTensorDescriptor,
}

#[derive(Debug)]
struct OpenShard {
    file: File,
    data_offset: u64,
    fingerprint: FileFingerprint,
}

#[derive(Debug)]
struct OpenIndex {
    file: File,
    fingerprint: FileFingerprint,
}

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

#[derive(Debug)]
pub struct ShardedSafetensors {
    source_path: PathBuf,
    root: PathBuf,
    structure_sha256: [u8; 32],
    declared_payload_bytes: Option<u64>,
    locations: BTreeMap<String, ShardedLocation>,
    shards: BTreeSet<PathBuf>,
    open_index: Option<OpenIndex>,
    open_shards: BTreeMap<PathBuf, OpenShard>,
}

impl ShardedSafetensors {
    pub fn open_auto(path: impl AsRef<Path>) -> Result<Self, SafeTensorError> {
        let path = path.as_ref();
        let metadata = path.symlink_metadata().map_err(SafeTensorError::Io)?;
        if metadata.file_type().is_symlink() {
            return Err(SafeTensorError::ShardPath(
                path.to_string_lossy().into_owned(),
            ));
        }
        if metadata.file_type().is_dir() {
            Self::open_directory(path)
        } else {
            Self::open(path)
        }
    }

    pub fn open(index_path: impl AsRef<Path>) -> Result<Self, SafeTensorError> {
        Self::open_with_workers(index_path, recommended_shard_open_workers())
    }

    fn open_with_workers(
        index_path: impl AsRef<Path>,
        requested_workers: usize,
    ) -> Result<Self, SafeTensorError> {
        let index_path = index_path.as_ref().to_owned();
        let (index_file, index_fingerprint) = open_retained_regular_file(&index_path)?;
        let index_bytes = index_fingerprint.bytes;
        if index_bytes == 0 || index_bytes > MAX_INDEX_BYTES {
            return Err(SafeTensorError::Index);
        }
        let mut bytes =
            vec![0_u8; usize::try_from(index_bytes).map_err(|_| SafeTensorError::Overflow)?];
        read_exact_at(&index_file, &mut bytes, 0)?;
        let raw: RawIndex = serde_json::from_slice(&bytes).map_err(SafeTensorError::Json)?;
        let declared_payload_bytes = match raw.metadata.get("total_size") {
            Some(value) => Some(value.as_u64().ok_or(SafeTensorError::Index)?),
            None => None,
        };
        if raw.weight_map.0.is_empty() {
            return Err(SafeTensorError::TensorCount);
        }
        let root = index_path
            .parent()
            .ok_or(SafeTensorError::Index)?
            .to_owned();
        let mut by_shard: BTreeMap<PathBuf, BTreeSet<String>> = BTreeMap::new();
        for (tensor, shard) in raw.weight_map.0 {
            validate_tensor_name(&tensor)?;
            let shard = validate_shard_path(&shard)?;
            if !by_shard.entry(shard).or_default().insert(tensor) {
                return Err(SafeTensorError::Index);
            }
        }

        let indexed_shards: Vec<_> = by_shard.into_iter().collect();
        let relative_paths: Vec<_> = indexed_shards
            .iter()
            .map(|(relative, _)| relative.clone())
            .collect();
        let opened_shards =
            open_retained_safetensor_files(&root, &relative_paths, requested_workers)?;
        let mut locations = BTreeMap::new();
        let mut shards = BTreeSet::new();
        let mut open_shards = BTreeMap::new();
        for ((relative, expected_tensors), shard) in indexed_shards.into_iter().zip(opened_shards) {
            let actual_tensors: BTreeSet<_> = shard.tensors().keys().cloned().collect();
            if actual_tensors != expected_tensors {
                return Err(SafeTensorError::ShardInventory(relative));
            }
            for name in expected_tensors {
                let descriptor = shard
                    .tensor(&name)
                    .cloned()
                    .ok_or_else(|| SafeTensorError::MissingTensor(name.clone()))?;
                if locations
                    .insert(
                        name,
                        ShardedLocation {
                            shard: relative.clone(),
                            descriptor,
                        },
                    )
                    .is_some()
                {
                    return Err(SafeTensorError::Index);
                }
            }
            let fingerprint = shard.fingerprint;
            open_shards.insert(
                relative.clone(),
                OpenShard {
                    file: shard.file,
                    data_offset: shard.data_offset,
                    fingerprint,
                },
            );
            shards.insert(relative);
        }
        let actual_payload_bytes = locations.values().try_fold(0_u64, |total, location| {
            total
                .checked_add(location.descriptor.bytes)
                .ok_or(SafeTensorError::Overflow)
        })?;
        if declared_payload_bytes.is_some_and(|declared| declared != actual_payload_bytes) {
            return Err(SafeTensorError::Index);
        }
        validate_retained_regular_file(&index_path, &index_file, index_fingerprint)?;
        Ok(Self {
            source_path: index_path,
            root,
            structure_sha256: Sha256::digest(&bytes).into(),
            declared_payload_bytes,
            locations,
            shards,
            open_index: Some(OpenIndex {
                file: index_file,
                fingerprint: index_fingerprint,
            }),
            open_shards,
        })
    }

    pub fn open_directory(path: impl AsRef<Path>) -> Result<Self, SafeTensorError> {
        let source_path = path.as_ref().to_owned();
        let metadata = source_path
            .symlink_metadata()
            .map_err(SafeTensorError::Io)?;
        if !metadata.file_type().is_dir() {
            return Err(SafeTensorError::ShardDirectory(source_path));
        }
        let mut shard_paths = BTreeSet::new();
        for entry in source_path.read_dir().map_err(SafeTensorError::Io)? {
            let entry = entry.map_err(SafeTensorError::Io)?;
            let file_type = entry.file_type().map_err(SafeTensorError::Io)?;
            if file_type.is_symlink() {
                return Err(SafeTensorError::ShardPath(
                    entry.file_name().to_string_lossy().into_owned(),
                ));
            }
            if !file_type.is_file() {
                continue;
            }
            if entry.metadata().map_err(SafeTensorError::Io)?.nlink() != 1 {
                return Err(SafeTensorError::ShardPath(
                    entry.file_name().to_string_lossy().into_owned(),
                ));
            }
            let file_name = entry
                .file_name()
                .into_string()
                .map_err(|name| SafeTensorError::ShardPath(name.to_string_lossy().into_owned()))?;
            if Path::new(&file_name)
                .extension()
                .and_then(|extension| extension.to_str())
                != Some("safetensors")
            {
                continue;
            }
            shard_paths.insert(validate_shard_path(&file_name)?);
        }
        if shard_paths.is_empty() {
            return Err(SafeTensorError::TensorCount);
        }

        let mut locations = BTreeMap::new();
        let mut open_shards = BTreeMap::new();
        let mut structure = Sha256::new();
        structure.update(b"glmaxx.safetensors-directory.v1\0");
        structure.update(
            u64::try_from(shard_paths.len())
                .map_err(|_| SafeTensorError::Overflow)?
                .to_le_bytes(),
        );
        for relative in &shard_paths {
            let shard = SafeTensorFile::open(source_path.join(relative))?;
            let relative_bytes = relative
                .to_str()
                .ok_or_else(|| SafeTensorError::ShardPath(relative.to_string_lossy().into_owned()))?
                .as_bytes();
            structure.update(
                u64::try_from(relative_bytes.len())
                    .map_err(|_| SafeTensorError::Overflow)?
                    .to_le_bytes(),
            );
            structure.update(relative_bytes);
            structure.update(shard.header_sha256());
            structure.update(shard.file_bytes().to_le_bytes());
            for (name, descriptor) in shard.tensors() {
                if locations
                    .insert(
                        name.clone(),
                        ShardedLocation {
                            shard: relative.clone(),
                            descriptor: descriptor.clone(),
                        },
                    )
                    .is_some()
                {
                    return Err(SafeTensorError::DuplicateTensor(name.clone()));
                }
            }
            let fingerprint = shard.fingerprint;
            open_shards.insert(
                relative.clone(),
                OpenShard {
                    file: shard.file,
                    data_offset: shard.data_offset,
                    fingerprint,
                },
            );
        }
        Ok(Self {
            root: source_path.clone(),
            source_path,
            structure_sha256: structure.finalize().into(),
            declared_payload_bytes: Some(locations.values().try_fold(
                0_u64,
                |total, location| {
                    total
                        .checked_add(location.descriptor.bytes)
                        .ok_or(SafeTensorError::Overflow)
                },
            )?),
            locations,
            shards: shard_paths,
            open_index: None,
            open_shards,
        })
    }

    #[must_use]
    pub fn source_path(&self) -> &Path {
        &self.source_path
    }

    #[must_use]
    pub const fn structure_sha256(&self) -> [u8; 32] {
        self.structure_sha256
    }

    #[must_use]
    pub const fn declared_payload_bytes(&self) -> Option<u64> {
        self.declared_payload_bytes
    }

    #[must_use]
    pub const fn index_sha256(&self) -> [u8; 32] {
        self.structure_sha256
    }

    #[must_use]
    pub fn tensor(&self, name: &str) -> Option<&SafeTensorDescriptor> {
        self.locations
            .get(name)
            .map(|location| &location.descriptor)
    }

    #[must_use]
    pub fn tensor_names(&self) -> impl ExactSizeIterator<Item = &str> {
        self.locations.keys().map(String::as_str)
    }

    #[must_use]
    pub fn shards(&self) -> &BTreeSet<PathBuf> {
        &self.shards
    }

    pub fn read_tensor(&self, name: &str) -> Result<Vec<u8>, SafeTensorError> {
        let (shard, location) = self.open_verified_shard(name)?;
        let mut bytes = vec![
            0_u8;
            usize::try_from(location.descriptor.bytes)
                .map_err(|_| SafeTensorError::Overflow)?
        ];
        read_exact_at(
            &shard.file,
            &mut bytes,
            shard
                .data_offset
                .checked_add(location.descriptor.data_offsets[0])
                .ok_or(SafeTensorError::Overflow)?,
        )?;
        self.validate_open_shard(&location.shard, shard)?;
        Ok(bytes)
    }

    pub fn tensor_reader(&self, name: &str) -> Result<ShardedTensorReader, SafeTensorError> {
        let (shard, location) = self.open_verified_shard(name)?;
        let absolute_offset = shard
            .data_offset
            .checked_add(location.descriptor.data_offsets[0])
            .ok_or(SafeTensorError::Overflow)?;
        Ok(ShardedTensorReader {
            file: shard.file.try_clone().map_err(SafeTensorError::Io)?,
            fingerprint: shard.fingerprint,
            absolute_offset,
            bytes: location.descriptor.bytes,
            position: 0,
        })
    }

    pub fn tensor_shard_reader(
        &self,
        name: &str,
        axis: u8,
        part: u8,
        parts: u8,
    ) -> Result<TensorShardReader, SafeTensorError> {
        let (shard, location) = self.open_verified_shard(name)?;
        TensorShardReader::new(
            shard.file.try_clone().map_err(SafeTensorError::Io)?,
            shard.fingerprint,
            shard
                .data_offset
                .checked_add(location.descriptor.data_offsets[0])
                .ok_or(SafeTensorError::Overflow)?,
            &location.descriptor,
            axis,
            part,
            parts,
        )
    }

    pub fn hash_tensor(&self, name: &str) -> Result<[u8; 32], SafeTensorError> {
        let (shard, location) = self.open_verified_shard(name)?;
        let mut hasher = Sha256::new();
        let mut buffer =
            vec![
                0_u8;
                usize::try_from(location.descriptor.bytes.min(HASH_CHUNK_BYTES as u64))
                    .map_err(|_| SafeTensorError::Overflow)?
            ];
        let mut consumed = 0_u64;
        while consumed < location.descriptor.bytes {
            let chunk =
                usize::try_from((location.descriptor.bytes - consumed).min(buffer.len() as u64))
                    .map_err(|_| SafeTensorError::Overflow)?;
            read_exact_at(
                &shard.file,
                &mut buffer[..chunk],
                shard
                    .data_offset
                    .checked_add(location.descriptor.data_offsets[0])
                    .and_then(|offset| offset.checked_add(consumed))
                    .ok_or(SafeTensorError::Overflow)?,
            )?;
            hasher.update(&buffer[..chunk]);
            consumed += chunk as u64;
        }
        self.validate_open_shard(&location.shard, shard)?;
        Ok(hasher.finalize().into())
    }

    /// Hashes one complete shard through the descriptor opened during index
    /// validation. The pathname and open descriptor fingerprints are checked
    /// before and after hashing so replacement cannot redirect conversion to
    /// a different file.
    pub fn hash_shard_file(
        &self,
        relative_path: impl AsRef<Path>,
    ) -> Result<[u8; 32], SafeTensorError> {
        let relative_path = relative_path.as_ref();
        let shard = self.open_shards.get(relative_path).ok_or_else(|| {
            SafeTensorError::ShardPath(relative_path.to_string_lossy().into_owned())
        })?;
        self.validate_open_shard(relative_path, shard)?;
        let digest = hash_retained_file(&shard.file, shard.fingerprint.bytes)?;
        self.validate_open_shard(relative_path, shard)?;
        Ok(digest)
    }

    /// Hashes the index through the descriptor retained before parsing it.
    /// Directory-mode inventories have no index and reject this operation.
    pub fn hash_source_index(&self) -> Result<([u8; 32], u64), SafeTensorError> {
        let index = self.open_index.as_ref().ok_or(SafeTensorError::Index)?;
        self.validate_open_index(index)?;
        let digest = hash_retained_file(&index.file, index.fingerprint.bytes)?;
        self.validate_open_index(index)?;
        Ok((digest, index.fingerprint.bytes))
    }

    /// Revalidates every retained source pathname and descriptor. Conversion
    /// must call this immediately before publishing a native rank set.
    pub fn revalidate_sources(&self) -> Result<(), SafeTensorError> {
        if let Some(index) = &self.open_index {
            self.validate_open_index(index)?;
        }
        for (relative_path, shard) in &self.open_shards {
            self.validate_open_shard(relative_path, shard)?;
        }
        Ok(())
    }

    fn open_verified_shard(
        &self,
        name: &str,
    ) -> Result<(&OpenShard, &ShardedLocation), SafeTensorError> {
        let location = self
            .locations
            .get(name)
            .ok_or_else(|| SafeTensorError::MissingTensor(name.to_owned()))?;
        let shard = self
            .open_shards
            .get(&location.shard)
            .ok_or(SafeTensorError::Index)?;
        self.validate_open_shard(&location.shard, shard)?;
        Ok((shard, location))
    }

    fn validate_open_shard(
        &self,
        relative_path: &Path,
        shard: &OpenShard,
    ) -> Result<(), SafeTensorError> {
        let path = self.root.join(relative_path);
        let path_metadata = path.symlink_metadata().map_err(SafeTensorError::Io)?;
        let descriptor_metadata = shard.file.metadata().map_err(SafeTensorError::Io)?;
        if !path_metadata.file_type().is_file()
            || path_metadata.file_type().is_symlink()
            || path_metadata.nlink() != 1
            || FileFingerprint::from_metadata(&path_metadata) != shard.fingerprint
            || FileFingerprint::from_metadata(&descriptor_metadata) != shard.fingerprint
        {
            return Err(SafeTensorError::ShardChanged(relative_path.to_owned()));
        }
        Ok(())
    }

    fn validate_open_index(&self, index: &OpenIndex) -> Result<(), SafeTensorError> {
        validate_retained_regular_file(&self.source_path, &index.file, index.fingerprint)
    }
}

#[derive(Debug)]
pub struct ShardedTensorReader {
    file: File,
    fingerprint: FileFingerprint,
    absolute_offset: u64,
    bytes: u64,
    position: u64,
}

impl ShardedTensorReader {
    #[must_use]
    pub const fn len(&self) -> u64 {
        self.bytes
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.bytes == 0
    }
}

impl Read for ShardedTensorReader {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        read_retained_cursor(
            &self.file,
            self.fingerprint,
            self.absolute_offset,
            self.bytes,
            &mut self.position,
            output,
        )
    }
}

#[derive(Debug)]
pub struct TensorShardReader {
    file: File,
    fingerprint: FileFingerprint,
    absolute_offset: u64,
    source_rows: u64,
    source_row_bytes: u64,
    shard_row_bytes: u64,
    contiguous_offset: u64,
    bytes: u64,
    position: u64,
    axis: u8,
    part: u8,
}

impl TensorShardReader {
    fn new(
        file: File,
        fingerprint: FileFingerprint,
        absolute_offset: u64,
        descriptor: &SafeTensorDescriptor,
        axis: u8,
        part: u8,
        parts: u8,
    ) -> Result<Self, SafeTensorError> {
        if parts == 0 || part >= parts || usize::from(axis) >= descriptor.shape.len() {
            return Err(SafeTensorError::TensorShard);
        }
        let axis_extent = descriptor.shape[usize::from(axis)];
        if !axis_extent.is_multiple_of(u64::from(parts))
            || !descriptor.dtype.bits().is_multiple_of(8)
        {
            return Err(SafeTensorError::TensorShard);
        }
        let bytes = descriptor
            .bytes
            .checked_div(u64::from(parts))
            .ok_or(SafeTensorError::Overflow)?;
        if axis == 0 {
            return Ok(Self {
                file,
                fingerprint,
                absolute_offset,
                source_rows: 1,
                source_row_bytes: descriptor.bytes,
                shard_row_bytes: bytes,
                contiguous_offset: bytes
                    .checked_mul(u64::from(part))
                    .ok_or(SafeTensorError::Overflow)?,
                bytes,
                position: 0,
                axis,
                part,
            });
        }
        if axis != 1 || descriptor.shape.len() != 2 {
            return Err(SafeTensorError::TensorShard);
        }
        let element_bytes = descriptor.dtype.bits() / 8;
        let source_rows = descriptor.shape[0];
        let source_row_bytes = descriptor.shape[1]
            .checked_mul(element_bytes)
            .ok_or(SafeTensorError::Overflow)?;
        let shard_row_bytes = source_row_bytes
            .checked_div(u64::from(parts))
            .ok_or(SafeTensorError::Overflow)?;
        if source_rows
            .checked_mul(source_row_bytes)
            .ok_or(SafeTensorError::Overflow)?
            != descriptor.bytes
            || source_rows
                .checked_mul(shard_row_bytes)
                .ok_or(SafeTensorError::Overflow)?
                != bytes
        {
            return Err(SafeTensorError::TensorShard);
        }
        Ok(Self {
            file,
            fingerprint,
            absolute_offset,
            source_rows,
            source_row_bytes,
            shard_row_bytes,
            contiguous_offset: 0,
            bytes,
            position: 0,
            axis,
            part,
        })
    }

    #[must_use]
    pub const fn len(&self) -> u64 {
        self.bytes
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.bytes == 0
    }

    fn read_unvalidated(&mut self, output: &mut [u8]) -> io::Result<usize> {
        if self.axis == 0 {
            return read_cursor(
                &self.file,
                self.absolute_offset
                    .checked_add(self.contiguous_offset)
                    .ok_or_else(|| io::Error::from(io::ErrorKind::InvalidData))?,
                self.bytes,
                &mut self.position,
                output,
            );
        }
        if output.is_empty() || self.position == self.bytes {
            return Ok(0);
        }
        let row = self.position / self.shard_row_bytes;
        if row >= self.source_rows {
            return Err(io::Error::from(io::ErrorKind::InvalidData));
        }
        let within_row = self.position % self.shard_row_bytes;
        let remaining_row = self
            .shard_row_bytes
            .checked_sub(within_row)
            .ok_or_else(|| io::Error::from(io::ErrorKind::InvalidData))?;
        let requested = output.len().min(
            usize::try_from(remaining_row)
                .map_err(|_| io::Error::from(io::ErrorKind::InvalidData))?,
        );
        let rank_column_offset = u64::from(self.part)
            .checked_mul(self.shard_row_bytes)
            .ok_or_else(|| io::Error::from(io::ErrorKind::InvalidData))?;
        let source_position = row
            .checked_mul(self.source_row_bytes)
            .and_then(|offset| offset.checked_add(rank_column_offset))
            .and_then(|offset| offset.checked_add(within_row))
            .and_then(|offset| self.absolute_offset.checked_add(offset))
            .ok_or_else(|| io::Error::from(io::ErrorKind::InvalidData))?;
        let read = self
            .file
            .read_at(&mut output[..requested], source_position)?;
        if read == 0 {
            return Err(io::Error::from(io::ErrorKind::UnexpectedEof));
        }
        self.position = self
            .position
            .checked_add(read as u64)
            .ok_or_else(|| io::Error::from(io::ErrorKind::InvalidData))?;
        Ok(read)
    }
}

impl Read for TensorShardReader {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        if self.position == 0 {
            validate_reader_descriptor(&self.file, self.fingerprint)?;
        }
        let read = self.read_unvalidated(output)?;
        if self.position == self.bytes {
            validate_reader_descriptor(&self.file, self.fingerprint)?;
        }
        Ok(read)
    }
}

pub fn load_exl3_projection(
    file: &SafeTensorFile,
    stem: &str,
    metadata: Exl3Metadata,
) -> Result<Exl3Trellis, SafeTensorError> {
    load_exl3_projection_impl(
        |name| file.tensor(name),
        |name| file.read_tensor(name),
        stem,
        metadata,
    )
}

pub fn load_exl3_projection_sharded(
    files: &ShardedSafetensors,
    stem: &str,
    metadata: Exl3Metadata,
) -> Result<Exl3Trellis, SafeTensorError> {
    load_exl3_projection_impl(
        |name| files.tensor(name),
        |name| files.read_tensor(name),
        stem,
        metadata,
    )
}

fn load_exl3_projection_impl<'a>(
    descriptor: impl Fn(&str) -> Option<&'a SafeTensorDescriptor>,
    read: impl Fn(&str) -> Result<Vec<u8>, SafeTensorError>,
    stem: &str,
    metadata: Exl3Metadata,
) -> Result<Exl3Trellis, SafeTensorError> {
    metadata.validate().map_err(SafeTensorError::Exl3)?;
    let mcg_name = format!("{stem}.mcg");
    let suh_name = format!("{stem}.suh");
    let svh_name = format!("{stem}.svh");
    let trellis_name = format!("{stem}.trellis");
    let marker =
        descriptor(&mcg_name).ok_or_else(|| SafeTensorError::MissingTensor(mcg_name.clone()))?;
    if !matches!(marker.shape.as_slice(), [] | [1])
        || !matches!(marker.dtype, SafeDtype::I32 | SafeDtype::U32)
    {
        return Err(SafeTensorError::Component(mcg_name));
    }
    validate_component(
        &descriptor,
        &suh_name,
        &[u64::from(metadata.logical_k)],
        &[SafeDtype::F16],
    )?;
    validate_component(
        &descriptor,
        &svh_name,
        &[u64::from(metadata.logical_n)],
        &[SafeDtype::F16],
    )?;
    validate_component(
        &descriptor,
        &trellis_name,
        &[
            u64::from(metadata.logical_k / 16),
            u64::from(metadata.logical_n / 16),
            u64::from(16 * u32::from(metadata.bits)),
        ],
        &[SafeDtype::I16],
    )?;

    let marker = read(&mcg_name)?;
    let suh = read(&suh_name)?;
    let svh = read(&svh_name)?;
    let trellis = read(&trellis_name)?;
    let mut aux = Vec::with_capacity(
        marker
            .len()
            .checked_add(suh.len())
            .and_then(|bytes| bytes.checked_add(svh.len()))
            .ok_or(SafeTensorError::Overflow)?,
    );
    aux.extend_from_slice(&marker);
    aux.extend_from_slice(&suh);
    aux.extend_from_slice(&svh);
    Exl3Trellis::from_container_planes(metadata, &trellis, &aux).map_err(SafeTensorError::Exl3)
}

fn validate_component<'a>(
    descriptor: &impl Fn(&str) -> Option<&'a SafeTensorDescriptor>,
    name: &str,
    shape: &[u64],
    dtypes: &[SafeDtype],
) -> Result<(), SafeTensorError> {
    let descriptor =
        descriptor(name).ok_or_else(|| SafeTensorError::MissingTensor(name.to_owned()))?;
    if descriptor.shape != shape || !dtypes.contains(&descriptor.dtype) {
        return Err(SafeTensorError::Component(name.to_owned()));
    }
    Ok(())
}

fn validate_contiguous_data(
    tensors: &BTreeMap<String, SafeTensorDescriptor>,
    data_bytes: u64,
) -> Result<(), SafeTensorError> {
    let mut ranges: Vec<_> = tensors
        .iter()
        .map(|(name, descriptor)| (descriptor.data_offsets[0], descriptor.data_offsets[1], name))
        .collect();
    ranges.sort_unstable();
    let mut cursor = 0_u64;
    for (start, end, _) in ranges {
        if start != cursor {
            return Err(SafeTensorError::NonContiguousData);
        }
        cursor = end;
    }
    if cursor != data_bytes {
        return Err(SafeTensorError::NonContiguousData);
    }
    Ok(())
}

fn validate_tensor_name(name: &str) -> Result<(), SafeTensorError> {
    if name.is_empty()
        || name == "__metadata__"
        || name
            .bytes()
            .any(|byte| byte == 0 || byte.is_ascii_control())
    {
        return Err(SafeTensorError::TensorName(name.to_owned()));
    }
    Ok(())
}

fn validate_shard_path(value: &str) -> Result<PathBuf, SafeTensorError> {
    let path = Path::new(value);
    if path.as_os_str().is_empty()
        || path.extension().and_then(|extension| extension.to_str()) != Some("safetensors")
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(SafeTensorError::ShardPath(value.to_owned()));
    }
    Ok(path.to_owned())
}

fn recommended_shard_open_workers() -> usize {
    thread::available_parallelism()
        .map_or(1, usize::from)
        .clamp(1, MAXIMUM_SHARD_OPEN_WORKERS)
}

fn open_retained_safetensor_files(
    root: &Path,
    relative_paths: &[PathBuf],
    requested_workers: usize,
) -> Result<Vec<SafeTensorFile>, SafeTensorError> {
    if requested_workers == 0 || relative_paths.is_empty() {
        return Err(SafeTensorError::Index);
    }
    let worker_count = requested_workers.min(relative_paths.len());
    if worker_count == 1 {
        return relative_paths
            .iter()
            .map(|relative| open_retained_safetensor_shard(root, relative))
            .collect();
    }
    let batches = thread::scope(|scope| {
        let mut handles = Vec::with_capacity(worker_count);
        for worker in 0..worker_count {
            handles.push(scope.spawn(move || {
                (worker..relative_paths.len())
                    .step_by(worker_count)
                    .map(|index| {
                        (
                            index,
                            open_retained_safetensor_shard(root, &relative_paths[index]),
                        )
                    })
                    .collect::<Vec<_>>()
            }));
        }
        handles
            .into_iter()
            .map(|handle| handle.join().map_err(|_| SafeTensorError::Index))
            .collect::<Result<Vec<_>, _>>()
    })?;

    let mut ordered: Vec<Option<Result<SafeTensorFile, SafeTensorError>>> =
        std::iter::repeat_with(|| None)
            .take(relative_paths.len())
            .collect();
    for batch in batches {
        for (index, result) in batch {
            let slot = ordered.get_mut(index).ok_or(SafeTensorError::Index)?;
            if slot.replace(result).is_some() {
                return Err(SafeTensorError::Index);
            }
        }
    }
    ordered
        .into_iter()
        .map(|result| {
            result
                .ok_or(SafeTensorError::Index)
                .and_then(std::convert::identity)
        })
        .collect()
}

fn open_retained_safetensor_shard(
    root: &Path,
    relative_path: &Path,
) -> Result<SafeTensorFile, SafeTensorError> {
    let path = root.join(relative_path);
    let path_metadata = path.symlink_metadata().map_err(SafeTensorError::Io)?;
    if !path_metadata.file_type().is_file()
        || path_metadata.file_type().is_symlink()
        || path_metadata.nlink() != 1
    {
        return Err(SafeTensorError::ShardPath(
            relative_path.to_string_lossy().into_owned(),
        ));
    }
    SafeTensorFile::open(path)
}

fn open_retained_regular_file(path: &Path) -> Result<(File, FileFingerprint), SafeTensorError> {
    let path_metadata = path.symlink_metadata().map_err(SafeTensorError::Io)?;
    if !path_metadata.file_type().is_file()
        || path_metadata.file_type().is_symlink()
        || path_metadata.nlink() != 1
    {
        return Err(SafeTensorError::SourceChanged(path.to_owned()));
    }
    let file = File::open(path).map_err(SafeTensorError::Io)?;
    let descriptor_metadata = file.metadata().map_err(SafeTensorError::Io)?;
    let fingerprint = FileFingerprint::from_metadata(&path_metadata);
    if FileFingerprint::from_metadata(&descriptor_metadata) != fingerprint {
        return Err(SafeTensorError::SourceChanged(path.to_owned()));
    }
    Ok((file, fingerprint))
}

fn validate_retained_regular_file(
    path: &Path,
    file: &File,
    fingerprint: FileFingerprint,
) -> Result<(), SafeTensorError> {
    let path_metadata = path.symlink_metadata().map_err(SafeTensorError::Io)?;
    let descriptor_metadata = file.metadata().map_err(SafeTensorError::Io)?;
    if !path_metadata.file_type().is_file()
        || path_metadata.file_type().is_symlink()
        || path_metadata.nlink() != 1
        || FileFingerprint::from_metadata(&path_metadata) != fingerprint
        || FileFingerprint::from_metadata(&descriptor_metadata) != fingerprint
    {
        return Err(SafeTensorError::SourceChanged(path.to_owned()));
    }
    Ok(())
}

fn hash_retained_file(file: &File, bytes: u64) -> Result<[u8; 32], SafeTensorError> {
    let mut hasher = Sha256::new();
    let mut buffer = vec![
        0_u8;
        usize::try_from(bytes.clamp(1, HASH_CHUNK_BYTES as u64))
            .map_err(|_| SafeTensorError::Overflow)?
    ];
    let mut consumed = 0_u64;
    while consumed < bytes {
        let chunk = usize::try_from((bytes - consumed).min(buffer.len() as u64))
            .map_err(|_| SafeTensorError::Overflow)?;
        read_exact_at(file, &mut buffer[..chunk], consumed)?;
        hasher.update(&buffer[..chunk]);
        consumed += chunk as u64;
    }
    Ok(hasher.finalize().into())
}

fn read_exact_at(
    file: &File,
    mut output: &mut [u8],
    mut offset: u64,
) -> Result<(), SafeTensorError> {
    while !output.is_empty() {
        let read = file.read_at(output, offset).map_err(SafeTensorError::Io)?;
        if read == 0 {
            return Err(SafeTensorError::Truncated);
        }
        offset = offset
            .checked_add(read as u64)
            .ok_or(SafeTensorError::Overflow)?;
        output = &mut output[read..];
    }
    Ok(())
}

fn read_cursor(
    file: &File,
    absolute_offset: u64,
    bytes: u64,
    position: &mut u64,
    output: &mut [u8],
) -> io::Result<usize> {
    if output.is_empty() || *position == bytes {
        return Ok(0);
    }
    let remaining = bytes
        .checked_sub(*position)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "tensor reader position"))?;
    let chunk = output
        .len()
        .min(usize::try_from(remaining).unwrap_or(usize::MAX));
    let offset = absolute_offset
        .checked_add(*position)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "tensor reader overflow"))?;
    let read = file.read_at(&mut output[..chunk], offset)?;
    if read == 0 {
        return Err(io::Error::from(io::ErrorKind::UnexpectedEof));
    }
    *position = position
        .checked_add(read as u64)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "tensor reader overflow"))?;
    Ok(read)
}

fn read_retained_cursor(
    file: &File,
    fingerprint: FileFingerprint,
    absolute_offset: u64,
    bytes: u64,
    position: &mut u64,
    output: &mut [u8],
) -> io::Result<usize> {
    if *position == 0 {
        validate_reader_descriptor(file, fingerprint)?;
    }
    let read = read_cursor(file, absolute_offset, bytes, position, output)?;
    if *position == bytes {
        validate_reader_descriptor(file, fingerprint)?;
    }
    Ok(read)
}

fn validate_reader_descriptor(file: &File, fingerprint: FileFingerprint) -> io::Result<()> {
    let metadata = file.metadata()?;
    if FileFingerprint::from_metadata(&metadata) != fingerprint {
        return Err(io::Error::from(io::ErrorKind::InvalidData));
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawTensor {
    dtype: String,
    shape: Vec<u64>,
    data_offsets: [u64; 2],
}

#[derive(Debug, Default)]
struct RawHeader {
    metadata: BTreeMap<String, String>,
    tensors: BTreeMap<String, RawTensor>,
}

impl<'de> Deserialize<'de> for RawHeader {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct RawHeaderVisitor;

        impl<'de> Visitor<'de> for RawHeaderVisitor {
            type Value = RawHeader;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a safetensors header object")
            }

            fn visit_map<M>(self, mut map: M) -> Result<Self::Value, M::Error>
            where
                M: MapAccess<'de>,
            {
                let mut header = RawHeader::default();
                let mut saw_metadata = false;
                while let Some(name) = map.next_key::<String>()? {
                    if name == "__metadata__" {
                        if saw_metadata {
                            return Err(serde::de::Error::duplicate_field("__metadata__"));
                        }
                        saw_metadata = true;
                        header.metadata = map.next_value()?;
                    } else {
                        let tensor = map.next_value()?;
                        if header.tensors.insert(name.clone(), tensor).is_some() {
                            return Err(serde::de::Error::custom(format!(
                                "duplicate tensor key {name}"
                            )));
                        }
                    }
                }
                Ok(header)
            }
        }
        deserializer.deserialize_map(RawHeaderVisitor)
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawIndex {
    #[serde(default, rename = "metadata")]
    metadata: BTreeMap<String, serde_json::Value>,
    weight_map: UniqueStringMap,
}

#[derive(Debug, Default)]
struct UniqueStringMap(BTreeMap<String, String>);

impl<'de> Deserialize<'de> for UniqueStringMap {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct UniqueStringMapVisitor;

        impl<'de> Visitor<'de> for UniqueStringMapVisitor {
            type Value = UniqueStringMap;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a map with unique string keys and values")
            }

            fn visit_map<M>(self, mut map: M) -> Result<Self::Value, M::Error>
            where
                M: MapAccess<'de>,
            {
                let mut values = BTreeMap::new();
                while let Some((key, value)) = map.next_entry::<String, String>()? {
                    if values.insert(key.clone(), value).is_some() {
                        return Err(serde::de::Error::custom(format!("duplicate map key {key}")));
                    }
                }
                Ok(UniqueStringMap(values))
            }
        }
        deserializer.deserialize_map(UniqueStringMapVisitor)
    }
}

#[derive(Debug)]
pub enum SafeTensorError {
    Io(io::Error),
    Json(serde_json::Error),
    Truncated,
    HeaderLength(u64),
    Header,
    Index,
    TensorCount,
    TensorName(String),
    UnsupportedDtype(String),
    ByteAccounting(String),
    NonContiguousData,
    MissingTensor(String),
    TensorRange(String),
    TensorShard,
    Component(String),
    ShardPath(String),
    ShardInventory(PathBuf),
    ShardChanged(PathBuf),
    ShardDirectory(PathBuf),
    DuplicateTensor(String),
    SourceChanged(PathBuf),
    Overflow,
    Exl3(Exl3Error),
}

impl fmt::Display for SafeTensorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for SafeTensorError {}

#[cfg(test)]
mod tests {
    use std::{
        fs::{self, FileTimes},
        sync::atomic::{AtomicU64, Ordering},
        time::{Duration, UNIX_EPOCH},
    };

    use serde_json::json;

    use super::*;
    use crate::{EXL3_MCG_MULTIPLIER, Exl3Projection};

    static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

    struct TempPath(PathBuf);

    impl TempPath {
        fn new(extension: &str) -> Self {
            let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "glmaxx-safetensors-{}-{sequence}.{extension}",
                std::process::id()
            ));
            Self(path)
        }
    }

    impl Drop for TempPath {
        fn drop(&mut self) {
            match self.0.symlink_metadata() {
                Ok(metadata) if metadata.file_type().is_dir() => {
                    let _ = fs::remove_dir_all(&self.0);
                }
                Ok(_) => {
                    let _ = fs::remove_file(&self.0);
                }
                Err(_) => {}
            }
        }
    }

    fn write_safe(path: &Path, entries: Vec<(&str, &str, Vec<u64>, Vec<u8>)>) {
        let mut header = serde_json::Map::new();
        let mut data = Vec::new();
        for (name, dtype, shape, bytes) in entries {
            let start = data.len();
            data.extend_from_slice(&bytes);
            header.insert(
                name.to_owned(),
                json!({
                    "dtype": dtype,
                    "shape": shape,
                    "data_offsets": [start, data.len()]
                }),
            );
        }
        header.insert(
            "__metadata__".into(),
            json!({"format": "pt", "fixture": "glmaxx"}),
        );
        let mut encoded = serde_json::to_vec(&header).unwrap();
        while !(8 + encoded.len()).is_multiple_of(8) {
            encoded.push(b' ');
        }
        let mut file = Vec::new();
        file.extend_from_slice(&(encoded.len() as u64).to_le_bytes());
        file.extend_from_slice(&encoded);
        file.extend_from_slice(&data);
        fs::write(path, file).unwrap();
    }

    fn force_distinct_modified_time(path: &Path, seconds: u64) {
        // Overlay and network filesystems may coalesce two same-size rewrites
        // into one timestamp tick. Make the fingerprint transition explicit
        // instead of relying on a sleep or wall-clock scheduling.
        let file = File::options().write(true).open(path).unwrap();
        file.set_times(FileTimes::new().set_modified(UNIX_EPOCH + Duration::from_secs(seconds)))
            .unwrap();
    }

    fn words_bytes(words: &[u16]) -> Vec<u8> {
        words.iter().flat_map(|word| word.to_le_bytes()).collect()
    }

    fn fixture(rank: u8) -> Exl3Trellis {
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

    #[test]
    fn file_inventory_hash_and_tensor_reads_are_exact() {
        let path = TempPath::new("safetensors");
        write_safe(
            &path.0,
            vec![
                ("first", "U8", vec![4], vec![1, 2, 3, 4]),
                ("second", "I16", vec![2], vec![5, 0, 6, 0]),
            ],
        );
        let file = SafeTensorFile::open(&path.0).unwrap();
        assert_eq!(file.tensors().len(), 2);
        assert_eq!(file.metadata()["fixture"], "glmaxx");
        assert_eq!(file.tensor("second").unwrap().dtype, SafeDtype::I16);
        assert_eq!(file.read_tensor("first").unwrap(), [1, 2, 3, 4]);
        let mut middle = [0_u8; 2];
        file.read_tensor_range("first", 1, &mut middle).unwrap();
        assert_eq!(middle, [2, 3]);
        let mut copied = Vec::new();
        file.copy_tensor("second", &mut copied).unwrap();
        assert_eq!(copied, [5, 0, 6, 0]);
        let mut streamed = Vec::new();
        file.tensor_reader("first")
            .unwrap()
            .read_to_end(&mut streamed)
            .unwrap();
        assert_eq!(streamed, [1, 2, 3, 4]);
        assert!(matches!(
            file.read_tensor_range("first", 3, &mut [0; 2]),
            Err(SafeTensorError::TensorRange(_))
        ));
        let expected: [u8; 32] = Sha256::digest([5, 0, 6, 0]).into();
        assert_eq!(file.hash_tensor("second").unwrap(), expected);
        let expected_file: [u8; 32] = Sha256::digest(fs::read(&path.0).unwrap()).into();
        assert_eq!(file.hash_file().unwrap(), expected_file);
    }

    #[test]
    fn standalone_file_retains_identity_and_rejects_reopen_aliases() {
        let directory = TempPath::new("dir");
        fs::create_dir(&directory.0).unwrap();
        let source = directory.0.join("source.safetensors");
        write_safe(&source, vec![("x", "U8", vec![2], vec![1, 2])]);
        let retained = SafeTensorFile::open(&source).unwrap();
        retained.revalidate().unwrap();

        let original = directory.0.join("original.safetensors");
        fs::rename(&source, &original).unwrap();
        fs::copy(&original, &source).unwrap();
        assert!(matches!(
            retained.revalidate(),
            Err(SafeTensorError::SourceChanged(path)) if path == source
        ));
        assert!(matches!(
            retained.hash_file(),
            Err(SafeTensorError::SourceChanged(path)) if path == source
        ));
        assert!(matches!(
            retained.read_tensor("x"),
            Err(SafeTensorError::SourceChanged(path)) if path == source
        ));

        let hard_link = directory.0.join("hard-link.safetensors");
        fs::hard_link(&original, &hard_link).unwrap();
        assert!(matches!(
            SafeTensorFile::open(&original),
            Err(SafeTensorError::SourceChanged(path)) if path == original
        ));
        let symlink = directory.0.join("symlink.safetensors");
        std::os::unix::fs::symlink(&original, &symlink).unwrap();
        assert!(matches!(
            SafeTensorFile::open(&symlink),
            Err(SafeTensorError::SourceChanged(path)) if path == symlink
        ));
    }

    #[test]
    fn row_major_tp_shards_stream_axis_zero_and_one_exactly() {
        let path = TempPath::new("safetensors");
        let source: Vec<u8> = (0_u16..32).flat_map(u16::to_le_bytes).collect();
        write_safe(&path.0, vec![("matrix", "U16", vec![4, 8], source.clone())]);
        let file = SafeTensorFile::open(&path.0).unwrap();

        let mut axis_zero = Vec::new();
        let mut reader = file.tensor_shard_reader("matrix", 0, 1, 2).unwrap();
        assert_eq!(reader.len(), 32);
        reader.read_to_end(&mut axis_zero).unwrap();
        assert_eq!(axis_zero, source[32..]);

        let mut axis_one = Vec::new();
        let mut reader = file.tensor_shard_reader("matrix", 1, 1, 2).unwrap();
        assert_eq!(reader.len(), 32);
        let mut tiny = [0_u8; 3];
        loop {
            let read = reader.read(&mut tiny).unwrap();
            if read == 0 {
                break;
            }
            axis_one.extend_from_slice(&tiny[..read]);
        }
        let expected: Vec<u8> = (0_u16..4)
            .flat_map(|row| (4_u16..8).map(move |column| row * 8 + column))
            .flat_map(u16::to_le_bytes)
            .collect();
        assert_eq!(axis_one, expected);

        assert!(matches!(
            file.tensor_shard_reader("matrix", 2, 0, 2),
            Err(SafeTensorError::TensorShard)
        ));
        assert!(matches!(
            file.tensor_shard_reader("matrix", 1, 2, 2),
            Err(SafeTensorError::TensorShard)
        ));
    }

    #[test]
    fn subbyte_f4_uses_ceil_bit_accounting() {
        let path = TempPath::new("safetensors");
        write_safe(&path.0, vec![("f4", "F4", vec![3], vec![0x21, 0x03])]);
        let file = SafeTensorFile::open(&path.0).unwrap();
        let descriptor = file.tensor("f4").unwrap();
        assert_eq!(descriptor.dtype, SafeDtype::F4);
        assert_eq!(descriptor.elements, 3);
        assert_eq!(descriptor.bytes, 2);
    }

    #[test]
    fn exl3_projection_imports_directly_from_source_components() {
        let source = fixture(0);
        let path = TempPath::new("safetensors");
        write_safe(
            &path.0,
            vec![
                (
                    "projection.mcg",
                    "I32",
                    vec![1],
                    source.mcg_marker.to_le_bytes().to_vec(),
                ),
                ("projection.suh", "F16", vec![128], words_bytes(&source.suh)),
                ("projection.svh", "F16", vec![128], words_bytes(&source.svh)),
                (
                    "projection.trellis",
                    "I16",
                    vec![8, 8, 48],
                    words_bytes(&source.trellis),
                ),
            ],
        );
        let file = SafeTensorFile::open(&path.0).unwrap();
        let imported = load_exl3_projection(&file, "projection", source.metadata.clone()).unwrap();
        assert_eq!(imported, source);
    }

    #[test]
    fn holes_wrong_lengths_and_unsupported_dtypes_fail_closed() {
        let path = TempPath::new("safetensors");
        let mut header = br#"{"x":{"dtype":"U8","shape":[1],"data_offsets":[1,2]}}"#.to_vec();
        while !(8 + header.len()).is_multiple_of(8) {
            header.push(b' ');
        }
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&(header.len() as u64).to_le_bytes());
        bytes.extend_from_slice(&header);
        bytes.extend_from_slice(&[0, 1]);
        fs::write(&path.0, bytes).unwrap();
        assert!(matches!(
            SafeTensorFile::open(&path.0),
            Err(SafeTensorError::NonContiguousData)
        ));

        write_safe(&path.0, vec![("x", "F4_E2M1", vec![1], vec![0])]);
        assert!(matches!(
            SafeTensorFile::open(&path.0),
            Err(SafeTensorError::UnsupportedDtype(_))
        ));
    }

    #[test]
    fn sharded_index_enforces_exact_inventory_and_detects_header_change() {
        let directory = TempPath::new("dir");
        fs::create_dir(&directory.0).unwrap();
        let shard_a = directory.0.join("a.safetensors");
        let shard_b = directory.0.join("b.safetensors");
        write_safe(&shard_a, vec![("a", "U8", vec![2], vec![1, 2])]);
        write_safe(&shard_b, vec![("b", "U8", vec![2], vec![3, 4])]);
        let index = directory.0.join("model.safetensors.index.json");
        fs::write(
            &index,
            serde_json::to_vec(&json!({
                "metadata": {"total_size": 4},
                "weight_map": {
                    "a": "a.safetensors",
                    "b": "b.safetensors"
                }
            }))
            .unwrap(),
        )
        .unwrap();
        let set = ShardedSafetensors::open(&index).unwrap();
        let sequential = ShardedSafetensors::open_with_workers(&index, 1).unwrap();
        let parallel = ShardedSafetensors::open_with_workers(&index, 16).unwrap();
        assert_eq!(sequential.structure_sha256(), parallel.structure_sha256());
        assert_eq!(sequential.shards(), parallel.shards());
        assert_eq!(
            sequential.tensor_names().collect::<Vec<_>>(),
            parallel.tensor_names().collect::<Vec<_>>()
        );
        for name in sequential.tensor_names() {
            assert_eq!(sequential.tensor(name), parallel.tensor(name));
        }
        assert_eq!(sequential.read_tensor("a").unwrap(), [1, 2]);
        assert_eq!(parallel.read_tensor("a").unwrap(), [1, 2]);
        assert!(matches!(
            ShardedSafetensors::open_with_workers(&index, 0),
            Err(SafeTensorError::Index)
        ));
        let expected_index_sha256: [u8; 32] = Sha256::digest(fs::read(&index).unwrap()).into();
        assert_eq!(
            set.hash_source_index().unwrap(),
            (expected_index_sha256, fs::metadata(&index).unwrap().len())
        );
        set.revalidate_sources().unwrap();
        assert_eq!(set.declared_payload_bytes(), Some(4));
        assert_eq!(set.tensor_names().collect::<Vec<_>>(), ["a", "b"]);
        assert_eq!(set.read_tensor("b").unwrap(), [3, 4]);
        let mut streamed = Vec::new();
        set.tensor_reader("a")
            .unwrap()
            .read_to_end(&mut streamed)
            .unwrap();
        assert_eq!(streamed, [1, 2]);
        let mut sharded_slice = Vec::new();
        set.tensor_shard_reader("a", 0, 1, 2)
            .unwrap()
            .read_to_end(&mut sharded_slice)
            .unwrap();
        assert_eq!(sharded_slice, [2]);
        assert_eq!(
            set.hash_shard_file("a.safetensors").unwrap(),
            SafeTensorFile::open(&shard_a).unwrap().hash_file().unwrap()
        );
        assert!(matches!(
            set.hash_shard_file("missing.safetensors"),
            Err(SafeTensorError::ShardPath(_))
        ));
        write_safe(&shard_b, vec![("b", "U8", vec![1], vec![4])]);
        force_distinct_modified_time(&shard_b, 1);
        assert!(matches!(
            set.read_tensor("b"),
            Err(SafeTensorError::ShardChanged(_))
        ));
        assert!(matches!(
            set.hash_shard_file("b.safetensors"),
            Err(SafeTensorError::ShardChanged(_))
        ));
        assert!(matches!(
            set.revalidate_sources(),
            Err(SafeTensorError::ShardChanged(_))
        ));
    }

    #[test]
    fn sharded_index_replacement_cannot_cross_parsing_and_hashing() {
        let directory = TempPath::new("dir");
        fs::create_dir(&directory.0).unwrap();
        let shard = directory.0.join("a.safetensors");
        write_safe(&shard, vec![("a", "U8", vec![2], vec![1, 2])]);
        let index = directory.0.join("model.safetensors.index.json");
        let index_bytes = serde_json::to_vec(&json!({
            "metadata": {"total_size": 2},
            "weight_map": {"a": "a.safetensors"}
        }))
        .unwrap();
        fs::write(&index, &index_bytes).unwrap();
        let set = ShardedSafetensors::open(&index).unwrap();

        let original = directory.0.join("original-index.json");
        fs::rename(&index, &original).unwrap();
        fs::write(&index, &index_bytes).unwrap();
        assert!(matches!(
            set.hash_source_index(),
            Err(SafeTensorError::SourceChanged(path)) if path == index
        ));
        assert!(matches!(
            set.revalidate_sources(),
            Err(SafeTensorError::SourceChanged(path)) if path == index
        ));

        let hard_link = directory.0.join("hard-link-index.json");
        fs::hard_link(&original, &hard_link).unwrap();
        assert!(matches!(
            ShardedSafetensors::open(&original),
            Err(SafeTensorError::SourceChanged(path)) if path == original
        ));
        let symlink = directory.0.join("symlink-index.json");
        std::os::unix::fs::symlink(&original, &symlink).unwrap();
        assert!(matches!(
            ShardedSafetensors::open(&symlink),
            Err(SafeTensorError::SourceChanged(path)) if path == symlink
        ));

        let mutable_index = directory.0.join("mutable-index.json");
        fs::write(&mutable_index, &index_bytes).unwrap();
        let mutable_set = ShardedSafetensors::open(&mutable_index).unwrap();
        let mut changed_bytes = index_bytes;
        let digit = changed_bytes
            .iter()
            .position(|&byte| byte == b'2')
            .expect("fixture contains total_size");
        changed_bytes[digit] = b'3';
        fs::write(&mutable_index, changed_bytes).unwrap();
        force_distinct_modified_time(&mutable_index, 3);
        assert!(matches!(
            mutable_set.revalidate_sources(),
            Err(SafeTensorError::SourceChanged(path)) if path == mutable_index
        ));
    }

    #[test]
    fn streaming_readers_reject_same_inode_mutation_before_completion() {
        let directory = TempPath::new("dir");
        fs::create_dir(&directory.0).unwrap();
        let shard = directory.0.join("a.safetensors");
        let original: Vec<u8> = (0_u8..8).collect();
        write_safe(&shard, vec![("matrix", "U8", vec![2, 4], original.clone())]);
        let index = directory.0.join("model.safetensors.index.json");
        fs::write(
            &index,
            serde_json::to_vec(&json!({
                "metadata": {"total_size": 8},
                "weight_map": {"matrix": "a.safetensors"}
            }))
            .unwrap(),
        )
        .unwrap();

        let set = ShardedSafetensors::open(&index).unwrap();
        let mut reader = set.tensor_reader("matrix").unwrap();
        let mut first = [0_u8; 1];
        reader.read_exact(&mut first).unwrap();
        assert_eq!(first, [0]);
        write_safe(&shard, vec![("matrix", "U8", vec![2, 4], vec![9; 8])]);
        force_distinct_modified_time(&shard, 4);
        assert_eq!(
            reader.read_to_end(&mut Vec::new()).unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );

        write_safe(&shard, vec![("matrix", "U8", vec![2, 4], original)]);
        force_distinct_modified_time(&shard, 5);
        let set = ShardedSafetensors::open(&index).unwrap();
        let mut reader = set.tensor_shard_reader("matrix", 1, 1, 2).unwrap();
        reader.read_exact(&mut first).unwrap();
        assert_eq!(first, [2]);
        write_safe(&shard, vec![("matrix", "U8", vec![2, 4], vec![7; 8])]);
        force_distinct_modified_time(&shard, 6);
        assert_eq!(
            reader.read_to_end(&mut Vec::new()).unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
    }

    #[test]
    fn shard_directory_inventory_is_sorted_deterministic_and_streamable() {
        let directory = TempPath::new("dir");
        fs::create_dir(&directory.0).unwrap();
        let shard_z = directory.0.join("model-layer-004.safetensors");
        let shard_a = directory.0.join("model-layer-003.safetensors");
        write_safe(&shard_z, vec![("z", "U8", vec![2], vec![3, 4])]);
        write_safe(&shard_a, vec![("a", "U8", vec![2], vec![1, 2])]);
        fs::write(directory.0.join("README.txt"), b"ignored").unwrap();

        let first = ShardedSafetensors::open_directory(&directory.0).unwrap();
        let automatic = ShardedSafetensors::open_auto(&directory.0).unwrap();
        assert_eq!(first.tensor_names().collect::<Vec<_>>(), ["a", "z"]);
        assert_eq!(
            first.shards().iter().collect::<Vec<_>>(),
            [
                &PathBuf::from("model-layer-003.safetensors"),
                &PathBuf::from("model-layer-004.safetensors")
            ]
        );
        assert_eq!(first.structure_sha256(), automatic.structure_sha256());
        assert_eq!(first.read_tensor("z").unwrap(), [3, 4]);
        let mut streamed = Vec::new();
        first
            .tensor_reader("a")
            .unwrap()
            .read_to_end(&mut streamed)
            .unwrap();
        assert_eq!(streamed, [1, 2]);

        let reopened = ShardedSafetensors::open_directory(&directory.0).unwrap();
        assert_eq!(first.structure_sha256(), reopened.structure_sha256());
        write_safe(&shard_z, vec![("z", "U8", vec![2], vec![4, 3])]);
        force_distinct_modified_time(&shard_z, 2);
        assert_eq!(first.structure_sha256(), reopened.structure_sha256());
        assert!(matches!(
            first.read_tensor("z"),
            Err(SafeTensorError::ShardChanged(_))
        ));
    }

    #[test]
    fn shard_directory_rejects_duplicate_tensors_and_symlinks() {
        let duplicates = TempPath::new("dir");
        fs::create_dir(&duplicates.0).unwrap();
        write_safe(
            &duplicates.0.join("model-layer-003.safetensors"),
            vec![("same", "U8", vec![1], vec![1])],
        );
        write_safe(
            &duplicates.0.join("model-layer-004.safetensors"),
            vec![("same", "U8", vec![1], vec![2])],
        );
        assert!(matches!(
            ShardedSafetensors::open_directory(&duplicates.0),
            Err(SafeTensorError::DuplicateTensor(name)) if name == "same"
        ));

        let symlinked = TempPath::new("dir");
        fs::create_dir(&symlinked.0).unwrap();
        let target = symlinked.0.join("target.bin");
        fs::write(&target, b"target").unwrap();
        std::os::unix::fs::symlink(&target, symlinked.0.join("bad.safetensors")).unwrap();
        assert!(matches!(
            ShardedSafetensors::open_directory(&symlinked.0),
            Err(SafeTensorError::ShardPath(_))
        ));
        let alias = TempPath::new("dir-link");
        std::os::unix::fs::symlink(&symlinked.0, &alias.0).unwrap();
        assert!(matches!(
            ShardedSafetensors::open_auto(&alias.0),
            Err(SafeTensorError::ShardPath(_))
        ));
    }

    #[test]
    fn exl3_projection_can_span_safetensor_shards() {
        let source = fixture(0);
        let directory = TempPath::new("dir");
        fs::create_dir(&directory.0).unwrap();
        let shard_a = directory.0.join("a.safetensors");
        let shard_b = directory.0.join("b.safetensors");
        write_safe(
            &shard_a,
            vec![
                (
                    "projection.mcg",
                    "I32",
                    vec![],
                    source.mcg_marker.to_le_bytes().to_vec(),
                ),
                ("projection.suh", "F16", vec![128], words_bytes(&source.suh)),
            ],
        );
        write_safe(
            &shard_b,
            vec![
                ("projection.svh", "F16", vec![128], words_bytes(&source.svh)),
                (
                    "projection.trellis",
                    "I16",
                    vec![8, 8, 48],
                    words_bytes(&source.trellis),
                ),
            ],
        );
        let index = directory.0.join("model.safetensors.index.json");
        fs::write(
            &index,
            br#"{"metadata":{"total_size":6660},"weight_map":{"projection.mcg":"a.safetensors","projection.suh":"a.safetensors","projection.svh":"b.safetensors","projection.trellis":"b.safetensors"}}"#,
        )
        .unwrap();
        let set = ShardedSafetensors::open(&index).unwrap();
        let imported =
            load_exl3_projection_sharded(&set, "projection", source.metadata.clone()).unwrap();
        assert_eq!(imported, source);
        let marker_hash: [u8; 32] = Sha256::digest(source.mcg_marker.to_le_bytes()).into();
        assert_eq!(set.hash_tensor("projection.mcg").unwrap(), marker_hash);
    }

    #[test]
    fn index_rejects_traversal_and_unmapped_shard_tensors() {
        let directory = TempPath::new("dir");
        fs::create_dir(&directory.0).unwrap();
        let index = directory.0.join("model.safetensors.index.json");
        fs::write(
            &index,
            br#"{"metadata":{},"weight_map":{"a":"../a.safetensors"}}"#,
        )
        .unwrap();
        assert!(matches!(
            ShardedSafetensors::open(&index),
            Err(SafeTensorError::ShardPath(_))
        ));

        let shard = directory.0.join("a.safetensors");
        write_safe(
            &shard,
            vec![
                ("a", "U8", vec![1], vec![1]),
                ("hidden", "U8", vec![1], vec![2]),
            ],
        );
        fs::write(
            &index,
            br#"{"metadata":{},"weight_map":{"a":"a.safetensors"}}"#,
        )
        .unwrap();
        assert!(matches!(
            ShardedSafetensors::open(&index),
            Err(SafeTensorError::ShardInventory(_))
        ));

        fs::write(
            &index,
            br#"{"metadata":{},"weight_map":{"a":"a.safetensors","a":"b.safetensors"}}"#,
        )
        .unwrap();
        assert!(matches!(
            ShardedSafetensors::open(&index),
            Err(SafeTensorError::Json(_))
        ));

        fs::write(
            &index,
            br#"{"metadata":{"total_size":99},"weight_map":{"a":"a.safetensors","hidden":"a.safetensors"}}"#,
        )
        .unwrap();
        assert!(matches!(
            ShardedSafetensors::open(&index),
            Err(SafeTensorError::Index)
        ));
    }
}
