//! A plain Keccak-f[1600], to check the AIRs against.

use stwo_cairo_common::keccak::*;

/// Applies round `round` of Keccak-f[1600] to `lanes` (lane `x + 5 y`).
pub fn reference_round(lanes: &mut [u64; 25], round: usize) {
    let c: [u64; 5] = std::array::from_fn(|x| (0..5).fold(0, |acc, y| acc ^ lanes[x + 5 * y]));
    for x in 0..5 {
        let d = c[(x + 4) % 5] ^ c[(x + 1) % 5].rotate_left(1);
        for y in 0..5 {
            lanes[x + 5 * y] ^= d;
        }
    }
    let mut b = [0u64; 25];
    for x in 0..5 {
        for y in 0..5 {
            b[y + 5 * ((2 * x + 3 * y) % 5)] =
                lanes[x + 5 * y].rotate_left(RHO_OFFSETS[x][y] as u32);
        }
    }
    for x in 0..5 {
        for y in 0..5 {
            lanes[x + 5 * y] = b[x + 5 * y] ^ (!b[(x + 1) % 5 + 5 * y] & b[(x + 2) % 5 + 5 * y]);
        }
    }
    lanes[0] ^= ROUND_CONSTANTS[round];
}

/// Keccak-f[1600].
pub fn keccak_f(lanes: &mut [u64; 25]) {
    for round in 0..N_ROUNDS {
        reference_round(lanes, round);
    }
}

#[test]
fn test_reference_keccak_f_on_zero() {
    let mut lanes = [0u64; 25];
    keccak_f(&mut lanes);
    assert_eq!(lanes[0], 0xf1258f7940e1dde7);
    assert_eq!(lanes[24], 0xeaf1ff7b5ceca249);
}

/// The 8 felts (low 128 bits, high 72 bits) whose little-endian concatenation is `lanes`.
pub fn lanes_to_felts(lanes: &[u64; 25]) -> [(u128, u128); N_FELTS] {
    std::array::from_fn(|felt| {
        let (mut low, mut high) = (0u128, 0u128);
        for bit in 0..BITS_PER_FELT {
            let k = felt * BITS_PER_FELT + bit;
            if (lanes[k / 64] >> (k % 64)) & 1 == 1 {
                if bit < 128 {
                    low |= 1 << bit;
                } else {
                    high |= 1 << (bit - 128);
                }
            }
        }
        (low, high)
    })
}
