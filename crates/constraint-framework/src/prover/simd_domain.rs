use std::ops::Mul;

use num_traits::Zero;
use stwo::core::Fraction;
use stwo::core::fields::m31::BaseField;
use stwo::core::fields::qm31::{SECURE_EXTENSION_DEGREE, SecureField};
use stwo::core::pcs::TreeVec;
use stwo::core::utils::offset_bit_reversed_circle_domain_index;
use stwo::prover::backend::Column;
use stwo::prover::backend::simd::SimdBackend;
use stwo::prover::backend::simd::column::VeryPackedBaseColumn;
use stwo::prover::backend::simd::m31::LOG_N_LANES;
use stwo::prover::backend::simd::very_packed_m31::{
    LOG_N_VERY_PACKED_ELEMS, VeryPackedBaseField, VeryPackedSecureField,
};
use stwo::prover::poly::BitReversedOrder;
use stwo::prover::poly::circle::CircleEvaluation;

use crate::logup::LogupAtRow;
use crate::{EvalAtRow, INTERACTION_TRACE_IDX, MAX_N_INTERACTIONS};

/// Evaluates constraints at an evaluation domain points.
pub struct SimdDomainEvaluator<'a> {
    pub trace_eval:
        &'a TreeVec<Vec<&'a CircleEvaluation<SimdBackend, BaseField, BitReversedOrder>>>,
    pub column_index_per_interaction: [usize; MAX_N_INTERACTIONS],
    /// The row index of the simd-vector row to evaluate the constraints at.
    pub vec_row: usize,
    pub random_coeff_powers: &'a [SecureField],
    pub row_res: VeryPackedSecureField,
    pub constraint_index: usize,
    pub domain_log_size: u32,
    pub eval_domain_log_size: u32,
    /// `Some((first, sign))`: the trace columns hold a window of one half coset of the
    /// evaluation domain in its natural order, where a mask offset `off` is `sign * off` rows
    /// away, and `vec_row` counts the evaluated rows from the window's vector `first`.
    pub window: Option<(usize, isize)>,
    pub logup: LogupAtRow<Self>,
}
impl<'a> SimdDomainEvaluator<'a> {
    pub fn new(
        trace_eval: &'a TreeVec<Vec<&CircleEvaluation<SimdBackend, BaseField, BitReversedOrder>>>,
        vec_row: usize,
        random_coeff_powers: &'a [SecureField],
        domain_log_size: u32,
        eval_log_size: u32,
        log_size: u32,
        claimed_sum: SecureField,
    ) -> Self {
        Self {
            trace_eval,
            column_index_per_interaction: {
                debug_assert!(trace_eval.len() <= MAX_N_INTERACTIONS);
                [0; MAX_N_INTERACTIONS]
            },
            vec_row,
            random_coeff_powers,
            row_res: VeryPackedSecureField::zero(),
            constraint_index: 0,
            domain_log_size,
            eval_domain_log_size: eval_log_size,
            window: None,
            logup: LogupAtRow::new(INTERACTION_TRACE_IDX, claimed_sum, log_size),
        }
    }

