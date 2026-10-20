//! A SIMD implementation of the BLAKE2s compression function.
//! Based on <https://github.com/oconnor663/blake2_simd/blob/master/blake2s/src/avx2.rs>.

use std::array;
use std::mem::transmute;
use std::simd::{simd_swizzle, u32x8, u32x16};
#[cfg(all(target_arch = "x86_64", target_feature = "avx2"))]
use std::arch::x86_64::{__m256i, _mm256_setr_epi8, _mm256_shuffle_epi8};
#[cfg(all(target_arch = "x86_64", target_feature = "avx2"))]
use std::hint::black_box;

use bytemuck::cast_slice;
use itertools::Itertools;
#[cfg(feature = "parallel")]
use rayon::prelude::*;

use super::SimdBackend;
use super::m31::LOG_N_LANES;
use crate::core::fields::m31::BaseField;
use crate::core::vcs::MerkleHasher;
use crate::core::vcs::blake2_hash::{Blake2sHash, reduce_to_m31};
use crate::core::vcs::blake2_merkle::{Blake2sM31MerkleHasher, Blake2sMerkleHasher};
use crate::parallel_iter;
use crate::prover::backend::{Col, Column, ColumnOps};
use crate::prover::vcs::ops::MerkleOps;

pub const IV: [u32; 8] = [
    0x6A09E667, 0xBB67AE85, 0x3C6EF372, 0xA54FF53A, 0x510E527F, 0x9B05688C, 0x1F83D9AB, 0x5BE0CD19,
];

pub const INITIAL_STATE: [u32x16; 8] = [
    // |Key| = 0x00, |HashLength| = 0x20.
    u32x16::splat(IV[0] ^ 0x01010020),
    u32x16::splat(IV[1]),
    u32x16::splat(IV[2]),
    u32x16::splat(IV[3]),
    u32x16::splat(IV[4]),
    u32x16::splat(IV[5]),
    u32x16::splat(IV[6]),
    u32x16::splat(IV[7]),
];

pub const SIGMA: [[u8; 16]; 10] = [
    [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15],
    [14, 10, 4, 8, 9, 15, 13, 6, 1, 12, 0, 2, 11, 7, 5, 3],
    [11, 8, 12, 0, 5, 2, 15, 13, 10, 14, 3, 6, 7, 1, 9, 4],
    [7, 9, 3, 1, 13, 12, 11, 14, 2, 6, 5, 10, 4, 0, 15, 8],
    [9, 0, 5, 7, 2, 4, 10, 15, 14, 1, 11, 12, 6, 8, 3, 13],
    [2, 12, 6, 10, 0, 11, 8, 3, 4, 13, 7, 5, 15, 14, 1, 9],
    [12, 5, 1, 15, 14, 13, 4, 10, 0, 7, 6, 3, 9, 2, 8, 11],
    [13, 11, 7, 14, 12, 1, 3, 9, 5, 0, 15, 4, 8, 6, 2, 10],
    [6, 15, 14, 9, 11, 3, 0, 8, 12, 2, 13, 7, 1, 4, 10, 5],
    [10, 2, 8, 4, 7, 6, 1, 5, 15, 11, 9, 14, 3, 12, 13, 0],
];

impl ColumnOps<Blake2sHash> for SimdBackend {
    type Column = Vec<Blake2sHash>;

    fn bit_reverse_column(_column: &mut Self::Column) {
        unimplemented!()
    }
}

