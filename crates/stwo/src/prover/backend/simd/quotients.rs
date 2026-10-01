use std::iter::zip;
use std::simd::u64x8;

use itertools::{Itertools, zip_eq};
use num_traits::Zero;
#[cfg(feature = "parallel")]
use rayon::iter::{IndexedParallelIterator, IntoParallelRefIterator, ParallelIterator};

use super::SimdBackend;
use super::column::CM31Column;
use super::domain::CircleDomainBitRevIterator;
use super::m31::{LOG_N_LANES, PackedBaseField};
use super::qm31::PackedSecureField;
use crate::core::circle::CirclePoint;
use crate::core::fields::FieldExpOps;
use crate::core::fields::m31::{BaseField, P};
use crate::core::fields::qm31::SecureField;
use crate::core::pcs::quotients::{ColumnSampleBatch, quotient_constants};
use crate::core::poly::circle::{CanonicCoset, CircleDomain};
use crate::prover::QuotientOps;
use crate::prover::backend::CpuBackend;
use crate::prover::backend::simd::cm31::PackedCM31;
use crate::prover::backend::simd::utils::to_lifted_simd;
use crate::prover::pcs::quotient_ops::AccumulatedNumerators;
use crate::prover::poly::BitReversedOrder;
use crate::prover::poly::circle::{CircleEvaluation, PolyOps, SecureEvaluation};
use crate::prover::poly::twiddles::{TwiddleBuffer, TwiddleTree};
use crate::prover::secure_column::SecureColumnByCoords;

pub struct QuotientConstants {
    pub line_coeffs: Vec<Vec<(SecureField, SecureField, SecureField)>>,
    pub denominator_inverses: Vec<CM31Column>,
}

impl QuotientOps for SimdBackend {
    fn accumulate_numerators(
        columns: &[&CircleEvaluation<Self, BaseField, BitReversedOrder>],
        sample_batches: &[ColumnSampleBatch],
        accumulated_numerators_vec: &mut Vec<AccumulatedNumerators<Self>>,
        log_blowup_factor: u32,
    ) {
        let domain = columns[0].domain;
        let (subdomain, _) = domain.split(log_blowup_factor);

        // Fall back to CPU for subdomains too small for SIMD.
        if subdomain.log_size() < LOG_N_LANES {
            let cpu_columns: Vec<_> = columns.iter().map(|c| c.to_cpu()).collect();
            let cpu_column_refs: Vec<_> = cpu_columns.iter().collect();
            let mut cpu_acc: Vec<AccumulatedNumerators<CpuBackend>> = vec![];
            CpuBackend::accumulate_numerators(
                &cpu_column_refs,
                sample_batches,
                &mut cpu_acc,
                log_blowup_factor,
            );
            for acc in cpu_acc {
                accumulated_numerators_vec.push(AccumulatedNumerators {
                    sample_point: acc.sample_point,
                    partial_numerators_acc: SecureColumnByCoords::from_cpu(
                        acc.partial_numerators_acc,
                    ),
                    first_linear_term_acc: acc.first_linear_term_acc,
                });
            }
            return;
        }

        let quotient_constants = quotient_constants(sample_batches);
        for (batch, coeffs) in zip(sample_batches, quotient_constants.line_coeffs) {
            let subdomain_acc =
                accumulate_numerators_on_subdomain(subdomain, batch, columns, &coeffs);
            let first_linear_term_acc: SecureField = coeffs.iter().map(|(a, ..)| a).sum();
            accumulated_numerators_vec.push(AccumulatedNumerators {
                sample_point: batch.point,
                partial_numerators_acc: subdomain_acc,
                first_linear_term_acc,
            })
        }
    }

