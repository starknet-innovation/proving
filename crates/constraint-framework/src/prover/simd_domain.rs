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
use crate::{EvalAtRow, INTERACTION_TRACE_IDX, MAX_N_INTERACTIONS, Relation, RelationEntry};

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
    /// With no window: the first vector row of the block of the evaluation domain's
    /// bit-reversed order being evaluated. A column of the whole domain is read at
    /// `block + vec_row`, a column of the block's size at `vec_row`.
    pub block: usize,
    pub logup: LogupAtRow<Self>,
    /// Base-field constraints not yet added to `row_res`, with their coefficient indices (see
    /// [`Self::flush_deferred`]).
    pub deferred: Vec<(VeryPackedBaseField, u32)>,
}

/// The deferred constraints added to the row result at a time: their values stay in L1.
const DEFERRED_CHUNK: usize = 32;

impl<'a> SimdDomainEvaluator<'a> {
    /// Adds the deferred base-field constraints to `row_res`, one coordinate at a time.
    ///
    /// Added one by one, each constraint loads and stores the whole row accumulator (a very packed
    /// secure value: more registers than AVX2 has), so the store pipe bounds the evaluators on
    /// Zen 3. Here each coordinate's accumulator stays in registers over the chunk. The products
    /// are the same field products and the sums are exact, so the result is unchanged.
    #[inline(always)]
    pub fn flush_deferred(&mut self) {
        if self.deferred.is_empty() {
            return;
        }
        let coords = self.row_res.into_very_packed_m31s();
        let coords: [VeryPackedBaseField; SECURE_EXTENSION_DEGREE] = std::array::from_fn(|j| {
            let mut acc = coords[j];
            for &(value, index) in &self.deferred {
                let coeff = self.random_coeff_powers[index as usize].to_m31_array()[j];
                acc += value * VeryPackedBaseField::broadcast(coeff);
            }
            acc
        });
        self.row_res = VeryPackedSecureField::from_very_packed_m31s(coords);
        self.deferred.clear();
    }

