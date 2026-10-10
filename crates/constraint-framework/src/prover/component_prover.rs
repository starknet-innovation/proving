use std::borrow::Cow;

use itertools::Itertools;
#[cfg(feature = "parallel")]
use rayon::prelude::*;
use stwo::core::air::Component;
use stwo::core::constraints::coset_vanishing;
use stwo::core::fields::m31::BaseField;
use stwo::core::fields::qm31::SecureField;
use stwo::core::pcs::TreeVec;
use stwo::core::poly::circle::{CanonicCoset, CircleDomain};
use stwo::core::utils::bit_reverse;
use stwo::prover::backend::simd::SimdBackend;
use stwo::prover::backend::simd::circle::evaluate_block_into;
use stwo::prover::backend::simd::column::{
    BaseColumn, VeryPackedSecureColumnByCoords, VeryPackedSecureColumnByCoordsMutSlice,
};
use stwo::prover::backend::simd::m31::LOG_N_LANES;
use stwo::prover::backend::simd::very_packed_m31::{LOG_N_VERY_PACKED_ELEMS, VeryPackedBaseField};
use stwo::prover::backend::{Backend, Column, CpuBackend};
use stwo::prover::poly::BitReversedOrder;
use stwo::prover::poly::circle::{CircleEvaluation, PolyOps};
use stwo::prover::poly::twiddles::TwiddleTree;
use stwo::prover::secure_column::SecureColumnByCoords;
use stwo::prover::{ComponentProver, DomainEvaluationAccumulator, EvaluationMode, Poly, Trace};
use tracing::{Level, span};

use super::{CpuDomainEvaluator, SimdDomainEvaluator};
use crate::logup::LogupAtRow;
use crate::{FrameworkComponent, FrameworkEval, INTERACTION_TRACE_IDX, PREPROCESSED_TRACE_IDX};

const CHUNK_SIZE: usize = 1;

/// Common inputs for constraint quotient evaluation, shared between the SIMD and CPU backends.
struct ConstraintQuotientInputs<'a, B: Backend> {
    eval_domain: CircleDomain,
    trace_domain: CanonicCoset,
    trace: TreeVec<Vec<Cow<'a, CircleEvaluation<B, BaseField, BitReversedOrder>>>>,
    denom_inv: Vec<BaseField>,
}

/// Prepares trace evaluations: borrows directly (subdomain) or extends to eval domain.
fn get_trace_columns<'a, B: Backend>(
    component_polys: TreeVec<Vec<&'a &Poly<B>>>,
    eval_domain: CircleDomain,
    mode: EvaluationMode,
    twiddles: Option<&TwiddleTree<B>>,
) -> TreeVec<Vec<Cow<'a, CircleEvaluation<B, BaseField, BitReversedOrder>>>> {
    match mode {
        EvaluationMode::SubDomain { .. } => {
            // Borrow committed evaluations directly. Only the first
            // 2^max_constraint_log_degree_bound indices are going to be used for the
            // constraint quotient evaluation (in bit-reversed order these form the
            // subdomain coset).
            //
            // Ideally we'd slice to just those indices, but the type system requires
            // borrowing the entire evaluation.
            component_polys.map_cols(|c| Cow::Borrowed(&c.evals))
        }
        EvaluationMode::ExtendToEvalDomain => {
            let _span = span!(Level::INFO, "Constraint Extension").entered();
            let owned;
            let twiddles = match twiddles {
                Some(twiddles) => twiddles,
                None => {
                    owned = B::precompute_twiddles(eval_domain.half_coset);
                    &owned
                }
            };
            #[cfg(not(feature = "parallel"))]
            {
                component_polys.as_cols_ref().map_cols(|col| {
                    Cow::Owned(col.get_evaluation_on_domain(eval_domain, &twiddles))
                })
            }
            #[cfg(feature = "parallel")]
            {
                component_polys.as_cols_ref().par_map_cols(|col| {
                    Cow::Owned(col.get_evaluation_on_domain(eval_domain, &twiddles))
                })
            }
        }
    }
}

