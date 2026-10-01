use std::ops::Range;
use std::simd::cmp::{SimdOrd, SimdPartialEq};
use std::simd::{simd_swizzle, u32x8, u32x16};

use bytemuck::cast_slice;
#[cfg(feature = "parallel")]
use rayon::prelude::*;
use tracing::{Level, span};

use super::SimdBackend;
use crate::core::channel::{Blake2sChannelGeneric, Channel, Keccak256Channel};
use crate::core::fields::m31::P;
use crate::core::proof_of_work::GrindOps;
use crate::core::vcs::blake2_hash::Blake2sHasherGeneric;
use crate::prover::backend::simd::blake2s::{IV, SIGMA};
use crate::prover::backend::simd::m31::N_LANES;

// GRIND_LOW_BITS must be <= 30 if we want to guarantee that the lowest 32 bits of the nonce are
// < 2^31 - 1.
const GRIND_LOW_BITS: u32 = 20;
// Threads take work in units of this many low bits, so once a solution is found no thread keeps
// grinding a stale unit for long. The nonce found does not depend on it.
#[cfg(feature = "parallel")]
const GRIND_UNIT_BITS: u32 = 16;

impl<const IS_M31_OUTPUT: bool> GrindOps<Blake2sChannelGeneric<IS_M31_OUTPUT>> for SimdBackend {
    /// Outputs the smallest nonce of the form `(a << 32) | b`, where `0 <= a < 2^31 - 1` and
    /// `0 <= b < 2^GRIND_LOW_BITS`.
    fn grind(channel: &Blake2sChannelGeneric<IS_M31_OUTPUT>, pow_bits: u32) -> u64 {
        let _span = span!(Level::TRACE, "Simd Blake2s Grind", class = "Blake2s Grind");

        // TODO(first): support more than 32 bits.
        assert!(pow_bits <= 32, "pow_bits > 32 is not supported");
        let digest = channel.digest();

        // Compute the prefix digest H(POW_PREFIX, [0_u8; 12], digest, n_bits).
        let mut hasher = Blake2sHasherGeneric::<IS_M31_OUTPUT>::default();
        hasher.update(&Blake2sChannelGeneric::<IS_M31_OUTPUT>::POW_PREFIX.to_le_bytes());
        hasher.update(&[0_u8; 12]);
        hasher.update(&digest.0[..]);
        hasher.update(&pow_bits.to_le_bytes());
        let prefixed_digest = hasher.finalize();
        let prefixed_digest: &[u32] = cast_slice(&prefixed_digest.0[..]);

        #[cfg(not(feature = "parallel"))]
        let res = (0..)
            .find_map(|hi| {
                grind_blake::<IS_M31_OUTPUT>(prefixed_digest, hi, 0..1 << GRIND_LOW_BITS, pow_bits)
            })
            .expect("Grind failed to find a solution.");

        #[cfg(feature = "parallel")]
        let res = parallel_grind(
            prefixed_digest,
            pow_bits,
            GRIND_LOW_BITS,
            grind_blake::<IS_M31_OUTPUT>,
        );

        assert!(
            ((res >> 32) as u32) < P,
            "The 32 high bits of the nonce are not reduced modulo the M31 prime."
        );
        assert!(
            (res as u32) < P,
            "The 32 low bits of the nonce are not reduced modulo the M31 prime."
        );
        res
    }
}

/// Rotates each 32-bit lane right by `N`.
#[inline(always)]
fn rotr8<const N: u32>(x: u32x8) -> u32x8 {
    (x >> N) | (x << (u32::BITS - N))
}

/// Adds message word `idx` of the grind block `digest (8 words) || nonce low || nonce high ||
/// zeros (6 words)` to `a`. The zero padding words are never materialized.
#[inline(always)]
fn add_grind_msg(a: u32x8, m: &[u32x8; 10], idx: u8) -> u32x8 {
    if (idx as usize) < m.len() { a + m[idx as usize] } else { a }
}

/// One BLAKE2s `G` mixing step on 8 nonce lanes.
#[inline(always)]
fn g8(v: &mut [u32x8; 16], [a, b, c, d]: [usize; 4], m: &[u32x8; 10], x: u8, y: u8) {
    v[a] = add_grind_msg(v[a] + v[b], m, x);
    v[d] = rotr8::<16>(v[d] ^ v[a]);
    v[c] += v[d];
    v[b] = rotr8::<12>(v[b] ^ v[c]);
    v[a] = add_grind_msg(v[a] + v[b], m, y);
    v[d] = rotr8::<8>(v[d] ^ v[a]);
    v[c] += v[d];
    v[b] = rotr8::<7>(v[b] ^ v[c]);
}

