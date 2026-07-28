use glm_format::{
    Codec, PackedNvfp4, decode_e2m1, decode_e4m3, encode_e2m1, encode_e4m3, scale_offset,
};

#[test]
fn exhaustive_numeric_classes() {
    for code in 0_u8..16 {
        let value = decode_e2m1(code);
        let encoded = encode_e2m1(value).unwrap();
        assert_eq!(encoded, if value == 0.0 { 0 } else { code });
    }
    for code in 0_u8..=0xfe {
        if code == 0x7f || code == 0xff {
            continue;
        }
        let value = decode_e4m3(code);
        let encoded = encode_e4m3(value).unwrap();
        assert_eq!(encoded, if value == 0.0 { 0 } else { code });
    }
}

#[test]
fn randomized_and_adversarial_pack_is_deterministic() {
    let mut state = 0x6a09_e667_f3bc_c909_u64;
    let mut values = Vec::with_capacity(257 * 193);
    for index in 0..257 * 193 {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        let base = ((state >> 40) as i32 - (1 << 23)) as f32 / (1 << 20) as f32;
        let value = match index % 97 {
            0 => 0.0,
            1 => f32::from_bits(1),
            2 => 448.0 * 6.0,
            3 => -448.0 * 6.0,
            _ => base,
        };
        values.push(value);
    }
    for codec in [Codec::OneDimensional, Codec::TwoDimensional] {
        let first = PackedNvfp4::pack(&values, 257, 193, codec).unwrap();
        let second = PackedNvfp4::pack(&values, 257, 193, codec).unwrap();
        assert_eq!(first, second);
        assert!(
            first
                .dequantize()
                .unwrap()
                .iter()
                .all(|value| value.is_finite())
        );
    }
}

#[test]
fn swizzle_matches_closed_form_examples() {
    assert_eq!(scale_offset(0, 0, 128, 64).unwrap(), 0);
    assert_eq!(scale_offset(1, 0, 128, 64).unwrap(), 16);
    assert_eq!(scale_offset(32, 0, 128, 64).unwrap(), 4);
    assert_eq!(scale_offset(127, 3, 128, 64).unwrap(), 511);
    assert_eq!(scale_offset(128, 0, 256, 64).unwrap(), 512);
}
