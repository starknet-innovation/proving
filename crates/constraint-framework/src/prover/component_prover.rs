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
use stwo::prover::backend::simd::circle::{
    block_chunk_log_size, block_twiddles, evaluate_block_first_phase_into, evaluate_block_into,
    finish_block_chunk_into,
};
use stwo::core::fields::qm31::SECURE_EXTENSION_DEGREE;
use stwo::prover::backend::simd::column::{
    BaseColumn, VeryPackedBaseColumnMutSlice, VeryPackedSecureColumnByCoordsMutSlice,
};
use stwo::prover::backend::simd::m31::{LOG_N_LANES, PackedBaseField};
use stwo::prover::backend::simd::very_packed_m31::{
    LOG_N_VERY_PACKED_ELEMS, VeryPackedBaseField, as_very_packed_mut,
};
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

/// The most bytes of columns that one block of [`FrameworkComponent::evaluate_in_blocks`] holds
/// (the columns that are not read at a mask offset); a wider component takes more, smaller
/// blocks. Every block folds each column's coefficients again: at 2^23 circuit rows the first
/// season's 96 MiB took the circuit's components to 32 blocks, and its composition to 9.3 of the
/// proof's 23 s; at 512 MiB it takes 3.6 s, and its memory stays under the decommitment's peak.
const BLOCK_COLUMNS_BYTES: usize = 768 << 20;
/// At most `2^MAX_LOG_N_BLOCKS` blocks: each block folds every column's coefficients again.
const MAX_LOG_N_BLOCKS: u32 = 5;
/// The most bytes of block columns for which a component is evaluated in two halves instead of
/// `2^LOG_N_BLOCKS` blocks: the circuit's narrow block-evaluated components (eq, triple_xor,
/// m31_to_u32, verify_bitwise_xor_12; at most ~190 MiB per half), never qm31_ops or
/// blake_g_gate, whose block windows set the composition's memory.
const NARROW_BLOCK_COLUMNS_BYTES: usize = 192 << 20;

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
            // Enough blocks that one block of the columns evaluated per block stays under
            // `BLOCK_COLUMNS_BYTES`: the window of a wide component is its blocks' columns.
            let n_block_columns = self.n_block_columns(trace);
            // A narrow component whose half-domain blocks stay under `NARROW_BLOCK_COLUMNS_BYTES`
            // takes two blocks: each is the evaluation on a subdomain of the coefficients' size
            // (no fold), and the coefficients are read twice instead of four times. Every row is
            // still evaluated once, from the same values, so the accumulator is unchanged.
            let mut log_n_blocks =
                if n_block_columns * (4usize << (eval_log_size - 1)) <= NARROW_BLOCK_COLUMNS_BYTES {
                    1
                } else {
                    LOG_N_BLOCKS
                };
            while log_n_blocks < MAX_LOG_N_BLOCKS
                && n_block_columns * (4usize << (eval_log_size - log_n_blocks)) > BLOCK_COLUMNS_BYTES
            {
                log_n_blocks += 1;
            }
            return self.evaluate_in_blocks(trace, evaluation_accumulator, log_n_blocks);
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
    /// The number of this component's trace columns that [`Self::evaluate_in_blocks`] evaluates
    /// block by block: those not read at a nonzero mask offset.
    fn n_block_columns(&self, trace: &Trace<'_, SimdBackend>) -> usize {
        let component_polys = get_component_polys(self, trace);
        let mask = self.shifted_columns();
        component_polys
            .iter()
            .enumerate()
            .map(|(t, polys)| match mask.get(t) {
                Some(m) if m.len() == polys.len() => m.iter().filter(|&&shifted| !shifted).count(),
                Some(m) if m.is_empty() && t == PREPROCESSED_TRACE_IDX => polys.len(),
                _ => 0,
            })
            .sum()
    }

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
        // Whole very packed rows only, as before (a trailing half is never evaluated).
        let n_packed = (col.len() >> (LOG_N_LANES + LOG_N_VERY_PACKED_ELEMS)) << LOG_N_VERY_PACKED_ELEMS;
        let [c0, c1, c2, c3] = &mut col.columns;
        let coords = [c0, c1, c2, c3].map(|c| &mut c.data[..n_packed]);
        self.accumulate_rows_into(
            trace_cols,
            coords,
            random_coeff_powers,
            eval_log_size,
            denom_inv,
            window,
            block,
            false,
        );
    }

    /// [`Self::accumulate_rows`] into four coordinate slices of packed rows (the rows of a whole
    /// column, or of one block of it): each row adds `row_res * denom_inv` to the value already
    /// there, as before.
    fn accumulate_rows_into(
        &self,
        trace_cols: &TreeVec<Vec<&CircleEvaluation<SimdBackend, BaseField, BitReversedOrder>>>,
        coords: [&mut [PackedBaseField]; SECURE_EXTENSION_DEGREE],
        random_coeff_powers: &[SecureField],
        eval_log_size: u32,
        denom_inv: &[BaseField],
        window: Option<(usize, isize)>,
        block: usize,
        serial: bool,
    ) {
        let [c0, c1, c2, c3] =
            coords.map(as_very_packed_mut);
        let range = 0..c0.len();

        #[cfg(not(feature = "parallel"))]
        let iter = range.step_by(CHUNK_SIZE).zip(
            itertools::izip!(
                c0.chunks_mut(CHUNK_SIZE),
                c1.chunks_mut(CHUNK_SIZE),
                c2.chunks_mut(CHUNK_SIZE),
                c3.chunks_mut(CHUNK_SIZE)
            )
            .map(|(a, b, c, d)| {
                VeryPackedSecureColumnByCoordsMutSlice([
                    VeryPackedBaseColumnMutSlice(a),
                    VeryPackedBaseColumnMutSlice(b),
                    VeryPackedBaseColumnMutSlice(c),
                    VeryPackedBaseColumnMutSlice(d),
                ])
            }),
        );

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

        let body = |(fracs, deferred): &mut (Vec<_>, Vec<_>),
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
                eval.deferred = std::mem::take(deferred);
                let mut eval = self_eval.evaluate(eval);
                let row_res = eval.row_result();
                *fracs = std::mem::take(&mut eval.logup.fracs);
                fracs.clear();
                *deferred = std::mem::take(&mut eval.deferred);

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
            let _ = serial;
            let mut bufs = (Vec::new(), Vec::new());
            iter.for_each(|item| body(&mut bufs, item));
        }

        #[cfg(feature = "parallel")]
        if serial {
            // The rows in order on this thread (a caller already running in parallel).
            let mut bufs = (Vec::new(), Vec::new());
            range.step_by(CHUNK_SIZE)
                .zip(itertools::izip!(
                    c0.chunks_mut(CHUNK_SIZE),
                    c1.chunks_mut(CHUNK_SIZE),
                    c2.chunks_mut(CHUNK_SIZE),
                    c3.chunks_mut(CHUNK_SIZE)
                ))
                .for_each(|(chunk_idx, (a, b, c, d))| {
                    body(
                        &mut bufs,
                        (
                            chunk_idx,
                            VeryPackedSecureColumnByCoordsMutSlice([
                                VeryPackedBaseColumnMutSlice(a),
                                VeryPackedBaseColumnMutSlice(b),
                                VeryPackedBaseColumnMutSlice(c),
                                VeryPackedBaseColumnMutSlice(d),
                            ]),
                        ),
                    )
                });
        } else {
            range
                .into_par_iter()
                .step_by(CHUNK_SIZE)
                .zip(
                    c0.par_chunks_mut(CHUNK_SIZE)
                        .zip(c1.par_chunks_mut(CHUNK_SIZE))
                        .zip(c2.par_chunks_mut(CHUNK_SIZE))
                        .zip(c3.par_chunks_mut(CHUNK_SIZE))
                        .map(|(((a, b), c), d)| {
                            VeryPackedSecureColumnByCoordsMutSlice([
                                VeryPackedBaseColumnMutSlice(a),
                                VeryPackedBaseColumnMutSlice(b),
                                VeryPackedBaseColumnMutSlice(c),
                                VeryPackedBaseColumnMutSlice(d),
                            ])
                        }),
                )
                .for_each_init(|| (Vec::new(), Vec::new()), body);
        }
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
        let extend = |(poly, shifted): (&&&Poly<SimdBackend>, &bool)| {
            shifted.then(|| poly.get_evaluation_on_domain(eval_domain, &twiddles))
        };
        // A block column kept without coefficients (a compact preprocessed column) is regrown
        // once here for all the blocks instead of once per block: every block folds the same
        // coefficients, so its values are unchanged. (A column is extended whole exactly when it
        // is shifted, so `!shifted` is the former `whole.is_none()`.)
        let regrow = |(poly, shifted): (&&&Poly<SimdBackend>, &bool)| {
            (!*shifted && poly.coeffs.is_none()).then(|| poly.regrown_coefficients())
        };
        let inputs = || component_polys.as_cols_ref().zip_cols(shifted.as_cols_ref());
        #[cfg(not(feature = "parallel"))]
        let (whole, regrown) = (inputs().map_cols(extend), inputs().map_cols(regrow));
        // The shifted columns' extensions (each FFT parallel on its own, formerly one column
        // after the other) and the compact columns' regrowth run together: each column's values
        // are computed exactly as before, only concurrently. The regrowth twiddles are found once
        // before the columns are regrown, instead of by every column racing on the empty cache.
        #[cfg(feature = "parallel")]
        let (whole, regrown) = rayon::join(
            || inputs().par_map_cols(extend),
            || {
                for (poly, shifted) in inputs().0.into_iter().flatten() {
                    if !*shifted && poly.coeffs.is_none() {
                        poly.warm_regrowth_twiddles();
                    }
                }
                inputs().par_map_cols(regrow)
            },
        );
        // A block's column buffers are reused by the next block (no fresh pages).
        let spare = std::sync::Mutex::new(Vec::<BaseColumn>::new());
        // The first block's buffers: every value is written by `evaluate_block_into`'s FFT, so they
        // are taken as the commitment's evaluation buffers are, without zeroing them first.
        let fresh = stwo::prover::mempool::BaseColumnPool::<SimdBackend>::new();
        // A large block's fft ends with independent chunks of its second phase: each chunk is
        // completed for every block column into small per-job buffers and its rows evaluated on
        // the same thread, instead of the second phase writing the whole block back to memory
        // and the evaluation reading it again (realadii, rgr1_47b6d3ff).
        let chunk_log = block_chunk_log_size(log_block);
        for block in 0..1usize << log_n_blocks {
            let fill = |((poly, whole), regrown): (
                (&&&Poly<SimdBackend>, &Option<CircleEvaluation<SimdBackend, BaseField, BitReversedOrder>>),
                &Option<stwo::prover::poly::circle::CircleCoefficients<SimdBackend>>,
            )| {
                if whole.is_some() {
                    return None;
                }
                let reused = spare.lock().unwrap().pop();
                let mut values = reused.unwrap_or_else(|| fresh.take_or_alloc(log_block));
                let coeffs = match regrown {
                    Some(coeffs) => Cow::Borrowed(coeffs),
                    None => coefficients(**poly),
                };
                // With chunks, only the fft's first phase: the second runs per chunk below.
                if chunk_log.is_some() {
                    evaluate_block_first_phase_into(
                        &coeffs, eval_domain, &twiddles, log_block, block, &mut values,
                    );
                } else {
                    evaluate_block_into(&coeffs, eval_domain, &twiddles, log_block, block, &mut values);
                }
                let mut evaluation = CircleEvaluation::<SimdBackend, BaseField, BitReversedOrder>::new(
                    CanonicCoset::new(1).circle_domain(),
                    BaseColumn::zeros(2),
                );
                evaluation.values = values;
                Some(evaluation)
            };
            let inputs =
                component_polys.as_cols_ref().zip_cols(whole.as_cols_ref()).zip_cols(regrown.as_cols_ref());
            #[cfg(not(feature = "parallel"))]
            let blocks = inputs.map_cols(fill);
            #[cfg(feature = "parallel")]
            let blocks = inputs.par_map_cols(fill);
            let columns = blocks
                .as_cols_ref()
                .zip_cols(whole.as_cols_ref())
                .map_cols(
                    |(block, whole): (
                        &Option<CircleEvaluation<SimdBackend, BaseField, BitReversedOrder>>,
                        &Option<CircleEvaluation<SimdBackend, BaseField, BitReversedOrder>>,
                    )| block.as_ref().or(whole.as_ref()).unwrap(),
                );
            // The block's rows accumulate straight into the accumulator's slice for the block:
            // each row adds `row_res * denom_inv` to the value there, the value the zeroed
            // per-block buffer and its add pass produced (the accumulator stays below P).
            if let Some(chunk_log) = chunk_log {
                drop(columns);
                let block_twiddles = block_twiddles(eval_domain, &twiddles, log_block, block);
                let chunk_packed = 1usize << (chunk_log - LOG_N_LANES);
                let start = (block * block_len) >> LOG_N_LANES;
                let n_packed = block_len >> LOG_N_LANES;
                let [c0, c1, c2, c3] = &mut accum.col.columns;
                let [c0, c1, c2, c3] = [c0, c1, c2, c3].map(|c| &mut c.data[start..start + n_packed]);
                let random_coeff_powers = &accum.random_coeff_powers;
                let chunk_buffers = || {
                    blocks.as_cols_ref().map_cols(
                        |block: &Option<CircleEvaluation<SimdBackend, BaseField, BitReversedOrder>>| {
                            block.as_ref().map(|_| {
                                let mut buffer =
                                    CircleEvaluation::<SimdBackend, BaseField, BitReversedOrder>::new(
                                        CanonicCoset::new(1).circle_domain(),
                                        BaseColumn::zeros(2),
                                    );
                                buffer.values = BaseColumn::zeros(1 << chunk_log);
                                buffer
                            })
                        },
                    )
                };
                let run_chunk = |buffers: &mut TreeVec<
                    Vec<Option<CircleEvaluation<SimdBackend, BaseField, BitReversedOrder>>>,
                >,
                                 (h, (((a, b), c), d)): (
                    usize,
                    (
                        (
                            (&mut [PackedBaseField], &mut [PackedBaseField]),
                            &mut [PackedBaseField],
                        ),
                        &mut [PackedBaseField],
                    ),
                )| {
                    for (buffer, block) in buffers.iter_mut().flatten().zip(blocks.iter().flatten()) {
                        if let (Some(buffer), Some(block)) = (buffer, block) {
                            finish_block_chunk_into(
                                &block.values,
                                &block_twiddles,
                                log_block,
                                h,
                                &mut buffer.values.data,
                            );
                        }
                    }
                    let columns = buffers.as_cols_ref().zip_cols(whole.as_cols_ref()).map_cols(
                        |(buffer, whole): (
                            &Option<CircleEvaluation<SimdBackend, BaseField, BitReversedOrder>>,
                            &Option<CircleEvaluation<SimdBackend, BaseField, BitReversedOrder>>,
                        )| buffer.as_ref().or(whole.as_ref()).unwrap(),
                    );
                    self.accumulate_rows_into(
                        &columns,
                        [a, b, c, d],
                        random_coeff_powers,
                        eval_domain.log_size(),
                        &denom_inv,
                        None,
                        (block * block_len + (h << chunk_log))
                            >> (LOG_N_LANES + LOG_N_VERY_PACKED_ELEMS),
                        true,
                    );
                };
                #[cfg(not(feature = "parallel"))]
                {
                    let mut buffers = chunk_buffers();
                    itertools::izip!(
                        c0.chunks_mut(chunk_packed),
                        c1.chunks_mut(chunk_packed),
                        c2.chunks_mut(chunk_packed),
                        c3.chunks_mut(chunk_packed)
                    )
                    .enumerate()
                    .for_each(|(h, (a, b, c, d))| run_chunk(&mut buffers, (h, (((a, b), c), d))));
                }
                #[cfg(feature = "parallel")]
                c0.par_chunks_mut(chunk_packed)
                    .zip(c1.par_chunks_mut(chunk_packed))
                    .zip(c2.par_chunks_mut(chunk_packed))
                    .zip(c3.par_chunks_mut(chunk_packed))
                    .enumerate()
                    .for_each_init(chunk_buffers, run_chunk);
            } else {
                let start = (block * block_len) >> LOG_N_LANES;
                let n_packed = block_len >> LOG_N_LANES;
                let [c0, c1, c2, c3] = &mut accum.col.columns;
                let coords = [c0, c1, c2, c3].map(|c| &mut c.data[start..start + n_packed]);
                self.accumulate_rows_into(
                    &columns,
                    coords,
                    &accum.random_coeff_powers,
                    eval_domain.log_size(),
                    &denom_inv,
                    None,
                    block * block_len >> (LOG_N_LANES + LOG_N_VERY_PACKED_ELEMS),
                    false,
                );
                drop(columns);
            }
            spare
                .lock()
                .unwrap()
                .extend(blocks.0.into_iter().flatten().flatten().map(|evaluation| evaluation.values));
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
