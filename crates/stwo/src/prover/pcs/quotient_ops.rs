use std::iter::zip;

use itertools::Itertools;
#[cfg(feature = "parallel")]
use rayon::prelude::*;
use tracing::{Level, span};

use crate::core::circle::CirclePoint;
use crate::core::fields::m31::BaseField;
use crate::core::fields::qm31::{SECURE_EXTENSION_DEGREE, SecureField};
use crate::core::pcs::TreeVec;
use crate::core::pcs::quotients::{
    ColumnSampleBatch, PointSample, build_samples_with_randomness_and_periodicity,
    quotient_constants,
};
use crate::core::poly::circle::CircleDomain;
use crate::prover::AccumulationOps;
use crate::prover::air::component_prover::Poly;
use crate::prover::backend::{Backend, Col, Column, ColumnOps};
use crate::prover::poly::BitReversedOrder;
use crate::prover::poly::circle::{
    CircleCoefficients, CircleEvaluation, PolyOps, SecureEvaluation,
};
use crate::prover::poly::twiddles::TwiddleTree;
use crate::prover::secure_column::SecureColumnByCoords;

pub trait QuotientOps: PolyOps {
    /// Receives a non-empty set of columns of the *same* size, and populates the vector
    /// `accumulated_numerators_vec` with their FRI numerators accumulations, across
    /// `sample_batches`.
    ///
    /// For each sample batch in `sample_batches`, accumulates the numerators of the columns
    /// involved in this batch over the evaluation subdomain (the first
    /// `column_size >> log_blowup_factor` rows in bit-reversed order) and pushes an
    /// `AccumulatedNumerators` object into `accumulated_numerators_vec`.
    fn accumulate_numerators(
        columns: &[&CircleEvaluation<Self, BaseField, BitReversedOrder>],
        sample_batches: &[ColumnSampleBatch],
        accumulated_numerators_vec: &mut Vec<AccumulatedNumerators<Self>>,
        log_blowup_factor: u32,
    );

    /// Given a vector of `AccumulatedNumerators` (computed on evaluation subdomains), computes
    /// the full quotient on the subdomain of size `2^(lifting_log_size - log_blowup_factor)`,
    /// then interpolates and evaluates on the full domain of size `2^lifting_log_size`.
    ///
    /// For each subdomain point:
    /// * Computes the denominator inverse for each sample point.
    /// * Multiplies it by the accumulated numerator for that (subdomain point, sample point).
    /// * Sums across sample points.
    ///
    /// The result is then extended to the full evaluation domain via interpolation on the
    /// subdomain followed by evaluation on the full domain, using the provided `twiddles`.
    fn compute_quotients_and_combine(
        accs: Vec<AccumulatedNumerators<Self>>,
        lifting_log_size: u32,
        log_blowup_factor: u32,
        twiddles: &TwiddleTree<Self>,
    ) -> SecureEvaluation<Self, BitReversedOrder>;

    /// Whether the backend implements [`Self::accumulate_weighted_columns`] and
    /// [`Self::canonicalize_and_subtract`], with which [`compute_fri_quotients_with_regrowth`]
    /// combines the sourced columns of a size before their transforms.
    const COMBINES_SOURCED_COLUMNS: bool = false;

    /// Adds `∑ cᵢ * columnᵢ` to `acc` row by row, over `terms` = `(columnᵢ, cᵢ)`, columns of the
    /// length of `acc` with values in `[0, P]`. Every value of `acc` is left in `[0, P)`.
    fn accumulate_weighted_columns(
        _acc: &mut SecureColumnByCoords<Self>,
        _terms: &[(&Col<Self, BaseField>, SecureField)],
    ) {
        unimplemented!("combined sourced columns are not supported by this backend")
    }

    /// Maps every value of `numerators` to its canonical representative in `[0, P)` (a
    /// transform may return `P` for zero), then subtracts `b_sum` from every row.
    fn canonicalize_and_subtract(
        _numerators: &mut SecureColumnByCoords<Self>,
        _b_sum: SecureField,
    ) {
        unimplemented!("combined sourced columns are not supported by this backend")
    }
}

