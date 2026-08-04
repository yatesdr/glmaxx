//! Pinned GLM-5.2 tokenizer, chat-template, and incremental output boundary.
//! The runtime accepts only the exact audited tokenizer bundle and masks the
//! model vocabulary's unmapped padding IDs.

mod chat;
mod decode;
mod ordered;

use std::{
    collections::BTreeSet,
    fmt,
    fs::{self, File},
    io::Read,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
    sync::Arc,
};

pub use chat::{
    ChatFunctionCall, ChatMessage, ChatRole, ChatTemplateError, ChatTemplateOptions, ChatToolCall,
    ReasoningEffort, render_chat,
};
pub use decode::{DecodeDelta, IncrementalDecoder, StreamFinish};
pub use ordered::OrderedValue;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use tokenizers::Tokenizer;

pub const MODEL_VOCABULARY: u32 = 154_880;
pub const TOKENIZER_VOCABULARY: u32 = 154_856;
pub const BASE_VOCABULARY: u32 = 154_820;
pub const TOKENIZER_SHA256: [u8; 32] =
    hex_array("19e773648cb4e65de8660ea6365e10acca112d42a854923df93db4a6f333a82d");
pub const TOKENIZER_CONFIG_SHA256: [u8; 32] =
    hex_array("98b1271574f41abf89427ae2dda030d94dc9478f0edc5a8bd240db213c6fd5fc");
pub const GENERATION_CONFIG_SHA256: [u8; 32] =
    hex_array("ac76b43d8683d3b930126870fc8be73d8679308fe752fa1f381096d8354f6a55");
pub const CHAT_TEMPLATE_SHA256: [u8; 32] =
    hex_array("172dc74a35e1752df75ecfb2b2cf9326d2852bb1379868ebeec9571654489679");
pub const TOKEN_OUTPUT_TABLE_SHA256: [u8; 32] =
    hex_array("31b20a4136e6f2854e40bdc34396cfcb6e893259c335fe6d1bdbfef48ea5fa1a");
pub const EOS_TOKEN_IDS: [u32; 3] = [154_820, 154_827, 154_829];
const SPECIAL_TOKEN_IDS: [u32; 18] = [
    154_820, 154_821, 154_822, 154_823, 154_824, 154_825, 154_826, 154_827, 154_828, 154_829,
    154_830, 154_831, 154_832, 154_833, 154_834, 154_835, 154_836, 154_837,
];
const EXPECTED_FILES: [(&str, usize, [u8; 32]); 4] = [
    ("tokenizer.json", 20_217_442, TOKENIZER_SHA256),
    ("tokenizer_config.json", 761, TOKENIZER_CONFIG_SHA256),
    ("generation_config.json", 194, GENERATION_CONFIG_SHA256),
    ("chat_template.jinja", 5_076, CHAT_TEMPLATE_SHA256),
];

#[derive(Clone, Debug)]
enum TokenOutput {
    Bytes(Box<[u8]>),
    Special,
    Invalid,
}

pub struct PinnedTokenizer {
    tokenizer: Tokenizer,
    outputs: Vec<TokenOutput>,
    eos_token_ids: BTreeSet<u32>,
    root: PathBuf,
}