    /// The row's result: `row_res` with the deferred constraints added.
    #[inline(always)]
    pub fn row_result(&mut self) -> VeryPackedSecureField {
        self.flush_deferred();
        self.row_res
    }

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
            block: 0,
            logup: LogupAtRow::new(INTERACTION_TRACE_IDX, claimed_sum, log_size),
            deferred: Vec::new(),
        }
    }

    pub fn new_with_logup(
        trace_eval: &'a TreeVec<Vec<&CircleEvaluation<SimdBackend, BaseField, BitReversedOrder>>>,
        vec_row: usize,
        random_coeff_powers: &'a [SecureField],
        domain_log_size: u32,
        eval_log_size: u32,
        logup: LogupAtRow<Self>,
    ) -> Self {
        Self {
            trace_eval,
            column_index_per_interaction: [0; MAX_N_INTERACTIONS],
            vec_row,
            random_coeff_powers,
            row_res: VeryPackedSecureField::zero(),
            constraint_index: 0,
            domain_log_size,
            eval_domain_log_size: eval_log_size,
            window: None,
            block: 0,
            logup,
            deferred: Vec::new(),
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
                    - mul_numerator(a.denominator, &b.numerator)
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
                    let vec_row = match self.window {
                        Some((first, _)) => self.vec_row + first,
                        None if self.block != 0
                            && col.len() == 1 << self.eval_domain_log_size =>
                        {
                            self.vec_row + self.block
                        }
                        None => self.vec_row,
                    };
                    return *very_packed_col.data.get_unchecked(vec_row);
                };
            }
            // Otherwise, we need to look up the value at the offset.
            // Since the domain is bit-reversed circle domain ordered, we need to look up the value
            // at the bit-reversed natural order index at an offset.
            let col = &self.trace_eval[interaction][col_index];
            match self.window {
                Some((first, sign)) => VeryPackedBaseField::from_array(std::array::from_fn(|i| {
                    let row = ((self.vec_row + first) << (LOG_N_LANES + LOG_N_VERY_PACKED_ELEMS)) + i;
                    col.at((row as isize + sign * off) as usize)
                })),
                None => {
                    let rows = offset_rows(
                        (self.vec_row + self.block) << (LOG_N_LANES + LOG_N_VERY_PACKED_ELEMS),
                        self.domain_log_size,
                        self.eval_domain_log_size,
                        off,
                    );
                    blended_rows(&col.values.data, &rows).unwrap_or_else(|| {
                        VeryPackedBaseField::from_array(std::array::from_fn(|i| col.at(rows[i] as usize)))
                    })
                }
            }
        })
    }
    // Inlined into each component's evaluate: called out of line, every constraint stored and
    // reloaded the row accumulator (two packed secure values) around the call.
    #[inline(always)]
    fn add_constraint<G>(&mut self, constraint: G)
    where
        Self::EF: Mul<G, Output = Self::EF> + From<G>,
    {
        // A base-field constraint (its upper coordinates are known zeros once inlined) is
        // deferred; a secure-field one is added at once.
        let value = Self::EF::from(constraint);
        match as_base(&value) {
            Some(base) => {
                self.deferred.push((base, self.constraint_index as u32));
                if self.deferred.len() == DEFERRED_CHUNK {
                    self.flush_deferred();
                }
            }
            None => {
                self.row_res += <VeryPackedSecureField as Mul<VeryPackedSecureField>>::mul(
                    VeryPackedSecureField::broadcast(self.random_coeff_powers[self.constraint_index]),
                    value,
                );
            }
        }
        self.constraint_index += 1;
    }

    fn combine_ef(values: [Self::F; SECURE_EXTENSION_DEGREE]) -> Self::EF {
        VeryPackedSecureField::from_very_packed_m31s(values)
    }

    // The default `add_to_relation`, inlined into each component's evaluate: called out of line
    // (hundreds of call sites in the Cairo components), every lookup entry went through memory.
    #[inline(always)]
    fn add_to_relation<R: Relation<Self::F, Self::EF>>(
        &mut self,
        entry: RelationEntry<'_, Self::F, Self::EF, R>,
    ) {
        let denominator = match entry.relation.lookup_elements() {
            Some((alpha_powers, z)) => combine_by_coordinates(alpha_powers, z, entry.values),
            None => entry.relation.combine(entry.values),
        };
        let frac = Fraction::new(entry.multiplicity.clone(), denominator);
        self.write_logup_frac(frac);
    }

    #[inline(always)]
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

/// [`crate::logup::LookupElements::combine`] of base-field values, one coordinate at a time.
///
/// Folded in the secure field, every term adds into a very packed secure accumulator (more
/// registers than AVX2 has), so each term stores and reloads it. One coordinate's accumulator fits
/// in registers. The same field products and exact sums: the result is unchanged.
#[inline(always)]
fn combine_by_coordinates(
    alpha_powers: &[SecureField],
    z: SecureField,
    values: &[VeryPackedBaseField],
) -> VeryPackedSecureField {
    assert!(alpha_powers.len() >= values.len(), "Not enough alpha powers to combine values");
    // As `LookupElements::combine`: a first power of one takes the first value as it is.
    let start = usize::from(!values.is_empty() && alpha_powers[0] == SecureField::from(BaseField::from(1)));
    let z = z.to_m31_array();
    VeryPackedSecureField::from_very_packed_m31s(std::array::from_fn(|j| {
        let mut acc = if start == 1 && j == 0 { values[0] } else { VeryPackedBaseField::zero() };
        for (value, power) in values[start..].iter().zip(&alpha_powers[start..]) {
            acc += *value * VeryPackedBaseField::broadcast(power.to_m31_array()[j]);
        }
        acc - VeryPackedBaseField::broadcast(z[j])
    }))
}

