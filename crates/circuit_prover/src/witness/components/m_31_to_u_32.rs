#![allow(unused_parens)]
use stwo::core::fields::FieldExpOps;
use stwo::core::fields::qm31::QM31;
use stwo_cairo_common::prover_types::simd::EqExtend;

use crate::circuit_air::components::m_31_to_u_32::{Claim, InteractionClaim, N_TRACE_COLUMNS};
use crate::witness::components::prelude::*;
use crate::witness::components::range_check_16;

pub type InputType = (M31, UInt32);
pub type PackedInputType = (PackedM31, PackedUInt32);

/// Retrieves the component's inputs from the context values, using the addresses provided in the
/// preprocessed trace.
#[allow(clippy::uninit_vec)]
pub fn extract_component_inputs(
    input_addr_col: &[usize],
    context_values: &[QM31],
) -> Vec<InputType> {
    let n_rows = input_addr_col.len();
    let mut inputs = Vec::with_capacity(n_rows);
    unsafe {
        inputs.set_len(n_rows);
    }

    (inputs.par_iter_mut(), input_addr_col).into_par_iter().for_each(|(input, &input_addr)| {
        let m31_input = context_values[input_addr].to_m31_array()[0];
        *input = (m31_input, UInt32::from(m31_input.0));
    });

    inputs
}

pub fn write_trace(
    context_values: &[QM31],
    preprocessed_trace: &PreProcessedTrace,
    range_check_16_state: &range_check_16::ClaimGenerator,
) -> (ComponentTrace<N_TRACE_COLUMNS>, Claim, InteractionClaimGenerator) {
    // The inputs are gathered per packed row straight from the context values at the
    // preprocessed addresses, read in place: no staged inputs, no packed copy of them and no
    // materialized address columns.
    let columns = ["m31_to_u32_input_addr", "m31_to_u32_output_addr", "m31_to_u32_multiplicity"]
        .map(|id| preprocessed_trace.column_rows(&PreProcessedColumnId { id: id.to_owned() }));

    let n_rows = columns[0].len();
    assert!(columns.iter().all(|column| column.len() == n_rows));
    assert_ne!(n_rows, 0);
    assert!(n_rows.is_multiple_of(N_LANES));
    let size = std::cmp::max(n_rows.next_power_of_two(), N_LANES);
    let log_size = size.ilog2();

    // A row's input, as `extract_component_inputs` stages it.
    let input = |row: usize| -> InputType {
        let m31_input = context_values[columns[0].at(row)].to_m31_array()[0];
        (m31_input, UInt32::from(m31_input.0))
    };

    let (trace, lookup_data, sub_component_inputs) = write_trace_simd(n_rows, input, &columns);
    for inputs in sub_component_inputs.range_check_16 {
        range_check_16_state.add_packed_inputs(&inputs, 0);
    }

    (trace, Claim { log_size }, InteractionClaimGenerator { log_size, lookup_data })
}

#[derive(Uninitialized, IterMut, ParIterMut)]
struct SubComponentInputs {
    range_check_16: [Vec<range_check_16::PackedInputType>; 3],
}