/// Helper struct that keeps track of the accumulation of the numerators involved in the FRI
/// quotients.
///
/// Note: `Clone` is derived only for benchmarking purposes.
#[derive(Clone)]
pub struct AccumulatedNumerators<B: ColumnOps<BaseField>> {
    /// One of the sample points received by the pcs.
    pub sample_point: CirclePoint<SecureField>,
    /// Stores a circle evaluation of the form:
    ///     p -> ∑ α^{k_i} * (cᵢ * f̃ᵢ(p) - bᵢ)
    /// where
    /// * p ∈ canonic coset of log size = l (where l is the log size of the column).
    /// * i runs over some column indices of the trace.
    /// * α is the random coefficient for the accumulation.
    /// * k_i is the randomness exponent for column i and sample point `sample_point`.
    /// * f̃ᵢ is the lift of the trace poly fᵢ to log size l.
    /// * (bᵢ, cᵢ) are the `b` and `c` line coefficients for column i and sample point
    ///   `sample_point`.
    pub partial_numerators_acc: SecureColumnByCoords<B>,
    /// Stores an accumulation of the form
    ///      ∑ α^{k_i} * aᵢ
    /// where
    /// * i runs over some column indices of the trace.
    /// * α and k_i are as in the previous docstring for `partial_numerators_acc`.
    /// * aᵢ is the `a` line coefficient for column i and sample point `sample_point`.
    ///
    /// The index set of the summation is equal to the index set of the summation referred in the
    /// previous docstring for `partial_numerators_acc`.
    pub first_linear_term_acc: SecureField,
}

pub fn compute_fri_quotients<B: Backend>(
    columns: &TreeVec<Vec<&CircleEvaluation<B, BaseField, BitReversedOrder>>>,
    samples: &TreeVec<Vec<Vec<PointSample>>>,
    random_coeff: SecureField,
    lifting_log_size: u32,
    twiddles: &TwiddleTree<B>,
    log_blowup_factor: u32,
) -> SecureEvaluation<B, BitReversedOrder> {
    compute_fri_quotients_with_regrowth(
        columns,
        None,
        samples,
        random_coeff,
        lifting_log_size,
        twiddles,
        log_blowup_factor,
    )
}

/// The polynomial of the column `(tree, column)` that stores no values: a sourced column (see
/// [`Poly::is_sourced`]), whose values are reproduced from its trace source.
pub type SourcedColumn<'a, B> = dyn Fn(usize, usize) -> &'a Poly<B> + Sync + 'a;

/// The most stored-nothing columns whose stripe 0 (or trace values, when combined) are regrown at
/// once in [`compute_fri_quotients_with_regrowth`]: the regrown columns are its only extra memory.
const REGROWN_BATCH: usize = 16;

/// The smallest log size of the sourced columns that are combined before their transforms (the
/// smallest size the SIMD transforms run at, `simd::fft::MIN_FFT_LOG_SIZE`): smaller columns have
/// no transform cost to save.
const MIN_COMBINED_LOG_SIZE: u32 = 5;

