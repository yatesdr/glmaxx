use std::fmt;

use glm_format::{decode_e2m1, decode_e4m3, encode_e2m1, encode_e4m3};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KvRecord(pub [u8; 368]);

impl KvRecord {
    pub fn encode(nope: &[f32; 512], rope: &[f32; 64]) -> Result<Self, KvError> {
        if nope.iter().chain(rope).any(|value| !value.is_finite()) {
            return Err(KvError::NonFinite);
        }
        let mut bytes = [0_u8; 368];
        let nope_amax = nope
            .iter()
            .fold(0.0_f32, |amax, value| amax.max(value.abs()));
        let outer = if nope_amax == 0.0 {
            1.0
        } else {
            nope_amax / (6.0 * 448.0)
        };
        for group in 0..32 {
            let start = group * 16;
            let amax = nope[start..start + 16]
                .iter()
                .fold(0.0_f32, |current, value| current.max(value.abs()));
            let scale_code = if amax == 0.0 {
                0
            } else {
                encode_e4m3((amax / 6.0) / outer).map_err(|_| KvError::NonFinite)?
            };
            bytes[256 + group] = scale_code;
            let scale = decode_e4m3(scale_code) * outer;
            for lane in 0..16 {
                let code = if scale_code == 0 {
                    0
                } else {
                    encode_e2m1(nope[start + lane] / scale).map_err(|_| KvError::NonFinite)?
                };
                let index = start + lane;
                if index & 1 == 0 {
                    bytes[index / 2] = code;
                } else {
                    bytes[index / 2] |= code << 4;
                }
            }
        }
        bytes[292..296].copy_from_slice(&outer.to_le_bytes());

        let rope_amax = rope
            .iter()
            .fold(0.0_f32, |amax, value| amax.max(value.abs()));
        let rope_scale = if rope_amax == 0.0 {
            1.0
        } else {
            rope_amax / 448.0
        };
        bytes[288..292].copy_from_slice(&rope_scale.to_le_bytes());
        for (index, &value) in rope.iter().enumerate() {
            bytes[304 + index] = if value == 0.0 {
                0
            } else {
                encode_e4m3(value / rope_scale).map_err(|_| KvError::NonFinite)?
            };
        }
        Ok(Self(bytes))
    }

