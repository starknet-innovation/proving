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
