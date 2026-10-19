//! The Keccak-f[1600] state as the Keccak builtin's components see it, and the round they prove.
//!
//! The builtin reads 8 felts of 200 bits each; their concatenation (little endian) is the 1600-bit
//! state, and bit `64 * (x + 5 * y) + z` of the state is bit `z` of lane `(x, y)`. Between rounds
//! the state travels as 64 limbs: each felt splits into 8 limbs of 27 bits (3 memory limbs of 9
//! bits each), the last of which holds the felt's top 11 bits.

/// The number of rounds of Keccak-f[1600].
pub const N_ROUNDS: usize = 24;
/// The number of felts the builtin reads (and writes).
pub const N_FELTS: usize = 8;
/// The bits of the state held by each felt.
pub const BITS_PER_FELT: usize = 200;
/// The bits of a full limb of the state between rounds (3 memory limbs of 9 bits).
pub const BITS_PER_LIMB: usize = 27;
/// The limbs each felt splits into between rounds.
pub const LIMBS_PER_FELT: usize = BITS_PER_FELT.div_ceil(BITS_PER_LIMB);
/// The limbs of the state between rounds.
pub const N_LIMBS: usize = N_FELTS * LIMBS_PER_FELT;
/// The memory limbs (9 bits each, except the last) of a felt below 2^200.
pub const MEMORY_LIMBS_PER_FELT: usize = BITS_PER_FELT.div_ceil(9);

/// The rotation offsets of the rho step, indexed `[x][y]`.
pub const RHO_OFFSETS: [[usize; 5]; 5] = [
    [0, 36, 3, 41, 18],
    [1, 44, 10, 45, 2],
    [62, 6, 43, 15, 61],
    [28, 55, 25, 21, 56],
    [27, 20, 39, 8, 14],
];

/// The round constants of the iota step.
pub const ROUND_CONSTANTS: [u64; N_ROUNDS] = [
    0x0000000000000001,
    0x0000000000008082,
    0x800000000000808a,
    0x8000000080008000,
    0x000000000000808b,
    0x0000000080000001,
    0x8000000080008081,
    0x8000000000008009,
    0x000000000000008a,
    0x0000000000000088,
    0x0000000080008009,
    0x000000008000000a,
    0x000000008000808b,
    0x800000000000008b,
    0x8000000000008089,
    0x8000000000008003,
    0x8000000000008002,
    0x8000000000000080,
    0x000000000000800a,
    0x800000008000000a,
    0x8000000080008081,
    0x8000000000008080,
    0x0000000080000001,
    0x8000000080008008,
];

/// The only bit positions of lane (0, 0) that a round constant can set: 2^j - 1 for j < 7.
pub const IOTA_BITS: [usize; 7] = [0, 1, 3, 7, 15, 31, 63];

/// The state bit holding bit `z` of lane `(x, y)`.
pub const fn state_bit(x: usize, y: usize, z: usize) -> usize {
    64 * (x + 5 * y) + z
}

/// The limb (between rounds) that holds state bit `k`, and the bit's position in that limb.
pub const fn limb_of_bit(k: usize) -> (usize, usize) {
    let felt = k / BITS_PER_FELT;
    let bit = k % BITS_PER_FELT;
    (felt * LIMBS_PER_FELT + bit / BITS_PER_LIMB, bit % BITS_PER_LIMB)
}

/// The number of state bits limb `limb` holds: 27, or 11 for a felt's last limb.
pub const fn limb_len(limb: usize) -> usize {
    let group = limb % LIMBS_PER_FELT;
    if group == LIMBS_PER_FELT - 1 { BITS_PER_FELT - group * BITS_PER_LIMB } else { BITS_PER_LIMB }
}

/// The state bit at position `position` of limb `limb`.
pub const fn bit_of_limb(limb: usize, position: usize) -> usize {
    (limb / LIMBS_PER_FELT) * BITS_PER_FELT + (limb % LIMBS_PER_FELT) * BITS_PER_LIMB + position
}

/// The state bits held by each limb (between rounds), in increasing position.
pub fn bits_of_limbs() -> [Vec<usize>; N_LIMBS] {
    core::array::from_fn(|limb| {
        (0..limb_len(limb)).map(|position| bit_of_limb(limb, position)).collect()
    })
}

/// Whether bit `j` (of 7) of round `round`'s constant is set: the constant's bit `IOTA_BITS[j]`.
pub const fn round_constant_bit(round: usize, j: usize) -> bool {
    (ROUND_CONSTANTS[round] >> IOTA_BITS[j]) & 1 == 1
}

