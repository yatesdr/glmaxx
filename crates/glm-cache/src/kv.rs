use std::fmt;

use glm_format::{decode_e2m1, decode_e4m3, encode_e2m1, encode_e4m3};

const F32_EXPONENT_MASK: u32 = 0x7f80_0000;
const F32_FRACTION_MASK: u32 = 0x007f_ffff;
const F32_EXPONENT_STEP: u32 = 0x0080_0000;

fn ceil_positive_power_of_two(value: f32) -> Result<f32, KvError> {
    if !value.is_finite() || value <= 0.0 {
        return Err(KvError::Scale);
    }

    let bits = value.to_bits();
    let exponent = bits & F32_EXPONENT_MASK;
    let fraction = bits & F32_FRACTION_MASK;
    if exponent == 0 {
        // Positive subnormals are integer multiples of 2^-149. The next
        // power-of-two coefficient is therefore also the exact next
        // representable power of two; 1 << 23 is the minimum normal value.
        return Ok(f32::from_bits(fraction.next_power_of_two()));
    }
    if fraction == 0 {
        return Ok(value);
    }

    let next_exponent = exponent + F32_EXPONENT_STEP;
    if next_exponent == F32_EXPONENT_MASK {
        return Err(KvError::Scale);
    }
    Ok(f32::from_bits(next_exponent))
}

