//! Keccak-f[1600] rounds for the Keccak builtin's witness, on the state between rounds (the 64
//! limbs of `stwo_cairo_common::keccak`).

use stwo::prover::backend::simd::conversion::{Pack, Unpack};
use stwo::prover::backend::simd::m31::PackedM31;
use stwo_cairo_common::keccak::{N_LIMBS, keccak_round, lanes_to_limbs, limbs_to_lanes};

use crate::witness::prelude::M31;

pub struct KeccakRound {}
impl KeccakRound {
    pub fn deduce_output(
        chain: M31,
        round: M31,
        state: [M31; N_LIMBS],
    ) -> (M31, M31, [M31; N_LIMBS]) {
        let lanes = limbs_to_lanes(&state.map(|limb| limb.0));
        let output = keccak_round(&lanes, round.0 as usize).output;
        (chain, round + M31::from(1), lanes_to_limbs(&output).map(M31::from))
    }
}

pub struct PackedKeccakRound {}
impl PackedKeccakRound {
    pub fn deduce_output(
        input: (PackedM31, PackedM31, [PackedM31; N_LIMBS]),
    ) -> (PackedM31, PackedM31, [PackedM31; N_LIMBS]) {
        <_ as Pack>::pack(
            input
                .unpack()
                .map(|(chain, round, state)| KeccakRound::deduce_output(chain, round, state)),
        )
    }
}

#[cfg(test)]
mod tests {
    use stwo_cairo_common::keccak::N_ROUNDS;

    use super::*;

    #[test]
    fn test_keccak_f_on_zero() {
        let mut state = [M31::from(0); N_LIMBS];
        for round in 0..N_ROUNDS {
            state = KeccakRound::deduce_output(M31::from(0), M31::from(round as u32), state).2;
        }
        let lanes = limbs_to_lanes(&state.map(|limb| limb.0));
        assert_eq!(lanes[0], 0xf1258f7940e1dde7);
        assert_eq!(lanes[24], 0xeaf1ff7b5ceca249);
    }
}