/// [`compute_fri_quotients`] where some columns store no values (an empty evaluation on their
/// domain) and `sourced` gives their polynomials, which reproduce them. Their numerators are
/// accumulated [`REGROWN_BATCH`] columns at a time and added to those of the stored columns of the
/// same size and sample point. Field addition is exact, so the accumulations, and the quotients,
/// are those of a single pass over every column.
///
/// Where the backend supports it and it saves transforms, the sourced columns of a size are
/// combined before their transforms instead of regrowing each one's stripe 0: see
/// [`combined_sourced_accumulations`].
pub fn compute_fri_quotients_with_regrowth<B: Backend>(
    columns: &TreeVec<Vec<&CircleEvaluation<B, BaseField, BitReversedOrder>>>,
    sourced: Option<&SourcedColumn<'_, B>>,
    samples: &TreeVec<Vec<Vec<PointSample>>>,
    random_coeff: SecureField,
    lifting_log_size: u32,
    twiddles: &TwiddleTree<B>,
    log_blowup_factor: u32,
) -> SecureEvaluation<B, BitReversedOrder> {
    let _span = span!(Level::INFO, "Compute FRI quotients", class = "FRIQuotients").entered();
    let mut accumulated_numerators_vec: Vec<AccumulatedNumerators<B>> = vec![];
    let samples_with_randomness = build_samples_with_randomness_and_periodicity(
        samples,
        columns.0.iter().map(|x| x.iter().map(|c| c.domain.log_size())).collect(),
        lifting_log_size,
        random_coeff,
    );

    // Populate `accumulated_numerators_vec`, per (log_size, sample_point). After this iteration,
    // `accumulated_numerators_vec` will have length equal to
    //
    //   ∑_k (# of distinct sample points per log size k).
    //
    // Every (log_size, sample_point) accumulation has its own buffer, so they are computed
    // concurrently: the small log sizes do not split into enough row chunks to occupy the
    // threads on their own. The order of the vector is the sequential one.
    let positions = columns
        .iter()
        .enumerate()
        .flat_map(|(tree, columns)| (0..columns.len()).map(move |column| (tree, column)))
        .collect_vec();
    let is_regrown = |column: &CircleEvaluation<B, BaseField, BitReversedOrder>| {
        sourced.is_some() && column.values.len() == 0
    };
    let mut regrown_groups = vec![];
    let groups: Vec<(Vec<_>, Vec<ColumnSampleBatch>)> = zip(
        zip(columns.iter().flatten(), samples_with_randomness.iter().flatten()),
        &positions,
    )
    .sorted_by_key(|((c, _), _)| c.domain.log_size())
    .group_by(|((c, _), _)| c.domain.log_size())
    .into_iter()
    .filter_map(|(_, tuples)| {
        let (stored, regrown): (Vec<_>, Vec<_>) = tuples.partition(|((c, _), _)| !is_regrown(c));
        if !regrown.is_empty() {
            regrown_groups.push(regrown);
        }
        if stored.is_empty() {
            return None;
        }
        let (columns, samples_with_randomness): (Vec<_>, Vec<_>) =
            stored.into_iter().map(|(tuple, _)| tuple).unzip();
        // TODO: slice.
        let sample_batches = ColumnSampleBatch::new_vec(&samples_with_randomness);
        Some((columns, sample_batches))
    })
    .collect();
    let accumulate_batch = |columns: &[&CircleEvaluation<B, BaseField, BitReversedOrder>],
                            batch: &ColumnSampleBatch|
     -> Vec<AccumulatedNumerators<B>> {
        let mut acc = vec![];
        B::accumulate_numerators(columns, std::slice::from_ref(batch), &mut acc, log_blowup_factor);
        acc
    };
    #[cfg(feature = "parallel")]
    let accumulations: Vec<Vec<AccumulatedNumerators<B>>> = groups
        .par_iter()
        .flat_map(|(columns, batches)| {
            batches.par_iter().map(|batch| accumulate_batch(columns, batch))
        })
        .collect();
    #[cfg(not(feature = "parallel"))]
    let accumulations: Vec<Vec<AccumulatedNumerators<B>>> = groups
        .iter()
        .flat_map(|(columns, batches)| batches.iter().map(|batch| accumulate_batch(columns, batch)))
        .collect();
    accumulated_numerators_vec.extend(accumulations.into_iter().flatten());

    // The columns that store no values, a bounded batch of regrown stripes at a time, or combined
    // before their transforms.
    for group in regrown_groups {
        let sourced = sourced.unwrap();
        let polys: Vec<&Poly<B>> =
            group.iter().map(|&(_, &(tree, column))| sourced(tree, column)).collect();
        let group_samples: Vec<&Vec<(PointSample, SecureField)>> =
            group.iter().map(|((_, s), _)| *s).collect();
        let domain: CircleDomain = group[0].0.0.domain;
        if group.iter().all(|((c, _), _)| c.domain == domain)
            && combines_sourced_columns(&polys, &group_samples)
        {
            for accumulation in
                combined_sourced_accumulations(&polys, &group_samples, domain, twiddles)
            {
                add_accumulation(&mut accumulated_numerators_vec, accumulation);
            }
            continue;
        }
        let regrow =
            |tree: usize, column: usize| sourced(tree, column).stripe0(twiddles).into_owned();
        for batch in group.chunks(REGROWN_BATCH) {
            #[cfg(feature = "parallel")]
            let stripes: Vec<_> =
                batch.par_iter().map(|&(_, &(tree, column))| regrow(tree, column)).collect();
            #[cfg(not(feature = "parallel"))]
            let stripes: Vec<_> =
                batch.iter().map(|&(_, &(tree, column))| regrow(tree, column)).collect();
            let stripes = stripes.iter().collect_vec();
            let samples_with_randomness = batch.iter().map(|((_, s), _)| *s).collect_vec();
            let sample_batches = ColumnSampleBatch::new_vec(&samples_with_randomness);
            #[cfg(feature = "parallel")]
            let accumulations: Vec<Vec<AccumulatedNumerators<B>>> =
                sample_batches.par_iter().map(|batch| accumulate_batch(&stripes, batch)).collect();
            #[cfg(not(feature = "parallel"))]
            let accumulations: Vec<Vec<AccumulatedNumerators<B>>> =
                sample_batches.iter().map(|batch| accumulate_batch(&stripes, batch)).collect();
            for accumulation in accumulations.into_iter().flatten() {
                add_accumulation(&mut accumulated_numerators_vec, accumulation);
            }
        }
    }

    // Group and accumulate the numerators per sample point: the accumulations (of different
    // lengths) get lifted and accumulated to a single vector. After this step, there is a single
    // accumulation per sample point.
    let accumulations_per_sample_point = accumulated_numerators_vec
        .into_iter()
        .sorted_by_key(|c| (c.sample_point.x, c.sample_point.y))
        .group_by(|c| c.sample_point)
        .into_iter()
        .map(|(sample_point, accumulations_per_log_size)| {
            let accumulations_per_log_size = accumulations_per_log_size.collect_vec();
            // Accumulate the `a` coefficients.
            let first_linear_term_acc: SecureField =
                accumulations_per_log_size.iter().map(|x| x.first_linear_term_acc).sum();
            // Lift and accumulate the partial numerators vectors.
            // `partial_numerators_acc` is already sorted increasingly by size as required by
            // `B::lift_and_accumulate`.
            let partial_numerators_acc = accumulations_per_log_size
                .into_iter()
                .map(|x| x.partial_numerators_acc)
                .collect_vec();
            let res = B::lift_and_accumulate(partial_numerators_acc).unwrap();

            AccumulatedNumerators {
                sample_point,
                partial_numerators_acc: res,
                first_linear_term_acc,
            }
        })
        .collect_vec();

    B::compute_quotients_and_combine(
        accumulations_per_sample_point,
        lifting_log_size,
        log_blowup_factor,
        twiddles,
    )
}

