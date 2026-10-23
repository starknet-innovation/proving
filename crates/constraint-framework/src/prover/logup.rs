use std::sync::LazyLock;

use itertools::{Itertools, multizip};
use num_traits::Zero;
#[cfg(feature = "parallel")]
use rayon::prelude::*;
use stwo::core::ColumnVec;
use stwo::core::fields::m31::BaseField;
use stwo::core::fields::qm31::{SECURE_EXTENSION_DEGREE, SecureField};
use stwo::core::poly::circle::CanonicCoset;
use stwo::core::utils::uninit_vec;
use stwo::prover::backend::Column;
use stwo::prover::backend::simd::SimdBackend;
use stwo::prover::backend::simd::column::{BaseColumn, SecureColumn};
use stwo::prover::backend::simd::m31::{LOG_N_LANES, N_LANES, PackedBaseField};
use stwo::prover::backend::simd::prefix_sum::inclusive_prefix_sum;
use stwo::prover::backend::simd::qm31::{PackedSecureField, batch_inverse_packed_qm31};
use stwo::prover::mempool::BaseColumnPool;
use stwo::prover::poly::BitReversedOrder;
use stwo::prover::poly::circle::CircleEvaluation;
use stwo::prover::secure_column::SecureColumnByCoords;

/// The pool the interaction writers' output columns come from and go back to.
///
/// Every interaction column was allocated fresh and written by its writer, then interpolated out
/// of place into a second fresh buffer: twice the interaction trace in pages the kernel had to
/// zero and map inside the phase, on a window that already left workers idle. Through this pool
/// the buffer a writer's columns were interpolated out of (which the prover's interpolation gives
/// back here) becomes the next writer's output column, or the next column's coefficients, as the
/// commitment's FFT buffers already circulate through its pool: the pages are resident, mapped by
/// an earlier writer of the same phase. Every element of a taken buffer is written before it is
/// read, exactly as the fresh buffer it replaces was, so no value depends on which buffer a column
/// lands in. The prover releases the idle buffers once the phase's writers are done
/// ([`BaseColumnPool::release_all_idle`]), so nothing stays resident past the phase that the
/// allocator's cache would not have kept.
static INTERACTION_POOL: LazyLock<BaseColumnPool<SimdBackend>> = LazyLock::new(BaseColumnPool::new);

/// The pool the interaction writers draw their output columns from; the prover gives the
/// buffers their interpolation frees back to it and releases it after the writers.
pub fn interaction_pool() -> &'static BaseColumnPool<SimdBackend> {
    &INTERACTION_POOL
}

// SIMD backend generator for logup interaction trace.
pub struct LogupTraceGenerator {
    log_size: u32,
    /// Current allocated interaction columns.
    trace: Vec<SecureColumnByCoords<SimdBackend>>,
    /// Denominator expressions (z + sum_i alpha^i * x_i) being generated for the current lookup.
    /// Allocated by [`Self::new`], or on the first column that writes denominators
    /// ([`Self::new_col`], [`Self::col_from_par_iter`]); [`Self::write_frac_cols`] never uses it.
    denom: SecureColumn,
    batch_inverse_buffer: Vec<PackedSecureField>,
}
impl LogupTraceGenerator {
    pub fn new(log_size: u32) -> Self {
        let trace = vec![];
        let denom = SecureColumn::zeros(1 << log_size);
        let batch_inverse_buffer = unsafe { uninit_vec(1 << (log_size - LOG_N_LANES)) };
        Self { log_size, trace, denom, batch_inverse_buffer }
    }

    /// # Safety
    ///
    /// Calling `finalize_last` on uninitialized LogupTraceGenerator leads to undefined behavior.
    pub unsafe fn uninitialized(log_size: u32) -> Self {
        // The denominators and the inverse buffer (together eight times a column) are allocated
        // by the first column that writes them: a writer that uses `write_frac_cols` alone never
        // does, and its mapping of them only counted against the allocator's idle budget.
        let trace = vec![];
        let denom = SecureColumn { data: vec![], length: 0 };
        let batch_inverse_buffer = vec![];
        Self { log_size, trace, denom, batch_inverse_buffer }
    }

