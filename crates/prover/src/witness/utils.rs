use std::sync::atomic::{AtomicU32, Ordering};

use bytemuck::Zeroable;
use cairo_air::claims::CairoClaim;
use itertools::Itertools;
use num_traits::{One, Zero};
use stwo::core::channel::MerkleChannel;
use stwo::core::fields::m31::M31;
use stwo::core::pcs::{TreeSubspan, TreeVec};
use stwo::core::utils::SliceExt;
use stwo::core::vcs_lifted::blake2_merkle::{Blake2sM31MerkleChannel, Blake2sMerkleChannel};
use stwo::prover::backend::simd::conversion::{Pack, Unpack};
use stwo::prover::backend::simd::m31::{N_LANES, PackedBaseField, PackedM31};
use stwo::prover::backend::{Backend, BackendForChannel};
use stwo::prover::poly::BitReversedOrder;
use stwo::prover::poly::circle::CircleEvaluation;
use stwo_cairo_common::preprocessed_columns::preprocessed_trace::{
    PreProcessedTrace, PreProcessedTraceVariant,
};
use stwo_constraint_framework::PREPROCESSED_TRACE_IDX;
use stwo_constraint_framework::preprocessed_columns::PreProcessedColumnId;

use crate::witness::preprocessed_trace::generate_preprocessed_commitment_root;


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

pub fn pack_values<T: Pack>(values: &[T]) -> Vec<T::SimdType> {
    values.checked_as_chunks::<N_LANES>().iter().map(|c| T::pack(*c)).collect()
}

/// A column of multiplicities for lookup arguments. Allows increasing the multiplicity at a given
/// index. This version uses atomic operations to increase the multiplicity, and is `Send`.
pub struct AtomicMultiplicityColumn {
    data: Vec<PackedM31>,
}
impl AtomicMultiplicityColumn {
    /// Creates a new `AtomicMultiplicityColumn` with the given size. The elements are initialized
    /// to 0.
    pub fn new(size: usize) -> Self {
        Self { data: vec![PackedBaseField::zeroed(); size.div_ceil(N_LANES)] }
    }

    /// Atomically increments the multiplicity address by 1.
    ///
    /// # Safety
    /// Caller must ensure `address` is in bounds for the column (no bounds check is performed).
    pub fn increase_at(&self, address: u32) {
        let ptr = unsafe { (self.data.as_ptr() as *mut u32).add(address as usize) };
        unsafe { AtomicU32::from_ptr(ptr).fetch_add(1, Ordering::Relaxed) };
    }

    /// Returns the internal data as a Vec<PackedM31>. The last element of the vector is padded with
    /// zeros if needed. This function performs a copy on the inner data, If atomics are not
    /// necessary, use [`MultiplicityColumn`] instead.
    pub fn into_simd_vec(self) -> Vec<PackedM31> {
        self.data
    }
}

/// The enabler column is a column of length `padding_offset.next_power_of_two()` where
/// 1. The first `padding_offset` elements are set to 1;
/// 2. The rest are set to 0.
#[derive(Debug, Clone)]
pub struct Enabler {
    pub padding_offset: usize,
}
impl Enabler {
    pub const fn new(padding_offset: usize) -> Self {
        Self { padding_offset }
    }

    pub fn packed_at(&self, vec_row: usize) -> PackedM31 {
        let row_offset = vec_row * N_LANES;
        if self.padding_offset <= row_offset {
            return PackedM31::zero();
        }
        if self.padding_offset >= row_offset + N_LANES {
            return PackedM31::one();
        }

        // The row is partially enabled.
        let mut res = [M31::zero(); N_LANES];
        for v in res.iter_mut().take(self.padding_offset - row_offset) {
            *v = M31::one();
        }
        PackedM31::from_array(res)
    }
}

/// Extenders of a commitment-tree with evaluations.
pub trait TreeBuilder<B: Backend> {
    fn extend_evals(
        &mut self,
        columns: Vec<CircleEvaluation<B, M31, BitReversedOrder>>,
    ) -> TreeSubspan;
}

impl<B: BackendForChannel<MC>, MC: MerkleChannel> TreeBuilder<B>
    for stwo::prover::TreeBuilder<'_, '_, B, MC>
{
    fn extend_evals(
        &mut self,
        columns: Vec<CircleEvaluation<B, M31, BitReversedOrder>>,
    ) -> TreeSubspan {
        self.extend_evals(columns)
    }
}