impl MerkleOps<Blake2sMerkleHasher> for SimdBackend {
    fn commit_on_layer(
        log_size: u32,
        prev_layer: Option<&Vec<Blake2sHash>>,
        columns: &[&Col<Self, BaseField>],
    ) -> Vec<Blake2sHash> {
        if log_size < LOG_N_LANES {
            return parallel_iter!(0..1 << log_size)
                .map(|i| {
                    Blake2sMerkleHasher::hash_node(
                        prev_layer.map(|prev_layer| (prev_layer[2 * i], prev_layer[2 * i + 1])),
                        &columns.iter().map(|column| column.at(i)).collect_vec(),
                    )
                })
                .collect();
        }

        if let Some(prev_layer) = prev_layer {
            assert_eq!(prev_layer.len(), 1 << (log_size + 1));
        }

        // Commit to columns.
        let mut res = vec![Blake2sHash::default(); 1 << log_size];
        #[cfg(not(feature = "parallel"))]
        let iter = res.chunks_mut(1 << LOG_N_LANES);

        #[cfg(feature = "parallel")]
        let iter = res.par_chunks_mut(1 << LOG_N_LANES);

        iter.enumerate().for_each(|(i, chunk)| {
            let mut state = INITIAL_STATE;
            // No columns in the layer.
            if columns.is_empty() {
                let (prev_chunk_u32s, t) = match prev_layer {
                    Some(prev_layer) => {
                        (cast_slice::<_, u32>(&prev_layer[(i << 5)..((i + 1) << 5)]), 64)
                    }
                    None => ([0; 16].as_slice(), 0),
                };
                let msgs: [u32x16; 16] = array::from_fn(|j| {
                    u32x16::from_array(std::array::from_fn(|k| prev_chunk_u32s[16 * j + k]))
                });
                let state = compress_finalize(state, transpose_msgs(msgs), t);
                let state: [Blake2sHash; 16] = unsafe { transmute(untranspose_states(state)) };
                chunk.copy_from_slice(&state);
                return;
            }

            let mut t: u64 = 0;
            // Hash prev_layer, if exists.
            if let Some(prev_layer) = prev_layer {
                let prev_chunk_u32s = cast_slice::<_, u32>(&prev_layer[(i << 5)..((i + 1) << 5)]);
                t += 64;
                // Note: prev_layer might be unaligned.
                let msgs: [u32x16; 16] = array::from_fn(|j| {
                    u32x16::from_array(std::array::from_fn(|k| prev_chunk_u32s[16 * j + k]))
                });
                state = compress_unfinalized(state, transpose_msgs(msgs), t);
            }

            // Hash columns in chunks of 16.
            let mut col_chunk_iter = columns.chunks(16);
            let last_chunk = unsafe { col_chunk_iter.next_back().unwrap_unchecked() };
            for col_chunk in &mut col_chunk_iter {
                t += 64;
                let mut msgs: [u32x16; 16] = unsafe { std::mem::zeroed() };
                for (j, column) in col_chunk.iter().enumerate() {
                    msgs[j] = column.data[i].into_simd();
                }
                state = compress_unfinalized(state, msgs, t);
            }

            t += last_chunk.len() as u64 * 4;
            let mut last_block: [u32x16; 16] = unsafe { std::mem::zeroed() };
            for (j, column) in last_chunk.iter().enumerate() {
                last_block[j] = column.data[i].into_simd();
            }
            let state = compress_finalize(state, last_block, t);
            let state: [Blake2sHash; 16] = unsafe { transmute(untranspose_states(state)) };
            chunk.copy_from_slice(&state);
        });
        res
    }
}

impl MerkleOps<Blake2sM31MerkleHasher> for SimdBackend {
    fn commit_on_layer(
        log_size: u32,
        prev_layer: Option<&Vec<Blake2sHash>>,
        columns: &[&Col<Self, BaseField>],
    ) -> Vec<Blake2sHash> {
        let mut res = <SimdBackend as MerkleOps<Blake2sMerkleHasher>>::commit_on_layer(
            log_size, prev_layer, columns,
        );
        for x in res.iter_mut() {
            x.0 = reduce_to_m31(x.0);
        }
        res
    }
}

pub const ZEROS: u32x16 = u32x16::splat(0);

// `t` is the number of compressed bytes including the current block.
pub fn compress_unfinalized(state: [u32x16; 8], chunk: [u32x16; 16], t: u64) -> [u32x16; 8] {
    compress16(
        state,
        chunk,
        u32x16::splat(t as u32),
        // t >> 32 is 0 for our use case.
        ZEROS,
        ZEROS,
        ZEROS,
    )
}

pub fn compress_finalize(state: [u32x16; 8], last_block: [u32x16; 16], t: u64) -> [u32x16; 8] {
    compress16(state, last_block, u32x16::splat(t as u32), ZEROS, u32x16::splat(0xFFFFFFFF), ZEROS)
}

/// Compresses and finalizes 16 instances of BLAKE2s.
///
/// # Arguments
///
/// * `msg_vecs` - 16 instances of BLAKE2s message (512 bits each).
/// * `bytes_in_msg` - Number of bytes in the message. NOTE: number of bytes above 64 is most likely
///   incorrect.
pub fn hash_16(msg_vecs: [u32x16; 16], bytes_in_msg: u64) -> [u32x16; 8] {
    compress_finalize(INITIAL_STATE, msg_vecs, bytes_in_msg)
}