    /// The denominators and the inverse buffer, allocated (uninitialized, as by
    /// [`Self::uninitialized`]) if this is the first column that writes them. Every row of the
    /// denominators is written by the column's fractions before the batch inverse reads it.
    fn ensure_denominators(&mut self) {
        let log_size = self.log_size;
        if self.denom.len() != 1 << log_size {
            self.denom = unsafe { SecureColumn::uninitialized(1 << log_size) };
        }
        if self.batch_inverse_buffer.len() != 1 << (log_size - LOG_N_LANES) {
            self.batch_inverse_buffer = unsafe { SecureColumn::uninitialized(1 << log_size) }.data;
        }
    }

    /// Allocate a new lookup column.
    pub fn new_col(&mut self) -> LogupColGenerator<'_> {
        let log_size = self.log_size;
        self.ensure_denominators();
        // Every value of the column is written (`write_frac`, every row) before `finalize_col`
        // reads it: its buffer is taken unzeroed, from the interaction pool where one is idle.
        let pool = interaction_pool();
        let numerator = SecureColumnByCoords::<SimdBackend> {
            columns: std::array::from_fn(|_| pool.take_or_fresh(log_size)),
        };
        LogupColGenerator { trace_gen: self, numerator }
    }

    pub fn col_from_iter(
        &mut self,
        iter: impl ExactSizeIterator<Item = (PackedSecureField, PackedSecureField)>,
    ) {
        let length = 1 << self.log_size;
        assert_eq!(iter.len() * N_LANES, length);
        let mut col_gen = self.new_col();
        for (vec_row, (numerator, denom)) in iter.enumerate() {
            col_gen.write_frac(vec_row, numerator, denom);
        }
        col_gen.finalize_col();
    }

    /// Generate consecutive logup columns in one pass over row chunks.
    /// The closure returns (numerator, denominator) for a column and packed row.
    pub fn write_frac_cols(
        &mut self,
        n_columns: usize,
        fraction: impl Fn(usize, usize) -> (PackedSecureField, PackedSecureField) + Sync,
    ) {
        if n_columns == 0 {
            return;
        }
        // 128 packed rows per chunk: the chunk's four temporaries (numerators, denominators,
        // inverses and running sums, 128 secure packed values each) take 128 KiB, which stays in
        // a core's L2 through the three passes even when two threads share a 512 KiB L2. At 1024
        // rows they take 1 MiB and go out to L3 between passes on such cores. The chunking does
        // not change any value: each row's fraction and running sum are computed alike.
        const CHUNK_SIZE: usize = 128;
        let n_rows: usize = 1 << (self.log_size - LOG_N_LANES);
        // Every value of the columns is written below (each task writes every row of its chunk of
        // every column): their buffers are taken as the commitment's FFT buffers are, unzeroed,
        // from the interaction pool where one is idle (see [`interaction_pool`]).
        let pool = interaction_pool();
        let mut columns: Vec<_> = (0..n_columns)
            .map(|_| SecureColumnByCoords::<SimdBackend> {
                columns: std::array::from_fn(|_| pool.take_or_fresh(self.log_size)),
            })
            .collect();
        let mut tasks: Vec<Vec<_>> =
            (0..n_rows.div_ceil(CHUNK_SIZE)).map(|_| Vec::with_capacity(n_columns)).collect();
        for column in &mut columns {
            for (task, chunk) in tasks.iter_mut().zip(column.chunks_mut(CHUNK_SIZE)) {
                task.push(chunk);
            }
        }
        let previous = self.trace.last();
        #[cfg(feature = "parallel")]
        let tasks = tasks.into_par_iter();
        #[cfg(not(feature = "parallel"))]
        let tasks = tasks.into_iter();
        tasks.enumerate().for_each(|(chunk_index, mut outputs)| {
            let start = chunk_index * CHUNK_SIZE;
            let len = n_rows - start;
            let len = len.min(CHUNK_SIZE);
            let mut numerators = vec![PackedSecureField::zero(); len];
            let mut denominators = vec![PackedSecureField::zero(); len];
            let mut inverses = vec![PackedSecureField::zero(); len];
            let mut running: Vec<_> = (start..start + len)
                .map(|row| {
                    previous.map_or_else(PackedSecureField::zero, |col| {
                        PackedSecureField::from_packed_m31s(
                            col.columns.each_ref().map(|coord| coord.data[row]),
                        )
                    })
                })
                .collect();
            for (col, output) in outputs.iter_mut().enumerate() {
                for row in 0..len {
                    (numerators[row], denominators[row]) = fraction(col, start + row);
                }
                batch_inverse_packed_qm31(&denominators, &mut inverses);
                for row in 0..len {
                    running[row] = numerators[row] * inverses[row] + running[row];
                    for (coord, value) in output.0.iter_mut().zip(running[row].into_packed_m31s()) {
                        coord.0[row] = value;
                    }
                }
            }
        });
        self.trace.extend(columns);
    }

    #[cfg(feature = "parallel")]
    pub fn col_from_par_iter(
        &mut self,
        iter: impl IndexedParallelIterator<Item = (PackedSecureField, PackedSecureField)>,
    ) {
        let length = 1 << self.log_size;
        assert_eq!(iter.len() * N_LANES, length);
        self.ensure_denominators();

        let (((c0, c1), c2), c3) = (self.denom.data.par_iter_mut())
            .zip(iter)
            .map(|(dst_denom, (numerator, denom))| {
                *dst_denom = denom;
                let [c0, c1, c2, c3] = numerator.into_packed_m31s();
                (((c0, c1), c2), c3)
            })
            .unzip();
        let columns = [c0, c1, c2, c3].map(BaseColumn::from_simd);
        let numerator = SecureColumnByCoords::<SimdBackend> { columns };

        LogupColGenerator { trace_gen: self, numerator }.finalize_col();
    }

    /// Finalize the trace. Returns the trace and the total sum of the last column.
    /// The last column is shifted by the cumsum_shift.
    pub fn finalize_last(
        mut self,
    ) -> (ColumnVec<CircleEvaluation<SimdBackend, BaseField, BitReversedOrder>>, SecureField) {
        let last_col_coords = self.trace.pop().unwrap().columns;

        // Compute cumsum_shift.
        let coordinate_sums = last_col_coords
            .each_ref()
            .map(|c| c.data.iter().copied().sum::<PackedBaseField>().pointwise_sum());
        let claimed_sum = SecureField::from_m31_array(coordinate_sums);
        let cumsum_shift = claimed_sum / BaseField::from_u32_unchecked(1 << self.log_size);
        let packed_cumsum_shift = PackedSecureField::broadcast(cumsum_shift);

        // Each coordinate is shifted and prefix-summed on its own; the four are independent
        // columns, so they run as separate tasks.
        let shifted_prefix_sum = |(mut c, shift): (BaseColumn, PackedBaseField)| {
            c.data.iter_mut().for_each(|x| *x -= shift);
            inclusive_prefix_sum(c)
        };
        let coords = last_col_coords.into_iter().zip(packed_cumsum_shift.into_packed_m31s());
        #[cfg(feature = "parallel")]
        let coord_prefix_sum: [BaseColumn; SECURE_EXTENSION_DEGREE] = coords
            .collect::<Vec<_>>()
            .into_par_iter()
            .map(shifted_prefix_sum)
            .collect::<Vec<_>>()
            .try_into()
            .unwrap();
        #[cfg(not(feature = "parallel"))]
        let coord_prefix_sum: [BaseColumn; SECURE_EXTENSION_DEGREE] =
            coords.map(shifted_prefix_sum).collect::<Vec<_>>().try_into().unwrap();
        let secure_prefix_sum = SecureColumnByCoords { columns: coord_prefix_sum };
        self.trace.push(secure_prefix_sum);
        let trace = self
            .trace
            .into_iter()
            .flat_map(|eval| {
                eval.columns.map(|col| {
                    CircleEvaluation::new(CanonicCoset::new(self.log_size).circle_domain(), col)
                })
            })
            .collect_vec();
        (trace, claimed_sum)
    }
}