fn tree_trace_cells(tree_log_sizes: TreeVec<Vec<u32>>) -> Vec<u64> {
    tree_log_sizes
        .iter()
        .map(|tree| tree.iter().map(|col_log_size| 1 << col_log_size).sum::<u64>())
        .collect()
}

/// Returns the number of cells in each trace interaction of the witness.
/// Preprocess trace is determined by the `pp_trace` parameter (and not by the claim).
pub fn witness_trace_cells(claim: &CairoClaim, pp_trace: &PreProcessedTrace) -> Vec<u64> {
    let mut log_sizes = claim.log_sizes();
    log_sizes.insert(PREPROCESSED_TRACE_IDX, pp_trace.log_sizes());

    tree_trace_cells(log_sizes)
}

/// Exports the preprocessed roots for both Blake2s and Poseidon252 channels.
/// Note: This function is very slow and is intended for generating the preprocessed roots when
/// needed.
pub fn export_preprocessed_roots() {
    let max_log_blowup_factor = 2;

    // Blake2s roots
    for log_blowup_factor in 1..=max_log_blowup_factor {
        let root = generate_preprocessed_commitment_root::<Blake2sMerkleChannel>(
            log_blowup_factor,
            PreProcessedTraceVariant::Canonical,
            0,
        );
        let root_bytes = root.0;
        let u32s_hex = root_bytes
            .checked_as_chunks::<4>()
            .iter()
            .map(|&bytes| format!("{:#010x}", u32::from_le_bytes(bytes)))
            .collect_vec()
            .join(", ");

        println!("log_blowup_factor: {log_blowup_factor}, blake root: [{u32s_hex}]");
    }

    // Poseidon252 roots.
    #[cfg(not(any(target_arch = "wasm32", target_arch = "wasm64")))]
    {
        use stwo::core::vcs_lifted::poseidon252_merkle::Poseidon252MerkleChannel;
        // Poseidon252 roots.
        for log_blowup_factor in 1..=max_log_blowup_factor {
            let root = generate_preprocessed_commitment_root::<Poseidon252MerkleChannel>(
                log_blowup_factor,
                PreProcessedTraceVariant::CanonicalWithoutPedersen,
                0,
            );
            println!("log_blowup_factor: {log_blowup_factor}, poseidon root: [{root:#010x}]");
        }
    }
}