/// Constructs the inputs needed for constraint quotient evaluation from a component and trace.
/// Computes the eval/trace domains, prepares trace columns (borrowing or extending as needed),
/// and precomputes denominator inverses.
fn get_constraint_quotient_inputs<'a, E: FrameworkEval, B: Backend>(
    component: &FrameworkComponent<E>,
    trace: &'a Trace<'a, B>,
    mode: EvaluationMode,
) -> ConstraintQuotientInputs<'a, B> {
    let max_constraint_log_degree_bound = component.max_constraint_log_degree_bound();
    let trace_domain = CanonicCoset::new(component.eval.log_size());
    let component_polys = get_component_polys(component, trace);

    let eval_domain = match mode {
        EvaluationMode::SubDomain { log_expansion } => {
            subdomain_eval_domain(max_constraint_log_degree_bound, log_expansion)
        }
        EvaluationMode::ExtendToEvalDomain => {
            CanonicCoset::new(max_constraint_log_degree_bound).circle_domain()
        }
    };
    let trace = get_trace_columns(component_polys, eval_domain, mode, trace.twiddles);

    let denom_inv = get_denom_inv(trace_domain, eval_domain);
    ConstraintQuotientInputs { eval_domain, trace_domain, trace, denom_inv }
}

/// The component's trace polynomials, with its preprocessed columns picked out.
fn get_component_polys<'a, E: FrameworkEval, B: Backend>(
    component: &FrameworkComponent<E>,
    trace: &'a Trace<'a, B>,
) -> TreeVec<Vec<&'a &'a Poly<B>>> {
    let mut component_polys = trace.polys.sub_tree(&component.trace_locations);
    component_polys[PREPROCESSED_TRACE_IDX] = component
        .preprocessed_column_indices
        .iter()
        .map(|idx| &trace.polys[PREPROCESSED_TRACE_IDX][*idx])
        .collect();
    component_polys
}

/// The inverses of the trace domain's vanishing polynomial on the cosets of the evaluation
/// domain, bit-reversed.
fn get_denom_inv(trace_domain: CanonicCoset, eval_domain: CircleDomain) -> Vec<BaseField> {
    let log_expand = eval_domain.log_size() - trace_domain.log_size();
    let mut denom_inv = (0..1 << log_expand)
        .map(|i| coset_vanishing(trace_domain.coset(), eval_domain.at(i)).inverse())
        .collect_vec();
    bit_reverse(&mut denom_inv);
    denom_inv
}

/// Windowed components are evaluated over `2^LOG_N_BLOCKS` blocks of the evaluation domain
/// ([`FrameworkComponent::evaluate_in_blocks`]).
const LOG_N_BLOCKS: u32 = 2;

/// The smallest evaluation domain that [`FrameworkComponent::evaluate_in_blocks`] evaluates
/// block by block; smaller components extend at once, as before.
const PARITY_MIN_LOG_SIZE: u32 = 20;

impl<E: FrameworkEval + Sync> ComponentProver<SimdBackend> for FrameworkComponent<E> {
    fn evaluate_constraint_quotients_on_domain(
        &self,
        trace: &Trace<'_, SimdBackend>,
        evaluation_accumulator: &mut DomainEvaluationAccumulator<SimdBackend>,
    ) {
        if self.n_constraints() == 0 {
            return;
        }
        let eval_log_size = self.max_constraint_log_degree_bound();
        if matches!(evaluation_accumulator.evaluation_mode(), EvaluationMode::ExtendToEvalDomain)
            && eval_log_size == self.eval.log_size() + 1
            && eval_log_size >= PARITY_MIN_LOG_SIZE
        {
            return self.evaluate_in_blocks(trace, evaluation_accumulator, LOG_N_BLOCKS);
        }

        let ConstraintQuotientInputs { eval_domain, trace_domain, trace, denom_inv } =
            get_constraint_quotient_inputs(self, trace, evaluation_accumulator.evaluation_mode());

        let [mut accum] =
            evaluation_accumulator.columns([(eval_domain.log_size(), self.n_constraints())]);
        accum.random_coeff_powers.reverse();

        let _span =
            span!(Level::INFO, "Constraint point-wise eval", class = "ConstraintEval").entered();

        // Fall back to CPU if the trace is too small.
        if trace_domain.log_size() < LOG_N_LANES + LOG_N_VERY_PACKED_ELEMS {
            let trace_cols = trace.as_cols_ref().map_cols(|c| c.to_cpu());
            let trace_cols = trace_cols.as_cols_ref();
            *accum.col = SecureColumnByCoords::from_cpu(accumulate_pointwise_cpu(
                self,
                trace_cols,
                eval_domain.log_size(),
                trace_domain.log_size(),
                denom_inv,
                &accum.random_coeff_powers,
                &accum.col.to_cpu(),
            ));
            return;
        }

        let trace_cols = trace.as_cols_ref().map_cols(|c| c.as_ref());
        self.accumulate_rows(
            &trace_cols,
            accum.col,
            &accum.random_coeff_powers,
            eval_domain.log_size(),
            &denom_inv,
            None,
            0,
        );
    }
}