    /// Emit a pair's relation in Horner form, avoiding one secure-field product.
    #[inline(always)]
    fn add_logup_batch_constraint(
        &mut self,
        diff: VeryPackedSecureField,
        fractions: &[Fraction<VeryPackedSecureField, VeryPackedSecureField>],
    ) {
        let constraint = match fractions {
            [a, b] => {
                (diff * a.denominator - a.numerator) * b.denominator
                    - b.numerator * a.denominator
            }
            _ => {
                let frac: Fraction<VeryPackedSecureField, VeryPackedSecureField> =
                    fractions.iter().copied().sum();
                diff * frac.denominator - frac.numerator
            }
        };
        self.add_constraint(constraint);
    }
}
impl EvalAtRow for SimdDomainEvaluator<'_> {
    type F = VeryPackedBaseField;
    type EF = VeryPackedSecureField;

    // TODO(Ohad): Add debug boundary checks.
    fn next_interaction_mask<const N: usize>(
        &mut self,
        interaction: usize,
        offsets: [isize; N],
    ) -> [Self::F; N] {
        let col_index = self.column_index_per_interaction[interaction];
        self.column_index_per_interaction[interaction] += 1;
        offsets.map(|off| {
            // If the offset is 0, we can just return the value directly from this row.
            if off == 0 {
                unsafe {
                    let col =
                        &self.trace_eval.get_unchecked(interaction).get_unchecked(col_index).values;
                    let very_packed_col = VeryPackedBaseColumn::transform_under_ref(col);
                    let vec_row = self.vec_row + self.window.map_or(0, |(first, _)| first);
                    return *very_packed_col.data.get_unchecked(vec_row);
                };
            }
            // Otherwise, we need to look up the value at the offset.
            // Since the domain is bit-reversed circle domain ordered, we need to look up the value
            // at the bit-reversed natural order index at an offset.
            VeryPackedBaseField::from_array(std::array::from_fn(|i| {
                let row_index = match self.window {
                    Some((first, sign)) => {
                        let row = ((self.vec_row + first) << (LOG_N_LANES + LOG_N_VERY_PACKED_ELEMS))
                            + i;
                        (row as isize + sign * off) as usize
                    }
                    None => offset_bit_reversed_circle_domain_index(
                        (self.vec_row << (LOG_N_LANES + LOG_N_VERY_PACKED_ELEMS)) + i,
                        self.domain_log_size,
                        self.eval_domain_log_size,
                        off,
                    ),
                };
                self.trace_eval[interaction][col_index].at(row_index)
            }))
        })
    }
    fn add_constraint<G>(&mut self, constraint: G)
    where
        Self::EF: Mul<G, Output = Self::EF> + From<G>,
    {
        self.row_res +=
            VeryPackedSecureField::broadcast(self.random_coeff_powers[self.constraint_index])
                * constraint;
        self.constraint_index += 1;
    }

    fn combine_ef(values: [Self::F; SECURE_EXTENSION_DEGREE]) -> Self::EF {
        VeryPackedSecureField::from_very_packed_m31s(values)
    }

    fn write_logup_frac(&mut self, fraction: Fraction<Self::EF, Self::EF>) {
        if self.logup.fracs.is_empty() {
            self.logup.is_finalized = false;
        }
        self.logup.fracs.push(fraction);
    }

    fn finalize_logup_batched(&mut self, batch_size: usize) {
        assert!(!self.logup.is_finalized, "LogupAtRow was already finalized");
        assert!(batch_size > 0, "Batch size must be positive");
        assert!(!self.logup.fracs.is_empty(), "No fractions to finalize");

        // Borrow the fractions independently of the evaluator while reading interaction masks.
        // Emit one batch at a time instead of allocating and copying a second fraction vector.
        let fracs = std::mem::take(&mut self.logup.fracs);
        let last_batch = (fracs.len() - 1) / batch_size;
        let mut prev_col_cumsum = Self::EF::zero();
        for (i, chunk) in fracs.chunks(batch_size).enumerate() {
            if i == last_batch {
                let [prev_row_cumsum, cur_cumsum] =
                    self.next_extension_interaction_mask(self.logup.interaction, [-1, 0]);
                let diff = cur_cumsum - prev_row_cumsum - prev_col_cumsum;
                let shifted_diff = diff + self.logup.cumsum_shift;
                self.add_logup_batch_constraint(shifted_diff, chunk);
            } else {
                let [cur_cumsum] =
                    self.next_extension_interaction_mask(self.logup.interaction, [0]);
                let diff = cur_cumsum - prev_col_cumsum;
                prev_col_cumsum = cur_cumsum;
                self.add_logup_batch_constraint(diff, chunk);
            }
        }
        self.logup.fracs = fracs;
        self.logup.is_finalized = true;
    }

    fn finalize_logup(&mut self) {
        self.finalize_logup_batched(1)
    }

    fn finalize_logup_in_pairs(&mut self) {
        self.finalize_logup_batched(2)
    }
}