/// One BLAKE2s round on 8 nonce lanes.
#[inline(always)]
fn round8<const R: usize>(v: &mut [u32x8; 16], m: &[u32x8; 10]) {
    let s = &SIGMA[R];
    g8(v, [0, 4, 8, 12], m, s[0], s[1]);
    g8(v, [1, 5, 9, 13], m, s[2], s[3]);
    g8(v, [2, 6, 10, 14], m, s[4], s[5]);
    g8(v, [3, 7, 11, 15], m, s[6], s[7]);
    g8(v, [0, 5, 10, 15], m, s[8], s[9]);
    g8(v, [1, 6, 11, 12], m, s[10], s[11]);
    g8(v, [2, 7, 8, 13], m, s[12], s[13]);
    g8(v, [3, 4, 9, 14], m, s[14], s[15]);
}

/// Scalar `G`, used for the part of the compression that does not depend on the nonce.
fn g_scalar(v: &mut [u32; 16], [a, b, c, d]: [usize; 4], x: u32, y: u32) {
    v[a] = v[a].wrapping_add(v[b]).wrapping_add(x);
    v[d] = (v[d] ^ v[a]).rotate_right(16);
    v[c] = v[c].wrapping_add(v[d]);
    v[b] = (v[b] ^ v[c]).rotate_right(12);
    v[a] = v[a].wrapping_add(v[b]).wrapping_add(y);
    v[d] = (v[d] ^ v[a]).rotate_right(8);
    v[c] = v[c].wrapping_add(v[d]);
    v[b] = (v[b] ^ v[c]).rotate_right(7);
}

/// The BLAKE2s state after every step of round 0 that does not depend on the nonce.
///
/// The grind block holds the digest in words 0..8, the nonce in words 8 and 9 and zeros in words 10
/// to 15. In round 0 the message words are used in order, so of the eight `G` steps only
/// `G(0, 5, 10, 15)` (words 8 and 9) varies with the nonce. The other seven are computed once.
struct GrindPrefix {
    v: [u32; 16],
    digest: [u32; 8],
}

impl GrindPrefix {
    /// `n_bytes` is the number of hashed bytes: the digest and the 64 bit nonce.
    fn new(digest: &[u32], n_bytes: u32) -> Self {
        let digest: [u32; 8] = std::array::from_fn(|i| digest[i]);
        // Initial state of the hasher (parameter block: |Key| = 0x00, |HashLength| = 0x20).
        let h0 = IV[0] ^ 0x01010020;
        let mut v = [
            h0,
            IV[1],
            IV[2],
            IV[3],
            IV[4],
            IV[5],
            IV[6],
            IV[7],
            IV[0],
            IV[1],
            IV[2],
            IV[3],
            IV[4] ^ n_bytes,
            IV[5],
            // Last block flag.
            IV[6] ^ 0xFFFF_FFFF,
            IV[7],
        ];
        g_scalar(&mut v, [0, 4, 8, 12], digest[0], digest[1]);
        g_scalar(&mut v, [1, 5, 9, 13], digest[2], digest[3]);
        g_scalar(&mut v, [2, 6, 10, 14], digest[4], digest[5]);
        g_scalar(&mut v, [3, 7, 11, 15], digest[6], digest[7]);
        g_scalar(&mut v, [1, 6, 11, 12], 0, 0);
        g_scalar(&mut v, [2, 7, 8, 13], 0, 0);
        g_scalar(&mut v, [3, 4, 9, 14], 0, 0);
        Self { v, digest }
    }

    /// Returns the first 32 bits of the hash of `digest || nonce` for 8 nonces that share the high
    /// word `high` and have the given low words.
    #[inline(always)]
    fn hash_word0(&self, high: u32x8, low: u32x8) -> u32x8 {
        let splat = u32x8::splat;
        let m: [u32x8; 10] = std::array::from_fn(|i| match i {
            0..=7 => splat(self.digest[i]),
            8 => low,
            _ => high,
        });
        let mut v: [u32x8; 16] = std::array::from_fn(|i| splat(self.v[i]));
        // Round 0, `G(0, 5, 10, 15)`: the only step that depends on the nonce.
        let a = splat(self.v[0].wrapping_add(self.v[5])) + low;
        let d = rotr8::<16>(splat(self.v[15]) ^ a);
        let c = splat(self.v[10]) + d;
        let b = rotr8::<12>(splat(self.v[5]) ^ c);
        let a = a + b + high;
        let d = rotr8::<8>(d ^ a);
        let c = c + d;
        let b = rotr8::<7>(b ^ c);
        v[0] = a;
        v[5] = b;
        v[10] = c;
        v[15] = d;
        round8::<1>(&mut v, &m);
        round8::<2>(&mut v, &m);
        round8::<3>(&mut v, &m);
        round8::<4>(&mut v, &m);
        round8::<5>(&mut v, &m);
        round8::<6>(&mut v, &m);
        round8::<7>(&mut v, &m);
        round8::<8>(&mut v, &m);
        // Only the first output word is tested, so the compiler drops every step of the last round
        // that does not feed `v[0]` or `v[8]`.
        round8::<9>(&mut v, &m);
        splat(IV[0] ^ 0x01010020) ^ v[0] ^ v[8]
    }
}

