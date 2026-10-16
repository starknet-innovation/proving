// This file was created by the AIR team.

#![allow(unused_parens)]
use cairo_air::components::range_check_20::{Claim, InteractionClaim, LOG_SIZE, N_TRACE_COLUMNS};

use crate::witness::prelude::*;

pub type InputType = [M31; 1];
pub type PackedInputType = [PackedM31; 1];

pub struct ClaimGenerator {
    pub mults: [AtomicMultiplicityColumn; 8],
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
        // The trace is the multiplicity columns. The interaction reads a copy of them (made on the
        // workers) and its lookup values from the preprocessed column, row by row.
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
            for [idx] in packed_input.unpack() {
                self.mults[relation_index].increase_at(idx.0);
            }
        });
    }
    fn add_input(&self, input: &InputType, relation_index: usize) {
        self.mults[relation_index].increase_at(input[0].0);
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

        // The relation ids of the lookups `range_check_20_0, range_check_20_b_1, ...,
        // range_check_20_h_7`, each of the values `[relation id, seq_20]`.
        let relation_ids: [u32; 8] = [
            1410849886, 514232941, 531010560, 480677703, 497455322, 447122465, 463900084,
            682009131,
        ];
        let relation_ids = relation_ids.map(|id| PackedM31::broadcast(M31::from(id)));
        let seq_20 =
            self.preprocessed_trace.get_column(&PreProcessedColumnId { id: "seq_20".to_owned() });
        let mults = &self.mults;

        // Sum logup terms in pairs: column `k` holds the lookups `2k` and `2k + 1`, all the columns
        // written in one pass over the rows.
        logup_gen.write_frac_cols(4, |col, row| {
            let seq_20 = seq_20.packed_at(row);
            let denom0: PackedQM31 =
                common_lookup_elements.combine(&[relation_ids[2 * col], seq_20]);
            let denom1: PackedQM31 =
                common_lookup_elements.combine(&[relation_ids[2 * col + 1], seq_20]);
            (-(denom0 * mults[2 * col + 1][row] + denom1 * mults[2 * col][row]), denom0 * denom1)
        });

        let (trace, claimed_sum) = logup_gen.finalize_last();

        (trace, InteractionClaim { claimed_sum })
    }
}
