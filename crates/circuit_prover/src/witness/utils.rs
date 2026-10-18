use rayon::iter::ParallelIterator;
use rayon::slice::ParallelSlice;
use stwo::prover::backend::simd::conversion::Pack;
use stwo::prover::backend::simd::m31::N_LANES;

pub fn pack_values<T: Pack + Copy + Sync>(values: &[T]) -> Vec<T::SimdType>
where
    T::SimdType: Send,
{
    assert!(
        values.len().is_multiple_of(N_LANES),
        "pack_values: length {} is not a multiple of N_LANES ({N_LANES})",
        values.len(),
    );
    values.par_chunks_exact(N_LANES).map(|c| T::pack(c.try_into().unwrap())).collect()
}

/// The packed row of rows `first..first + N_LANES` of preprocessed columns read in place: per
/// column, the packed word [`pack_usize_column`] of the materialized column holds there.
#[inline]
pub fn packed_column_rows<const K: usize>(
    columns: &[circuit_common::preprocessed::ColumnRows<'_>; K],
    first: usize,
) -> [stwo::prover::backend::simd::m31::PackedM31; K] {
    std::array::from_fn(|k| columns[k].packed_m31_at(first))
}

/// The inputs of the packed row of rows `first..first + N_LANES`, packed as [`pack_values`] packs
/// the staged inputs of those rows.
#[inline]
pub fn packed_input<T: Pack>(input: impl Fn(usize) -> T, first: usize) -> T::SimdType {
    T::pack(std::array::from_fn(|lane| input(first + lane)))
}

/// A preprocessed column's values as packed field elements: what
/// `Col::<SimdBackend, M31>::from_iter` of their `M31::from` holds (the last packed word padded
/// with zeros), converted in parallel instead of one value after the other. At 2^23 rows the
/// qm31_ops writer converted eight such columns in series.
pub fn pack_usize_column(
    values: &[usize],
) -> Vec<stwo::prover::backend::simd::m31::PackedM31> {
    use stwo::core::fields::m31::M31;
    use stwo::prover::backend::simd::m31::PackedM31;
    values
        .par_chunks(N_LANES)
        .map(|chunk| {
            PackedM31::from_array(std::array::from_fn(|i| {
                chunk.get(i).map_or(M31::from(0u32), |&value| M31::from(value))
            }))
        })
        .collect()
}
