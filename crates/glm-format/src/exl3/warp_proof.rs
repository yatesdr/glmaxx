use serde::Serialize;
use sha2::{Digest, Sha256};

use super::{
    Exl3Error, decode_3inst_f16, decode_native_at, f16_bits_to_f32, f32_to_f16_bits,
    inverse_trellis_slot,
};

const BITS: usize = 3;
const TILE: usize = 16;
const STAGE_TILES: usize = 8;
const WORDS_PER_TILE: usize = 8 * BITS;
const LOAD_THREADS: usize = STAGE_TILES * WORDS_PER_TILE;
const CTA_THREADS: usize = 256;
const PROOF_ROWS: usize = 8;
const STAGE_BYTES: usize = LOAD_THREADS * size_of::<u32>();
const DESIGN_SHA256: &str = "67fb3bcb5b839cc50f3462990f1ef6056ca7c9d991851efc5e668fed9d0b3325";

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Exl3WarpShapeProof {
    pub projection_family: &'static str,
    pub logical_k: u32,
    pub logical_n: u32,
    pub k_tiles: u32,
    pub n_tiles: u32,
    pub stage_iterations: u32,
    pub compared_weights: u64,
    pub scheduled_trellis_bytes: u64,
    pub scalar_weight_sha256: String,
    pub staged_weight_sha256: String,
    pub scalar_projection_f16_sha256: String,
    pub staged_projection_f16_sha256: String,
    pub projection_rows: u8,
    pub bitwise_equal: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Exl3WarpStagingProof {
    pub schema: &'static str,
    pub design_sha256: &'static str,
    pub cta_threads: u16,
    pub load_threads: u16,
    pub idle_load_threads: u16,
    pub stage_tiles: u8,
    pub words_per_tile: u8,
    pub stage_bytes: u16,
    pub rows_proven: [u8; PROOF_ROWS],
    pub active_threads_by_rows: [u16; PROOF_ROWS],
    pub barrier_arrivals_by_rows: [u16; PROOF_ROWS],
    pub load_mapping_sha256: String,
    pub slot_mapping_sha256: String,
    pub shapes: [Exl3WarpShapeProof; 2],
    pub verdict: &'static str,
}

/// Exhaustively proves the accepted EXL3 warp-staging schedule on the CPU.
///
/// The staged path reconstructs its local `(lane, weight)` lookup from the
/// forward scatter, while the scalar path retains the source decoder's
/// inverse mapping. Every weight in both real projection geometries is
/// compared, and both paths accumulate eight deterministic activation rows in
/// the same ascending-K order before the FP16 projection-store boundary.
pub fn prove_exl3_warp_staging_v2() -> Result<Exl3WarpStagingProof, Exl3Error> {
    let (slot_table, slot_mapping_sha256) = build_forward_slot_table()?;
    let (active_threads_by_rows, barrier_arrivals_by_rows) = prove_row_schedule()?;
    let load_mapping_sha256 = prove_load_mapping()?;
    let gate_up = prove_shape("gate-up", 6_144, 512, 0x3a4e_2f15_7c91_b608, &slot_table)?;
    let down = prove_shape("down", 512, 6_144, 0xa276_5cf8_11d0_49eb, &slot_table)?;
    Ok(Exl3WarpStagingProof {
        schema: "glmaxx.exl3-warp-staging-cpu-proof.v2",
        design_sha256: DESIGN_SHA256,
        cta_threads: CTA_THREADS as u16,
        load_threads: LOAD_THREADS as u16,
        idle_load_threads: (CTA_THREADS - LOAD_THREADS) as u16,
        stage_tiles: STAGE_TILES as u8,
        words_per_tile: WORDS_PER_TILE as u8,
        stage_bytes: STAGE_BYTES as u16,
        rows_proven: [1, 2, 3, 4, 5, 6, 7, 8],
        active_threads_by_rows,
        barrier_arrivals_by_rows,
        load_mapping_sha256,
        slot_mapping_sha256,
        shapes: [gate_up, down],
        verdict: "EXHAUSTIVE_STAGED_SOURCE_AND_ASCENDING_K_BITWISE_PASS",
    })
}

fn prove_shape(
    projection_family: &'static str,
    logical_k: usize,
    logical_n: usize,
    seed: u64,
    slot_table: &[[u16; TILE]; TILE],
) -> Result<Exl3WarpShapeProof, Exl3Error> {
    if !logical_k.is_multiple_of(TILE * STAGE_TILES) || !logical_n.is_multiple_of(TILE) {
        return Err(Exl3Error::WarpStageMapping);
    }
    let k_tiles = logical_k / TILE;
    let n_tiles = logical_n / TILE;
    let stage_iterations = k_tiles / STAGE_TILES;
    let source_halves = k_tiles
        .checked_mul(n_tiles)
        .and_then(|tiles| tiles.checked_mul(WORDS_PER_TILE * 2))
        .ok_or(Exl3Error::Overflow)?;
    let trellis = deterministic_trellis(source_halves, seed);
    let source_u32: Vec<u32> = trellis
        .chunks_exact(2)
        .map(|halves| u32::from(halves[0]) | (u32::from(halves[1]) << 16))
        .collect();
    let activations = deterministic_activations(logical_k);
    let mut scalar_weight_hash = Sha256::new();
    let mut staged_weight_hash = Sha256::new();
    let mut scalar_projection_hash = Sha256::new();
    let mut staged_projection_hash = Sha256::new();
    let mut compared_weights = 0_u64;
    let mut scheduled_words = 0_u64;

    for n_tile in 0..n_tiles {
        let mut scalar_accumulators = [[0.0_f32; TILE]; PROOF_ROWS];
        let mut staged_accumulators = [[0.0_f32; TILE]; PROOF_ROWS];
        for stage_iteration in 0..stage_iterations {
            let first_k_tile = stage_iteration * STAGE_TILES;
            let stage = load_stage(&source_u32, n_tiles, first_k_tile, n_tile)?;
            scheduled_words = scheduled_words
                .checked_add(LOAD_THREADS as u64)
                .ok_or(Exl3Error::Overflow)?;
            for (stage_tile, staged_words) in stage.iter().enumerate() {
                let k_tile = first_k_tile + stage_tile;
                for (local_k, slots) in slot_table.iter().enumerate() {
                    let k = k_tile * TILE + local_k;
                    for (local_n, &slot) in slots.iter().enumerate() {
                        let n = n_tile * TILE + local_n;
                        let scalar_bits = decode_native_at(&trellis, logical_n, BITS, k, n);
                        let staged_bits = decode_staged_at(staged_words, slot);
                        scalar_weight_hash.update(scalar_bits.to_le_bytes());
                        staged_weight_hash.update(staged_bits.to_le_bytes());
                        compared_weights =
                            compared_weights.checked_add(1).ok_or(Exl3Error::Overflow)?;
                        if scalar_bits != staged_bits {
                            return Err(Exl3Error::WarpStageMismatch);
                        }
                        let scalar_weight = f16_bits_to_f32(scalar_bits);
                        let staged_weight = f16_bits_to_f32(staged_bits);
                        for row in 0..PROOF_ROWS {
                            scalar_accumulators[row][local_n] = multiply_then_add(
                                scalar_accumulators[row][local_n],
                                activations[row][k],
                                scalar_weight,
                            );
                            staged_accumulators[row][local_n] = multiply_then_add(
                                staged_accumulators[row][local_n],
                                activations[row][k],
                                staged_weight,
                            );
                        }
                    }
                }
            }
        }
        for row in 0..PROOF_ROWS {
            for local_n in 0..TILE {
                if scalar_accumulators[row][local_n].to_bits()
                    != staged_accumulators[row][local_n].to_bits()
                {
                    return Err(Exl3Error::WarpStageMismatch);
                }
                scalar_projection_hash
                    .update(f32_to_f16_bits(scalar_accumulators[row][local_n]).to_le_bytes());
                staged_projection_hash
                    .update(f32_to_f16_bits(staged_accumulators[row][local_n]).to_le_bytes());
            }
        }
    }

    let expected_weights = logical_k
        .checked_mul(logical_n)
        .ok_or(Exl3Error::Overflow)?;
    let scheduled_trellis_bytes = scheduled_words
        .checked_mul(size_of::<u32>() as u64)
        .ok_or(Exl3Error::Overflow)?;
    if compared_weights != expected_weights as u64
        || scheduled_trellis_bytes != source_halves as u64 * size_of::<u16>() as u64
    {
        return Err(Exl3Error::WarpStageMapping);
    }
    let scalar_weight_sha256 = digest_hex(scalar_weight_hash.finalize().into());
    let staged_weight_sha256 = digest_hex(staged_weight_hash.finalize().into());
    let scalar_projection_f16_sha256 = digest_hex(scalar_projection_hash.finalize().into());
    let staged_projection_f16_sha256 = digest_hex(staged_projection_hash.finalize().into());
    if scalar_weight_sha256 != staged_weight_sha256
        || scalar_projection_f16_sha256 != staged_projection_f16_sha256
    {
        return Err(Exl3Error::WarpStageMismatch);
    }
    Ok(Exl3WarpShapeProof {
        projection_family,
        logical_k: logical_k as u32,
        logical_n: logical_n as u32,
        k_tiles: k_tiles as u32,
        n_tiles: n_tiles as u32,
        stage_iterations: stage_iterations as u32,
        compared_weights,
        scheduled_trellis_bytes,
        scalar_weight_sha256,
        staged_weight_sha256,
        scalar_projection_f16_sha256,
        staged_projection_f16_sha256,
        projection_rows: PROOF_ROWS as u8,
        bitwise_equal: true,
    })
}

fn load_stage(
    source_u32: &[u32],
    n_tiles: usize,
    first_k_tile: usize,
    n_tile: usize,
) -> Result<[[u32; WORDS_PER_TILE]; STAGE_TILES], Exl3Error> {
    let mut stage = [[0_u32; WORDS_PER_TILE]; STAGE_TILES];
    for thread in 0..LOAD_THREADS {
        let stage_tile = thread / WORDS_PER_TILE;
        let word = thread % WORDS_PER_TILE;
        let source_word = (first_k_tile + stage_tile)
            .checked_mul(n_tiles)
            .and_then(|tile| tile.checked_add(n_tile))
            .and_then(|tile| tile.checked_mul(WORDS_PER_TILE))
            .and_then(|base| base.checked_add(word))
            .ok_or(Exl3Error::Overflow)?;
        stage[stage_tile][word] = *source_u32
            .get(source_word)
            .ok_or(Exl3Error::WarpStageMapping)?;
    }
    Ok(stage)
}

fn decode_staged_at(stage_tile: &[u32; WORDS_PER_TILE], slot: u16) -> u16 {
    let lane = usize::from(slot / 8);
    let weight = usize::from(slot % 8);
    let end_bit = (lane * 8 + weight + 257) * BITS;
    let start_bit = end_bit - 16;
    let first_word = start_bit / 32;
    let last_word = (end_bit - 1) / 32;
    let shift = (last_word + 1) * 32 - end_bit;
    let merged = (u64::from(stage_tile[first_word % WORDS_PER_TILE]) << 32)
        | u64::from(stage_tile[last_word % WORDS_PER_TILE]);
    decode_3inst_f16(((merged >> shift) & 0xffff) as u16)
}

fn build_forward_slot_table() -> Result<([[u16; TILE]; TILE], String), Exl3Error> {
    let mut table = [[u16::MAX; TILE]; TILE];
    let mut hasher = Sha256::new();
    for lane in 0..32 {
        for weight in 0..8 {
            let row0 = (lane % 4) * 2;
            let rows = [row0, row0 + 1, row0 + 8, row0 + 9];
            let column0 = lane / 8;
            let column1 = column0 + 4;
            let parity = (lane >> 2) & 1;
            let row = rows[weight % 4];
            let column = 2 * (if weight < 4 { column0 } else { column1 }) + parity;
            if table[row][column] != u16::MAX {
                return Err(Exl3Error::WarpStageMapping);
            }
            let slot = (lane * 8 + weight) as u16;
            table[row][column] = slot;
            hasher.update([row as u8, column as u8, lane as u8, weight as u8]);
        }
    }
    for (row, slots) in table.iter().enumerate() {
        for (column, &slot) in slots.iter().enumerate() {
            if slot == u16::MAX
                || inverse_trellis_slot(row, column)
                    != (usize::from(slot / 8), usize::from(slot % 8))
            {
                return Err(Exl3Error::WarpStageMapping);
            }
        }
    }
    Ok((table, digest_hex(hasher.finalize().into())))
}

fn prove_load_mapping() -> Result<String, Exl3Error> {
    let mut seen = [false; LOAD_THREADS];
    let mut hasher = Sha256::new();
    for thread in 0..CTA_THREADS {
        if thread < LOAD_THREADS {
            let stage_tile = thread / WORDS_PER_TILE;
            let word = thread % WORDS_PER_TILE;
            let linear = stage_tile * WORDS_PER_TILE + word;
            if seen[linear] {
                return Err(Exl3Error::WarpStageMapping);
            }
            seen[linear] = true;
            hasher.update([
                (thread & 0xff) as u8,
                (thread >> 8) as u8,
                stage_tile as u8,
                word as u8,
            ]);
        } else {
            hasher.update([(thread & 0xff) as u8, (thread >> 8) as u8, u8::MAX, u8::MAX]);
        }
    }
    if seen.iter().any(|&value| !value) {
        return Err(Exl3Error::WarpStageMapping);
    }
    Ok(digest_hex(hasher.finalize().into()))
}

fn prove_row_schedule() -> Result<([u16; PROOF_ROWS], [u16; PROOF_ROWS]), Exl3Error> {
    let mut active_threads = [0_u16; PROOF_ROWS];
    let mut barrier_arrivals = [0_u16; PROOF_ROWS];
    for rows in 1..=PROOF_ROWS {
        let mut owners = [[0_u8; TILE]; PROOF_ROWS];
        for thread in 0..CTA_THREADS {
            let warp = thread / 32;
            let lane = thread % 32;
            let row = warp * 2 + lane / TILE;
            let column = lane % TILE;
            barrier_arrivals[rows - 1] += 2;
            if row < rows {
                owners[row][column] = owners[row][column]
                    .checked_add(1)
                    .ok_or(Exl3Error::WarpStageMapping)?;
                active_threads[rows - 1] += 1;
            }
        }
        if owners[..rows]
            .iter()
            .flatten()
            .any(|&owner_count| owner_count != 1)
            || owners[rows..]
                .iter()
                .flatten()
                .any(|&owner_count| owner_count != 0)
            || usize::from(active_threads[rows - 1]) != rows * TILE
            || usize::from(barrier_arrivals[rows - 1]) != CTA_THREADS * 2
        {
            return Err(Exl3Error::WarpStageMapping);
        }
    }
    Ok((active_threads, barrier_arrivals))
}

fn deterministic_trellis(halves: usize, seed: u64) -> Vec<u16> {
    let mut state = seed;
    let mut output = Vec::with_capacity(halves);
    for _ in 0..halves {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        output.push(state as u16);
    }
    output
}

fn deterministic_activations(logical_k: usize) -> Vec<Vec<f32>> {
    (0..PROOF_ROWS)
        .map(|row| {
            (0..logical_k)
                .map(|k| {
                    let integer = ((row * 1_009 + k * 7_919 + 17) % 1_021) as i32 - 510;
                    f16_bits_to_f32(f32_to_f16_bits(integer as f32 / 4_096.0))
                })
                .collect()
        })
        .collect()
}

#[inline(never)]
fn multiply_then_add(accumulator: f32, activation: f32, weight: f32) -> f32 {
    let product = std::hint::black_box(activation * weight);
    accumulator + product
}

fn digest_hex(digest: [u8; 32]) -> String {
    let mut output = String::with_capacity(64);
    for byte in digest {
        use std::fmt::Write;
        write!(&mut output, "{byte:02x}").expect("writing to String cannot fail");
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepted_warp_schedule_is_exhaustive_for_both_real_shapes() {
        let proof = prove_exl3_warp_staging_v2().unwrap();
        assert_eq!(proof.rows_proven, [1, 2, 3, 4, 5, 6, 7, 8]);
        assert_eq!(
            proof.active_threads_by_rows,
            [16, 32, 48, 64, 80, 96, 112, 128]
        );
        assert_eq!(proof.barrier_arrivals_by_rows, [512; 8]);
        assert_eq!(proof.stage_bytes, 768);
        assert_eq!(proof.shapes[0].compared_weights, 6_144 * 512);
        assert_eq!(proof.shapes[1].compared_weights, 512 * 6_144);
        assert!(proof.shapes.iter().all(|shape| shape.bitwise_equal
            && shape.scheduled_trellis_bytes == 1_179_648
            && shape.scalar_weight_sha256 == shape.staged_weight_sha256
            && shape.scalar_projection_f16_sha256 == shape.staged_projection_f16_sha256));
    }
}