impl<E: FrameworkEval + Sync> FrameworkComponent<E> {
    /// Adds the constraint quotients of the rows of `col`: every row of the evaluation domain, or
    /// with `window` the rows of a window of a half coset (see
    /// [`SimdDomainEvaluator::window`]), in its natural order.
    #[allow(clippy::too_many_arguments)]
    fn accumulate_rows(
        &self,
        trace_cols: &TreeVec<Vec<&CircleEvaluation<SimdBackend, BaseField, BitReversedOrder>>>,
        col: &mut SecureColumnByCoords<SimdBackend>,
        random_coeff_powers: &[SecureField],
        eval_log_size: u32,
        denom_inv: &[BaseField],
        window: Option<(usize, isize)>,
        block: usize,
    ) {
        let range = 0..(col.len() >> (LOG_N_LANES + LOG_N_VERY_PACKED_ELEMS));
        let col = unsafe { VeryPackedSecureColumnByCoords::transform_under_mut(col) };

        #[cfg(not(feature = "parallel"))]
        let iter = range.step_by(CHUNK_SIZE).zip(col.chunks_mut(CHUNK_SIZE));

        #[cfg(feature = "parallel")]
        let iter = range.into_par_iter().step_by(CHUNK_SIZE).zip(col.par_chunks_mut(CHUNK_SIZE));

        // Define any `self` values outside the loop to prevent the compiler thinking there is a
        // `Sync` requirement on `Self`.
        let self_eval = &self.eval;
        let trace_log_size = self_eval.log_size();
        let cumsum_shift = self.claimed_sum / BaseField::from_u32_unchecked(1 << trace_log_size);
        let denom_shift = trace_log_size - LOG_N_LANES - LOG_N_VERY_PACKED_ELEMS;
        // In a window, the coset of the trace domain alternates with the row: the parity of the
        // natural row is the top bit of the bit-reversed one, which picks the denominator.
        let window_denom_inv = window.map(|_| {
            VeryPackedBaseField::from_array(std::array::from_fn(|i| denom_inv[i & 1]))
        });

        let body = |fracs: &mut Vec<_>,
                    (chunk_idx, mut chunk): (usize, VeryPackedSecureColumnByCoordsMutSlice<'_>)| {
            for idx_in_chunk in 0..CHUNK_SIZE {
                let vec_row = chunk_idx * CHUNK_SIZE + idx_in_chunk;
                // Evaluate constrains at row.
                let mut eval = SimdDomainEvaluator::new_with_logup(
                    trace_cols,
                    vec_row,
                    random_coeff_powers,
                    trace_log_size,
                    eval_log_size,
                    LogupAtRow::new_with_shift(
                        INTERACTION_TRACE_IDX,
                        cumsum_shift,
                        trace_log_size,
                        std::mem::take(fracs),
                    ),
                );
                eval.window = window;
                eval.block = block;
                let mut eval = self_eval.evaluate(eval);
                let row_res = eval.row_res;
                *fracs = std::mem::take(&mut eval.logup.fracs);
                fracs.clear();

                // Finalize row.
                unsafe {
                    let row_denom_inv = window_denom_inv.unwrap_or_else(|| {
                        VeryPackedBaseField::broadcast(denom_inv[(vec_row + block) >> denom_shift])
                    });
                    chunk.set_packed(
                        idx_in_chunk,
                        chunk.packed_at(idx_in_chunk) + row_res * row_denom_inv,
                    )
                }
            }
        };

        #[cfg(not(feature = "parallel"))]
        {
            let mut fracs = Vec::new();
            iter.for_each(|item| body(&mut fracs, item));
        }

        #[cfg(feature = "parallel")]
        iter.for_each_init(Vec::new, body);
    }