/// Applies [`u32::rotate_right(N)`] to each element of the vector
///
/// [`u32::rotate_right(N)`]: u32::rotate_right
#[inline(always)]
fn rotate<const N: u32>(x: u32x16) -> u32x16 {
    (x >> N) | (x << (u32::BITS - N))
}

// `inline(always)` can cause code parsing errors for wasm: "locals exceed maximum".
#[cfg_attr(not(target_arch = "wasm32"), inline(always))]
pub fn round(v: &mut [u32x16; 16], m: [u32x16; 16], r: usize) {
    v[0] += m[SIGMA[r][0] as usize];
    v[1] += m[SIGMA[r][2] as usize];
    v[2] += m[SIGMA[r][4] as usize];
    v[3] += m[SIGMA[r][6] as usize];
    v[0] += v[4];
    v[1] += v[5];
    v[2] += v[6];
    v[3] += v[7];
    v[12] ^= v[0];
    v[13] ^= v[1];
    v[14] ^= v[2];
    v[15] ^= v[3];
    v[12] = rotate::<16>(v[12]);
    v[13] = rotate::<16>(v[13]);
    v[14] = rotate::<16>(v[14]);
    v[15] = rotate::<16>(v[15]);
    v[8] += v[12];
    v[9] += v[13];
    v[10] += v[14];
    v[11] += v[15];
    v[4] ^= v[8];
    v[5] ^= v[9];
    v[6] ^= v[10];
    v[7] ^= v[11];
    v[4] = rotate::<12>(v[4]);
    v[5] = rotate::<12>(v[5]);
    v[6] = rotate::<12>(v[6]);
    v[7] = rotate::<12>(v[7]);
    v[0] += m[SIGMA[r][1] as usize];
    v[1] += m[SIGMA[r][3] as usize];
    v[2] += m[SIGMA[r][5] as usize];
    v[3] += m[SIGMA[r][7] as usize];
    v[0] += v[4];
    v[1] += v[5];
    v[2] += v[6];
    v[3] += v[7];
    v[12] ^= v[0];
    v[13] ^= v[1];
    v[14] ^= v[2];
    v[15] ^= v[3];
    v[12] = rotate::<8>(v[12]);
    v[13] = rotate::<8>(v[13]);
    v[14] = rotate::<8>(v[14]);
    v[15] = rotate::<8>(v[15]);
    v[8] += v[12];
    v[9] += v[13];
    v[10] += v[14];
    v[11] += v[15];
    v[4] ^= v[8];
    v[5] ^= v[9];
    v[6] ^= v[10];
    v[7] ^= v[11];
    v[4] = rotate::<7>(v[4]);
    v[5] = rotate::<7>(v[5]);
    v[6] = rotate::<7>(v[6]);
    v[7] = rotate::<7>(v[7]);

    v[0] += m[SIGMA[r][8] as usize];
    v[1] += m[SIGMA[r][10] as usize];
    v[2] += m[SIGMA[r][12] as usize];
    v[3] += m[SIGMA[r][14] as usize];
    v[0] += v[5];
    v[1] += v[6];
    v[2] += v[7];
    v[3] += v[4];
    v[15] ^= v[0];
    v[12] ^= v[1];
    v[13] ^= v[2];
    v[14] ^= v[3];
    v[15] = rotate::<16>(v[15]);
    v[12] = rotate::<16>(v[12]);
    v[13] = rotate::<16>(v[13]);
    v[14] = rotate::<16>(v[14]);
    v[10] += v[15];
    v[11] += v[12];
    v[8] += v[13];
    v[9] += v[14];
    v[5] ^= v[10];
    v[6] ^= v[11];
    v[7] ^= v[8];
    v[4] ^= v[9];
    v[5] = rotate::<12>(v[5]);
    v[6] = rotate::<12>(v[6]);
    v[7] = rotate::<12>(v[7]);
    v[4] = rotate::<12>(v[4]);
    v[0] += m[SIGMA[r][9] as usize];
    v[1] += m[SIGMA[r][11] as usize];
    v[2] += m[SIGMA[r][13] as usize];
    v[3] += m[SIGMA[r][15] as usize];
    v[0] += v[5];
    v[1] += v[6];
    v[2] += v[7];
    v[3] += v[4];
    v[15] ^= v[0];
    v[12] ^= v[1];
    v[13] ^= v[2];
    v[14] ^= v[3];
    v[15] = rotate::<8>(v[15]);
    v[12] = rotate::<8>(v[12]);
    v[13] = rotate::<8>(v[13]);
    v[14] = rotate::<8>(v[14]);
    v[10] += v[15];
    v[11] += v[12];
    v[8] += v[13];
    v[9] += v[14];
    v[5] ^= v[10];
    v[6] ^= v[11];
    v[7] ^= v[8];
    v[4] ^= v[9];
    v[5] = rotate::<7>(v[5]);
    v[6] = rotate::<7>(v[6]);
    v[7] = rotate::<7>(v[7]);
    v[4] = rotate::<7>(v[4]);
}

