//! The Cairo VM's Keccak builtin: each instance holds 16 memory cells, 8 input felts and 8 output
//! felts of 200 bits each, with `output = Keccak-f[1600](input)` on their 1600-bit concatenation.
//!
//! Three components prove it, in the pattern of the Poseidon builtin:
//! - `KeccakBuiltin`, one row per instance: reads the 16 memory ids and calls the aggregator.
//! - `KeccakAggregator`, one row per instance: reads the inputs below 2^200, packs them into the 64
//!   limbs of `stwo_cairo_common::keccak`, runs the 24-round chain and verifies the outputs.
//! - `KeccakRound`, one row per round, bit-sliced as in Plonky3's keccak-air (see its docs).
//!
//! A permutation costs 24 round rows of 2406 columns, plus 433 aggregator and 52 builtin columns:
//! about 58 thousand trace cells. As in Poseidon, every padding row of the aggregator runs its own
//! chain, so a proof pays about 120 thousand cells per permutation (measured from 21 to 168
//! chained permutations), against about 1.2 million for cairo-lang's `packed_keccak_func`.
pub mod keccak_aggregator;
pub mod keccak_builtin;
#[cfg(test)]
mod keccak_builtin_test;
pub mod keccak_round;
#[cfg(test)]
mod keccak_round_test;
#[cfg(test)]
mod reference;