/// Exports the preprocessed roots for the circuit cairo verifier.
/// Note: This function is very slow and is intended for generating the preprocessed roots when
/// needed.
pub fn export_circuit_cairo_verifier_preprocessed_roots() {
    let max_log_blowup_factor = 3;

    for log_blowup_factor in 1..=max_log_blowup_factor {
        let root = generate_preprocessed_commitment_root::<Blake2sM31MerkleChannel>(
            log_blowup_factor,
            PreProcessedTraceVariant::CanonicalSmall,
            20 + log_blowup_factor,
        );

        let root_bytes = root.0;
        let u32s = root_bytes
            .checked_as_chunks::<4>()
            .iter()
            .map(|&bytes| format!("{:}", u32::from_le_bytes(bytes)))
            .collect_vec()
            .join(", ");

        println!("log_blowup_factor: {log_blowup_factor}, blake root: [{u32s}]");
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
    let log_size = columns[0].log_size();
    assert!(
        columns.iter().all(|c| c.log_size() == log_size),
        "input_to_row columns of different sizes"
    );
    InputToRow::new(1 << log_size, |i, row| {
        columns[i].packed_at(row / N_LANES).to_array()[row % N_LANES].0 as u64
    })
}

pub trait AddInputs {
    type PackedInputType: Unpack<CpuType = Self::InputType>;
    type InputType: Pack<SimdType = Self::PackedInputType>;

    fn add_packed_inputs(&self, packed_inputs: &[Self::PackedInputType], _relation_index: usize);
    fn add_input(&self, input: &Self::InputType, _relation_index: usize);
}

pub fn add_inputs<C, Packed, Unpacked>(
    component: &C,
    packed_inputs: &[Packed],
    num_inputs: usize,
    relation_index: usize,
) where
    C: AddInputs<PackedInputType = Packed, InputType = Unpacked>,
    Packed: Unpack<CpuType = Unpacked>,
    Unpacked: Pack<SimdType = Packed>,
{
    let full_packed_inputs = num_inputs / N_LANES;
    component.add_packed_inputs(&packed_inputs[..full_packed_inputs], relation_index);
    let remainder = num_inputs % N_LANES;
    if remainder > 0 {
        let last_block = packed_inputs[full_packed_inputs];
        let unpacked_last_block = last_block.unpack();
        for input in unpacked_last_block.iter().take(remainder) {
            component.add_input(input, relation_index);
        }
    }
}

/// Packs a slice of unpacked inputs and concatenates it to the already packed inputs
///
/// Used to handle "remainder inputs": When a caller wants to add an amount of inputs
/// that is not a multiple of N_LANES, the remainder is saved in the callee's
/// remainder_inputs field. After all callers are done, this function is used to add
/// these inputs to the main vector of packed inputs. Note that if there are multiple
/// callers, remainder_inputs may grow to > N_LANES elements.
pub fn add_remainder<PackedInput, UnpackedInput>(
    packed_inputs: &mut Vec<PackedInput>,
    remainder: &[UnpackedInput],
) where
    PackedInput: Unpack,
    UnpackedInput: Pack<SimdType = PackedInput>,
{
    if remainder.is_empty() {
        return;
    }

    let mut padded = remainder.to_owned();
    padded.resize(remainder.len().next_multiple_of(N_LANES), remainder[0]);
    for chunk in padded.into_iter().array_chunks() {
        packed_inputs.push(UnpackedInput::pack(chunk));
    }
}

#[cfg(test)]
mod tests {
    use rand::rngs::SmallRng;
    use rand::{Rng, SeedableRng};
    use rayon::iter::{IntoParallelIterator, ParallelIterator};
    use stwo::core::fields::m31::M31;
    use stwo::core::pcs::TreeVec;
    use stwo::prover::backend::simd::m31::N_LANES;

    use super::Enabler;
    use crate::witness::utils::tree_trace_cells;

    #[test]
    fn test_atomic_multiplicities_column() {
        let size = N_LANES;
        let n_loops = 10;
        let col = super::AtomicMultiplicityColumn::new(size);
        let n_threads = 32;

        (0..n_threads).into_par_iter().for_each(|_| {
            (0..size * n_loops).for_each(|i| col.increase_at((i % size) as u32));
        });
        let result = col.into_simd_vec().into_iter().flat_map(|p| p.to_array().map(|v| v.0));

        for value in result {
            assert_eq!(value, n_threads * n_loops as u32);
        }
    }

    #[test]
    fn test_multiplicities_column_into_simd() {
        let mut rng = SmallRng::seed_from_u64(0u64);
        let expected_length = 6;
        let cpu_length = expected_length * N_LANES - 2;

        let multiplicity_column = super::AtomicMultiplicityColumn::new(cpu_length);
        (0..10 * N_LANES).for_each(|_| {
            let addr = rng.random_range(0..cpu_length as u32);
            multiplicity_column.increase_at(addr);
        });
        let actual_length = multiplicity_column.into_simd_vec().len();

        assert_eq!(actual_length, expected_length);
    }

    #[test]
    fn test_enabler_packed_at_single_row() {
        let n_calls = 1;

        let enabler_column = Enabler::new(n_calls);

        assert_eq!(
            enabler_column.packed_at(0).to_array(),
            [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0].map(M31::from)
        );
    }

    #[test]
    fn test_enabler_packed_row_n_lanes() {
        let enabler_column = Enabler::new(N_LANES);

        assert_eq!(enabler_column.packed_at(0).to_array(), [1; N_LANES].map(M31::from));
        assert_eq!(enabler_column.packed_at(1).to_array(), [0; N_LANES].map(M31::from));
    }

    #[test]
    fn test_enabler_packed_at() {
        let n_calls = 30;

        let enabler_column = Enabler::new(n_calls);

        assert_eq!(enabler_column.packed_at(0).to_array(), [1; N_LANES].map(M31::from));
        assert_eq!(
            enabler_column.packed_at(1).to_array(),
            [1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0].map(M31::from)
        );
        assert_eq!(enabler_column.packed_at(2).to_array(), [0; N_LANES].map(M31::from));
    }

    #[test]
    fn test_tree_trace_cells() {
        let tree_sizes = TreeVec::new(vec![vec![1, 2], vec![3, 4], vec![5]]);

        let result = tree_trace_cells(tree_sizes);

        assert_eq!(result, vec![6, 24, 32]);
    }
}