impl PinnedTokenizer {
    pub fn open(root: impl AsRef<Path>) -> Result<Self, TokenizerError> {
        let root = root.as_ref();
        if !root.is_dir() {
            return Err(TokenizerError::Bundle);
        }
        for &(name, bytes, digest) in &EXPECTED_FILES {
            verify_file(&root.join(name), bytes, digest)?;
        }
        validate_configuration(root)?;
        let tokenizer = Tokenizer::from_file(root.join("tokenizer.json"))
            .map_err(|error| TokenizerError::Library(error.to_string()))?;
        if tokenizer.get_vocab_size(false) != BASE_VOCABULARY as usize
            || tokenizer.get_vocab_size(true) != TOKENIZER_VOCABULARY as usize
        {
            return Err(TokenizerError::Vocabulary);
        }
        let outputs = build_outputs(&tokenizer)?;
        if hash_output_table(&outputs) != TOKEN_OUTPUT_TABLE_SHA256 {
            return Err(TokenizerError::Vocabulary);
        }
        Ok(Self {
            tokenizer,
            outputs,
            eos_token_ids: EOS_TOKEN_IDS.into_iter().collect(),
            root: root.to_owned(),
        })
    }

    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn encode(&self, text: &str) -> Result<Vec<u32>, TokenizerError> {
        let encoding = self
            .tokenizer
            .encode(text, false)
            .map_err(|error| TokenizerError::Library(error.to_string()))?;
        if encoding
            .get_ids()
            .iter()
            .any(|&token| token >= TOKENIZER_VOCABULARY)
        {
            return Err(TokenizerError::Vocabulary);
        }
        Ok(encoding.get_ids().to_vec())
    }

    pub fn decode_reference(
        &self,
        tokens: &[u32],
        skip_special: bool,
    ) -> Result<String, TokenizerError> {
        self.validate_tokens(tokens)?;
        self.tokenizer
            .decode(tokens, skip_special)
            .map_err(|error| TokenizerError::Library(error.to_string()))
    }

    pub fn stream(
        self: &Arc<Self>,
        stops: Vec<String>,
    ) -> Result<IncrementalDecoder, TokenizerError> {
        IncrementalDecoder::new(Arc::clone(self), stops)
    }

    /// Creates a decoder with an explicit EOS policy. Fixed-duration decode
    /// benchmarks can count EOS as an ordinary token while stop strings and
    /// the configured output-token limit remain authoritative.
    pub fn stream_with_eos_policy(
        self: &Arc<Self>,
        stops: Vec<String>,
        ignore_eos: bool,
    ) -> Result<IncrementalDecoder, TokenizerError> {
        IncrementalDecoder::new_with_eos_policy(Arc::clone(self), stops, ignore_eos)
    }

    #[must_use]
    pub fn output_table_sha256(&self) -> [u8; 32] {
        hash_output_table(&self.outputs)
    }

    #[must_use]
    pub fn is_eos(&self, token_id: u32) -> bool {
        self.eos_token_ids.contains(&token_id)
    }

    #[must_use]
    pub const fn is_mapped_token(token_id: u32) -> bool {
        token_id < TOKENIZER_VOCABULARY
    }

    #[must_use]
    pub const fn is_padding_token(token_id: u32) -> bool {
        token_id >= TOKENIZER_VOCABULARY && token_id < MODEL_VOCABULARY
    }

    fn token_bytes(&self, token_id: u32) -> Result<Option<&[u8]>, TokenizerError> {
        match self.outputs.get(token_id as usize) {
            Some(TokenOutput::Bytes(bytes)) => Ok(Some(bytes)),
            Some(TokenOutput::Special) => Ok(None),
            Some(TokenOutput::Invalid) | None => Err(TokenizerError::UnmappedToken(token_id)),
        }
    }

    fn validate_tokens(&self, tokens: &[u32]) -> Result<(), TokenizerError> {
        if let Some(&token) = tokens.iter().find(|&&token| !Self::is_mapped_token(token)) {
            return Err(TokenizerError::UnmappedToken(token));
        }
        Ok(())
    }
}

fn validate_configuration(root: &Path) -> Result<(), TokenizerError> {
    #[derive(Deserialize)]
    struct GenerationConfig {
        eos_token_id: Vec<u32>,
        pad_token_id: u32,
    }
    #[derive(Deserialize)]
    struct TokenizerConfig {
        backend: String,
        clean_up_tokenization_spaces: bool,
        model_max_length: u32,
        pad_token: String,
        padding_side: String,
        tokenizer_class: String,
    }
    let generation: GenerationConfig =
        serde_json::from_slice(&fs::read(root.join("generation_config.json"))?)
            .map_err(|error| TokenizerError::Json(error.to_string()))?;
    if generation.eos_token_id != EOS_TOKEN_IDS || generation.pad_token_id != EOS_TOKEN_IDS[0] {
        return Err(TokenizerError::Configuration);
    }
    let config: TokenizerConfig =
        serde_json::from_slice(&fs::read(root.join("tokenizer_config.json"))?)
            .map_err(|error| TokenizerError::Json(error.to_string()))?;
    if config.backend != "tokenizers"
        || config.clean_up_tokenization_spaces
        || config.model_max_length != 1_048_576
        || config.pad_token != "<|endoftext|>"
        || config.padding_side != "left"
        || config.tokenizer_class != "TokenizersBackend"
    {
        return Err(TokenizerError::Configuration);
    }
    Ok(())
}