    pub fn decode(&self) -> Result<([f32; 512], [f32; 64]), KvError> {
        if self.0[296..304].iter().any(|&value| value != 0) {
            return Err(KvError::Padding);
        }
        let outer = f32::from_le_bytes(self.0[292..296].try_into().unwrap());
        let rope_scale = f32::from_le_bytes(self.0[288..292].try_into().unwrap());
        if !outer.is_finite() || outer <= 0.0 || !rope_scale.is_finite() || rope_scale <= 0.0 {
            return Err(KvError::Scale);
        }
        if self.0[256..288]
            .iter()
            .any(|&code| code & 0x80 != 0 || !decode_e4m3(code).is_finite())
        {
            return Err(KvError::Encoding);
        }
        let mut nope = [0.0_f32; 512];
        for (index, value) in nope.iter_mut().enumerate() {
            let byte = self.0[index / 2];
            let code = if index & 1 == 0 {
                byte & 0x0f
            } else {
                byte >> 4
            };
            *value = decode_e2m1(code) * decode_e4m3(self.0[256 + index / 16]) * outer;
        }
        let mut rope = [0.0_f32; 64];
        for (index, value) in rope.iter_mut().enumerate() {
            let decoded = decode_e4m3(self.0[304 + index]);
            if !decoded.is_finite() {
                return Err(KvError::Encoding);
            }
            *value = decoded * rope_scale;
        }
        Ok((nope, rope))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IndexerKeyRecord(pub [u8; 132]);

impl IndexerKeyRecord {
    pub fn encode(key: &[f32; 128]) -> Result<Self, KvError> {
        if key.iter().any(|value| !value.is_finite()) {
            return Err(KvError::NonFinite);
        }
        let amax = key
            .iter()
            .fold(0.0_f32, |current, value| current.max(value.abs()));
        let raw_scale = amax.max(1.0e-4) / 448.0;
        let scale = 2.0_f32.powf(raw_scale.log2().ceil());
        let mut bytes = [0_u8; 132];
        for (index, &value) in key.iter().enumerate() {
            bytes[index] = encode_e4m3(value / scale).map_err(|_| KvError::NonFinite)?;
        }
        bytes[128..132].copy_from_slice(&scale.to_le_bytes());
        Ok(Self(bytes))
    }

    pub fn decode(&self) -> Result<[f32; 128], KvError> {
        let scale = f32::from_le_bytes(self.0[128..132].try_into().unwrap());
        if !scale.is_finite() || scale <= 0.0 || scale.log2().fract() != 0.0 {
            return Err(KvError::Scale);
        }
        let mut output = [0.0_f32; 128];
        for (index, value) in output.iter_mut().enumerate() {
            let decoded = decode_e4m3(self.0[index]);
            if !decoded.is_finite() {
                return Err(KvError::Encoding);
            }
            *value = decoded * scale;
        }
        Ok(output)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KvError {
    NonFinite,
    Padding,
    Scale,
    Encoding,
}

impl fmt::Display for KvError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for KvError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_zero_kv_is_canonical() {
        let record = KvRecord::encode(&[0.0; 512], &[0.0; 64]).unwrap();
        assert!(record.0[..288].iter().all(|&value| value == 0));
        assert_eq!(&record.0[288..292], &1.0_f32.to_le_bytes());
        assert_eq!(&record.0[292..296], &1.0_f32.to_le_bytes());
        assert!(record.0[296..].iter().all(|&value| value == 0));
        let (nope, rope) = record.decode().unwrap();
        assert!(nope.iter().chain(&rope).all(|&value| value == 0.0));
    }

    #[test]
    fn indexer_record_has_power_of_two_scale() {
        let key = std::array::from_fn(|index| (index as f32 - 63.0) / 11.0);
        let record = IndexerKeyRecord::encode(&key).unwrap();
        let scale = f32::from_le_bytes(record.0[128..132].try_into().unwrap());
        assert_eq!(scale.log2().fract(), 0.0);
        assert!(
            record
                .decode()
                .unwrap()
                .iter()
                .all(|value| value.is_finite())
        );
    }

    #[test]
    fn patterned_kv_round_trip_is_finite_and_bounded() {
        let nope = std::array::from_fn(|index| {
            let sign = if index % 3 == 0 { -1.0 } else { 1.0 };
            sign * ((index * 37 % 257) as f32 / 17.0)
        });
        let rope = std::array::from_fn(|index| ((index as f32 - 31.5) * 0.03125).sin() * 19.0);
        let record = KvRecord::encode(&nope, &rope).unwrap();
        let rope_scale = f32::from_le_bytes(record.0[288..292].try_into().unwrap());
        let (decoded_nope, decoded_rope) = record.decode().unwrap();
        for (actual, expected) in decoded_nope.iter().zip(nope) {
            assert!(actual.is_finite());
            assert!((actual - expected).abs() <= 4.0);
        }
        for (actual, expected) in decoded_rope.iter().zip(rope) {
            assert!(actual.is_finite());
            assert!((actual - expected).abs() <= 16.0 * rope_scale + f32::EPSILON);
        }
    }

    #[test]
    fn record_corruption_and_nonfinite_input_fail_closed() {
        let mut nope = [0.0; 512];
        nope[0] = f32::INFINITY;
        assert_eq!(KvRecord::encode(&nope, &[0.0; 64]), Err(KvError::NonFinite));

        let mut bad_padding = KvRecord::encode(&[0.0; 512], &[0.0; 64]).unwrap();
        bad_padding.0[300] = 1;
        assert_eq!(bad_padding.decode(), Err(KvError::Padding));

        let mut bad_scale = KvRecord::encode(&[0.0; 512], &[0.0; 64]).unwrap();
        bad_scale.0[256] = 0x7f;
        assert_eq!(bad_scale.decode(), Err(KvError::Encoding));
        bad_scale.0[256] = 0x80;
        assert_eq!(bad_scale.decode(), Err(KvError::Encoding));
    }

    #[test]
    fn full_page_preserves_first_and_last_token_records() {
        let page: Vec<KvRecord> = (0..64)
            .map(|token| {
                let nope = std::array::from_fn(|lane| (token * 512 + lane) as f32 / 1024.0);
                let rope = std::array::from_fn(|lane| (token * 64 + lane) as f32 / 2048.0);
                KvRecord::encode(&nope, &rope).unwrap()
            })
            .collect();
        assert_eq!(page.len() * 368, 23_552);
        assert_ne!(page.first().unwrap(), page.last().unwrap());
        assert!(page.first().unwrap().decode().is_ok());
        assert!(page.last().unwrap().decode().is_ok());
    }
}