/// Trace generator for a single lookup column.
pub struct LogupColGenerator<'a> {
    trace_gen: &'a mut LogupTraceGenerator,
    /// Numerator expressions (i.e. multiplicities) being generated for the current lookup.
    numerator: SecureColumnByCoords<SimdBackend>,
}
impl LogupColGenerator<'_> {
    /// Write a fraction to the column at a row.
    pub fn write_frac(
        &mut self,
        vec_row: usize,
        numerator: PackedSecureField,
        denom: PackedSecureField,
    ) {
        debug_assert!(
            denom.to_array().iter().all(|x| *x != SecureField::zero()),
            "{:?}",
            ("denom at vec_row {} is zero {}", denom, vec_row)
        );
        unsafe {
            self.numerator.set_packed(vec_row, numerator);
            *self.trace_gen.denom.data.get_unchecked_mut(vec_row) = denom;
        }
    }

    /// Finalizes generating the column.
    pub fn finalize_col(mut self) {
        // Column size is a power of 2.
        let chunk_size = std::cmp::min(4, self.trace_gen.denom.data.len());
        batch_inverse_packed_qm31(
            &self.trace_gen.denom.data,
            &mut self.trace_gen.batch_inverse_buffer,
        );

        // `trace` isn't mutated until after this loop, so the last column is the same for every
        // row; look it up once instead of re-deriving it from the `Vec` on every row.
        let prev_col = self.trace_gen.trace.last();

        #[cfg(feature = "parallel")]
        let chunks_iter = {
            let denom_inv_chunks = self.trace_gen.batch_inverse_buffer.par_chunks(chunk_size);
            let numerator_chunks = self.numerator.par_chunks_mut(chunk_size);
            (numerator_chunks, denom_inv_chunks).into_par_iter()
        };

        #[cfg(not(feature = "parallel"))]
        let chunks_iter = {
            let denom_inv_chunks = self.trace_gen.batch_inverse_buffer.chunks(chunk_size);
            let numerator_chunks = self.numerator.chunks_mut(chunk_size);
            numerator_chunks.zip(denom_inv_chunks)
        };

        chunks_iter.enumerate().for_each(|(chunk_idx, (mut numerator_chunk, denom_inv_chunk))| {
            for (idx_in_chunk, denom_item) in denom_inv_chunk.iter().enumerate() {
                unsafe {
                    let vec_row = chunk_idx * chunk_size + idx_in_chunk;
                    let value = numerator_chunk.packed_at(idx_in_chunk) * *denom_item;
                    let prev_value = prev_col
                        .map(|col| col.packed_at(vec_row))
                        .unwrap_or_else(PackedSecureField::zero);
                    numerator_chunk.set_packed(idx_in_chunk, value + prev_value)
                }
            }
        });

        self.trace_gen.trace.push(self.numerator)
    }

    // TODO(Ohad): remove.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = FractionWriter<'_>> {
        let denom = self.trace_gen.denom.data.iter_mut();
        let [coord0, coord1, coord2, coord3] =
            self.numerator.columns.each_mut().map(|s| &mut s.data);
        multizip((coord0, coord1, coord2, coord3, denom))
            .map(|(n0, n1, n2, n3, d)| FractionWriter { numerator: [n0, n1, n2, n3], denom: d })
    }

    // TODO(Ohad): remove.
    #[cfg(feature = "parallel")]
    pub fn par_iter_mut(&mut self) -> impl IndexedParallelIterator<Item = FractionWriter<'_>> {
        let [coord0, coord1, coord2, coord3] =
            self.numerator.columns.each_mut().map(|s| &mut s.data);
        (coord0, coord1, coord2, coord3, &mut self.trace_gen.denom.data)
            .into_par_iter()
            .map(|(n0, n1, n2, n3, d)| FractionWriter { numerator: [n0, n1, n2, n3], denom: d })
    }
}

