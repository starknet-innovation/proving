// This file was created by the AIR team.

#![allow(unused_parens)]
use cairo_air::components::range_check_9_9::{Claim, InteractionClaim, LOG_SIZE, N_TRACE_COLUMNS};

use crate::witness::prelude::*;

pub type InputType = [M31; 2];
pub type PackedInputType = [PackedM31; 2];

pub struct ClaimGenerator {
    pub mults: [AtomicMultiplicityColumn; 8],
    input_to_row: InputToRow<2>,
    preprocessed_trace: Arc<PreProcessedTrace>,
}

impl ClaimGenerator {
    pub fn new(preprocessed_trace: Arc<PreProcessedTrace>) -> Self {
        let mults = from_fn(|_| AtomicMultiplicityColumn::new(1 << LOG_SIZE));
        let column_ids = [
            PreProcessedColumnId { id: "range_check_9_9_column_0".to_owned() },
            PreProcessedColumnId { id: "range_check_9_9_column_1".to_owned() },
        ];

        Self {
            mults,
            input_to_row: make_input_to_row(&preprocessed_trace, column_ids),
            preprocessed_trace,
        }
    }

    /// Adds the `N_LANES` inputs of one packed row, as [`AddInputs::add_packed_inputs`] does for
    /// each of its rows (without a parallel iterator for the one row).
    pub fn add_packed_input(&self, packed_input: &PackedInputType, relation_index: usize) {
        for row in self.input_to_row.rows_packed(packed_input) {
            self.mults[relation_index].increase_at(row);
        }
    }

    pub fn write_trace(
        self,
    ) -> (ComponentTrace<N_TRACE_COLUMNS>, Claim, InteractionClaimGenerator) {
        // The trace is the multiplicity columns. The interaction reads a copy of them (made on the
        // workers) and its lookup values from the preprocessed columns, row by row.
        let mults = self.mults.map(|v| v.into_simd_vec());
        let lookup_mults = mults.par_iter().cloned().collect::<Vec<_>>().try_into().unwrap();
        let trace = ComponentTrace::from_columns(mults, LOG_SIZE);

        (
            trace,
            Claim {},
            InteractionClaimGenerator {
                mults: lookup_mults,
                preprocessed_trace: self.preprocessed_trace,
            },
        )
    }
}

impl AddInputs for ClaimGenerator {
    type PackedInputType = PackedInputType;
    type InputType = InputType;

    fn add_packed_inputs(&self, packed_inputs: &[PackedInputType], relation_index: usize) {
        packed_inputs.into_par_iter().for_each(|packed_input| {
            for input in packed_input.unpack() {
                self.add_input(&input, relation_index);
            }
        });
    }
    fn add_input(&self, input: &InputType, relation_index: usize) {
        self.mults[relation_index].increase_at(self.input_to_row.row(input).try_into().unwrap());
    }
}

pub struct InteractionClaimGenerator {
    mults: [Vec<PackedM31>; 8],
    preprocessed_trace: Arc<PreProcessedTrace>,
}
impl InteractionClaimGenerator {
    pub fn write_interaction_trace(
        self,
        common_lookup_elements: &relations::CommonLookupElements,
    ) -> (Vec<CircleEvaluation<SimdBackend, M31, BitReversedOrder>>, InteractionClaim) {
        let mut logup_gen = unsafe { LogupTraceGenerator::uninitialized(LOG_SIZE) };

        // The relation ids of the lookups `range_check_9_9_0, range_check_9_9_b_1, ...,
        // range_check_9_9_h_7`, each of the values `[relation id, column_0, column_1]`.
        let relation_ids: [u32; 8] = [
            517791011, 1897792095, 1881014476, 1864236857, 1847459238, 1830681619, 1813904000,
            2065568285,
        ];
        let relation_ids = relation_ids.map(|id| PackedM31::broadcast(M31::from(id)));
        let column_0 = self
            .preprocessed_trace
            .get_column(&PreProcessedColumnId { id: "range_check_9_9_column_0".to_owned() });
        let column_1 = self
            .preprocessed_trace
            .get_column(&PreProcessedColumnId { id: "range_check_9_9_column_1".to_owned() });
        let mults = &self.mults;

        // Sum logup terms in pairs: column `k` holds the lookups `2k` and `2k + 1`, all the columns
        // written in one pass over the rows.
        logup_gen.write_frac_cols(4, |col, row| {
            let (column_0, column_1) = (column_0.packed_at(row), column_1.packed_at(row));
            let denom0: PackedQM31 =
                common_lookup_elements.combine(&[relation_ids[2 * col], column_0, column_1]);
            let denom1: PackedQM31 =
                common_lookup_elements.combine(&[relation_ids[2 * col + 1], column_0, column_1]);
            (-(denom0 * mults[2 * col + 1][row] + denom1 * mults[2 * col][row]), denom0 * denom1)
        });

        let (trace, claimed_sum) = logup_gen.finalize_last();

        (trace, InteractionClaim { claimed_sum })
    }
}