    /// Evaluates the constraint quotients of a component whose evaluation domain is twice its trace
    /// domain over `2^log_n_blocks` contiguous blocks of the evaluation
    /// domain's bit-reversed order instead of windows of its half cosets. A block is a subdomain,
    /// so each column's block is one small FFT of coefficients folded to its size
    /// ([`evaluate_block_into`]), with no extension of the whole domain and no reordering. The few
    /// columns read at a nonzero mask offset (the logup cumulative sums) reach rows of other
    /// blocks: they are extended to the whole domain once, for every block.
    fn evaluate_in_blocks(
        &self,
        trace: &Trace<'_, SimdBackend>,
        evaluation_accumulator: &mut DomainEvaluationAccumulator<SimdBackend>,
        log_n_blocks: u32,
    ) {
        let log_size = self.eval.log_size();
        let trace_domain = CanonicCoset::new(log_size);
        let eval_domain = CanonicCoset::new(log_size + 1).circle_domain();
        let component_polys = get_component_polys(self, trace);
        let denom_inv = get_denom_inv(trace_domain, eval_domain);
        let [mut accum] =
            evaluation_accumulator.columns([(eval_domain.log_size(), self.n_constraints())]);
        accum.random_coeff_powers.reverse();
        let owned;
        let twiddles = match trace.twiddles {
            Some(twiddles) => twiddles,
            None => {
                owned = SimdBackend::precompute_twiddles(eval_domain.half_coset);
                &owned
            }
        };
        let log_block = eval_domain.log_size() - log_n_blocks;
        let block_len = 1usize << log_block;
        // Per column of `component_polys`: whether a nonzero mask offset reads it. A tree whose
        // mask does not match its columns one to one counts as read at an offset (extended whole).
        let mask = self.shifted_columns();
        let shifted = TreeVec::new(
            component_polys
                .iter()
                .enumerate()
                .map(|(t, polys)| match mask.get(t) {
                    Some(m) if m.len() == polys.len() => m.clone(),
                    // Preprocessed columns are read through their own accessor, at offset 0 (a
                    // block column read at an offset would index past its end and panic).
                    Some(m) if m.is_empty() && t == PREPROCESSED_TRACE_IDX => vec![false; polys.len()],
                    _ => vec![true; polys.len()],
                })
                .collect(),
        );
        fn coefficients(
            poly: &Poly<SimdBackend>,
        ) -> Cow<'_, stwo::prover::poly::circle::CircleCoefficients<SimdBackend>> {
            match &poly.coeffs {
                Some(coeffs) => Cow::Borrowed(coeffs),
                None => Cow::Owned(poly.regrown_coefficients()),
            }
        }
        // The shifted columns, extended to the whole domain.
        let whole = component_polys
            .as_cols_ref()
            .zip_cols(shifted.as_cols_ref())
            .map_cols(|(poly, shifted): (&&&Poly<SimdBackend>, &bool)| {
                shifted.then(|| poly.get_evaluation_on_domain(eval_domain, &twiddles))
            });
        // A block's column buffers are reused by the next block (no fresh pages).
        let spare = std::sync::Mutex::new(Vec::<BaseColumn>::new());
        for block in 0..1usize << log_n_blocks {
            let fill = |(poly, whole): (&&&Poly<SimdBackend>, &Option<CircleEvaluation<SimdBackend, BaseField, BitReversedOrder>>)| {
                if whole.is_some() {
                    return None;
                }
                let mut values =
                    spare.lock().unwrap().pop().unwrap_or_else(|| BaseColumn::zeros(block_len));
                evaluate_block_into(&coefficients(**poly), eval_domain, &twiddles, log_block, block, &mut values);
                let mut evaluation = CircleEvaluation::<SimdBackend, BaseField, BitReversedOrder>::new(
                    CanonicCoset::new(1).circle_domain(),
                    BaseColumn::zeros(2),
                );
                evaluation.values = values;
                Some(evaluation)
            };
            #[cfg(not(feature = "parallel"))]
            let blocks = component_polys.as_cols_ref().zip_cols(whole.as_cols_ref()).map_cols(fill);
            #[cfg(feature = "parallel")]
            let blocks = component_polys.as_cols_ref().zip_cols(whole.as_cols_ref()).par_map_cols(fill);
            let columns = blocks
                .as_cols_ref()
                .zip_cols(whole.as_cols_ref())
                .map_cols(
                    |(block, whole): (
                        &Option<CircleEvaluation<SimdBackend, BaseField, BitReversedOrder>>,
                        &Option<CircleEvaluation<SimdBackend, BaseField, BitReversedOrder>>,
                    )| block.as_ref().or(whole.as_ref()).unwrap(),
                );
            let mut rows = SecureColumnByCoords::<SimdBackend>::zeros(block_len);
            self.accumulate_rows(
                &columns,
                &mut rows,
                &accum.random_coeff_powers,
                eval_domain.log_size(),
                &denom_inv,
                None,
                block * block_len >> (LOG_N_LANES + LOG_N_VERY_PACKED_ELEMS),
            );
            drop(columns);
            spare
                .lock()
                .unwrap()
                .extend(blocks.0.into_iter().flatten().flatten().map(|evaluation| evaluation.values));
            for (dst, src) in accum.col.columns.iter_mut().zip(&rows.columns) {
                let dst = &mut dst.as_mut_slice()[block * block_len..][..block_len];
                #[cfg(not(feature = "parallel"))]
                let pairs = dst.iter_mut().zip(src.as_slice());
                #[cfg(feature = "parallel")]
                let pairs = dst.par_iter_mut().zip(src.as_slice().par_iter());
                pairs.for_each(|(dst, src)| *dst += *src);
            }
        }
    }
}