fn grind_blake<const IS_M31_OUTPUT: bool>(
    digest: &[u32],
    hi: u32,
    lows: Range<u32>,
    pow_bits: u32,
) -> Option<u64> {
    const DIGEST_SIZE: usize = std::mem::size_of::<[u32; 8]>();
    const NONCE_SIZE: usize = std::mem::size_of::<u64>();
    let prefix = GrindPrefix::new(digest, (DIGEST_SIZE + NONCE_SIZE) as u32);
    let offsets_vec = u32x16::from(std::array::from_fn(|i| i as u32));
    // `trailing_zeros(x) >= pow_bits` iff the `pow_bits` lowest bits of `x` are zero.
    let mask = u32x8::splat(if pow_bits >= u32::BITS { u32::MAX } else { (1 << pow_bits) - 1 });
    let zero = u32x8::splat(0);
    let modulus = u32x8::splat(P);

    let mut attempt_low = offsets_vec + u32x16::splat(lows.start);
    let attempt_high = u32x8::splat(hi);
    for low in lows.step_by(N_LANES) {
        // The 16 nonces are hashed as two groups of 8, one after the other: a full state of 8-lane
        // vectors fits the 16 AVX2 registers.
        let groups: [u32x8; 2] = [
            simd_swizzle!(attempt_low, [0, 1, 2, 3, 4, 5, 6, 7]),
            simd_swizzle!(attempt_low, [8, 9, 10, 11, 12, 13, 14, 15]),
        ];
        for (group, group_low) in groups.into_iter().enumerate() {
            let mut res0 = prefix.hash_word0(attempt_high, group_low);
            if IS_M31_OUTPUT {
                // Reduce modulo `P`, as `reduce_to_m31` does for the hash output.
                res0 = res0.simd_min(res0 - modulus);
                res0 = res0.simd_min(res0 - modulus);
            }
            let success_mask = (res0 & mask).simd_eq(zero);
            if success_mask.any() {
                let i = success_mask.to_array().iter().position(|&x| x).unwrap();
                return Some(((hi as u64) << 32) + low as u64 + (group * 8 + i) as u64);
            }
        }
        attempt_low += u32x16::splat(N_LANES as u32);
    }
    None
}

// Deterministically finds the smallest nonce that satisfies:
// `hash(digest, nonce).trailing_zeros() >= pow_bits`.
// Units of `1 << GRIND_UNIT_BITS` low nonces are handed out in nonce order, so every unit below
// the first solution is searched, and a thread stops after at most one short unit.
// Short units: the maintainers' positive control (workshop thread bt1_2ebe688baa6d50e483d4a9d2).
#[cfg(feature = "parallel")]
fn parallel_grind<GRIND, DIGEST>(digest: DIGEST, pow_bits: u32, low_bits: u32, grind: GRIND) -> u64
where
    GRIND: Fn(DIGEST, u32, Range<u32>, u32) -> Option<u64> + Send + Sync,
    DIGEST: Send + Sync + Copy,
{
    use core::sync::atomic::Ordering;
    use std::sync::atomic::AtomicU64;

    let unit_bits = low_bits.min(GRIND_UNIT_BITS);
    let log_units_per_hi = low_bits - unit_bits;
    let n_workers = rayon::current_num_threads() as u64;
    let next_unit = AtomicU64::new(n_workers);
    let smallest_good_unit = AtomicU64::new(u64::MAX);
    let found = (0..n_workers)
        .into_par_iter()
        .filter_map(|thread_id| {
            let mut unit = thread_id;
            loop {
                let hi = (unit >> log_units_per_hi) as u32;
                let start = ((unit & ((1 << log_units_per_hi) - 1)) << unit_bits) as u32;
                if let Some(found) = grind(digest, hi, start..start + (1 << unit_bits), pow_bits) {
                    // Signal higher units to stop. Every thread that found an answer returns
                    // it, and the results are compared.
                    smallest_good_unit.fetch_min(unit, Ordering::Relaxed);
                    return Some(found);
                }
                // Assign the next unit to this thread.
                unit = next_unit.fetch_add(1, Ordering::Relaxed);
                if unit >= smallest_good_unit.load(Ordering::Relaxed) {
                    break;
                }
            }
            None
        })
        .min();

    found.expect("Grind failed to find a solution.")
}