fn build_outputs(tokenizer: &Tokenizer) -> Result<Vec<TokenOutput>, TokenizerError> {
    let special: BTreeSet<_> = SPECIAL_TOKEN_IDS.into_iter().collect();
    let byte_decoder = byte_decoder();
    let mut outputs = Vec::with_capacity(MODEL_VOCABULARY as usize);
    for token_id in 0..MODEL_VOCABULARY {
        if special.contains(&token_id) {
            outputs.push(TokenOutput::Special);
            continue;
        }
        let Some(token) = tokenizer.id_to_token(token_id) else {
            outputs.push(TokenOutput::Invalid);
            continue;
        };
        if token_id >= BASE_VOCABULARY {
            outputs.push(TokenOutput::Bytes(token.into_bytes().into_boxed_slice()));
            continue;
        }
        let mut bytes = Vec::with_capacity(token.len());
        for character in token.chars() {
            let code = character as usize;
            let byte = byte_decoder
                .get(code)
                .copied()
                .flatten()
                .ok_or(TokenizerError::Vocabulary)?;
            bytes.push(byte);
        }
        outputs.push(TokenOutput::Bytes(bytes.into_boxed_slice()));
    }
    if outputs[..TOKENIZER_VOCABULARY as usize]
        .iter()
        .any(|output| matches!(output, TokenOutput::Invalid))
        || outputs[TOKENIZER_VOCABULARY as usize..]
            .iter()
            .any(|output| !matches!(output, TokenOutput::Invalid))
    {
        return Err(TokenizerError::Vocabulary);
    }
    Ok(outputs)
}

fn hash_output_table(outputs: &[TokenOutput]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(b"glmaxx-token-output-table-v1\0");
    for (token_id, output) in outputs.iter().enumerate() {
        hasher.update(u32::try_from(token_id).unwrap_or(u32::MAX).to_le_bytes());
        match output {
            TokenOutput::Bytes(bytes) => {
                hasher.update(b"B");
                hasher.update(u32::try_from(bytes.len()).unwrap_or(u32::MAX).to_le_bytes());
                hasher.update(bytes);
            }
            TokenOutput::Special => hasher.update(b"S"),
            TokenOutput::Invalid => hasher.update(b"I"),
        }
    }
    hasher.finalize().into()
}

fn byte_decoder() -> Vec<Option<u8>> {
    let mut bytes = Vec::new();
    bytes.extend(33_u16..=126);
    bytes.extend(161_u16..=172);
    bytes.extend(174_u16..=255);
    let mut codepoints = bytes.clone();
    let mut extra = 0_u16;
    for byte in 0_u16..=255 {
        if !bytes.contains(&byte) {
            bytes.push(byte);
            codepoints.push(256 + extra);
            extra += 1;
        }
    }
    let maximum = codepoints.iter().copied().max().unwrap_or(0) as usize;
    let mut decoder = vec![None; maximum + 1];
    for (byte, codepoint) in bytes.into_iter().zip(codepoints) {
        decoder[codepoint as usize] = Some(u8::try_from(byte).expect("byte domain"));
    }
    decoder
}