fn is_positive_finite_power_of_two(value: f32) -> bool {
    if !value.is_finite() || value <= 0.0 {
        return false;
    }
    let bits = value.to_bits();
    let exponent = bits & F32_EXPONENT_MASK;
    let fraction = bits & F32_FRACTION_MASK;
    if exponent == 0 {
        fraction.is_power_of_two()
    } else {
        fraction == 0
    }
}

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
            if !scale.is_finite() {
                return Err(KvError::NonFinite);
            }
            for lane in 0..16 {
                let code = if scale_code == 0 {
                    0
                } else {
                    encode_e2m1(nope[start + lane] / scale).map_err(|_| KvError::NonFinite)?
                };
                if !(decode_e2m1(code) * scale).is_finite() {
                    return Err(KvError::NonFinite);
                }
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
            let code = if value == 0.0 {
                0
            } else {
                encode_e4m3(value / rope_scale).map_err(|_| KvError::NonFinite)?
            };
            if !(decode_e4m3(code) * rope_scale).is_finite() {
                return Err(KvError::NonFinite);
            }
            bytes[304 + index] = code;
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
            let group_scale = decode_e4m3(self.0[256 + index / 16]) * outer;
            let restored = decode_e2m1(code) * group_scale;
            if !group_scale.is_finite() || !restored.is_finite() {
                return Err(KvError::Encoding);
            }
            *value = restored;
        }
        let mut rope = [0.0_f32; 64];
        for (index, value) in rope.iter_mut().enumerate() {
            let decoded = decode_e4m3(self.0[304 + index]);
            if !decoded.is_finite() {
                return Err(KvError::Encoding);
            }
            let restored = decoded * rope_scale;
            if !restored.is_finite() {
                return Err(KvError::Encoding);
            }
            *value = restored;
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
        let scale = ceil_positive_power_of_two(raw_scale)?;
        let mut bytes = [0_u8; 132];
        for (index, &value) in key.iter().enumerate() {
            let code = encode_e4m3(value / scale).map_err(|_| KvError::NonFinite)?;
            if !(decode_e4m3(code) * scale).is_finite() {
                return Err(KvError::NonFinite);
            }
            bytes[index] = code;
        }
        bytes[128..132].copy_from_slice(&scale.to_le_bytes());
        Ok(Self(bytes))
    }

    pub fn decode(&self) -> Result<[f32; 128], KvError> {
        let scale = f32::from_le_bytes(self.0[128..132].try_into().unwrap());
        if !is_positive_finite_power_of_two(scale) {
            return Err(KvError::Scale);
        }
        let mut output = [0.0_f32; 128];
        for (index, value) in output.iter_mut().enumerate() {
            let decoded = decode_e4m3(self.0[index]);
            if !decoded.is_finite() {
                return Err(KvError::Encoding);
            }
            let restored = decoded * scale;
            if !restored.is_finite() {
                return Err(KvError::Encoding);
            }
            *value = restored;
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
        assert!(is_positive_finite_power_of_two(scale));
        assert!(
            record
                .decode()
                .unwrap()
                .iter()
                .all(|value| value.is_finite())
        );
    }

    #[test]
    fn power_of_two_ceiling_is_exact_at_every_f32_exponent_boundary() {
        for shift in 0..23 {
            let exact = f32::from_bits(1 << shift);
            assert_eq!(ceil_positive_power_of_two(exact), Ok(exact));
            if shift > 1 {
                let below = f32::from_bits((1 << shift) - 1);
                assert_eq!(ceil_positive_power_of_two(below), Ok(exact));
            }
            let above = f32::from_bits((1 << shift) + 1);
            let expected = f32::from_bits(1 << (shift + 1));
            assert_eq!(ceil_positive_power_of_two(above), Ok(expected));
        }

        for exponent in 1..=254_u32 {
            let exact_bits = exponent << 23;
            let exact = f32::from_bits(exact_bits);
            assert_eq!(ceil_positive_power_of_two(exact), Ok(exact));
            assert_eq!(
                ceil_positive_power_of_two(f32::from_bits(exact_bits - 1)),
                Ok(exact)
            );
            let above = f32::from_bits(exact_bits + 1);
            if exponent == 254 {
                assert_eq!(ceil_positive_power_of_two(above), Err(KvError::Scale));
            } else {
                assert_eq!(
                    ceil_positive_power_of_two(above),
                    Ok(f32::from_bits((exponent + 1) << 23))
                );
            }
        }
    }

    #[test]
    fn power_of_two_validation_is_bit_exact_and_fail_closed() {
        for shift in 0..23 {
            assert!(is_positive_finite_power_of_two(f32::from_bits(1 << shift)));
        }
        for exponent in 1..=254_u32 {
            assert!(is_positive_finite_power_of_two(f32::from_bits(
                exponent << 23
            )));
            assert!(!is_positive_finite_power_of_two(f32::from_bits(
                (exponent << 23) | 1
            )));
        }
        for invalid in [
            0.0,
            -0.0,
            -1.0,
            1.5,
            f32::INFINITY,
            f32::NEG_INFINITY,
            f32::NAN,
        ] {
            assert!(!is_positive_finite_power_of_two(invalid));
        }

        let mut record = IndexerKeyRecord::encode(&[0.0; 128]).unwrap();
        record.0[128..132].copy_from_slice(&1.5_f32.to_le_bytes());
        assert_eq!(record.decode(), Err(KvError::Scale));
    }

    #[test]
    fn indexer_extreme_finite_key_fails_closed_before_nonfinite_restore() {
        let raw_scale = f32::MAX / 448.0;
        let scale = ceil_positive_power_of_two(raw_scale).unwrap();
        assert!(scale.is_finite());
        assert!(scale >= raw_scale);

        let mut key = [0.0; 128];
        key[0] = f32::MAX;
        key[127] = -f32::MAX;
        assert_eq!(IndexerKeyRecord::encode(&key), Err(KvError::NonFinite));

        let mut corrupt = IndexerKeyRecord([0; 132]);
        corrupt.0[0] = encode_e4m3(448.0).unwrap();
        corrupt.0[128..132].copy_from_slice(&2.0_f32.powi(127).to_le_bytes());
        assert_eq!(corrupt.decode(), Err(KvError::Encoding));
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
    fn finite_kv_factors_cannot_reconstruct_nonfinite_values() {
        let mut bad_nope = KvRecord::encode(&[0.0; 512], &[0.0; 64]).unwrap();
        bad_nope.0[0] = 0x07;
        bad_nope.0[256] = encode_e4m3(448.0).unwrap();
        bad_nope.0[292..296].copy_from_slice(&f32::MAX.to_le_bytes());
        assert_eq!(bad_nope.decode(), Err(KvError::Encoding));

        let mut bad_rope = KvRecord::encode(&[0.0; 512], &[0.0; 64]).unwrap();
        bad_rope.0[288..292].copy_from_slice(&f32::MAX.to_le_bytes());
        bad_rope.0[304] = encode_e4m3(448.0).unwrap();
        assert_eq!(bad_rope.decode(), Err(KvError::Encoding));

        let mut nope = [0.0; 512];
        nope[0] = f32::MAX;
        nope[511] = -f32::MAX;
        let mut rope = [0.0; 64];
        rope[0] = f32::MAX;
        rope[63] = -f32::MAX;
        let record = KvRecord::encode(&nope, &rope).unwrap();
        let (decoded_nope, decoded_rope) = record.decode().unwrap();
        assert!(
            decoded_nope
                .iter()
                .chain(&decoded_rope)
                .all(|value| value.is_finite())
        );
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
