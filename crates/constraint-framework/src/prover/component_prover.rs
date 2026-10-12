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
const LOG_N_BLOCKS: u32 = 4;

/// The most bytes of columns that one block of [`FrameworkComponent::evaluate_in_blocks`] holds
/// (the columns that are not read at a mask offset); a wider component takes more, smaller
/// blocks.
const BLOCK_COLUMNS_BYTES: usize = 96 << 20;
/// At most `2^MAX_LOG_N_BLOCKS` blocks: each block folds every column's coefficients again.
const MAX_LOG_N_BLOCKS: u32 = 5;

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
            // SCREEN (a8n Q1457): with `A8N_REGROW1`, the coefficients regrown once per component
            // are resident through its blocks, so the budget counts them.
            let regrown_bytes = if stwo::prover::m11_env("A8N_REGROW1", 0) != 0 {
                let column_bytes = 4usize << (eval_log_size - 1);
                // SCREEN (a8n Q1464): `A8N_REGROW1_MIB` caps the bytes regrown once.
                let cap = stwo::prover::m11_env("A8N_REGROW1_MIB", usize::MAX >> 20) << 20;
                (self.n_regrown_columns(trace) * column_bytes).min(cap / column_bytes * column_bytes)
            } else {
                0
            };
            let mut log_n_blocks = LOG_N_BLOCKS;
            while log_n_blocks < MAX_LOG_N_BLOCKS
                && n_block_columns * (4usize << (eval_log_size - log_n_blocks)) + regrown_bytes
                    > stwo::prover::m11_env("M11_BLOCK_MIB", BLOCK_COLUMNS_BYTES >> 20) << 20
            {
                log_n_blocks += 1;
            }
            let mut once_budget = if stwo::prover::m11_env("A8N_REGROW1", 0) != 0 {
                stwo::prover::m11_env("A8N_REGROW1_MIB", usize::MAX >> 20) << 20
            } else {
                0
            };
            // SCREEN (f2 Q1650): `F2_OPTNB=1` picks the block count and the columns regrown once
            // together: the fewest coefficient regrowths (IFFTs) per block, at no more bytes than
            // the rule above holds (plus `F2_SLACK_MIB`).
            if stwo::prover::m11_env("F2_OPTNB", 0) != 0 {
                let column_bytes = 4usize << (eval_log_size - 1);
                let n_regrown = self.n_regrown_columns(trace);
                let held = n_block_columns * (4usize << (eval_log_size - log_n_blocks)) + regrown_bytes
                    + (stwo::prover::m11_env("F2_SLACK_MIB", 0) << 20);
                let mut best = (usize::MAX, log_n_blocks, regrown_bytes / column_bytes);
                for nb in LOG_N_BLOCKS..=stwo::prover::m11_env("F2_NBMAX", MAX_LOG_N_BLOCKS as usize) as u32 {
                    // Under `F2_SUPER=g` a column regrown per block also holds g - 1 blocks ahead.
                    let g = stwo::prover::m11_env("F2_SUPER", 1).max(1).min(1 << nb);
                    let block_bytes = 4usize << (eval_log_size - nb);
                    let blocks = n_block_columns * block_bytes + n_regrown * (g - 1) * block_bytes;
                    if blocks > held {
                        continue;
                    }
                    // A column regrown once frees its g - 1 blocks ahead.
                    let k = ((held - blocks) / (column_bytes - (g - 1) * block_bytes).max(1)).min(n_regrown);
                    let cost = (n_block_columns + (n_regrown - k) * (eval_log_size as usize + 1) / g) << nb;
                    if cost < best.0 {
                        best = (cost, nb, k);
                    }
                }
                if best.0 != usize::MAX {
                    log_n_blocks = best.1;
                    once_budget = best.2 * column_bytes;
                }
            }
            return self.evaluate_in_blocks(trace, evaluation_accumulator, log_n_blocks, once_budget);
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

    /// The number of block columns (see [`Self::n_block_columns`]) that keep no coefficients,
    /// which [`Self::evaluate_in_blocks`] regrows.
    fn n_regrown_columns(&self, trace: &Trace<'_, SimdBackend>) -> usize {
        let component_polys = get_component_polys(self, trace);
        let mask = self.shifted_columns();
        component_polys
            .iter()
            .enumerate()
            .map(|(t, polys)| match mask.get(t) {
                Some(m) if m.len() == polys.len() => polys
                    .iter()
                    .zip(m)
                    .filter(|(poly, shifted)| !**shifted && poly.coeffs.is_none())
                    .count(),
                Some(m) if m.is_empty() && t == PREPROCESSED_TRACE_IDX => {
                    polys.iter().filter(|poly| poly.coeffs.is_none()).count()
                }
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
        self.accumulate_rows_at(trace_cols, col, None, random_coeff_powers, eval_log_size, denom_inv, window, block, None)
    }

    /// [`Self::accumulate_rows`] into the rows `at = (start, len)` of `col` (all of it if `None`);
    /// the evaluated rows are numbered from 0 as they are without `at`.
    #[allow(clippy::too_many_arguments)]
    fn accumulate_rows_at(
        &self,
        trace_cols: &TreeVec<Vec<&CircleEvaluation<SimdBackend, BaseField, BitReversedOrder>>>,
        col: &mut SecureColumnByCoords<SimdBackend>,
        at: Option<(usize, usize)>,
        random_coeff_powers: &[SecureField],
        eval_log_size: u32,
        denom_inv: &[BaseField],
        window: Option<(usize, isize)>,
        block: usize,
        shift_window: Option<(u32, u32)>,
    ) {
        let (start, len) = at.unwrap_or((0, col.len()));
        let shift = LOG_N_LANES + LOG_N_VERY_PACKED_ELEMS;
        let range = 0..(len >> shift);
        let col = unsafe { VeryPackedSecureColumnByCoords::transform_under_mut(col) };

        #[cfg(not(feature = "parallel"))]
        let iter = range
            .step_by(CHUNK_SIZE)
            .zip(col.chunks_mut(CHUNK_SIZE).skip((start >> shift) / CHUNK_SIZE));

        #[cfg(feature = "parallel")]
        let iter = range
            .into_par_iter()
            .step_by(CHUNK_SIZE)
            .zip(col.par_chunks_mut(CHUNK_SIZE).skip((start >> shift) / CHUNK_SIZE));

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
                eval.shift_window = shift_window;
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
        once_budget: usize,
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
        // SCREEN (w5 Q1608): `W5_SHIFTWIN=1` keeps the shifted columns as a sliding window of four
        // blocks instead of extending them to the whole domain: a block's rows at mask offset -1
        // or +1 lie in the blocks next to it in natural low-bit order (`r - 1`, `r + 1` for block
        // `reverse(r)`), so the blocks are visited in that order and each is evaluated once (the
        // first one twice).
        let shiftwin = stwo::prover::m11_env("W5_SHIFTWIN", 0) != 0 && log_n_blocks >= 2;
        let window_coeffs = component_polys
            .as_cols_ref()
            .zip_cols(shifted.as_cols_ref())
            .map_cols(|(poly, shifted): (&&&Poly<SimdBackend>, &bool)| {
                (shiftwin && *shifted).then(|| coefficients(**poly))
            });
        // The shifted columns, extended to the whole domain.
        let mut whole = component_polys
            .as_cols_ref()
            .zip_cols(shifted.as_cols_ref())
            .map_cols(|(poly, shifted): (&&&Poly<SimdBackend>, &bool)| {
                if shiftwin && *shifted {
                    let mut evaluation = CircleEvaluation::<SimdBackend, BaseField, BitReversedOrder>::new(
                        CanonicCoset::new(1).circle_domain(),
                        BaseColumn::zeros(2),
                    );
                    evaluation.values = BaseColumn::zeros(4 << log_block);
                    return Some(evaluation);
                }
                shifted.then(|| poly.get_evaluation_on_domain(eval_domain, &twiddles))
            });
        let mut slots: [Option<usize>; 4] = [None; 4];
        let reverse = |r: usize| r.reverse_bits() >> (usize::BITS - log_n_blocks);
        // SCREEN (w5 Q1606 census): `W5_WHOLEDBG=1` prints, per windowed component, its size, the
        // columns extended whole (read at an offset) and their bytes, and the block plan.
        if false {
            let n_whole = whole.as_cols_ref().flatten().iter().filter(|w| w.is_some()).count();
            eprintln!(
                "W5WHOLE log_size {log_size} blocks {log_n_blocks} whole_cols {n_whole} whole_mib {} block_cols {} block_mib {}",
                (n_whole << (log_size + 3)) >> 20,
                self.n_block_columns(trace),
                (self.n_block_columns(trace) << (log_block + 2)) >> 20,
            );
        }
        // SCREEN (a8n Q1457): `A8N_REGROW1=1` regrows a block column's coefficients once for all
        // the blocks instead of once per block.
        // SCREEN (a8n Q1464): at most `once_budget` bytes of coefficients regrown once, the first
        // columns in order; the others are regrown per block as before.
        let regrow_once = once_budget > 0;
        let mut regrow_cap = once_budget;
        // SCREEN (w5 Q1609): `W5_COMPRSS=<MiB>` sizes the bytes regrown once from the headroom
        // under a target resident set: the target minus the resident set now, minus this
        // component's block columns and accumulator, minus 16 MiB of margin (never below the
        // static cap). Linux only (reads /proc/self/statm); the proof is the same at any cap.
        // SCREEN (w5 Q1612): `W5_COMPEST=<MiB>` is the same budget from a deterministic stand-in for
        // the resident set: the tree bytes held at `prove_ex` plus `W5_COMPOFF` MiB (default 320).
        let est_target = stwo::prover::m11_env("W5_COMPEST", 0) << 20;
        if est_target > 0 {
            let resident = stwo::prover::W5_HELD.load(std::sync::atomic::Ordering::Relaxed)
                + (stwo::prover::m11_env("W5_COMPOFF", 320) << 20);
            let blocks = self.n_block_columns(trace) * (4usize << log_block) + (16usize << log_block);
            regrow_cap = regrow_cap.max(est_target.saturating_sub(resident + blocks + (16 << 20)));
        }
        let column_bytes = 4usize << log_size;
        let mut budget = regrow_cap;
        let allowed = component_polys.as_cols_ref().zip_cols(whole.as_cols_ref()).map_cols(
            |(poly, whole): (&&&Poly<SimdBackend>, &Option<CircleEvaluation<SimdBackend, BaseField, BitReversedOrder>>)| {
                let take = regrow_once && whole.is_none() && poly.coeffs.is_none() && budget >= column_bytes;
                if take {
                    budget -= column_bytes;
                }
                take
            },
        );
        let once_fn = |((poly, _whole), &take): ((&&&Poly<SimdBackend>, &Option<CircleEvaluation<SimdBackend, BaseField, BitReversedOrder>>), &bool)| {
            take.then(|| poly.regrown_coefficients())
        };
        #[cfg(not(feature = "parallel"))]
        let once = component_polys.as_cols_ref().zip_cols(whole.as_cols_ref()).zip_cols(allowed.as_cols_ref()).map_cols(once_fn);
        #[cfg(feature = "parallel")]
        let once = component_polys.as_cols_ref().zip_cols(whole.as_cols_ref()).zip_cols(allowed.as_cols_ref()).par_map_cols(once_fn);
        let seq = stwo::prover::m11_env("F2_SEQ", 0) != 0;
        // SCREEN (f2 Q1663): `F2_SUPER=g` regrows a column without coefficients once per `g`
        // blocks and keeps the next `g - 1` blocks' values (a block is 1/16..1/32 of the
        // coefficients' bytes), instead of regrowing it for every block.
        let mut superg = stwo::prover::m11_env("F2_SUPER", 1).max(1);
        let n_blocks = 1usize << log_n_blocks;
        // SCREEN (f2 Q1686): `F2_SUPERMIB=m` caps the blocks kept ahead at m MiB per component: g is
        // lowered until (g - 1) blocks of every per-block column fit.
        let super_mib = stwo::prover::m11_env("F2_SUPERMIB", 0);
        if super_mib > 0 && superg > 1 {
            let n_per_block = component_polys
                .as_cols_ref()
                .zip_cols(whole.as_cols_ref())
                .zip_cols(once.as_cols_ref())
                .map_cols(|((poly, whole), once): ((&&&Poly<SimdBackend>, &Option<CircleEvaluation<SimdBackend, BaseField, BitReversedOrder>>), &Option<stwo::prover::poly::circle::CircleCoefficients<SimdBackend>>)| {
                    whole.is_none() && once.is_none() && poly.coeffs.is_none()
                })
                .0
                .iter()
                .flatten()
                .filter(|&&b| b)
                .count();
            let per_block_bytes = (n_per_block * 4 * block_len).max(1);
            superg = superg.min(1 + (super_mib << 20) / per_block_bytes);
        }
        // A block's column buffers are reused by the next block (no fresh pages).
        let spare = std::sync::Mutex::new(Vec::<BaseColumn>::new());
        let ahead = component_polys.as_cols_ref().map_cols(|_| std::sync::Mutex::new(Vec::<BaseColumn>::new()));
        type Ahead = std::sync::Mutex<Vec<BaseColumn>>;
        // Under W5_SHIFTWIN the blocks are visited out of order: look ahead in visiting order.
        let visit = |s: usize| if shiftwin { reverse(s) } else { s };
        let per_block = |poly: &Poly<SimdBackend>, ahead: &Ahead, step: usize, dst: &mut BaseColumn| {
            let mut ahead = ahead.lock().unwrap();
            if let Some(next) = ahead.pop() {
                let old = std::mem::replace(dst, next);
                spare.lock().unwrap().push(old);
                return;
            }
            let coeffs = poly.regrown_coefficients();
            evaluate_block_into(&coeffs, eval_domain, &twiddles, log_block, visit(step), dst);
            if superg > 1 {
                for next in (step + 1..(step + superg).min(n_blocks)).rev() {
                    let mut values =
                        spare.lock().unwrap().pop().unwrap_or_else(|| BaseColumn::zeros(block_len));
                    evaluate_block_into(&coeffs, eval_domain, &twiddles, log_block, visit(next), &mut values);
                    ahead.push(values);
                }
            }
        };
        for step in 0..n_blocks {
            let block = visit(step);
            if shiftwin {
                let n = 1usize << log_n_blocks;
                for r in [(step + n - 1) % n, step, (step + 1) % n] {
                    let slot = r & 3;
                    if slots[slot] == Some(r) {
                        continue;
                    }
                    slots[slot] = Some(r);
                    let fill_slot = |(coeffs, whole): (
                        &Option<Cow<'_, stwo::prover::poly::circle::CircleCoefficients<SimdBackend>>>,
                        &mut Option<CircleEvaluation<SimdBackend, BaseField, BitReversedOrder>>,
                    )| {
                        if let (Some(coeffs), Some(whole)) = (coeffs, whole) {
                            let mut values = BaseColumn::zeros(block_len);
                            evaluate_block_into(coeffs, eval_domain, &twiddles, log_block, reverse(r), &mut values);
                            let n_vecs = block_len >> LOG_N_LANES;
                            whole.values.data[slot * n_vecs..(slot + 1) * n_vecs].copy_from_slice(&values.data);
                        }
                    };
                    #[cfg(not(feature = "parallel"))]
                    window_coeffs.as_cols_ref().zip_cols(TreeVec::new(whole.iter_mut().map(|c| c.iter_mut().collect::<Vec<_>>()).collect::<Vec<_>>())).map_cols(fill_slot);
                    #[cfg(feature = "parallel")]
                    window_coeffs.as_cols_ref().zip_cols(TreeVec::new(whole.iter_mut().map(|c| c.iter_mut().collect::<Vec<_>>()).collect::<Vec<_>>())).par_map_cols(fill_slot);
                }
            }
            let fill = |(((poly, whole), once), ahead): (((&&&Poly<SimdBackend>, &Option<CircleEvaluation<SimdBackend, BaseField, BitReversedOrder>>), &Option<stwo::prover::poly::circle::CircleCoefficients<SimdBackend>>), &Ahead)| {
                if whole.is_some() {
                    return None;
                }
                let mut values =
                    spare.lock().unwrap().pop().unwrap_or_else(|| BaseColumn::zeros(block_len));
                // SCREEN (f2 Q1651): under `F2_SEQ`, columns regrown per block are filled after
                // this parallel pass, one at a time (each IFFT is parallel inside).
                if let Some(coeffs) = once {
                    evaluate_block_into(coeffs, eval_domain, &twiddles, log_block, block, &mut values);
                } else if poly.coeffs.is_some() {
                    let coeffs = coefficients(**poly);
                    evaluate_block_into(&coeffs, eval_domain, &twiddles, log_block, block, &mut values);
                } else if !seq {
                    per_block(**poly, ahead, step, &mut values);
                }
                let mut evaluation = CircleEvaluation::<SimdBackend, BaseField, BitReversedOrder>::new(
                    CanonicCoset::new(1).circle_domain(),
                    BaseColumn::zeros(2),
                );
                evaluation.values = values;
                Some(evaluation)
            };
            #[cfg(not(feature = "parallel"))]
            let mut blocks = component_polys.as_cols_ref().zip_cols(whole.as_cols_ref()).zip_cols(once.as_cols_ref()).zip_cols(ahead.as_cols_ref()).map_cols(fill);
            #[cfg(feature = "parallel")]
            let mut blocks = component_polys.as_cols_ref().zip_cols(whole.as_cols_ref()).zip_cols(once.as_cols_ref()).zip_cols(ahead.as_cols_ref()).par_map_cols(fill);
            if seq {
                for ((bt, pt), ((ot, wt), at)) in blocks.0.iter_mut().zip(component_polys.0.iter()).zip(once.0.iter().zip(whole.0.iter()).zip(ahead.0.iter())) {
                    for ((b, p), ((o, w), a)) in bt.iter_mut().zip(pt.iter()).zip(ot.iter().zip(wt.iter()).zip(at.iter())) {
                        if w.is_none() && o.is_none() && p.coeffs.is_none() {
                            per_block(p, a, step, &mut b.as_mut().unwrap().values);
                        }
                    }
                }
            }
            let columns = blocks
                .as_cols_ref()
                .zip_cols(whole.as_cols_ref())
                .map_cols(
                    |(block, whole): (
                        &Option<CircleEvaluation<SimdBackend, BaseField, BitReversedOrder>>,
                        &Option<CircleEvaluation<SimdBackend, BaseField, BitReversedOrder>>,
                    )| block.as_ref().or(whole.as_ref()).unwrap(),
                );
            // SCREEN (r77, d9x DIRECT): `R77_DIRECT=1` accumulates the rows straight into the block
            // of the accumulator (the same additions: `acc + (0 + r)` is `acc + r`), with no zeroed
            // block of rows and no second pass adding it in.
            let direct = true
                && block_len % (CHUNK_SIZE << (LOG_N_LANES + LOG_N_VERY_PACKED_ELEMS)) == 0;
            let mut rows =
                SecureColumnByCoords::<SimdBackend>::zeros(if direct { 0 } else { block_len });
            self.accumulate_rows_at(
                &columns,
                if direct { &mut *accum.col } else { &mut rows },
                direct.then_some((block * block_len, block_len)),
                &accum.random_coeff_powers,
                eval_domain.log_size(),
                &denom_inv,
                None,
                block * block_len >> (LOG_N_LANES + LOG_N_VERY_PACKED_ELEMS),
                shiftwin.then_some((log_block, log_n_blocks)),
            );
            drop(columns);
            spare
                .lock()
                .unwrap()
                .extend(blocks.0.into_iter().flatten().flatten().map(|evaluation| evaluation.values));
            for (dst, src) in accum.col.columns.iter_mut().zip(&rows.columns).filter(|_| !direct) {
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