/// Exposes a writer for writing a fraction to a single index in a column.
// TODO(Ohad): iterate in chunks, consider VeryPacked.
pub struct FractionWriter<'a> {
    numerator: [&'a mut PackedBaseField; SECURE_EXTENSION_DEGREE],
    denom: &'a mut PackedSecureField,
}
impl FractionWriter<'_> {
    pub fn write_frac(self, numerator: PackedSecureField, denom: PackedSecureField) {
        debug_assert!(
            denom.to_array().iter().all(|x| *x != SecureField::zero()),
            "denom is zero {denom:?}"
        );
        let [c0, c1, c2, c3] = numerator.into_packed_m31s();
        *self.numerator[0] = c0;
        *self.numerator[1] = c1;
        *self.numerator[2] = c2;
        *self.numerator[3] = c3;
        *self.denom = denom;
    }
}

#[cfg(test)]
mod tests {
    use stwo::core::fields::FieldExpOps;
    use stwo::prover::backend::simd::m31::LOG_N_LANES;
    use stwo::prover::backend::simd::qm31::PackedSecureField;

    use crate::prover::logup::LogupTraceGenerator;
    use crate::{m31, qm31};

    #[test]
    fn test_frac_writer() {
        let expected_sum = (qm31!(1, 2, 3, 4) * qm31!(5, 6, 7, 8).inverse()) * m31!(1 << 6);

        let mut log_gen = LogupTraceGenerator::new(6);
        let mut col_gen = log_gen.new_col();
        for writer in col_gen.iter_mut() {
            let num = PackedSecureField::broadcast(qm31!(1, 2, 3, 4));
            let den = PackedSecureField::broadcast(qm31!(5, 6, 7, 8));
            writer.write_frac(num, den);
        }

        col_gen.finalize_col();
        let (_, sum) = log_gen.finalize_last();
        assert_eq!(sum, expected_sum);
    }