/// Whether the sourced columns of one size, `polys`, with the samples `samples`, are combined
/// before their transforms by [`combined_sourced_accumulations`]: the backend supports it, every
/// column is sourced and of the same log size, at least [`MIN_COMBINED_LOG_SIZE`], and the
/// combination saves transforms. It interpolates and evaluates one column per coordinate and
/// sample point, four per point, where the per-column path transforms each column once.
fn combines_sourced_columns<B: Backend>(
    polys: &[&Poly<B>],
    samples: &[&Vec<(PointSample, SecureField)>],
) -> bool {
    if !B::COMBINES_SOURCED_COLUMNS || !polys.iter().all(|poly| poly.is_sourced()) {
        return false;
    }
    let log_sizes: Vec<u32> = polys
        .iter()
        .filter_map(|poly| poly.source.as_ref().map(|source| source.log_size()))
        .collect();
    let Some(&log_size) = log_sizes.first() else {
        return false;
    };
    if log_size < MIN_COMBINED_LOG_SIZE || log_sizes.iter().any(|&other| other != log_size) {
        return false;
    }
    let n_points = ColumnSampleBatch::new_vec(samples).len();
    polys.len() > SECURE_EXTENSION_DEGREE * n_points
}

/// The accumulated numerators of the sourced columns of one size, `polys`, sampled at `samples`,
/// with their stripes 0 on `domain`, combined before their transforms. Per sample point the trace
/// values are summed as `∑ cᵢ * vᵢ` over the point's columns, with the `c` line coefficients of
/// [`QuotientOps::accumulate_numerators`], [`REGROWN_BATCH`] columns at a time; then each of the
/// four coordinates of the sum is interpolated and evaluated on stripe 0 once (as
/// [`Poly::stripe0`] regrows a sourced column), canonicalized, and offset by `-∑ bᵢ`.
///
/// Interpolation and evaluation are linear, so stripe 0 of the sum is `∑ cᵢ * fᵢ`, where `fᵢ` is
/// column `i`'s stripe 0, modulo `P`. The per-column path reduces the same sums into `[0, P)`, so
/// once canonical the values are those of that path word for word; the transforms may return the
/// redundant `P` for zero, which is why they are canonicalized before the offset.
///
/// Mechanism: nadai2010 (rgr1_d04ed031, rgr1_d5852b0c), coefficient-domain combination of sourced
/// quotient columns; canonicalization at the partial-numerator boundary from calcutatator's port
/// (rgr1_d0601f81).
fn combined_sourced_accumulations<B: Backend>(
    polys: &[&Poly<B>],
    samples: &[&Vec<(PointSample, SecureField)>],
    domain: CircleDomain,
    twiddles: &TwiddleTree<B>,
) -> Vec<AccumulatedNumerators<B>> {
    let sample_batches: Vec<ColumnSampleBatch> = ColumnSampleBatch::new_vec(samples);
    let line_coeffs: Vec<Vec<(SecureField, SecureField, SecureField)>> =
        quotient_constants(&sample_batches).line_coeffs;
    let mut trace_domain: Option<CircleDomain> = None;
    let mut sums: Vec<Option<SecureColumnByCoords<B>>> =
        sample_batches.iter().map(|_| None).collect();
    for (chunk_index, chunk) in polys.chunks(REGROWN_BATCH).enumerate() {
        let first = chunk_index * REGROWN_BATCH;
        let source_evaluation =
            |poly: &&Poly<B>| poly.source_evaluation().expect("combined columns are sourced");
        #[cfg(feature = "parallel")]
        let evaluations: Vec<CircleEvaluation<B, BaseField, BitReversedOrder>> =
            chunk.par_iter().map(source_evaluation).collect();
        #[cfg(not(feature = "parallel"))]
        let evaluations: Vec<CircleEvaluation<B, BaseField, BitReversedOrder>> =
            chunk.iter().map(source_evaluation).collect();
        let chunk_domain = evaluations[0].domain;
        assert!(evaluations.iter().all(|evaluation| evaluation.domain == chunk_domain));
        assert_eq!(*trace_domain.get_or_insert(chunk_domain), chunk_domain);
        for ((batch, coeffs), sum) in zip(zip(&sample_batches, &line_coeffs), &mut sums) {
            let terms: Vec<(&Col<B, BaseField>, SecureField)> =
                zip(&batch.cols_vals_randpows, coeffs)
                    .filter(|(data, _)| (first..first + chunk.len()).contains(&data.column_index))
                    .map(|(data, &(_, _, c))| (&evaluations[data.column_index - first].values, c))
                    .collect();
            if terms.is_empty() {
                continue;
            }
            let sum = sum.get_or_insert_with(|| SecureColumnByCoords::zeros(chunk_domain.size()));
            B::accumulate_weighted_columns(sum, &terms);
        }
    }
    let trace_domain = trace_domain.expect("a group has columns");
    zip(zip(sample_batches, line_coeffs), sums)
        .map(|((batch, coeffs), sum)| {
            let sum = sum.expect("every sample point has columns");
            let mut partial_numerators_acc =
                stripe0_of_coordinates(sum, trace_domain, domain, twiddles);
            let b_sum: SecureField = coeffs.iter().map(|(_, b, _)| *b).sum();
            B::canonicalize_and_subtract(&mut partial_numerators_acc, b_sum);
            AccumulatedNumerators {
                sample_point: batch.point,
                partial_numerators_acc,
                first_linear_term_acc: coeffs.iter().map(|(a, ..)| *a).sum(),
            }
        })
        .collect()
}