    // TODO(Leo): Consider receiving the denominator inverses from the call site and
    // having them computed in parallel to other task.
    fn compute_quotients_and_combine(
        accumulations: Vec<AccumulatedNumerators<Self>>,
        lifting_log_size: u32,
        log_blowup_factor: u32,
        twiddles: &TwiddleTree<Self>,
    ) -> SecureEvaluation<Self, BitReversedOrder> {
        // This constant is chosen empirically by benchmarking.
        const COMBINE_CHUNK_SIZE: usize = 16;

        let eval_domain = CanonicCoset::new(lifting_log_size).circle_domain();
        let (eval_subdomain, _) = eval_domain.split(log_blowup_factor);

        // Fall back to CPU for subdomains too small for SIMD.
        if eval_subdomain.log_size() < LOG_N_LANES {
            let cpu_twiddles = CpuBackend::precompute_twiddles(eval_domain.half_coset);
            let cpu_accumulations: Vec<AccumulatedNumerators<CpuBackend>> = accumulations
                .into_iter()
                .map(|acc| AccumulatedNumerators {
                    sample_point: acc.sample_point,
                    partial_numerators_acc: acc.partial_numerators_acc.to_cpu(),
                    first_linear_term_acc: acc.first_linear_term_acc,
                })
                .collect();
            let cpu_result = CpuBackend::compute_quotients_and_combine(
                cpu_accumulations,
                lifting_log_size,
                log_blowup_factor,
                &cpu_twiddles,
            );
            return SecureEvaluation::new(
                cpu_result.domain,
                SecureColumnByCoords::from_cpu(cpu_result.values),
            );
        }
        let subdomain_points: Vec<CirclePoint<PackedBaseField>> =
            CircleDomainBitRevIterator::new(eval_subdomain).collect();
        let subdomain_log_size = eval_subdomain.log_size();
        let mut quotients: SecureColumnByCoords<SimdBackend> =
            unsafe { SecureColumnByCoords::uninitialized(1 << subdomain_log_size) };
        let sample_points: Vec<CirclePoint<SecureField>> =
            accumulations.iter().map(|x| x.sample_point).collect();
        let denominators_inverses = denominator_inverses(&sample_points, eval_subdomain);

        // Precompute values needed inside the loop.
        let log_ratios: Vec<u32> = accumulations
            .iter()
            .map(|acc| subdomain_log_size - acc.partial_numerators_acc.len().ilog2())
            .collect();
        let first_linear_terms: Vec<PackedSecureField> = accumulations
            .iter()
            .map(|acc| PackedSecureField::broadcast(acc.first_linear_term_acc))
            .collect();

        // Populate `quotients`.
        #[cfg(not(feature = "parallel"))]
        let iter = quotients.chunks_mut(COMBINE_CHUNK_SIZE).enumerate();

        #[cfg(feature = "parallel")]
        let iter = quotients.par_chunks_mut(COMBINE_CHUNK_SIZE).enumerate();

        iter.for_each(|(chunk_idx, mut value_dst)| {
            let chunk_start = chunk_idx * COMBINE_CHUNK_SIZE;
            let packed_chunk_len = value_dst.0[0].0.len();

            let mut chunk_acc = [PackedSecureField::zero(); COMBINE_CHUNK_SIZE];
            let chunk_acc = &mut chunk_acc[..packed_chunk_len];

            for (((acc, den_inv), log_ratio), first_linear_term) in accumulations
                .iter()
                .zip_eq(denominators_inverses.iter())
                .zip_eq(log_ratios.iter())
                .zip_eq(first_linear_terms.iter())
            {
                for (i, accumulator) in chunk_acc.iter_mut().enumerate() {
                    let domain_idx = chunk_start + i;
                    let lifted_partial_numerator =
                        PackedSecureField::from_packed_m31s(std::array::from_fn(|j| {
                            let lifted_simd = to_lifted_simd(
                                acc.partial_numerators_acc.columns[j].data[domain_idx >> log_ratio]
                                    .into_simd(),
                                *log_ratio,
                                domain_idx,
                            );
                            unsafe { PackedBaseField::from_simd_unchecked(lifted_simd) }
                        }));

                    let numerator = lifted_partial_numerator
                        - *first_linear_term * subdomain_points[domain_idx].y;
                    *accumulator += numerator * den_inv[domain_idx];
                }
            }

            for (i, accumulator) in chunk_acc.iter().enumerate() {
                unsafe {
                    value_dst.set_packed(i, *accumulator);
                }
            }
        });
        let subdomain_twiddles = TwiddleTree {
            root_coset: eval_subdomain.half_coset,
            // Only itwiddles are needed for interpolation.
            twiddles: TwiddleBuffer::empty(),
            itwiddles: twiddles
                .itwiddles
                .extract_subdomain_twiddles(eval_domain.log_size(), eval_subdomain.log_size()),
        };
        let evals = SecureColumnByCoords {
            columns: quotients.columns.map(|eval| {
                let poly = CircleEvaluation::<SimdBackend, BaseField, BitReversedOrder>::new(
                    eval_subdomain,
                    eval,
                )
                .interpolate_with_twiddles(&subdomain_twiddles);
                poly.evaluate_with_twiddles(eval_domain, twiddles).values
            }),
        };

        SecureEvaluation::new(eval_domain, evals)
    }
}