/// Deinterleaves the even and odd lanes of two `u32x16` vectors.
#[inline(always)]
pub(super) fn deinterleave_u32x16(a: u32x16, b: u32x16) -> (u32x16, u32x16) {
    #[cfg(all(target_arch = "x86_64", target_feature = "avx2"))]
    unsafe {
        use std::arch::x86_64::{
            _mm256_castps_si256, _mm256_castsi256_ps, _mm256_permute4x64_epi64, _mm256_shuffle_ps,
        };
        let [a_lo, a_hi]: [__m256i; 2] = transmute(a);
        let [b_lo, b_hi]: [__m256i; 2] = transmute(b);

        let a_evens = _mm256_permute4x64_epi64::<0xD8>(_mm256_castps_si256(
            _mm256_shuffle_ps::<0x88>(_mm256_castsi256_ps(a_lo), _mm256_castsi256_ps(a_hi)),
        ));
        let b_evens = _mm256_permute4x64_epi64::<0xD8>(_mm256_castps_si256(
            _mm256_shuffle_ps::<0x88>(_mm256_castsi256_ps(b_lo), _mm256_castsi256_ps(b_hi)),
        ));
        let a_odds = _mm256_permute4x64_epi64::<0xD8>(_mm256_castps_si256(
            _mm256_shuffle_ps::<0xDD>(_mm256_castsi256_ps(a_lo), _mm256_castsi256_ps(a_hi)),
        ));
        let b_odds = _mm256_permute4x64_epi64::<0xD8>(_mm256_castps_si256(
            _mm256_shuffle_ps::<0xDD>(_mm256_castsi256_ps(b_lo), _mm256_castsi256_ps(b_hi)),
        ));

        (transmute([a_evens, b_evens]), transmute([a_odds, b_odds]))
    }
    #[cfg(not(all(target_arch = "x86_64", target_feature = "avx2")))]
    {
        a.deinterleave(b)
    }
}

/// Transposes input chunks (16 chunks of 16 `u32`s each), to get 16 `u32x16`, each
/// representing 16 packed instances of a message word.
pub fn transpose_msgs(mut data: [u32x16; 16]) -> [u32x16; 16] {
    // Index abcd:xyzw, refers to a specific word in data as follows:
    //   abcd - chunk index (in base 2)
    //   xyzw - word offset (in base 2)
    // Transpose by applying 4 times the index permutation:
    //   abcd:xyzw => wabc:dxyz
    // In other words, rotate the index to the right by 1.
    for _ in 0..4 {
        let (d0, d8) = deinterleave_u32x16(data[0], data[1]);
        let (d1, d9) = deinterleave_u32x16(data[2], data[3]);
        let (d2, d10) = deinterleave_u32x16(data[4], data[5]);
        let (d3, d11) = deinterleave_u32x16(data[6], data[7]);
        let (d4, d12) = deinterleave_u32x16(data[8], data[9]);
        let (d5, d13) = deinterleave_u32x16(data[10], data[11]);
        let (d6, d14) = deinterleave_u32x16(data[12], data[13]);
        let (d7, d15) = deinterleave_u32x16(data[14], data[15]);
        data = [d0, d1, d2, d3, d4, d5, d6, d7, d8, d9, d10, d11, d12, d13, d14, d15];
    }

    data
}