/// Stripe 0 of `domain` of each coordinate of `values`, given on `trace_domain`: interpolated
/// with the twiddles of `trace_domain` and evaluated on the first subdomain of `domain` with
/// `twiddles`, as [`Poly::stripe0`] regrows a sourced column.
fn stripe0_of_coordinates<B: PolyOps>(
    values: SecureColumnByCoords<B>,
    trace_domain: CircleDomain,
    domain: CircleDomain,
    twiddles: &TwiddleTree<B>,
) -> SecureColumnByCoords<B> {
    let shared_trace_twiddles = B::subdomain_twiddles(trace_domain.half_coset);
    let trace_twiddles: &TwiddleTree<B> = &shared_trace_twiddles;
    let stripe0 = |column: Col<B, BaseField>| -> Col<B, BaseField> {
        let coefficients: CircleCoefficients<B> =
            CircleEvaluation::<B, BaseField, BitReversedOrder>::new(trace_domain, column)
                .interpolate_with_twiddles(trace_twiddles);
        let mut stripe = Col::<B, BaseField>::zeros(1 << coefficients.log_size());
        B::evaluate_stripe_into(&coefficients, domain, twiddles, 0, &mut stripe);
        stripe
    };
    #[cfg(feature = "parallel")]
    let columns: Vec<Col<B, BaseField>> = values.columns.into_par_iter().map(stripe0).collect();
    #[cfg(not(feature = "parallel"))]
    let columns: Vec<Col<B, BaseField>> = values.columns.into_iter().map(stripe0).collect();
    SecureColumnByCoords {
        columns: columns.try_into().unwrap_or_else(|_| unreachable!("four coordinates")),
    }
}

