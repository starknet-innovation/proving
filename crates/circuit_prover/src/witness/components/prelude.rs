pub use std::array::from_fn;
pub use std::simd::num::{SimdInt, SimdUint};
pub use std::simd::{Simd, u32x16};
pub use std::sync::Arc;
pub use std::sync::atomic::{AtomicU32, Ordering};

pub use circuit_common::Qm31OpsTraceGenerator;
pub use circuit_common::preprocessed::PreProcessedTrace;
pub use circuit_verifier::circuit_claim::{ClaimedSum, ComponentLogSize};
pub use circuit_verifier::relations;
pub use itertools::{Itertools, multizip};
pub use num_traits::{One, Zero};
pub use rayon::iter::{
    IndexedParallelIterator, IntoParallelIterator, IntoParallelRefIterator,
    IntoParallelRefMutIterator, ParallelIterator,
};
pub use stwo::core::fields::m31::M31;
pub use stwo::core::fields::qm31::{QM31, SecureField};
pub use stwo::core::poly::circle::CanonicCoset;
pub use stwo::core::vcs_lifted::blake2_merkle::Blake2sM31MerkleChannel;
pub use stwo::prover::TreeBuilder;
pub use stwo::prover::backend::Col;
pub use stwo::prover::backend::simd::SimdBackend;
pub use stwo::prover::backend::simd::column::BaseColumn;
pub use stwo::prover::backend::simd::conversion::{Pack, Unpack};
pub use stwo::prover::backend::simd::m31::{LOG_N_LANES, N_LANES, PackedM31};
pub use stwo::prover::backend::simd::qm31::PackedQM31;
pub use stwo::prover::poly::BitReversedOrder;
pub use stwo::prover::poly::circle::CircleEvaluation;
pub use stwo_air_utils::trace::component_trace::ComponentTrace;
pub use stwo_air_utils_derive::{IterMut, ParIterMut, Uninitialized};
pub use stwo_cairo_common::preprocessed_columns::blake::{
    BLAKE_SIGMA, BLAKE_SIGMA_TABLE, N_BLAKE_ROUNDS, N_BLAKE_SIGMA_COLS, sigma, sigma_m31,
};
pub use stwo_cairo_common::preprocessed_columns::preprocessed_trace::{PreProcessedColumn, Seq};
pub use stwo_cairo_common::prover_types::cpu::{UInt16, UInt32};
pub use stwo_cairo_common::prover_types::simd::{
    PackedBool, PackedM31Type, PackedUInt16, PackedUInt32, SIMD_ENUMERATION_0,
};
pub use stwo_cairo_prover::witness::fast_deduction::blake::{
    G_STATE_INDICES, PackedBlakeRoundSigma, PackedTripleXor32,
};
pub use stwo_cairo_prover::witness::utils::{AtomicMultiplicityColumn, Enabler};
pub use stwo_constraint_framework::preprocessed_columns::PreProcessedColumnId;
pub use stwo_constraint_framework::{LogupTraceGenerator, Relation};

pub use crate::witness::utils::pack_values;

const NUM_INPUT_WORDS_G: usize = 6;
const NUM_OUTPUT_WORDS_G: usize = 4;

/// Local shim for stwo-cairo's `PackedBlakeG` with a public `blake_g` method
/// (upstream keeps it private).
#[derive(Debug)]
pub struct PackedBlakeG {}

impl PackedBlakeG {
    pub fn deduce_output(
        input: [PackedUInt32; NUM_INPUT_WORDS_G],
    ) -> [PackedUInt32; NUM_OUTPUT_WORDS_G] {
        PackedBlakeG::blake_g(input.map(|x| x.simd)).map(|simd| PackedUInt32 { simd })
    }

    pub fn blake_g(input: [u32x16; NUM_INPUT_WORDS_G]) -> [u32x16; NUM_OUTPUT_WORDS_G] {
        let [mut a, mut b, mut c, mut d, m0, m1] = input;

        a = a + b + m0;
        d ^= a;
        d = (d >> 16) | (d << (u32::BITS - 16));

        c += d;
        b ^= c;
        b = (b >> 12) | (b << (u32::BITS - 12));

        a = a + b + m1;
        d ^= a;
        d = (d >> 8) | (d << (u32::BITS - 8));

        c += d;
        b ^= c;
        b = (b >> 7) | (b << (u32::BITS - 7));

        [a, b, c, d]
    }
}

/// Create the input_to_row map used in const-size components.
///
/// `preprocessed_trace` - The preprocessed trace.
/// `column_ids` - PreProcessedColumnId for each input column of the component.
///
/// Returns a mapping from input tuple to its row number. Used to find
/// out which multiplicity value to update for a given input.
pub fn make_input_to_row<const N: usize>(
    preprocessed_trace: &PreProcessedTrace,
    column_ids: [PreProcessedColumnId; N],
) -> InputToRow<N> {
    let columns = column_ids.iter().map(|id| preprocessed_trace.get_column(id)).collect_vec();
    let log_size = columns[0].len().ilog2();
    assert!(
        columns.iter().all(|c| c.len().ilog2() == log_size),
        "input_to_row columns of different sizes"
    );
    InputToRow::new(1 << log_size, |i, row| columns[i][row] as u64)
}