#[cfg(all(target_arch = "x86_64", target_feature = "avx2"))]
#[inline(always)]
unsafe fn transpose_8x8_avx2(r: [__m256i; 8]) -> [u32x16; 4] {
    use std::arch::x86_64::{
        _mm256_permute2x128_si256, _mm256_unpackhi_epi32, _mm256_unpackhi_epi64,
        _mm256_unpacklo_epi32, _mm256_unpacklo_epi64,
    };
    let t0 = _mm256_unpacklo_epi32(r[0], r[1]);
    let t1 = _mm256_unpackhi_epi32(r[0], r[1]);
    let t2 = _mm256_unpacklo_epi32(r[2], r[3]);
    let t3 = _mm256_unpackhi_epi32(r[2], r[3]);
    let t4 = _mm256_unpacklo_epi32(r[4], r[5]);
    let t5 = _mm256_unpackhi_epi32(r[4], r[5]);
    let t6 = _mm256_unpacklo_epi32(r[6], r[7]);
    let t7 = _mm256_unpackhi_epi32(r[6], r[7]);

    let u0 = _mm256_unpacklo_epi64(t0, t2);
    let u1 = _mm256_unpackhi_epi64(t0, t2);
    let u2 = _mm256_unpacklo_epi64(t1, t3);
    let u3 = _mm256_unpackhi_epi64(t1, t3);
    let u4 = _mm256_unpacklo_epi64(t4, t6);
    let u5 = _mm256_unpackhi_epi64(t4, t6);
    let u6 = _mm256_unpacklo_epi64(t5, t7);
    let u7 = _mm256_unpackhi_epi64(t5, t7);

    let h0 = _mm256_permute2x128_si256::<0x20>(u0, u4);
    let h1 = _mm256_permute2x128_si256::<0x20>(u1, u5);
    let h2 = _mm256_permute2x128_si256::<0x20>(u2, u6);
    let h3 = _mm256_permute2x128_si256::<0x20>(u3, u7);
    let h4 = _mm256_permute2x128_si256::<0x31>(u0, u4);
    let h5 = _mm256_permute2x128_si256::<0x31>(u1, u5);
    let h6 = _mm256_permute2x128_si256::<0x31>(u2, u6);
    let h7 = _mm256_permute2x128_si256::<0x31>(u3, u7);

    unsafe {
        [
            transmute([h0, h1]),
            transmute([h2, h3]),
            transmute([h4, h5]),
            transmute([h6, h7]),
        ]
    }
}

#[inline(always)]
#[allow(unused_mut)]
pub fn untranspose_states(mut states: [u32x16; 8]) -> [u32x16; 8] {
    #[cfg(all(target_arch = "x86_64", target_feature = "avx2"))]
    unsafe {
        let halves: [[__m256i; 2]; 8] = transmute(states);
        let lo = transpose_8x8_avx2(array::from_fn(|w| halves[w][0]));
        let hi = transpose_8x8_avx2(array::from_fn(|w| halves[w][1]));
        return [lo[0], lo[1], lo[2], lo[3], hi[0], hi[1], hi[2], hi[3]];
    }
    #[cfg(not(all(target_arch = "x86_64", target_feature = "avx2")))]
    {
        // Index abc:xyzw, refers to a specific word in data as follows:
        //   abc - chunk index (in base 2)
        //   xyzw - word offset (in base 2)
        // Transpose by applying 3 times the index permutation:
        //   abc:xyzw => bcx:yzwa
        // In other words, rotate the index to the left by 1.
        for _ in 0..3 {
            let (d0, d1) = states[0].interleave(states[4]);
            let (d2, d3) = states[1].interleave(states[5]);
            let (d4, d5) = states[2].interleave(states[6]);
            let (d6, d7) = states[3].interleave(states[7]);
            states = [d0, d1, d2, d3, d4, d5, d6, d7];
        }
        states
    }
}

