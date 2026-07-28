use crate::Nvfp4Error;

const E2M1_POSITIVE: [f32; 8] = [0.0, 0.5, 1.0, 1.5, 2.0, 3.0, 4.0, 6.0];

#[must_use]
pub fn decode_e2m1(code: u8) -> f32 {
    let magnitude = E2M1_POSITIVE[usize::from(code & 0x07)];
    if code & 0x08 == 0 {
        magnitude
    } else {
        -magnitude
    }
}

pub fn encode_e2m1(value: f32) -> Result<u8, Nvfp4Error> {
    if !value.is_finite() {
        return Err(Nvfp4Error::NonFinite);
    }
    if value == 0.0 {
        return Ok(0);
    }
    let sign = if value.is_sign_negative() { 0x08 } else { 0 };
    let magnitude = value.abs();
    let mut best = 0_u8;
    let mut best_distance = f32::INFINITY;
    for code in 0_u8..=7 {
        let distance = (magnitude - E2M1_POSITIVE[usize::from(code)]).abs();
        if distance < best_distance || (distance == best_distance && code & 1 == 0 && best & 1 == 1)
        {
            best = code;
            best_distance = distance;
        }
    }
    Ok(sign | best)
}

#[must_use]
pub fn decode_e4m3(code: u8) -> f32 {
    let sign = if code & 0x80 == 0 { 1.0 } else { -1.0 };
    let exponent = (code >> 3) & 0x0f;
    let mantissa = code & 0x07;
    let magnitude = if exponent == 0 {
        f32::from(mantissa) * 2.0_f32.powi(-9)
    } else if exponent == 15 && mantissa == 7 {
        f32::NAN
    } else {
        (1.0 + f32::from(mantissa) / 8.0) * 2.0_f32.powi(i32::from(exponent) - 7)
    };
    sign * magnitude
}

pub fn encode_e4m3(value: f32) -> Result<u8, Nvfp4Error> {
    if !value.is_finite() {
        return Err(Nvfp4Error::NonFinite);
    }
    if value == 0.0 {
        return Ok(0);
    }
    let sign = if value.is_sign_negative() { 0x80 } else { 0 };
    let magnitude = value.abs();
    let mut best = 0_u8;
    let mut best_distance = f32::INFINITY;
    for code in 0_u8..=0x7e {
        let candidate = decode_e4m3(code);
        let distance = (magnitude - candidate).abs();
        if distance < best_distance || (distance == best_distance && code & 1 == 0 && best & 1 == 1)
        {
            best = code;
            best_distance = distance;
        }
    }
    Ok(sign | best)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn e2m1_all_codes_decode_and_reencode() {
        for code in 0_u8..16 {
            let value = decode_e2m1(code);
            let encoded = encode_e2m1(value).unwrap();
            if value == 0.0 {
                assert_eq!(encoded, 0);
            } else {
                assert_eq!(encoded, code);
            }
        }
    }

    #[test]
    fn e2m1_ties_are_even() {
        let cases = [
            (0.25, 0),
            (0.75, 2),
            (1.25, 2),
            (1.75, 4),
            (2.5, 4),
            (3.5, 6),
            (5.0, 6),
        ];
        for (value, expected) in cases {
            assert_eq!(encode_e2m1(value).unwrap(), expected);
        }
    }

    #[test]
    fn e4m3_finite_codes_round_trip() {
        for code in 0_u8..=0x7e {
            assert_eq!(encode_e4m3(decode_e4m3(code)).unwrap(), code);
        }
        for code in 0x80_u8..=0xfe {
            if code == 0x80 {
                assert_eq!(encode_e4m3(decode_e4m3(code)).unwrap(), 0);
            } else {
                assert_eq!(encode_e4m3(decode_e4m3(code)).unwrap(), code);
            }
        }
        assert_eq!(decode_e4m3(0x7e), 448.0);
    }
}