/// Adds `accumulation` to the accumulation in `accumulations` of the same sample point and size,
/// or appends it, keeping the vector sorted by size.
fn add_accumulation<B: AccumulationOps>(
    accumulations: &mut Vec<AccumulatedNumerators<B>>,
    accumulation: AccumulatedNumerators<B>,
) {
    let len = accumulation.partial_numerators_acc.len();
    if let Some(existing) = accumulations.iter_mut().find(|existing| {
        existing.sample_point == accumulation.sample_point
            && existing.partial_numerators_acc.len() == len
    }) {
        B::accumulate(&mut existing.partial_numerators_acc, &accumulation.partial_numerators_acc);
        existing.first_linear_term_acc += accumulation.first_linear_term_acc;
        return;
    }
    let index = accumulations
        .iter()
        .position(|existing| existing.partial_numerators_acc.len() > len)
        .unwrap_or(accumulations.len());
    accumulations.insert(index, accumulation);
}

#[cfg(test)]
mod tests {
    use itertools::Itertools;
    use num_traits::Zero;
    use rand::rngs::SmallRng;
    use rand::{Rng, SeedableRng};

    use crate::core::channel::Blake2sChannel;
    use crate::core::circle::SECURE_FIELD_CIRCLE_GEN;
    use crate::core::fields::m31::M31;
    use crate::core::fri::FriConfig;
    use crate::core::pcs::quotients::PointSample;
    use crate::core::pcs::{CommitmentSchemeVerifier, PcsConfig, TreeVec};
    use crate::core::poly::circle::CanonicCoset;
    use crate::core::proof_of_work::Grinding;
    use crate::core::vcs_lifted::blake2_merkle::Blake2sMerkleChannel;
    use crate::core::verifier::VerificationError;
    use crate::prover::backend::cpu::{CpuCircleEvaluation, CpuCirclePoly};
    use crate::prover::backend::simd::SimdBackend;
    use crate::prover::backend::simd::column::BaseColumn;
    use crate::prover::backend::{Backend, BackendForChannel, Column, CpuBackend};
    use crate::prover::pcs::quotient_ops::compute_fri_quotients;
    use crate::prover::poly::circle::{CircleCoefficients, CircleEvaluation, PolyOps};
    use crate::prover::{CommitmentSchemeProver, SecureField};