// TODO: replace with an optimized SIMD implementation using parallel keccak permutations
// (e.g. `keccak::parallel` `f1600x4`/`x8`), similar to the parallel + SIMD path used for
// `Blake2sChannelGeneric` above.
impl GrindOps<Keccak256Channel> for SimdBackend {
    fn grind(channel: &Keccak256Channel, pow_bits: u32) -> u64 {
        let mut nonce = 0u64;
        loop {
            if channel.verify_pow_nonce(pow_bits, nonce) {
                return nonce;
            }
            nonce += 1;
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub mod poseidon252 {
    use starknet_crypto::poseidon_hash_many;
    use starknet_ff::FieldElement as FieldElement252;

    use super::*;
    use crate::core::channel::Poseidon252Channel;

    const GRIND_LOW_BITS: u32 = 14;

    impl GrindOps<Poseidon252Channel> for SimdBackend {
        /// Outputs the smallest nonce of the form `(a << 32) | b`, where `0 <= a < 2^31 - 1` and
        /// `0 <= b < 2^GRIND_LOW_BITS`.
        fn grind(channel: &Poseidon252Channel, pow_bits: u32) -> u64 {
            let digest = channel.digest();
            let prefixed_digest = poseidon_hash_many(&[
                Poseidon252Channel::POW_PREFIX.into(),
                digest,
                pow_bits.into(),
            ]);
            #[cfg(not(feature = "parallel"))]
            let res = (0..)
                .find_map(|hi| grind_poseidon(prefixed_digest, hi, 0..1 << GRIND_LOW_BITS, pow_bits))
                .expect("Grind failed to find a solution.");

            #[cfg(feature = "parallel")]
            let res = parallel_grind(prefixed_digest, pow_bits, GRIND_LOW_BITS, grind_poseidon);

            assert!(
                ((res >> 32) as u32) < P,
                "The 32 high bits of the solution are larger than the M31 prime."
            );
            res
        }
    }

    fn grind_poseidon(
        digest: FieldElement252,
        chunk_id: u32,
        lows: Range<u32>,
        pow_bits: u32,
    ) -> Option<u64> {
        for low in lows {
            let nonce = u64::from(low) | ((chunk_id as u64) << 32);
            let hash = starknet_crypto::poseidon_hash(digest, nonce.into());
            let trailing_zeros =
                u128::from_be_bytes(hash.to_bytes_be()[16..].try_into().unwrap()).trailing_zeros();
            if trailing_zeros >= pow_bits {
                return Some(nonce);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use itertools::Itertools;

    use super::*;
    use crate::core::channel::{Blake2sChannel, Channel};

    #[cfg(all(feature = "parallel", feature = "slow-tests"))]
    #[test]
    fn test_parallel_grind_with_high_pow_bits() {
        let mut channel = Blake2sChannel::default();
        channel.mix_u64(0x1111222233334344);
        let pow_bits = 26;
        for _ in 0..10 {
            let res = SimdBackend::grind(&channel, pow_bits);
            assert!(channel.verify_pow_nonce(pow_bits, res));
            channel.mix_u64(res);
            channel.mix_u64(0x1111222233334344);
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn test_grind_poseidon() {
        let pow_bits = 10;
        let mut channel = crate::core::channel::Poseidon252Channel::default();
        channel.mix_u64(0x1111222233334344);

        let nonce = SimdBackend::grind(&channel, pow_bits);
        assert!(channel.verify_pow_nonce(pow_bits, nonce));
    }

    fn test_grind_is_deterministic<C: Channel>()
    where
        SimdBackend: GrindOps<C>,
    {
        let pow_bits = 2;
        let n_attempts = 1000;
        let mut channel = C::default();
        channel.mix_u64(0);

        let results = (0..n_attempts).map(|_| SimdBackend::grind(&channel, pow_bits)).collect_vec();

        assert!(results.iter().all(|r| r == &results[0]));
    }

    #[test]
    fn test_grind_blake_is_deterministic() {
        test_grind_is_deterministic::<Blake2sChannel>();
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn test_grind_poseidon_is_deterministic() {
        test_grind_is_deterministic::<crate::core::channel::Poseidon252Channel>();
    }
}