    #[test]
    fn test_col_from_iter() {
        let log_size = 8;
        let expected_sum = (qm31!(1, 2, 3, 4) * qm31!(5, 6, 7, 8).inverse()) * m31!(1 << log_size);

        let mut log_gen = LogupTraceGenerator::new(log_size);
        let col_iter = (0..1 << (log_size - LOG_N_LANES)).map(|_| {
            let num = PackedSecureField::broadcast(qm31!(1, 2, 3, 4));
            let den = PackedSecureField::broadcast(qm31!(5, 6, 7, 8));
            (num, den)
        });
        log_gen.col_from_iter(col_iter);

        let (_, sum) = log_gen.finalize_last();
        assert_eq!(sum, expected_sum);
    }

    #[cfg(feature = "parallel")]
    #[test]
    fn test_col_from_par_iter() {
        use rayon::iter::{IntoParallelIterator, ParallelIterator};

        let log_size = 8;
        let expected_sum = (qm31!(1, 2, 3, 4) * qm31!(5, 6, 7, 8).inverse()) * m31!(1 << log_size);

        let mut log_gen = LogupTraceGenerator::new(log_size);
        let col_iter = (0..1 << (log_size - LOG_N_LANES)).into_par_iter().map(|_| {
            let num = PackedSecureField::broadcast(qm31!(1, 2, 3, 4));
            let den = PackedSecureField::broadcast(qm31!(5, 6, 7, 8));
            (num, den)
        });
        log_gen.col_from_par_iter(col_iter);

        let (_, sum) = log_gen.finalize_last();
        assert_eq!(sum, expected_sum);
    }

    #[cfg(feature = "parallel")]
    #[test]
    fn test_parallel_frac_writer() {
        use std::array;

        use rayon::prelude::*;
        // Sequential version.
        let mut log_gen_seq = LogupTraceGenerator::new(6);
        let mut col_gen_seq = log_gen_seq.new_col();
        col_gen_seq.iter_mut().enumerate().for_each(|(i, writer)| {
            let num = array::from_fn(|j| qm31!(i as u32, j as u32, 0, 1));
            let den = array::from_fn(|j| qm31!(i as u32, j as u32, 2, 3));
            let [num, den] = [num, den].map(PackedSecureField::from_array);
            writer.write_frac(num, den);
        });
        col_gen_seq.finalize_col();
        let (_, sum_seq) = log_gen_seq.finalize_last();

        // Parallel version.
        let mut log_gen_par = LogupTraceGenerator::new(6);
        let mut col_gen_par = log_gen_par.new_col();
        col_gen_par.par_iter_mut().enumerate().for_each(|(i, writer)| {
            let num = array::from_fn(|j| qm31!(i as u32, j as u32, 0, 1));
            let den = array::from_fn(|j| qm31!(i as u32, j as u32, 2, 3));
            let [num, den] = [num, den].map(PackedSecureField::from_array);
            writer.write_frac(num, den);
        });
        col_gen_par.finalize_col();
        let (_, sum_par) = log_gen_par.finalize_last();

        assert_eq!(sum_seq, sum_par);
    }
}