/// The lane `(x0, y0)` of theta's output that rho and pi move to lane `(x, y)`, rotated left by
/// `RHO_OFFSETS[x0][y0]`: pi sends `(x0, y0)` to `(y0, 2 x0 + 3 y0)`.
pub const fn pi_source(x: usize, y: usize) -> (usize, usize) {
    let y0 = x;
    // 2 x0 + 3 y0 = y (mod 5), and 3 is the inverse of 2 modulo 5.
    let x0 = (3 * (y + 5 * 3 - 3 * y0)) % 5;
    (x0, y0)
}

/// The lanes (indexed `x + 5 y`) of the state held by `limbs`.
pub fn limbs_to_lanes(limbs: &[u32; N_LIMBS]) -> [u64; 25] {
    let mut lanes = [0u64; 25];
    for k in 0..N_FELTS * BITS_PER_FELT {
        let (limb, position) = limb_of_bit(k);
        lanes[k / 64] |= u64::from((limbs[limb] >> position) & 1) << (k % 64);
    }
    lanes
}

/// The limbs (between rounds) of the state with `lanes` (indexed `x + 5 y`).
pub fn lanes_to_limbs(lanes: &[u64; 25]) -> [u32; N_LIMBS] {
    let mut limbs = [0u32; N_LIMBS];
    for k in 0..N_FELTS * BITS_PER_FELT {
        let (limb, position) = limb_of_bit(k);
        limbs[limb] |= (((lanes[k / 64] >> (k % 64)) & 1) as u32) << position;
    }
    limbs
}

/// The values a Keccak round component commits for one round, lane by lane (indexed `x + 5 y`).
pub struct RoundValues {
    /// The column parities of the input state.
    pub c: [u64; 5],
    /// `c[x] ^ d[x]`, where `d[x] = c[x - 1] ^ rot(c[x + 1], 1)` is what theta adds to column x.
    pub c_prime: [u64; 5],
    /// Theta applied to the input state.
    pub a_prime: [u64; 25],
    /// Lane (0, 0) after chi, before iota.
    pub chi_lane_0: u64,
    /// The output state.
    pub output: [u64; 25],
}

/// Round `round` of Keccak-f[1600] on `lanes`, with the values a round component commits.
pub fn keccak_round(lanes: &[u64; 25], round: usize) -> RoundValues {
    let c: [u64; 5] = core::array::from_fn(|x| (0..5).fold(0, |acc, y| acc ^ lanes[x + 5 * y]));
    let d: [u64; 5] = core::array::from_fn(|x| c[(x + 4) % 5] ^ c[(x + 1) % 5].rotate_left(1));
    let c_prime: [u64; 5] = core::array::from_fn(|x| c[x] ^ d[x]);
    let a_prime: [u64; 25] = core::array::from_fn(|lane| lanes[lane] ^ d[lane % 5]);
    let b: [u64; 25] = core::array::from_fn(|lane| {
        let (x0, y0) = pi_source(lane % 5, lane / 5);
        a_prime[x0 + 5 * y0].rotate_left(RHO_OFFSETS[x0][y0] as u32)
    });
    let mut output: [u64; 25] = core::array::from_fn(|lane| {
        let (x, y) = (lane % 5, lane / 5);
        b[lane] ^ (!b[(x + 1) % 5 + 5 * y] & b[(x + 2) % 5 + 5 * y])
    });
    let chi_lane_0 = output[0];
    output[0] ^= ROUND_CONSTANTS[round];
    RoundValues { c, c_prime, a_prime, chi_lane_0, output }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pi_source_inverts_pi() {
        for x0 in 0..5 {
            for y0 in 0..5 {
                assert_eq!(pi_source(y0, (2 * x0 + 3 * y0) % 5), (x0, y0));
            }
        }
    }

    #[test]
    fn test_limbs_round_trip() {
        let lanes: [u64; 25] =
            core::array::from_fn(|i| 0x9e37_79b9_7f4a_7c15u64.wrapping_mul(i as u64 + 1));
        assert_eq!(limbs_to_lanes(&lanes_to_limbs(&lanes)), lanes);
        assert_eq!(bits_of_limbs().iter().map(Vec::len).sum::<usize>(), 1600);
    }

    #[test]
    fn test_keccak_f_on_zero() {
        let mut lanes = [0u64; 25];
        for round in 0..N_ROUNDS {
            lanes = keccak_round(&lanes, round).output;
        }
        assert_eq!(lanes[0], 0xf1258f7940e1dde7);
        assert_eq!(lanes[24], 0xeaf1ff7b5ceca249);
    }
}