fn verify_file(
    path: &Path,
    expected_bytes: usize,
    expected: [u8; 32],
) -> Result<(), TokenizerError> {
    let path_metadata = fs::symlink_metadata(path)?;
    if !path_metadata.file_type().is_file()
        || path_metadata.file_type().is_symlink()
        || path_metadata.len() != expected_bytes as u64
    {
        return Err(TokenizerError::Bundle);
    }
    let mut file = File::open(path)?;
    let opened_metadata = file.metadata()?;
    if !opened_metadata.file_type().is_file()
        || opened_metadata.dev() != path_metadata.dev()
        || opened_metadata.ino() != path_metadata.ino()
    {
        return Err(TokenizerError::Bundle);
    }
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 1 << 20];
    let mut total = 0_usize;
    loop {
        let bytes = file.read(&mut buffer)?;
        if bytes == 0 {
            break;
        }
        total = total.checked_add(bytes).ok_or(TokenizerError::Bundle)?;
        hasher.update(&buffer[..bytes]);
    }
    if total != expected_bytes || <[u8; 32]>::from(hasher.finalize()) != expected {
        return Err(TokenizerError::Hash);
    }
    Ok(())
}

const fn hex_array(value: &str) -> [u8; 32] {
    let bytes = value.as_bytes();
    let mut output = [0_u8; 32];
    let mut index = 0;
    while index < output.len() {
        output[index] = (hex_nibble(bytes[index * 2]) << 4) | hex_nibble(bytes[index * 2 + 1]);
        index += 1;
    }
    output
}

const fn hex_nibble(value: u8) -> u8 {
    match value {
        b'0'..=b'9' => value - b'0',
        b'a'..=b'f' => value - b'a' + 10,
        _ => panic!("invalid pinned hex"),
    }
}

#[derive(Debug)]
pub enum TokenizerError {
    Bundle,
    Hash,
    Configuration,
    Vocabulary,
    Stops,
    Decode,
    StreamFinished,
    UnmappedToken(u32),
    Io(std::io::Error),
    Json(String),
    Library(String),
}

impl fmt::Display for TokenizerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for TokenizerError {}

impl From<std::io::Error> for TokenizerError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

#[cfg(test)]
fn test_tokenizer(outputs: Vec<TokenOutput>, eos_token_ids: &[u32]) -> Arc<PinnedTokenizer> {
    let tokenizer = Tokenizer::new(tokenizers::models::wordlevel::WordLevel::default());
    let mut all = outputs;
    if all.is_empty() {
        all.push(TokenOutput::Special);
    }
    Arc::new(PinnedTokenizer {
        tokenizer,
        outputs: all,
        eos_token_ids: eos_token_ids.iter().copied().collect(),
        root: PathBuf::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn byte_decoder_is_a_bijection_over_every_byte() {
        let decoder = byte_decoder();
        let mut seen = [false; 256];
        for byte in decoder.into_iter().flatten() {
            seen[byte as usize] = true;
        }
        assert!(seen.into_iter().all(|value| value));
    }

    #[test]
    fn model_padding_ids_are_never_tokenizer_output() {
        assert!(PinnedTokenizer::is_mapped_token(154_855));
        assert!(PinnedTokenizer::is_padding_token(154_856));
        assert!(PinnedTokenizer::is_padding_token(154_879));
        assert!(!PinnedTokenizer::is_padding_token(154_880));
    }

    #[test]
    fn bundle_files_require_exact_regular_file_identity() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "glmaxx-tokenizer-file-test-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir(&root).unwrap();
        let file = root.join("tokenizer.json");
        fs::write(&file, b"abc").unwrap();
        assert!(verify_file(&file, 3, Sha256::digest(b"abc").into()).is_ok());
        assert!(matches!(
            verify_file(&file, 3, Sha256::digest(b"abd").into()),
            Err(TokenizerError::Hash)
        ));
        let link = root.join("link");
        std::os::unix::fs::symlink(&file, &link).unwrap();
        assert!(matches!(
            verify_file(&link, 3, Sha256::digest(b"abc").into()),
            Err(TokenizerError::Bundle)
        ));
        fs::remove_file(link).unwrap();
        fs::remove_file(file).unwrap();
        fs::remove_dir(root).unwrap();
    }
}