#[inline(always)]
fn as_base(x: &VeryPackedSecureField) -> Option<VeryPackedBaseField> {
    let upper_zero = x.0.iter().all(|q| {
        let [_, b, c, d] = q.into_packed_m31s();
        (b.into_simd() | c.into_simd() | d.into_simd()) == std::simd::Simd::splat(0)
    });
    upper_zero.then(|| VeryPackedBaseField::from_fn(|i| x.0[i].into_packed_m31s()[0]))
}

#[inline(always)]
fn mul_numerator(d: VeryPackedSecureField, n: &VeryPackedSecureField) -> VeryPackedSecureField {
    match as_base(n) {
        Some(n) => d * n,
        None => d * *n,
    }
}

/// The values at `rows` (32 consecutive evaluation rows moved by a mask offset) when they keep
/// their lanes and come from two aligned pairs of packed rows: the even lanes from one pair, the
/// odd lanes from the other, which is where an offset of one trace row sends all but a few
/// blocks of a bit-reversed circle domain at log expansion 1. Four packed loads and a blend
/// instead of 32 scalar reads; the values are reduced into `[0, P)`, as `Column::at` returns
/// them. Mechanism: starkience's packed shifted constraint masks (lead rgr1_231c008f).
#[inline(always)]
fn blended_rows(data: &[stwo::prover::backend::simd::m31::PackedBaseField], rows: &[u32; 32]) -> Option<VeryPackedBaseField> {
    let even = rows[0] as usize;
    let odd = (rows[1] as usize).wrapping_sub(1);
    if even % 32 != 0 || odd % 32 != 0 {
        return None;
    }
    if !rows.iter().enumerate().all(|(i, &row)| row as usize == if i % 2 == 0 { even + i } else { odd + i }) {
        return None;
    }
    let lanes = std::simd::Mask::<i32, 16>::from_array(std::array::from_fn(|l| l % 2 == 0));
    use stwo::prover::backend::simd::m31::PackedBaseField;
    let blend = |a: PackedBaseField, b: PackedBaseField| {
        PackedBaseField::reduce_simd(lanes.select(a.into_simd(), b.into_simd()))
    };
    let (even, odd) = (even / 16, odd / 16);
    Some(VeryPackedBaseField::from([blend(data[even], data[odd]), blend(data[even + 1], data[odd + 1])]))
}

#[inline(always)]
fn offset_rows(first: usize, domain_log_size: u32, eval_log_size: u32, offset: isize) -> [u32; 32] {
    use std::simd::cmp::SimdPartialOrd;
    use std::simd::num::SimdUint;
    use std::simd::{Simd, u32x16};
    debug_assert!((5..32).contains(&eval_log_size) && eval_log_size > domain_log_size);
    let shift = u32::BITS - eval_log_size;
    let half = 1u32 << (eval_log_size - 1);
    let mask = u32x16::splat(half - 1);
    let step =
        (offset * (1isize << (eval_log_size - domain_log_size - 1))).rem_euclid(half as isize);
    let step = u32x16::splat(step as u32);
    let lanes = u32x16::from_array(std::array::from_fn(|l| l as u32));
    let mut rows = [0; 32];
    for (v, out) in rows.chunks_exact_mut(16).enumerate() {
        let i = Simd::splat((first + v * 16) as u32) + lanes;
        let prev = i.reverse_bits() >> shift;
        let lower = prev.simd_lt(u32x16::splat(half));
        let up = (prev + step) & mask;
        let down = ((prev - step) & mask) + u32x16::splat(half);
        let next = lower.select(up, down);
        out.copy_from_slice((next.reverse_bits() >> shift).as_array());
    }
    debug_assert!(rows.iter().enumerate().all(|(l, &r)| r as usize
        == offset_bit_reversed_circle_domain_index(
            first + l,
            domain_log_size,
            eval_log_size,
            offset
        )));
    rows
}