impl<E: FrameworkEval + Sync> ComponentProver<CpuBackend> for FrameworkComponent<E> {
    fn evaluate_constraint_quotients_on_domain(
        &self,
        trace: &Trace<'_, CpuBackend>,
        evaluation_accumulator: &mut DomainEvaluationAccumulator<CpuBackend>,
    ) {
        if self.n_constraints() == 0 {
            return;
        }

        let ConstraintQuotientInputs { eval_domain, trace_domain, trace, denom_inv } =
            get_constraint_quotient_inputs(self, trace, evaluation_accumulator.evaluation_mode());

        let [mut accum] =
            evaluation_accumulator.columns([(eval_domain.log_size(), self.n_constraints())]);
        accum.random_coeff_powers.reverse();

        let _span =
            span!(Level::INFO, "Constraint point-wise eval", class = "ConstraintEval").entered();
        let trace_cols = trace.as_cols_ref().map_cols(|c| c.as_ref());

        *accum.col = accumulate_pointwise_cpu(
            self,
            trace_cols,
            eval_domain.log_size(),
            trace_domain.log_size(),
            denom_inv,
            &accum.random_coeff_powers,
            accum.col,
        );
    }
}

/// Computes the evaluation subdomain for a component given its constraint degree bound
/// and the log_expansion from `EvaluationMode::SubDomain`.
///
/// When `log_expansion == 0`, returns the canonical domain.
/// When `log_expansion > 0`, returns the first subdomain obtained by splitting the
/// committed domain `log_expansion` times.
fn subdomain_eval_domain(max_constraint_log_degree_bound: u32, log_expansion: u32) -> CircleDomain {
    let committed_domain =
        CanonicCoset::new(max_constraint_log_degree_bound + log_expansion).circle_domain();
    committed_domain.split(log_expansion).0
}

fn accumulate_pointwise_cpu<E: FrameworkEval>(
    component: &FrameworkComponent<E>,
    trace_cols: TreeVec<Vec<&CircleEvaluation<CpuBackend, BaseField, BitReversedOrder>>>,
    eval_log_size: u32,
    trace_log_size: u32,
    denom_inv: Vec<BaseField>,
    random_coeff_powers: &[SecureField],
    accum: &SecureColumnByCoords<CpuBackend>,
) -> SecureColumnByCoords<CpuBackend> {
    let mut res = SecureColumnByCoords::zeros(1 << eval_log_size);
    for row in 0..(1 << eval_log_size) {
        // Evaluate constrains at row.
        let eval = CpuDomainEvaluator::new(
            &trace_cols,
            row,
            random_coeff_powers,
            trace_log_size,
            eval_log_size,
            component.eval.log_size(),
            component.claimed_sum,
        );
        let row_res = component.eval.evaluate(eval).row_res;

        // Finalize row.
        let row_denom_inv = denom_inv[row >> trace_log_size];
        res.set(row, accum.at(row) + row_res * row_denom_inv)
    }
    res
}
