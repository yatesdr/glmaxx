use std::sync::Arc;

use crate::{PinnedTokenizer, TokenizerError};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StreamFinish {
    EosToken(u32),
    StopString(usize),
    EndOfStream,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodeDelta {
    pub text: String,
    pub finish: Option<StreamFinish>,
}

pub struct IncrementalDecoder {
    tokenizer: Arc<PinnedTokenizer>,
    stops: Vec<String>,
    ignore_eos: bool,
    pending_utf8: Vec<u8>,
    held_text: String,
    finish: Option<StreamFinish>,
}

impl IncrementalDecoder {
    pub(crate) fn new(
        tokenizer: Arc<PinnedTokenizer>,
        stops: Vec<String>,
    ) -> Result<Self, TokenizerError> {
        Self::new_with_eos_policy(tokenizer, stops, false)
    }

    pub(crate) fn new_with_eos_policy(
        tokenizer: Arc<PinnedTokenizer>,
        stops: Vec<String>,
        ignore_eos: bool,
    ) -> Result<Self, TokenizerError> {
        if stops.len() > 16
            || stops
                .iter()
                .any(|stop| stop.is_empty() || stop.len() > 256 || stop.contains('\0'))
        {
            return Err(TokenizerError::Stops);
        }
        Ok(Self {
            tokenizer,
            stops,
            ignore_eos,
            pending_utf8: Vec::with_capacity(8),
            held_text: String::new(),
            finish: None,
        })
    }

    pub fn push(&mut self, token_id: u32) -> Result<DecodeDelta, TokenizerError> {
        if self.finish.is_some() {
            return Err(TokenizerError::StreamFinished);
        }
        if self.tokenizer.is_eos(token_id) && !self.ignore_eos {
            let mut decoded = self.decode_utf8(true);
            decoded.push_str(&self.release_held());
            let finish = StreamFinish::EosToken(token_id);
            self.finish = Some(finish.clone());
            return Ok(DecodeDelta {
                text: decoded,
                finish: Some(finish),
            });
        }
        if let Some(bytes) = self.tokenizer.token_bytes(token_id)? {
            self.pending_utf8.extend_from_slice(bytes);
        }
        let decoded = self.decode_utf8(false);
        self.apply_stops(&decoded)
    }

    pub fn finish(&mut self) -> Result<DecodeDelta, TokenizerError> {
        if self.finish.is_some() {
            return Err(TokenizerError::StreamFinished);
        }
        let mut text = self.decode_utf8(true);
        let stop_delta = self.apply_stops(&text)?;
        text = stop_delta.text;
        if let Some(finish) = stop_delta.finish {
            return Ok(DecodeDelta {
                text,
                finish: Some(finish),
            });
        }
        text.push_str(&self.release_held());
        let finish = StreamFinish::EndOfStream;
        self.finish = Some(finish.clone());
        Ok(DecodeDelta {
            text,
            finish: Some(finish),
        })
    }

    fn decode_utf8(&mut self, final_chunk: bool) -> String {
        let mut output = String::new();
        loop {
            match std::str::from_utf8(&self.pending_utf8) {
                Ok(text) => {
                    output.push_str(text);
                    self.pending_utf8.clear();
                    break;
                }
                Err(error) => {
                    let valid = error.valid_up_to();
                    if valid != 0 {
                        let text = std::str::from_utf8(&self.pending_utf8[..valid])
                            .expect("valid_up_to must identify UTF-8");
                        output.push_str(text);
                        self.pending_utf8.drain(..valid);
                    }
                    match error.error_len() {
                        Some(invalid) => {
                            output.push('\u{fffd}');
                            self.pending_utf8.drain(..invalid);
                        }
                        None if final_chunk => {
                            output.push('\u{fffd}');
                            self.pending_utf8.clear();
                            break;
                        }
                        None => break,
                    }
                }
            }
        }
        output
    }

    fn apply_stops(&mut self, decoded: &str) -> Result<DecodeDelta, TokenizerError> {
        self.held_text.push_str(decoded);
        let match_result = self
            .stops
            .iter()
            .enumerate()
            .filter_map(|(index, stop)| {
                self.held_text
                    .find(stop)
                    .map(|offset| (offset, index, stop.len()))
            })
            .min_by_key(|&(offset, index, _)| (offset, index));
        if let Some((offset, stop_index, _)) = match_result {
            let text = self.held_text[..offset].to_owned();
            self.held_text.clear();
            self.pending_utf8.clear();
            let finish = StreamFinish::StopString(stop_index);
            self.finish = Some(finish.clone());
            return Ok(DecodeDelta {
                text,
                finish: Some(finish),
            });
        }
        let retained = self.longest_stop_prefix_suffix();
        let emitted_bytes = self
            .held_text
            .len()
            .checked_sub(retained)
            .ok_or(TokenizerError::Decode)?;
        if !self.held_text.is_char_boundary(emitted_bytes) {
            return Err(TokenizerError::Decode);
        }
        let retained_text = self.held_text.split_off(emitted_bytes);
        let text = std::mem::replace(&mut self.held_text, retained_text);
        Ok(DecodeDelta { text, finish: None })
    }

    fn longest_stop_prefix_suffix(&self) -> usize {
        self.stops
            .iter()
            .map(|stop| {
                let maximum = self.held_text.len().min(stop.len());
                (1..=maximum)
                    .rev()
                    .find(|&length| {
                        stop.is_char_boundary(length)
                            && self
                                .held_text
                                .as_bytes()
                                .ends_with(&stop.as_bytes()[..length])
                    })
                    .unwrap_or(0)
            })
            .max()
            .unwrap_or(0)
    }

    fn release_held(&mut self) -> String {
        std::mem::take(&mut self.held_text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{TokenOutput, test_tokenizer};

    #[test]
    fn utf8_and_stop_strings_span_arbitrary_token_boundaries() {
        let tokenizer = test_tokenizer(
            vec![
                TokenOutput::Bytes(b"hello ST".to_vec().into_boxed_slice()),
                TokenOutput::Bytes(b"OP hidden".to_vec().into_boxed_slice()),
            ],
            &[],
        );
        let mut decoder = IncrementalDecoder::new(tokenizer, vec!["STOP".to_owned()]).unwrap();
        assert_eq!(
            decoder.push(0).unwrap(),
            DecodeDelta {
                text: "hello ".to_owned(),
                finish: None
            }
        );
        assert_eq!(
            decoder.push(1).unwrap(),
            DecodeDelta {
                text: String::new(),
                finish: Some(StreamFinish::StopString(0))
            }
        );
    }

    #[test]
    fn split_utf8_is_held_until_complete_and_incomplete_final_is_lossy() {
        let tokenizer = test_tokenizer(
            vec![
                TokenOutput::Bytes(vec![0xe5, 0x8c].into_boxed_slice()),
                TokenOutput::Bytes(vec![0x97].into_boxed_slice()),
                TokenOutput::Bytes(vec![0xe5].into_boxed_slice()),
            ],
            &[],
        );
        let mut decoder = IncrementalDecoder::new(Arc::clone(&tokenizer), Vec::new()).unwrap();
        assert_eq!(decoder.push(0).unwrap().text, "");
        assert_eq!(decoder.push(1).unwrap().text, "北");
        assert_eq!(decoder.push(2).unwrap().text, "");
        assert_eq!(decoder.finish().unwrap().text, "\u{fffd}");
    }

    #[test]
    fn eos_releases_a_partial_stop_prefix_but_emits_no_special_text() {
        let tokenizer = test_tokenizer(
            vec![
                TokenOutput::Bytes(b"tail ST".to_vec().into_boxed_slice()),
                TokenOutput::Special,
            ],
            &[1],
        );
        let mut decoder = IncrementalDecoder::new(tokenizer, vec!["STOP".to_owned()]).unwrap();
        assert_eq!(decoder.push(0).unwrap().text, "tail ");
        assert_eq!(
            decoder.push(1).unwrap(),
            DecodeDelta {
                text: "ST".to_owned(),
                finish: Some(StreamFinish::EosToken(1))
            }
        );
    }

    #[test]
    fn ignored_eos_does_not_finish_the_stream() {
        let tokenizer = test_tokenizer(
            vec![
                TokenOutput::Bytes(b"after".to_vec().into_boxed_slice()),
                TokenOutput::Special,
            ],
            &[1],
        );
        let mut decoder =
            IncrementalDecoder::new_with_eos_policy(tokenizer, Vec::new(), true).unwrap();
        assert_eq!(
            decoder.push(1).unwrap(),
            DecodeDelta {
                text: String::new(),
                finish: None
            }
        );
        assert_eq!(decoder.push(0).unwrap().text, "after");
        assert_eq!(
            decoder.finish().unwrap().finish,
            Some(StreamFinish::EndOfStream)
        );
    }
}