/// Performs the pointwise accumulation of the numerators on `subdomain`.
///
/// Note that `columns` are assumed to be evaluations over a possibly larger domain containing
/// `subdomain`. It must hold that the points of `subdomain` in bit-reversed order form a prefix of
/// the points of the larger domain in bit-reversed order.
///
/// Every term `c * val - b` of a coordinate is accumulated with deferred reduction: the exact
/// 64-bit products (each below 2^62) of up to four terms are summed in `u64` lanes, folded once
/// (`x = hi * 2^31 + lo == hi + lo` modulo `P`) and added to a wide accumulator, and the `b`
/// coefficients, which do not depend on the row, are summed once per batch. The result equals
/// the per-term reduced computation modulo `P` (here it is fully reduced into `[0, P)`).
///
/// Deferred-reduction mechanism: workshop thread bt1_4527483bd364509b01f72c85.
fn accumulate_numerators_on_subdomain(
    subdomain: CircleDomain,
    sample_batch: &ColumnSampleBatch,
    columns: &[&CircleEvaluation<SimdBackend, BaseField, BitReversedOrder>],
    quotient_coeffs: &[(SecureField, SecureField, SecureField)],
) -> SecureColumnByCoords<SimdBackend> {
    // This constant is chosen empirically by benchmarking.
    const NUMERATORS_CHUNK_SIZE: usize = 1 << 6;
    // Number of packed rows accumulated together in a pass over all the terms.
    const ROW_BLOCK: usize = 16;

    type RowAcc = [[u64x8; 2]; 4];

    #[inline(always)]
    fn accumulate_group<const G: usize>(
        acc: &mut [RowAcc],
        cols: [&[PackedBaseField]; G],
        coeffs: &[[u32; 4]; G],
    ) {
        for (r, row_acc) in acc.iter_mut().enumerate() {
            let mut even = [u64x8::splat(0); G];
            let mut odd = [u64x8::splat(0); G];
            for g in 0..G {
                let w = u64x8::from_array(bytemuck::cast::<PackedBaseField, [u64; 8]>(cols[g][r]));
                even[g] = w & u64x8::splat(0xFFFF_FFFF);
                odd[g] = w >> 32;
            }
            for k in 0..4 {
                let mut pe = even[0] * u64x8::splat(coeffs[0][k] as u64);
                let mut po = odd[0] * u64x8::splat(coeffs[0][k] as u64);
                for g in 1..G {
                    pe += even[g] * u64x8::splat(coeffs[g][k] as u64);
                    po += odd[g] * u64x8::splat(coeffs[g][k] as u64);
                }
                row_acc[k][0] += (pe & u64x8::splat(P as u64)) + (pe >> 31);
                row_acc[k][1] += (po & u64x8::splat(P as u64)) + (po >> 31);
            }
        }
    }

    let mut values =
        unsafe { SecureColumnByCoords::<SimdBackend>::uninitialized(subdomain.size()) };

    // Per-term data, shared by all chunks.
    let terms: Vec<(&[PackedBaseField], [u32; 4])> =
        zip_eq(&sample_batch.cols_vals_randpows, quotient_coeffs)
            .map(|(numerator_data, (_, _, c))| {
                let col_data = &columns[numerator_data.column_index].data;
                (&col_data[..], [c.0.0.0, c.0.1.0, c.1.0.0, c.1.1.0])
            })
            .collect();
    // The sum of the row independent `b` terms, negated.
    let b_sum: SecureField = quotient_coeffs.iter().map(|(_, b, _)| b).sum();
    let neg_b_sum = -b_sum;
    let neg_b = [
        PackedBaseField::broadcast(neg_b_sum.0.0),
        PackedBaseField::broadcast(neg_b_sum.0.1),
        PackedBaseField::broadcast(neg_b_sum.1.0),
        PackedBaseField::broadcast(neg_b_sum.1.1),
    ];

    #[cfg(not(feature = "parallel"))]
    let iter = values.chunks_mut(NUMERATORS_CHUNK_SIZE);

    #[cfg(feature = "parallel")]
    let iter = values.par_chunks_mut(NUMERATORS_CHUNK_SIZE);

    iter.enumerate().for_each(|(chunk_idx, mut values_dst)| {
        let chunk_start = chunk_idx * NUMERATORS_CHUNK_SIZE;
        // This is needed because the last chunk may be smaller than
        // `NUMERATORS_CHUNK_SIZE`.
        let packed_chunk_len = values_dst.0[0].0.len();

        for block_start in (0..packed_chunk_len).step_by(ROW_BLOCK) {
            let n_rows = ROW_BLOCK.min(packed_chunk_len - block_start);
            let first = chunk_start + block_start;
            let mut acc = [[[u64x8::splat(0); 2]; 4]; ROW_BLOCK];
            let acc_rows = &mut acc[..n_rows];

            let mut quads = terms.chunks_exact(4);
            for q in &mut quads {
                accumulate_group::<4>(
                    acc_rows,
                    [
                        &q[0].0[first..first + n_rows],
                        &q[1].0[first..first + n_rows],
                        &q[2].0[first..first + n_rows],
                        &q[3].0[first..first + n_rows],
                    ],
                    &[q[0].1, q[1].1, q[2].1, q[3].1],
                );
            }
            match quads.remainder() {
                [a, b, c] => accumulate_group::<3>(
                    acc_rows,
                    [
                        &a.0[first..first + n_rows],
                        &b.0[first..first + n_rows],
                        &c.0[first..first + n_rows],
                    ],
                    &[a.1, b.1, c.1],
                ),
                [a, b] => accumulate_group::<2>(
                    acc_rows,
                    [&a.0[first..first + n_rows], &b.0[first..first + n_rows]],
                    &[a.1, b.1],
                ),
                [a] => accumulate_group::<1>(acc_rows, [&a.0[first..first + n_rows]], &[a.1]),
                _ => {}
            }

            for (r, row_acc) in acc_rows.iter().enumerate() {
                let coords: [PackedBaseField; 4] = std::array::from_fn(|k| {
                    let fold = |x: u64x8| (x & u64x8::splat(P as u64)) + (x >> 31);
                    let packed = fold(row_acc[k][0]) | (fold(row_acc[k][1]) << 32);
                    let lanes = bytemuck::cast::<[u64; 8], PackedBaseField>(packed.to_array());
                    PackedBaseField::reduce_simd(lanes.into_simd()) + neg_b[k]
                });
                unsafe {
                    values_dst.set_packed(block_start + r, PackedSecureField::from_packed_m31s(coords));
                }
            }
        }
    });
    values
}