    #[test]
    fn test_quotients_are_low_degree() {
        let mut rng = SmallRng::seed_from_u64(0);
        const LOG_SIZE: u32 = 3;
        const LOG_BLOWUP_FACTOR: u32 = 4;

        let polynomial = CpuCirclePoly::new((0..1 << LOG_SIZE).map(M31::from).collect());
        let eval_domain = CanonicCoset::new(LOG_SIZE + LOG_BLOWUP_FACTOR).circle_domain();
        let eval = polynomial.evaluate(eval_domain);

        let sample_points = [
            SECURE_FIELD_CIRCLE_GEN.mul(rng.random::<u128>()),
            SECURE_FIELD_CIRCLE_GEN.mul(rng.random::<u128>()),
        ];
        let samples = sample_points
            .into_iter()
            .map(|x| PointSample { point: x, value: polynomial.eval_at_point(x) })
            .collect_vec();
        let rand_coeff =
            SecureField::from_m31_array(std::array::from_fn(|_| M31::from(rng.random::<u32>())));
        let quot_eval = compute_fri_quotients(
            &TreeVec(vec![vec![&eval]]),
            &TreeVec(vec![vec![samples]]),
            rand_coeff,
            LOG_SIZE + LOG_BLOWUP_FACTOR,
            &CpuBackend::precompute_twiddles(eval_domain.half_coset),
            LOG_BLOWUP_FACTOR,
        );
        let mut coeffs = quot_eval
            .values
            .columns
            .iter()
            .map(|c| CpuCircleEvaluation::new(eval_domain, c.clone()).interpolate())
            .collect_vec();
        let zeros = coeffs[0].coeffs.split_off((1 << LOG_SIZE) - 1);

        assert!(zeros.iter().all(|c| c.is_zero()));
    }

    /// Generates a vector of random polynomials, such that the last one is of degree
    /// `LIFTING_LOG_SIZE - 1` and all the previous ones are of degree < `LIFTING_LOG_SIZE - 1`.
    fn prepare_polys<B: Backend, const N_COLS: usize, const LIFTING_LOG_SIZE: u32>()
    -> Vec<CircleCoefficients<B>> {
        let mut rng = SmallRng::seed_from_u64(0);
        let mut polys: Vec<CircleCoefficients<B>> = (0..N_COLS - 1)
            .map(|_| {
                CircleCoefficients::new(
                    (0..1 << rng.random_range(4..LIFTING_LOG_SIZE - 1)).map(M31::from).collect(),
                )
            })
            .collect();
        polys.push(CircleCoefficients::new((0..1 << LIFTING_LOG_SIZE).map(M31::from).collect()));
        polys
    }

    fn prove_and_verify_pcs<
        B: BackendForChannel<Blake2sMerkleChannel>,
        const STORE_COEFFS: bool,
    >() -> Result<(), VerificationError> {
        prove_and_verify_pcs_grinding::<B, STORE_COEFFS>(Grinding::NONE, Grinding::NONE)
    }

    fn prove_and_verify_pcs_grinding<
        B: BackendForChannel<Blake2sMerkleChannel>,
        const STORE_COEFFS: bool,
    >(
        prover_grinding: Grinding,
        verifier_grinding: Grinding,
    ) -> Result<(), VerificationError> {
        const N_COLS: usize = 10;
        const LIFTING_LOG_SIZE: u32 = 8;

        // Setup the prover side of the pcs.
        let mut channel = Blake2sChannel::default();
        let base = PcsConfig::from_fri_and_trace_size(FriConfig::default(), LIFTING_LOG_SIZE);
        let config = base.with_grinding(prover_grinding);
        let twiddles = B::precompute_twiddles(
            CanonicCoset::new(LIFTING_LOG_SIZE + config.fri_config.log_blowup_factor).half_coset(),
        );
        let mut commitment_scheme =
            CommitmentSchemeProver::<B, Blake2sMerkleChannel>::new(config, &twiddles);
        if STORE_COEFFS {
            commitment_scheme.set_store_polynomials_coefficients();
        }
        let polys = prepare_polys::<B, N_COLS, LIFTING_LOG_SIZE>();
        let sizes = polys.iter().map(|poly| poly.log_size()).collect_vec();

        let mut tree_builder = commitment_scheme.tree_builder();
        tree_builder.extend_polys(polys);
        tree_builder.commit(&mut channel);

        let mut rng = SmallRng::seed_from_u64(0);
        let mask_structure = (0..N_COLS).map(|_| rng.random_range(1..=2)).collect_vec();
        let samples = [
            SECURE_FIELD_CIRCLE_GEN.mul(rng.random::<u128>()),
            SECURE_FIELD_CIRCLE_GEN.mul(rng.random::<u128>()),
        ];
        let sampled_points = vec![
            (0..N_COLS)
                .zip(mask_structure.iter())
                .map(|(_, i)| samples.into_iter().take(*i).collect_vec())
                .collect_vec(),
        ];

        let proof = commitment_scheme.prove_values(TreeVec(sampled_points.clone()), &mut channel);

        // Verifier side of the pcs.
        let mut channel = Blake2sChannel::default();
        let verifier_config = base.with_grinding(verifier_grinding);
        let mut verifier = CommitmentSchemeVerifier::<Blake2sMerkleChannel>::new(verifier_config);
        verifier.commit(proof.proof.commitments[0], &sizes, &mut channel);
        verifier.verify_values(TreeVec(sampled_points), proof.proof, &mut channel)
    }

