#![allow(unused_parens)]
use crate::circuit_air::components::range_check_16::{
    Claim, InteractionClaim, LOG_SIZE, N_TRACE_COLUMNS,
};
use crate::witness::components::prelude::*;

pub type InputType = [M31; 1];
pub type PackedInputType = [PackedM31; 1];

pub struct ClaimGenerator {
    pub mults: [AtomicMultiplicityColumn; 1],
    preprocessed_trace: Arc<PreProcessedTrace>,
}

impl ClaimGenerator {
    pub fn new(preprocessed_trace: Arc<PreProcessedTrace>) -> Self {
        let mults = from_fn(|_| AtomicMultiplicityColumn::new(1 << LOG_SIZE));

        Self { mults, preprocessed_trace }
    }

    pub fn write_trace(
        self,
    ) -> (ComponentTrace<N_TRACE_COLUMNS>, Claim, InteractionClaimGenerator) {
        let mults = self.mults.into_iter().map(|v| v.into_simd_vec()).collect::<Vec<_>>();

        let (trace, lookup_data) = write_trace_simd(&self.preprocessed_trace, mults);

        (trace, Claim {}, InteractionClaimGenerator { lookup_data })
    }

    pub fn add_packed_inputs(&self, packed_inputs: &[PackedInputType], relation_index: usize) {
        packed_inputs.into_par_iter().for_each(|packed_input| {
            for [idx] in packed_input.unpack() {
                self.mults[relation_index].increase_at(idx.0);
            }
        });
    }

    /// [`Self::add_packed_inputs`] (relation 0) of the values of lookup columns, each tuple's
    /// last value, read in place and in one pass over the columns' rows.
    pub fn add_lookup_columns<const K: usize>(&self, columns: [&[[PackedM31; 2]]; K]) {
        let n_rows = columns[0].len();
        assert!(columns.iter().all(|column| column.len() == n_rows));
        (0..n_rows).into_par_iter().for_each(|row| {
            for column in &columns {
                for [idx] in [column[row][1]].unpack() {
                    self.mults[0].increase_at(idx.0);
                }
            }
        });
    }
}

#[allow(clippy::useless_conversion)]
#[allow(unused_variables)]
#[allow(clippy::double_parens)]
#[allow(non_snake_case)]
fn write_trace_simd(
    preprocessed_trace: &PreProcessedTrace,
    mults: Vec<Vec<PackedM31>>,
) -> (ComponentTrace<N_TRACE_COLUMNS>, LookupData) {
    let log_n_packed_rows = LOG_SIZE - LOG_N_LANES;
    let (mut trace, mut lookup_data) = unsafe {
        (
            ComponentTrace::<N_TRACE_COLUMNS>::uninitialized(LOG_SIZE),
            LookupData::uninitialized(log_n_packed_rows),
        )
    };

    let M31_1008385708 = PackedM31::broadcast(M31::from(1008385708));
    let seq_16 =
        preprocessed_trace.get_packed_column(&PreProcessedColumnId { id: "seq_16".to_owned() });

    (trace.par_iter_mut(), lookup_data.par_iter_mut()).into_par_iter().enumerate().for_each(
        |(row_index, (row, lookup_data))| {
            let seq_16 = seq_16[row_index];
            *lookup_data.range_check_16_0 = [M31_1008385708, seq_16];
            let mult = &mults[0];
            let mult_at_row = *mult.get(row_index).unwrap_or(&PackedM31::zero());
            *row[0] = mult_at_row;
            *lookup_data.mults_0 = mult_at_row;
        },
    );

    (trace, lookup_data)
}

#[derive(Uninitialized, IterMut, ParIterMut)]
struct LookupData {
    range_check_16_0: Vec<[PackedM31; 2]>,
    mults_0: Vec<PackedM31>,
}

pub struct InteractionClaimGenerator {
    lookup_data: LookupData,
}
impl InteractionClaimGenerator {
    pub fn write_interaction_trace(
        self,
        common_lookup_elements: &relations::CommonLookupElements,
    ) -> (Vec<CircleEvaluation<SimdBackend, M31, BitReversedOrder>>, InteractionClaim) {
        let mut logup_gen = unsafe { LogupTraceGenerator::uninitialized(LOG_SIZE) };

        // Sum last logup term.
        // In one pass per chunk of rows (fractions, inverses and the running sum), as the
        // gate components' columns are written (omarespejel, rgr1_3f4875e7).
        let lookup_data = &self.lookup_data;
        logup_gen.write_frac_cols(1, |_, row| {
            let denom = common_lookup_elements.combine(&lookup_data.range_check_16_0[row]);
            (-PackedQM31::one() * lookup_data.mults_0[row], denom)
        });

        let (trace, claimed_sum) = logup_gen.finalize_last();

        (trace, InteractionClaim { claimed_sum })
    }
}