pub fn pack_preprocessed_column(column: &[usize]) -> Vec<PackedM31> {
    let values: Vec<M31> = column.par_iter().map(|&v| M31::from(v)).collect();
    pack_values(&values)
}

/// A fast, deterministic hasher for the witness' lookup maps: their keys are small arrays of
/// field elements, and SipHash dominates both the construction of the maps and the lookups.
/// Iteration order is never relied on (the default hasher is randomized), so the choice of
/// hasher cannot change the trace.
#[derive(Default, Clone, Copy)]
pub struct FastHasher(u64);

impl std::hash::Hasher for FastHasher {
    #[inline]
    fn finish(&self) -> u64 {
        self.0
    }

    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        for chunk in bytes.chunks(8) {
            let mut word = [0u8; 8];
            word[..chunk.len()].copy_from_slice(chunk);
            self.write_u64(u64::from_le_bytes(word));
        }
    }

    #[inline]
    fn write_u32(&mut self, i: u32) {
        self.write_u64(i as u64);
    }

    #[inline]
    fn write_u64(&mut self, i: u64) {
        self.0 = (self.0.rotate_left(5) ^ i).wrapping_mul(0x517c_c1b7_2722_0a95);
    }

    #[inline]
    fn write_usize(&mut self, i: usize) {
        self.write_u64(i as u64);
    }
}

pub type FastHasherBuilder = std::hash::BuildHasherDefault<FastHasher>;

/// [`std::collections::HashMap`] with [`FastHasher`].
pub type HashMap<K, V> = std::collections::HashMap<K, V, FastHasherBuilder>;

/// The row of a const-size component's preprocessed table for an input tuple.
///
/// When the key columns are bit fields of the row index that cover all its bits (the range-check
/// and bitwise-xor tables: column `i` holds the bits `shift_i..shift_i + width_i` of the row,
/// which is checked over every row when the table is built), the row is the sum of the shifted
/// key values; otherwise it is looked up in a map.
pub struct InputToRow<const N: usize> {
    /// `(key index, shift)` of the bit fields; empty when the map is used.
    fields: Vec<(usize, u32)>,
    map: HashMap<[M31; N], usize>,
}

impl<const N: usize> InputToRow<N> {
    pub fn rows_packed(&self, input: &[PackedM31; N]) -> [u32; 16] {
        if self.fields.is_empty() {
            let lanes = input.map(|p| p.to_array());
            std::array::from_fn(|l| {
                self.row(&std::array::from_fn(|i| lanes[i][l])).try_into().unwrap()
            })
        } else {
            let mut rows = std::simd::Simd::<u32, 16>::splat(0);
            for &(i, shift) in &self.fields {
                rows += input[i].into_simd() << shift;
            }
            rows.to_array()
        }
    }

    /// Builds the lookup for a table given as `n_rows` rows of `N` key values, `at(column, row)`.
    pub fn new(n_rows: usize, at: impl Fn(usize, usize) -> u64) -> Self {
        let log_size = n_rows.ilog2();
        assert_eq!(1 << log_size, n_rows);
        let mut fields: Vec<(usize, u32)> = Vec::new();
        let mut covered: u64 = 0;
        for i in 0..N {
            // The values at the rows 1, 2, 4, ... say which bits a bit-field column would hold.
            if at(i, 0) != 0 {
                continue;
            }
            let powers: Vec<u64> = (0..log_size).map(|k| at(i, 1 << k)).collect();
            let nonzero: Vec<usize> = (0..log_size as usize).filter(|&k| powers[k] != 0).collect();
            let Some(&shift) = nonzero.first() else { continue };
            let width = nonzero.len();
            if nonzero != (shift..shift + width).collect::<Vec<_>>()
                || !(0..width).all(|j| powers[shift + j] == 1 << j)
            {
                continue;
            }
            let field_mask = ((1u64 << width) - 1) << shift;
            if covered & field_mask != 0 {
                continue;
            }
            let low_mask = (1u64 << width) - 1;
            if !(0..n_rows).all(|r| at(i, r) == ((r as u64) >> shift) & low_mask) {
                continue;
            }
            covered |= field_mask;
            fields.push((i, shift as u32));
        }
        if covered == (1u64 << log_size) - 1 {
            return Self { fields, map: HashMap::default() };
        }
        let mut map: HashMap<[M31; N], usize> =
            HashMap::with_capacity_and_hasher(n_rows, Default::default());
        for r in 0..n_rows {
            let key: [M31; N] = std::array::from_fn(|i| M31::from(at(i, r) as u32));
            map.insert(key, r);
        }
        Self { fields: Vec::new(), map }
    }

    /// The row holding `input`.
    #[inline]
    pub fn row(&self, input: &[M31; N]) -> usize {
        if self.fields.is_empty() {
            *self.map.get(input).unwrap()
        } else {
            self.fields.iter().map(|&(i, shift)| (input[i].0 as usize) << shift).sum()
        }
    }
}