/// The two byte rotations of BLAKE2s, `rotate_right(16)` and `rotate_right(8)`, on 8-lane vectors.
///
/// With AVX2 a rotation by whole bytes is one byte shuffle (`vpshufb`). Written as shifts, or as a
/// shuffle whose mask the compiler can see, LLVM moves the shuffle through the xor it rotates
/// (`rotr(d ^ a)` becomes `rotr(d) ^ rotr(a)`), and every `G` step carries two `vpshufb` per
/// rotation: 152 of `compress16_8lane`'s 1,862 instructions. The masks are made once per
/// compression behind `black_box`, so each rotation stays one shuffle. The results are the same
/// rotations, bit for bit. Without AVX2 the shifts are used.
#[derive(Clone, Copy)]
pub(super) struct ByteRotations {
    #[cfg(all(target_arch = "x86_64", target_feature = "avx2"))]
    by16: __m256i,
    #[cfg(all(target_arch = "x86_64", target_feature = "avx2"))]
    by8: __m256i,
}

impl ByteRotations {
    #[inline(always)]
    pub(super) fn new() -> Self {
        #[cfg(all(target_arch = "x86_64", target_feature = "avx2"))]
        {
            // Byte `i` of each 4-byte lane comes from byte `(i + 2) % 4` (16 bits) or
            // `(i + 1) % 4` (8 bits) of the same lane: a right rotation in little-endian lanes.
            let by16 = unsafe {
                _mm256_setr_epi8(
                    2, 3, 0, 1, 6, 7, 4, 5, 10, 11, 8, 9, 14, 15, 12, 13, 18, 19, 16, 17, 22, 23,
                    20, 21, 26, 27, 24, 25, 30, 31, 28, 29,
                )
            };
            let by8 = unsafe {
                _mm256_setr_epi8(
                    1, 2, 3, 0, 5, 6, 7, 4, 9, 10, 11, 8, 13, 14, 15, 12, 17, 18, 19, 16, 21, 22,
                    23, 20, 25, 26, 27, 24, 29, 30, 31, 28,
                )
            };
            Self { by16: black_box(by16), by8: black_box(by8) }
        }
        #[cfg(not(all(target_arch = "x86_64", target_feature = "avx2")))]
        {
            Self {}
        }
    }

    /// Applies [`u32::rotate_right(N)`] to each element of the 8-lane vector.
    #[inline(always)]
    pub(super) fn rotate<const N: u32>(self, x: u32x8) -> u32x8 {
        #[cfg(all(target_arch = "x86_64", target_feature = "avx2"))]
        {
            if N == 16 {
                let rotated: __m256i = unsafe { _mm256_shuffle_epi8(x.into(), self.by16) };
                return rotated.into();
            }
            if N == 8 {
                let rotated: __m256i = unsafe { _mm256_shuffle_epi8(x.into(), self.by8) };
                return rotated.into();
            }
        }
        (x >> N) | (x << (u32::BITS - N))
    }
}

/// One BLAKE2s `G` mixing step on an 8-lane slice of the state.
#[inline(always)]
fn g8(
    v: &mut [u32x8; 16],
    rot: ByteRotations,
    a: usize,
    b: usize,
    c: usize,
    d: usize,
    x: u32x8,
    y: u32x8,
) {
    v[a] = v[a] + v[b] + x;
    v[d] = rot.rotate::<16>(v[d] ^ v[a]);
    v[c] += v[d];
    v[b] = rot.rotate::<12>(v[b] ^ v[c]);
    v[a] = v[a] + v[b] + y;
    v[d] = rot.rotate::<8>(v[d] ^ v[a]);
    v[c] += v[d];
    v[b] = rot.rotate::<7>(v[b] ^ v[c]);
}

/// One BLAKE2s round `R` on an 8-lane slice of the state.
#[inline(always)]
fn round8<const R: usize>(v: &mut [u32x8; 16], m_base: *const [u32x8; 2], rot: ByteRotations) {
    #[cfg(all(target_arch = "x86_64", target_feature = "avx2"))]
    let m_base = black_box(m_base);
    let s = &SIGMA[R];
    let load_m = |idx: u8| -> u32x8 {
        // SAFETY: `idx < 16` for all entries of `SIGMA`, and `m_base` points to the active half of
        // a 16-element `[[u32x8; 2]; 16]` array.
        unsafe { *m_base.add(idx as usize).cast::<u32x8>() }
    };
    g8(v, rot, 0, 4, 8, 12, load_m(s[0]), load_m(s[1]));
    g8(v, rot, 1, 5, 9, 13, load_m(s[2]), load_m(s[3]));
    g8(v, rot, 2, 6, 10, 14, load_m(s[4]), load_m(s[5]));
    g8(v, rot, 3, 7, 11, 15, load_m(s[6]), load_m(s[7]));
    g8(v, rot, 0, 5, 10, 15, load_m(s[8]), load_m(s[9]));
    g8(v, rot, 1, 6, 11, 12, load_m(s[10]), load_m(s[11]));
    g8(v, rot, 2, 7, 8, 13, load_m(s[12]), load_m(s[13]));
    g8(v, rot, 3, 4, 9, 14, load_m(s[14]), load_m(s[15]));
}

