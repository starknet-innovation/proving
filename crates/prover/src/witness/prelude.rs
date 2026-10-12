pub use std::array::from_fn;
pub use std::iter::zip;
pub use std::simd::Simd;
pub use std::sync::atomic::{AtomicU32, Ordering};
pub use std::sync::{Arc, Mutex};

pub(crate) use cairo_air::relations;
pub use dashmap::DashMap;
pub use num_traits::{One, Zero};
pub use rayon::prelude::*;
pub use stwo::core::fields::FieldExpOps;
pub use stwo::core::fields::m31::M31;
pub use stwo::core::poly::circle::CanonicCoset;
pub use stwo::prover::backend::Column;
pub use stwo::prover::backend::simd::SimdBackend;
pub use stwo::prover::backend::simd::column::BaseColumn;
pub use stwo::prover::backend::simd::conversion::Unpack;
pub use stwo::prover::backend::simd::m31::{LOG_N_LANES, N_LANES, PackedM31};
pub use stwo::prover::backend::simd::qm31::PackedQM31;
pub use stwo::prover::poly::BitReversedOrder;
pub use stwo::prover::poly::circle::CircleEvaluation;
pub use stwo_air_utils::trace::component_trace::ComponentTrace;
pub use stwo_air_utils_derive::{IterMut, ParIterMut, Uninitialized};
pub use stwo_cairo_common::preprocessed_columns::bitwise_xor::BitwiseXor;
pub use stwo_cairo_common::preprocessed_columns::pedersen::{
    PedersenPointsWindowBits9, PedersenPointsWindowBits18,
};
pub use stwo_cairo_common::preprocessed_columns::poseidon::PoseidonRoundKeys;
pub use stwo_cairo_common::preprocessed_columns::preprocessed_trace::{
    PreProcessedColumn, PreProcessedTrace, Seq,
};
pub use stwo_cairo_common::prover_types::cpu::*;
pub use stwo_cairo_common::prover_types::simd::*;
pub use stwo_constraint_framework::preprocessed_columns::PreProcessedColumnId;
pub use stwo_constraint_framework::{LogupTraceGenerator, Relation};

pub use crate::witness::fast_deduction::blake::{
    BlakeRound, PackedBlakeG, PackedBlakeRoundSigma, PackedTripleXor32,
};
pub use crate::witness::fast_deduction::ec_op::PackedPartialEcMulGeneric;
pub use crate::witness::fast_deduction::pedersen::{
    PackedPartialEcMulWindowBits9, PackedPartialEcMulWindowBits18,
    PackedPedersenPointsTableWindowBits9, PackedPedersenPointsTableWindowBits18,
};
pub use crate::witness::fast_deduction::poseidon::{
    PackedCube252, PackedPoseidon3PartialRoundsChain, PackedPoseidonFullRoundChain,
    PackedPoseidonRoundKeys,
};
pub use crate::witness::utils::{HashMap, InputToRow, *};

// a8 (Q1017/Q1018): lookup tuples held from the trace to the interaction trace, narrowed to the bits their
// values occupy (the AIR range-checks them: XOR operands below 2^16, felt252 limbs below 2^9).

/// `[relation, a, b, a ^ b]` -> `a | b << 16`.
pub fn narrow_xor4([_, a, b, c]: [PackedM31; 4]) -> std::simd::u32x16 {
    let (a, b) = (a.into_simd(), b.into_simd());
    debug_assert_eq!(a ^ b, c.into_simd());
    a | (b << 16)
}

pub fn widen_xor4(w: std::simd::u32x16, relation: u32) -> [PackedM31; 4] {
    let (a, b) = (w & std::simd::u32x16::splat(0xFFFF), w >> 16);
    // a, b < 2^16 < P, so the reduction is the identity.
    [
        PackedM31::broadcast(M31::from(relation)),
        PackedM31::reduce_simd(a),
        PackedM31::reduce_simd(b),
        PackedM31::reduce_simd(a ^ b),
    ]
}

/// `[relation, id, 28 limbs of 9 bits]` -> `[id, 10 words of three limbs]`.
pub fn narrow_big(t: [PackedM31; 30]) -> [std::simd::u32x16; 11] {
    std::array::from_fn(|w| {
        if w == 0 {
            return t[1].into_simd();
        }
        let limb = |k: usize| if k < 28 { t[2 + k].into_simd() } else { std::simd::u32x16::splat(0) };
        let k = 3 * (w - 1);
        debug_assert!((0..3).all(|j| k + j >= 28 || limb(k + j) == (limb(k + j) & std::simd::u32x16::splat(0x1FF))));
        limb(k) | (limb(k + 1) << 9) | (limb(k + 2) << 18)
    })
}

pub fn widen_big(w: [std::simd::u32x16; 11], relation: u32) -> [PackedM31; 30] {
    std::array::from_fn(|i| match i {
        0 => PackedM31::broadcast(M31::from(relation)),
        1 => PackedM31::reduce_simd(w[0]),
        _ => {
            let k = i - 2;
            // A limb is below 2^9 < P, so the reduction is the identity.
            PackedM31::reduce_simd((w[1 + k / 3] >> (9 * (k % 3) as u32)) & std::simd::u32x16::splat(0x1FF))
        }
    })
}