    #[test]
    fn test_pcs_prove_and_verify_cpu() {
        assert!(prove_and_verify_pcs::<CpuBackend, true>().is_ok());
    }
    #[test]
    fn test_pcs_prove_and_verify_simd() {
        assert!(prove_and_verify_pcs::<SimdBackend, true>().is_ok());
    }
    #[test]
    fn test_pcs_prove_and_verify_simd_with_barycentric() {
        assert!(prove_and_verify_pcs::<SimdBackend, false>().is_ok());
    }

    /// Proof of work before the batching challenge: no proof carries its nonce, so the verifier
    /// must grind exactly as the prover did, or every later challenge differs.
    #[test]
    fn test_pcs_batching_grinding_must_match() {
        let grinding = Grinding { deep_bits: 0, batching_bits: 12 };
        let none = Grinding::NONE;
        assert!(prove_and_verify_pcs_grinding::<SimdBackend, true>(grinding, grinding).is_ok());
        assert!(prove_and_verify_pcs_grinding::<SimdBackend, true>(grinding, none).is_err());
        assert!(prove_and_verify_pcs_grinding::<SimdBackend, true>(none, grinding).is_err());
    }

    /// Tests that SIMD quotient computation produces low-degree quotients even when the trace
    /// polynomial is very small (subdomain size < N_LANES).
    #[test]
    fn test_simd_quotients_are_low_degree_small_trace() {
        let mut rng = SmallRng::seed_from_u64(0);
        const LOG_SIZE: u32 = 3;
        const LOG_BLOWUP_FACTOR: u32 = 4;

        let polynomial = CpuCirclePoly::new((0..1 << LOG_SIZE).map(M31::from).collect());
        let eval_domain = CanonicCoset::new(LOG_SIZE + LOG_BLOWUP_FACTOR).circle_domain();
        let cpu_eval = polynomial.evaluate(eval_domain);
        let simd_eval = CircleEvaluation::new(eval_domain, BaseColumn::from_cpu(&cpu_eval.values));

        let sample_points = [
            SECURE_FIELD_CIRCLE_GEN.mul(rng.random::<u128>()),
            SECURE_FIELD_CIRCLE_GEN.mul(rng.random::<u128>()),
        ];
        let samples = sample_points
            .into_iter()
            .map(|x| PointSample { point: x, value: polynomial.eval_at_point(x) })
            .collect_vec();
        let rand_coeff =
            SecureField::from_m31_array(std::array::from_fn(|_| M31::from(rng.random::<u32>())));
        let twiddles = SimdBackend::precompute_twiddles(eval_domain.half_coset);
        let quot_eval = compute_fri_quotients(
            &TreeVec(vec![vec![&simd_eval]]),
            &TreeVec(vec![vec![samples]]),
            rand_coeff,
            LOG_SIZE + LOG_BLOWUP_FACTOR,
            &twiddles,
            LOG_BLOWUP_FACTOR,
        );
        let mut coeffs = quot_eval
            .values
            .columns
            .iter()
            .map(|c| CpuCircleEvaluation::new(eval_domain, c.to_cpu()).interpolate())
            .collect_vec();
        let zeros = coeffs[0].coeffs.split_off((1 << LOG_SIZE) - 1);

        assert!(zeros.iter().all(|c| c.is_zero()));
    }
}