/// Compresses an 8-lane half (`h_base`, `m_base`) of the 16 BLAKE2s instances.
///
/// A full 16-word state of 8-lane vectors fits in the 16 AVX2 vector registers, whereas the
/// 16-lane state needs twice as many and spills on every step. Running the two halves one after
/// the other keeps each of them in registers; the result is bit-identical.
#[inline(never)]
fn compress16_8lane(
    h_base: *const [u32x8; 2],
    m_base: *const [u32x8; 2],
    count_low: u32x8,
    count_high: u32x8,
    lastblock: u32x8,
    lastnode: u32x8,
) -> [u32x8; 8] {
    let rot = ByteRotations::new();
    let load_h = |base: *const [u32x8; 2], i: usize| -> u32x8 {
        // SAFETY: `i < 8` and `base` points to the active half of `[[u32x8; 2]; 8]`.
        unsafe { *base.add(i).cast::<u32x8>() }
    };

    let mut v = [
        load_h(h_base, 0),
        load_h(h_base, 1),
        load_h(h_base, 2),
        load_h(h_base, 3),
        load_h(h_base, 4),
        load_h(h_base, 5),
        load_h(h_base, 6),
        load_h(h_base, 7),
        u32x8::splat(IV[0]),
        u32x8::splat(IV[1]),
        u32x8::splat(IV[2]),
        u32x8::splat(IV[3]),
        u32x8::splat(IV[4]) ^ count_low,
        u32x8::splat(IV[5]) ^ count_high,
        u32x8::splat(IV[6]) ^ lastblock,
        u32x8::splat(IV[7]) ^ lastnode,
    ];

    round8::<0>(&mut v, m_base, rot);
    round8::<1>(&mut v, m_base, rot);
    round8::<2>(&mut v, m_base, rot);
    round8::<3>(&mut v, m_base, rot);
    round8::<4>(&mut v, m_base, rot);
    round8::<5>(&mut v, m_base, rot);
    round8::<6>(&mut v, m_base, rot);
    round8::<7>(&mut v, m_base, rot);
    round8::<8>(&mut v, m_base, rot);
    round8::<9>(&mut v, m_base, rot);

    #[cfg(all(target_arch = "x86_64", target_feature = "avx2"))]
    let h_base = black_box(h_base);

    [
        load_h(h_base, 0) ^ v[0] ^ v[8],
        load_h(h_base, 1) ^ v[1] ^ v[9],
        load_h(h_base, 2) ^ v[2] ^ v[10],
        load_h(h_base, 3) ^ v[3] ^ v[11],
        load_h(h_base, 4) ^ v[4] ^ v[12],
        load_h(h_base, 5) ^ v[5] ^ v[13],
        load_h(h_base, 6) ^ v[6] ^ v[14],
        load_h(h_base, 7) ^ v[7] ^ v[15],
    ]
}

/// Compresses 16 blake2s instances.
#[inline(always)]
pub fn compress16(
    h_vecs: [u32x16; 8],
    msg_vecs: [u32x16; 16],
    count_low: u32x16,
    count_high: u32x16,
    lastblock: u32x16,
    lastnode: u32x16,
) -> [u32x16; 8] {
    let pick_lo = |x: u32x16| -> u32x8 { simd_swizzle!(x, [0, 1, 2, 3, 4, 5, 6, 7]) };
    let pick_hi = |x: u32x16| -> u32x8 { simd_swizzle!(x, [8, 9, 10, 11, 12, 13, 14, 15]) };
    let h_ptr = h_vecs.as_ptr().cast::<[u32x8; 2]>();
    let m_ptr = msg_vecs.as_ptr().cast::<[u32x8; 2]>();
    let lo = compress16_8lane(
        h_ptr,
        m_ptr,
        pick_lo(count_low),
        pick_lo(count_high),
        pick_lo(lastblock),
        pick_lo(lastnode),
    );
    // SAFETY: Adding 1 `u32x8` (32 bytes) points to the high 8-lane half of each `[u32x8; 2]`
    // element at the same 64-byte stride.
    let hi = unsafe {
        compress16_8lane(
            h_ptr.byte_add(32),
            m_ptr.byte_add(32),
            pick_hi(count_low),
            pick_hi(count_high),
            pick_hi(lastblock),
            pick_hi(lastnode),
        )
    };
    std::array::from_fn(|i| {
        simd_swizzle!(lo[i], hi[i], [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15])
    })
}