fn denominator_inverses(
    sample_points: &[CirclePoint<SecureField>],
    domain: CircleDomain,
) -> Vec<Vec<PackedCM31>> {
    let domain_points = CircleDomainBitRevIterator::new(domain);

    #[cfg(not(feature = "parallel"))]
    let (domain_points_iter, sample_points_iter) = (domain_points, sample_points.iter());
    #[cfg(feature = "parallel")]
    let (domain_points_iter, sample_points_iter) =
        (domain_points.par_iter(), sample_points.par_iter());

    sample_points_iter
        .map(|sample_point| {
            // Extract Pr, Pi.
            let prx = PackedCM31::broadcast(sample_point.x.0);
            let pry = PackedCM31::broadcast(sample_point.y.0);
            let pix = PackedCM31::broadcast(sample_point.x.1);
            let piy = PackedCM31::broadcast(sample_point.y.1);

            // The iter itself is cloned for each sample batch.
            let denominators = domain_points_iter
                .clone()
                .map(|points| (prx - points.x) * piy - (pry - points.y) * pix)
                .collect::<Vec<_>>();
            PackedCM31::batch_inverse(&denominators)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use itertools::Itertools;
    use rand::rngs::SmallRng;
    use rand::{Rng, SeedableRng};

    use crate::core::circle::SECURE_FIELD_CIRCLE_GEN;
    use crate::core::fields::m31::BaseField;
    use crate::core::fields::qm31::SecureField;
    use crate::core::pcs::TreeVec;
    use crate::core::pcs::quotients::{
        ColumnSampleBatch, PointSample, build_samples_with_randomness_and_periodicity,
    };
    use crate::core::poly::circle::CanonicCoset;
    use crate::prover::QuotientOps;
    use crate::prover::backend::CpuBackend;
    use crate::prover::backend::simd::SimdBackend;
    use crate::prover::backend::simd::column::BaseColumn;
    use crate::prover::pcs::quotient_ops::AccumulatedNumerators;
    use crate::prover::poly::BitReversedOrder;
    use crate::prover::poly::circle::CircleEvaluation;
    use crate::qm31;

    #[test]
    fn test_simd_and_cpu_numerators_are_consistent() {
        const LOG_SIZE: u32 = 10;
        const N_COLS: usize = 100;
        const LOG_BLOWUP_FACTOR: u32 = 3;
        let mut rng = SmallRng::seed_from_u64(0);
        let domain = CanonicCoset::new(LOG_SIZE).circle_domain();
        let values = BaseColumn::from_cpu(&(0..1 << LOG_SIZE).map(BaseField::from).collect_vec());
        let columns =
            CircleEvaluation::<SimdBackend, BaseField, BitReversedOrder>::new(domain, values);

        let mask_structure = (0..N_COLS).map(|_| rng.random_range(1..=2)).collect_vec();
        let points = [
            SECURE_FIELD_CIRCLE_GEN.mul(rng.random::<u128>()),
            SECURE_FIELD_CIRCLE_GEN.mul(rng.random::<u128>()),
        ];
        let samples = (0..N_COLS)
            .zip(mask_structure.iter())
            .map(|(_, i)| {
                points
                    .into_iter()
                    .zip_eq([
                        SecureField::from(rng.random::<u32>()),
                        SecureField::from(rng.random::<u32>()),
                    ])
                    .take(*i)
                    .map(|(point, value)| PointSample { point, value })
                    .collect_vec()
            })
            .collect_vec();
        let random_coeff = qm31!(98, 76, 54, 32);
        let sample_batches = ColumnSampleBatch::new_vec(
            &build_samples_with_randomness_and_periodicity(
                &TreeVec(vec![samples]),
                vec![vec![LOG_SIZE; N_COLS].into_iter()],
                LOG_SIZE,
                random_coeff,
            )
            .iter()
            .flatten()
            .collect_vec(),
        );
        // SIMD (still accumulates over full domain).
        let mut accumulated_numerators_vec_simd: Vec<AccumulatedNumerators<SimdBackend>> = vec![];
        let columns_simd: Vec<CircleEvaluation<SimdBackend, BaseField, BitReversedOrder>> =
            (0..N_COLS).map(|_| columns.clone()).collect();

        SimdBackend::accumulate_numerators(
            &columns_simd.iter().collect_vec(),
            &sample_batches,
            &mut accumulated_numerators_vec_simd,
            LOG_BLOWUP_FACTOR,
        );
        // CPU (accumulates over subdomain).
        let mut accumulated_numerators_vec_cpu: Vec<AccumulatedNumerators<CpuBackend>> = vec![];
        let columns_cpu: Vec<CircleEvaluation<CpuBackend, BaseField, BitReversedOrder>> =
            (0..N_COLS).map(|_| columns.to_cpu().clone()).collect();
        CpuBackend::accumulate_numerators(
            &columns_cpu.iter().collect_vec(),
            &sample_batches,
            &mut accumulated_numerators_vec_cpu,
            LOG_BLOWUP_FACTOR,
        );

        accumulated_numerators_vec_simd.iter().zip_eq(accumulated_numerators_vec_cpu).for_each(
            |(acc_simd, acc_cpu)| {
                assert_eq!(acc_simd.first_linear_term_acc, acc_cpu.first_linear_term_acc);
                assert_eq!(
                    acc_simd.partial_numerators_acc.to_cpu().columns,
                    acc_cpu.partial_numerators_acc.columns
                );
            },
        );
    }
}