#[allow(clippy::useless_conversion)]
#[allow(unused_variables)]
#[allow(clippy::double_parens)]
#[allow(non_snake_case)]
fn write_trace_simd(
    n_rows: usize,
    input: impl Fn(usize) -> InputType + Sync,
    columns: &[circuit_common::preprocessed::ColumnRows<'_>; 3],
) -> (ComponentTrace<N_TRACE_COLUMNS>, LookupData, SubComponentInputs) {
    let log_n_packed_rows = (n_rows / N_LANES).ilog2();
    let log_size = log_n_packed_rows + LOG_N_LANES;
    let (mut trace, mut lookup_data, mut sub_component_inputs) = unsafe {
        (
            ComponentTrace::<N_TRACE_COLUMNS>::uninitialized(log_size),
            LookupData::uninitialized(log_n_packed_rows),
            SubComponentInputs::uninitialized(log_n_packed_rows),
        )
    };

    let M31_0 = PackedM31::broadcast(M31::from(0));
    let M31_1 = PackedM31::broadcast(M31::from(1));
    let M31_1008385708 = PackedM31::broadcast(M31::from(1008385708));
    let M31_32767 = PackedM31::broadcast(M31::from(32767));
    let M31_378353459 = PackedM31::broadcast(M31::from(378353459));

    (
        trace.par_iter_mut(),
        lookup_data.par_iter_mut(),
        sub_component_inputs.par_iter_mut(),
    )
        .into_par_iter()
        .enumerate()
        .for_each(
            |(row_index, (row, lookup_data, sub_component_inputs))| {
                let first = row_index * N_LANES;
                let m_31_to_u_32_input: PackedInputType =
                    crate::witness::utils::packed_input(&input, first);
                let [m31_to_u32_input_addr, m31_to_u32_output_addr, m31_to_u32_multiplicity] =
                    crate::witness::utils::packed_column_rows(columns, first);
                let input_m31_col0 = m_31_to_u_32_input.0;
                *row[0] = input_m31_col0;
                let input_u32_limb_0_col1 = m_31_to_u_32_input.1.low().as_m31();
                *row[1] = input_u32_limb_0_col1;
                let input_u32_limb_1_col2 = m_31_to_u_32_input.1.high().as_m31();
                *row[2] = input_u32_limb_1_col2;
                *sub_component_inputs.range_check_16[0] = [input_u32_limb_0_col1];
                *lookup_data.range_check_16_0 = [M31_1008385708, input_u32_limb_0_col1];
                *sub_component_inputs.range_check_16[1] = [input_u32_limb_1_col2];
                *lookup_data.range_check_16_1 = [M31_1008385708, input_u32_limb_1_col2];
                *sub_component_inputs.range_check_16[2] = [((M31_32767) - (input_u32_limb_1_col2))];
                *lookup_data.range_check_16_2 =
                    [M31_1008385708, ((M31_32767) - (input_u32_limb_1_col2))];
                let input_is_zero_tmp_c1f7a_0 = M31_0.eq(input_m31_col0);
                let inv_or_one_col3 =
                    ((input_is_zero_tmp_c1f7a_0.as_m31()) + (input_m31_col0)).inverse();
                *row[3] = inv_or_one_col3;
                *lookup_data.gate_0 = [M31_378353459, m31_to_u32_input_addr, input_m31_col0];
                *lookup_data.gate_1 = [
                    M31_378353459,
                    m31_to_u32_output_addr,
                    input_u32_limb_0_col1,
                    input_u32_limb_1_col2,
                ];
                *lookup_data.mults_0 = m31_to_u32_multiplicity;
            },
        );

    (trace, lookup_data, sub_component_inputs)
}

#[derive(Uninitialized, IterMut, ParIterMut)]
struct LookupData {
    gate_0: Vec<[PackedM31; 3]>,
    gate_1: Vec<[PackedM31; 4]>,
    range_check_16_0: Vec<[PackedM31; 2]>,
    range_check_16_1: Vec<[PackedM31; 2]>,
    range_check_16_2: Vec<[PackedM31; 2]>,
    mults_0: Vec<PackedM31>,
}

pub struct InteractionClaimGenerator {
    log_size: u32,
    lookup_data: LookupData,
}
impl InteractionClaimGenerator {
    pub fn write_interaction_trace(
        self,
        _common_lookup_elements: &relations::CommonLookupElements,
    ) -> (Vec<CircleEvaluation<SimdBackend, M31, BitReversedOrder>>, InteractionClaim) {
        if self.log_size == 0 {
            return (vec![], InteractionClaim { claimed_sum: QM31::zero() });
        }
        let common_lookup_elements = _common_lookup_elements;
        let mut logup_gen = unsafe { LogupTraceGenerator::uninitialized(self.log_size) };

        // Sum logup terms in pairs.
        logup_gen.write_frac_cols(1, |_, row| {
            let values0 = &self.lookup_data.range_check_16_0[row];
            let values1 = &self.lookup_data.range_check_16_1[row];
            let denom0: PackedQM31 = common_lookup_elements.combine(values0);
            let denom1: PackedQM31 = common_lookup_elements.combine(values1);
            (denom0 + denom1, denom0 * denom1)
        });

        logup_gen.write_frac_cols(1, |_, row| {
            let values0 = &self.lookup_data.range_check_16_2[row];
            let gate_values = &self.lookup_data.gate_0[row];
            let denom0: PackedQM31 = common_lookup_elements.combine(values0);
            let denom1: PackedQM31 = common_lookup_elements.combine(gate_values);
            (denom0 + denom1, denom0 * denom1)
        });

        // Sum last logup term.
        logup_gen.write_frac_cols(1, |_, row| {
            let values = &self.lookup_data.gate_1[row];
            let mults_0 = self.lookup_data.mults_0[row];
            let denom = common_lookup_elements.combine(values);
            (-PackedQM31::one() * mults_0, denom)
        });

        let (trace, claimed_sum) = logup_gen.finalize_last();

        (trace, InteractionClaim { claimed_sum })
    }
}