#[cfg(test)]
mod tests {
    use std::array;
    use std::mem::transmute;
    use std::simd::u32x16;

    use aligned::{A64, Aligned};
    use bytemuck::cast_slice;
    use rand::rngs::SmallRng;
    use rand::{Rng, SeedableRng};

    use super::{compress16, hash_16, transpose_msgs, untranspose_states};
    use crate::core::vcs::blake2_hash::Blake2sHasher;
    use crate::prover::backend::simd::blake2s_ref::compress;

    #[test]
    fn compress16_works() {
        let states: Aligned<A64, [[u32; 8]; 16]> =
            Aligned(array::from_fn(|i| array::from_fn(|j| (i + j) as u32)));
        let msgs: Aligned<A64, [[u32; 16]; 16]> =
            Aligned(array::from_fn(|i| array::from_fn(|j| (i + j + 20) as u32)));
        let count_low = 1;
        let count_high = 2;
        let lastblock = 3;
        let lastnode = 4;
        let res_unvectorized = array::from_fn(|i| {
            compress(states[i], msgs[i], count_low, count_high, lastblock, lastnode)
        });

        let res_vectorized: [[u32; 8]; 16] = unsafe {
            transmute(untranspose_states(compress16(
                transpose_states(transmute::<Aligned<A64, [[u32; 8]; 16]>, [u32x16; 8]>(states)),
                transpose_msgs(transmute::<Aligned<A64, [[u32; 16]; 16]>, [u32x16; 16]>(msgs)),
                u32x16::splat(count_low),
                u32x16::splat(count_high),
                u32x16::splat(lastblock),
                u32x16::splat(lastnode),
            )))
        };

        assert_eq!(res_vectorized, res_unvectorized);
    }

    #[test]
    fn untranspose_states_is_transpose_states_inverse() {
        let states = array::from_fn(|i| u32x16::from(array::from_fn(|j| (i + j) as u32)));
        let transposed_states = transpose_states(states);

        let untrasponsed_transposed_states = untranspose_states(transposed_states);

        assert_eq!(untrasponsed_transposed_states, states)
    }

    #[test]
    fn hash_16_works() {
        let mut rng = SmallRng::seed_from_u64(1055);
        let msgs: [[u32; 16]; 16] = array::from_fn(|_| rng.random::<[u32; 16]>());
        let expected: [[u8; 32]; 16] = array::from_fn(|i| {
            let state = msgs[i];
            let mut hasher = Blake2sHasher::new();
            hasher.update(cast_slice(&state));
            hasher.finalize().0
        });
        let transposed_msgs: [u32x16; 16] =
            array::from_fn(|i| u32x16::from_array(array::from_fn(|j| msgs[j][i])));

        let res = hash_16(transposed_msgs, 64);

        let res: [[u8; 32]; 16] = unsafe { transmute(untranspose_states(res)) };
        assert_eq!(res, expected);
    }

    /// Transposes states, from 8 packed words, to get 16 results, each of size 32B.
    fn transpose_states(mut states: [u32x16; 8]) -> [u32x16; 8] {
        // Index abc:xyzw, refers to a specific word in data as follows:
        //   abc - chunk index (in base 2)
        //   xyzw - word offset (in base 2)
        // Transpose by applying 3 times the index permutation:
        //   abc:xyzw => wab:cxyz
        // In other words, rotate the index to the right by 1.
        for _ in 0..3 {
            let (s0, s4) = states[0].deinterleave(states[1]);
            let (s1, s5) = states[2].deinterleave(states[3]);
            let (s2, s6) = states[4].deinterleave(states[5]);
            let (s3, s7) = states[6].deinterleave(states[7]);
            states = [s0, s1, s2, s3, s4, s5, s6, s7];
        }

        states
    }
}
