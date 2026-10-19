// This file was created by the AIR team.

#![allow(unused_parens)]
use cairo_air::components::keccak_builtin::{Claim, InteractionClaim, N_TRACE_COLUMNS};

use crate::witness::components::{keccak_aggregator, memory_address_to_id};
use crate::witness::prelude::*;

#[derive(Default)]
pub struct ClaimGenerator {
    pub log_size: u32,
    pub keccak_builtin_segment_start: u32,
}

impl ClaimGenerator {
    pub fn new(log_size: u32, keccak_builtin_segment_start: u32) -> Self {
        assert!(log_size >= LOG_N_LANES);
        Self { log_size, keccak_builtin_segment_start }
    }

    pub fn write_trace(
        self,
        memory_address_to_id_state: &memory_address_to_id::ClaimGenerator,
        keccak_aggregator_state: &keccak_aggregator::ClaimGenerator,
    ) -> (ComponentTrace<N_TRACE_COLUMNS>, Claim, InteractionClaimGenerator) {
        let log_size = self.log_size;
        let size = 1 << log_size;

        let (trace, lookup_data, sub_component_inputs) = write_trace_simd(
            log_size,
            self.keccak_builtin_segment_start,
            memory_address_to_id_state,
            keccak_aggregator_state,
        );
        for inputs in sub_component_inputs.memory_address_to_id {
            add_inputs(memory_address_to_id_state, &inputs, size, 0);
        }
        for inputs in sub_component_inputs.keccak_aggregator {
            add_inputs(keccak_aggregator_state, &inputs, size, 0);
        }

        (trace, Claim { log_size }, InteractionClaimGenerator { log_size, lookup_data })
    }
}

#[derive(Uninitialized, IterMut, ParIterMut)]
struct SubComponentInputs {
    memory_address_to_id: [Vec<memory_address_to_id::PackedInputType>; 16],
    keccak_aggregator: [Vec<keccak_aggregator::PackedInputType>; 1],
}

#[allow(clippy::useless_conversion)]
#[allow(unused_variables)]
#[allow(clippy::double_parens)]
#[allow(non_snake_case)]
fn write_trace_simd(
    log_size: u32,
    keccak_builtin_segment_start: u32,
    memory_address_to_id_state: &memory_address_to_id::ClaimGenerator,
    keccak_aggregator_state: &keccak_aggregator::ClaimGenerator,
) -> (ComponentTrace<N_TRACE_COLUMNS>, LookupData, SubComponentInputs) {
    let log_n_packed_rows = log_size - LOG_N_LANES;
    let (mut trace, mut lookup_data, mut sub_component_inputs) = unsafe {
        (
            ComponentTrace::<N_TRACE_COLUMNS>::uninitialized(log_size),
            LookupData::uninitialized(log_n_packed_rows),
            SubComponentInputs::uninitialized(log_n_packed_rows),
        )
    };

    let M31_1 = PackedM31::broadcast(M31::from(1));
    let M31_10 = PackedM31::broadcast(M31::from(10));
    let M31_11 = PackedM31::broadcast(M31::from(11));
    let M31_12 = PackedM31::broadcast(M31::from(12));
    let M31_13 = PackedM31::broadcast(M31::from(13));
    let M31_14 = PackedM31::broadcast(M31::from(14));
    let M31_1444891767 = PackedM31::broadcast(M31::from(1444891767));
    let M31_15 = PackedM31::broadcast(M31::from(15));
    let M31_16 = PackedM31::broadcast(M31::from(16));
    let M31_1973115366 = PackedM31::broadcast(M31::from(1973115366));
    let M31_2 = PackedM31::broadcast(M31::from(2));
    let M31_3 = PackedM31::broadcast(M31::from(3));
    let M31_4 = PackedM31::broadcast(M31::from(4));
    let M31_5 = PackedM31::broadcast(M31::from(5));
    let M31_6 = PackedM31::broadcast(M31::from(6));
    let M31_7 = PackedM31::broadcast(M31::from(7));
    let M31_8 = PackedM31::broadcast(M31::from(8));
    let M31_9 = PackedM31::broadcast(M31::from(9));
    let seq = Seq::new(log_size);

    (trace.par_iter_mut(), lookup_data.par_iter_mut(), sub_component_inputs.par_iter_mut())
        .into_par_iter()
        .enumerate()
        .for_each(|(row_index, (row, lookup_data, sub_component_inputs))| {
            let seq = seq.packed_at(row_index);
            let instance_addr_tmp_24554_0 = (((seq) * (M31_16))
                + (PackedM31::broadcast(M31::from(keccak_builtin_segment_start))));

            // Read Id.

            let memory_address_to_id_value_tmp_24554_1 =
                memory_address_to_id_state.deduce_output(instance_addr_tmp_24554_0);
            let input_0_id_col0 = memory_address_to_id_value_tmp_24554_1;
            *row[0] = input_0_id_col0;
            *sub_component_inputs.memory_address_to_id[0] = instance_addr_tmp_24554_0;
            *lookup_data.memory_address_to_id_0 =
                [M31_1444891767, instance_addr_tmp_24554_0, input_0_id_col0];

            // Read Id.

            let memory_address_to_id_value_tmp_24554_3 =
                memory_address_to_id_state.deduce_output(((instance_addr_tmp_24554_0) + (M31_1)));
            let input_1_id_col1 = memory_address_to_id_value_tmp_24554_3;
            *row[1] = input_1_id_col1;
            *sub_component_inputs.memory_address_to_id[1] = ((instance_addr_tmp_24554_0) + (M31_1));
            *lookup_data.memory_address_to_id_1 =
                [M31_1444891767, ((instance_addr_tmp_24554_0) + (M31_1)), input_1_id_col1];

            // Read Id.

            let memory_address_to_id_value_tmp_24554_5 =
                memory_address_to_id_state.deduce_output(((instance_addr_tmp_24554_0) + (M31_2)));
            let input_2_id_col2 = memory_address_to_id_value_tmp_24554_5;
            *row[2] = input_2_id_col2;
            *sub_component_inputs.memory_address_to_id[2] = ((instance_addr_tmp_24554_0) + (M31_2));
            *lookup_data.memory_address_to_id_2 =
                [M31_1444891767, ((instance_addr_tmp_24554_0) + (M31_2)), input_2_id_col2];

            // Read Id.

            let memory_address_to_id_value_tmp_24554_7 =
                memory_address_to_id_state.deduce_output(((instance_addr_tmp_24554_0) + (M31_3)));
            let input_3_id_col3 = memory_address_to_id_value_tmp_24554_7;
            *row[3] = input_3_id_col3;
            *sub_component_inputs.memory_address_to_id[3] = ((instance_addr_tmp_24554_0) + (M31_3));
            *lookup_data.memory_address_to_id_3 =
                [M31_1444891767, ((instance_addr_tmp_24554_0) + (M31_3)), input_3_id_col3];

            // Read Id.

            let memory_address_to_id_value_tmp_24554_9 =
                memory_address_to_id_state.deduce_output(((instance_addr_tmp_24554_0) + (M31_4)));
            let input_4_id_col4 = memory_address_to_id_value_tmp_24554_9;
            *row[4] = input_4_id_col4;
            *sub_component_inputs.memory_address_to_id[4] = ((instance_addr_tmp_24554_0) + (M31_4));
            *lookup_data.memory_address_to_id_4 =
                [M31_1444891767, ((instance_addr_tmp_24554_0) + (M31_4)), input_4_id_col4];

            // Read Id.

            let memory_address_to_id_value_tmp_24554_11 =
                memory_address_to_id_state.deduce_output(((instance_addr_tmp_24554_0) + (M31_5)));
            let input_5_id_col5 = memory_address_to_id_value_tmp_24554_11;
            *row[5] = input_5_id_col5;
            *sub_component_inputs.memory_address_to_id[5] = ((instance_addr_tmp_24554_0) + (M31_5));
            *lookup_data.memory_address_to_id_5 =
                [M31_1444891767, ((instance_addr_tmp_24554_0) + (M31_5)), input_5_id_col5];

            // Read Id.

            let memory_address_to_id_value_tmp_24554_13 =
                memory_address_to_id_state.deduce_output(((instance_addr_tmp_24554_0) + (M31_6)));
            let input_6_id_col6 = memory_address_to_id_value_tmp_24554_13;
            *row[6] = input_6_id_col6;
            *sub_component_inputs.memory_address_to_id[6] = ((instance_addr_tmp_24554_0) + (M31_6));
            *lookup_data.memory_address_to_id_6 =
                [M31_1444891767, ((instance_addr_tmp_24554_0) + (M31_6)), input_6_id_col6];

            // Read Id.

            let memory_address_to_id_value_tmp_24554_15 =
                memory_address_to_id_state.deduce_output(((instance_addr_tmp_24554_0) + (M31_7)));
            let input_7_id_col7 = memory_address_to_id_value_tmp_24554_15;
            *row[7] = input_7_id_col7;
            *sub_component_inputs.memory_address_to_id[7] = ((instance_addr_tmp_24554_0) + (M31_7));
            *lookup_data.memory_address_to_id_7 =
                [M31_1444891767, ((instance_addr_tmp_24554_0) + (M31_7)), input_7_id_col7];

            // Read Id.

            let memory_address_to_id_value_tmp_24554_17 =
                memory_address_to_id_state.deduce_output(((instance_addr_tmp_24554_0) + (M31_8)));
            let output_0_id_col8 = memory_address_to_id_value_tmp_24554_17;
            *row[8] = output_0_id_col8;
            *sub_component_inputs.memory_address_to_id[8] = ((instance_addr_tmp_24554_0) + (M31_8));
            *lookup_data.memory_address_to_id_8 =
                [M31_1444891767, ((instance_addr_tmp_24554_0) + (M31_8)), output_0_id_col8];

            // Read Id.

            let memory_address_to_id_value_tmp_24554_19 =
                memory_address_to_id_state.deduce_output(((instance_addr_tmp_24554_0) + (M31_9)));
            let output_1_id_col9 = memory_address_to_id_value_tmp_24554_19;
            *row[9] = output_1_id_col9;
            *sub_component_inputs.memory_address_to_id[9] = ((instance_addr_tmp_24554_0) + (M31_9));
            *lookup_data.memory_address_to_id_9 =
                [M31_1444891767, ((instance_addr_tmp_24554_0) + (M31_9)), output_1_id_col9];

            // Read Id.

            let memory_address_to_id_value_tmp_24554_21 =
                memory_address_to_id_state.deduce_output(((instance_addr_tmp_24554_0) + (M31_10)));
            let output_2_id_col10 = memory_address_to_id_value_tmp_24554_21;
            *row[10] = output_2_id_col10;
            *sub_component_inputs.memory_address_to_id[10] =
                ((instance_addr_tmp_24554_0) + (M31_10));
            *lookup_data.memory_address_to_id_10 =
                [M31_1444891767, ((instance_addr_tmp_24554_0) + (M31_10)), output_2_id_col10];

            // Read Id.

            let memory_address_to_id_value_tmp_24554_23 =
                memory_address_to_id_state.deduce_output(((instance_addr_tmp_24554_0) + (M31_11)));
            let output_3_id_col11 = memory_address_to_id_value_tmp_24554_23;
            *row[11] = output_3_id_col11;
            *sub_component_inputs.memory_address_to_id[11] =
                ((instance_addr_tmp_24554_0) + (M31_11));
            *lookup_data.memory_address_to_id_11 =
                [M31_1444891767, ((instance_addr_tmp_24554_0) + (M31_11)), output_3_id_col11];

            // Read Id.

            let memory_address_to_id_value_tmp_24554_25 =
                memory_address_to_id_state.deduce_output(((instance_addr_tmp_24554_0) + (M31_12)));
            let output_4_id_col12 = memory_address_to_id_value_tmp_24554_25;
            *row[12] = output_4_id_col12;
            *sub_component_inputs.memory_address_to_id[12] =
                ((instance_addr_tmp_24554_0) + (M31_12));
            *lookup_data.memory_address_to_id_12 =
                [M31_1444891767, ((instance_addr_tmp_24554_0) + (M31_12)), output_4_id_col12];

            // Read Id.

            let memory_address_to_id_value_tmp_24554_27 =
                memory_address_to_id_state.deduce_output(((instance_addr_tmp_24554_0) + (M31_13)));
            let output_5_id_col13 = memory_address_to_id_value_tmp_24554_27;
            *row[13] = output_5_id_col13;
            *sub_component_inputs.memory_address_to_id[13] =
                ((instance_addr_tmp_24554_0) + (M31_13));
            *lookup_data.memory_address_to_id_13 =
                [M31_1444891767, ((instance_addr_tmp_24554_0) + (M31_13)), output_5_id_col13];

            // Read Id.

            let memory_address_to_id_value_tmp_24554_29 =
                memory_address_to_id_state.deduce_output(((instance_addr_tmp_24554_0) + (M31_14)));
            let output_6_id_col14 = memory_address_to_id_value_tmp_24554_29;
            *row[14] = output_6_id_col14;
            *sub_component_inputs.memory_address_to_id[14] =
                ((instance_addr_tmp_24554_0) + (M31_14));
            *lookup_data.memory_address_to_id_14 =
                [M31_1444891767, ((instance_addr_tmp_24554_0) + (M31_14)), output_6_id_col14];

            // Read Id.

            let memory_address_to_id_value_tmp_24554_31 =
                memory_address_to_id_state.deduce_output(((instance_addr_tmp_24554_0) + (M31_15)));
            let output_7_id_col15 = memory_address_to_id_value_tmp_24554_31;
            *row[15] = output_7_id_col15;
            *sub_component_inputs.memory_address_to_id[15] =
                ((instance_addr_tmp_24554_0) + (M31_15));
            *lookup_data.memory_address_to_id_15 =
                [M31_1444891767, ((instance_addr_tmp_24554_0) + (M31_15)), output_7_id_col15];

            *sub_component_inputs.keccak_aggregator[0] = (
                [
                    input_0_id_col0,
                    input_1_id_col1,
                    input_2_id_col2,
                    input_3_id_col3,
                    input_4_id_col4,
                    input_5_id_col5,
                    input_6_id_col6,
                    input_7_id_col7,
                ],
                [
                    output_0_id_col8,
                    output_1_id_col9,
                    output_2_id_col10,
                    output_3_id_col11,
                    output_4_id_col12,
                    output_5_id_col13,
                    output_6_id_col14,
                    output_7_id_col15,
                ],
            );
            *lookup_data.keccak_aggregator_16 = [
                M31_1973115366,
                input_0_id_col0,
                input_1_id_col1,
                input_2_id_col2,
                input_3_id_col3,
                input_4_id_col4,
                input_5_id_col5,
                input_6_id_col6,
                input_7_id_col7,
                output_0_id_col8,
                output_1_id_col9,
                output_2_id_col10,
                output_3_id_col11,
                output_4_id_col12,
                output_5_id_col13,
                output_6_id_col14,
                output_7_id_col15,
            ];
            *lookup_data.mults_0 = M31_1;
        });

    (trace, lookup_data, sub_component_inputs)
}

#[derive(Uninitialized, IterMut, ParIterMut)]
struct LookupData {
    memory_address_to_id_0: Vec<[PackedM31; 3]>,
    memory_address_to_id_1: Vec<[PackedM31; 3]>,
    memory_address_to_id_2: Vec<[PackedM31; 3]>,
    memory_address_to_id_3: Vec<[PackedM31; 3]>,
    memory_address_to_id_4: Vec<[PackedM31; 3]>,
    memory_address_to_id_5: Vec<[PackedM31; 3]>,
    memory_address_to_id_6: Vec<[PackedM31; 3]>,
    memory_address_to_id_7: Vec<[PackedM31; 3]>,
    memory_address_to_id_8: Vec<[PackedM31; 3]>,
    memory_address_to_id_9: Vec<[PackedM31; 3]>,
    memory_address_to_id_10: Vec<[PackedM31; 3]>,
    memory_address_to_id_11: Vec<[PackedM31; 3]>,
    memory_address_to_id_12: Vec<[PackedM31; 3]>,
    memory_address_to_id_13: Vec<[PackedM31; 3]>,
    memory_address_to_id_14: Vec<[PackedM31; 3]>,
    memory_address_to_id_15: Vec<[PackedM31; 3]>,
    keccak_aggregator_16: Vec<[PackedM31; 17]>,
    mults_0: Vec<PackedM31>,
}

pub struct InteractionClaimGenerator {
    log_size: u32,
    lookup_data: LookupData,
}
impl InteractionClaimGenerator {
    pub fn write_interaction_trace(
        self,
        common_lookup_elements: &relations::CommonLookupElements,
    ) -> (Vec<CircleEvaluation<SimdBackend, M31, BitReversedOrder>>, InteractionClaim) {
        let mut logup_gen = unsafe { LogupTraceGenerator::uninitialized(self.log_size) };

        // Sum logup terms in pairs.
        let mut col_gen = logup_gen.new_col();
        (
            col_gen.par_iter_mut(),
            &self.lookup_data.memory_address_to_id_0,
            &self.lookup_data.memory_address_to_id_1,
            &self.lookup_data.mults_0,
            &self.lookup_data.mults_0,
        )
            .into_par_iter()
            .for_each(|(writer, values0, values1, mult0, mult1)| {
                let denom0: PackedQM31 = common_lookup_elements.combine(values0);
                let denom1: PackedQM31 = common_lookup_elements.combine(values1);
                writer.write_frac(denom0 * *mult1 + denom1 * *mult0, denom0 * denom1);
            });
        col_gen.finalize_col();

        let mut col_gen = logup_gen.new_col();
        (
            col_gen.par_iter_mut(),
            &self.lookup_data.memory_address_to_id_2,
            &self.lookup_data.memory_address_to_id_3,
            &self.lookup_data.mults_0,
            &self.lookup_data.mults_0,
        )
            .into_par_iter()
            .for_each(|(writer, values0, values1, mult0, mult1)| {
                let denom0: PackedQM31 = common_lookup_elements.combine(values0);
                let denom1: PackedQM31 = common_lookup_elements.combine(values1);
                writer.write_frac(denom0 * *mult1 + denom1 * *mult0, denom0 * denom1);
            });
        col_gen.finalize_col();

        let mut col_gen = logup_gen.new_col();
        (
            col_gen.par_iter_mut(),
            &self.lookup_data.memory_address_to_id_4,
            &self.lookup_data.memory_address_to_id_5,
            &self.lookup_data.mults_0,
            &self.lookup_data.mults_0,
        )
            .into_par_iter()
            .for_each(|(writer, values0, values1, mult0, mult1)| {
                let denom0: PackedQM31 = common_lookup_elements.combine(values0);
                let denom1: PackedQM31 = common_lookup_elements.combine(values1);
                writer.write_frac(denom0 * *mult1 + denom1 * *mult0, denom0 * denom1);
            });
        col_gen.finalize_col();

        let mut col_gen = logup_gen.new_col();
        (
            col_gen.par_iter_mut(),
            &self.lookup_data.memory_address_to_id_6,
            &self.lookup_data.memory_address_to_id_7,
            &self.lookup_data.mults_0,
            &self.lookup_data.mults_0,
        )
            .into_par_iter()
            .for_each(|(writer, values0, values1, mult0, mult1)| {
                let denom0: PackedQM31 = common_lookup_elements.combine(values0);
                let denom1: PackedQM31 = common_lookup_elements.combine(values1);
                writer.write_frac(denom0 * *mult1 + denom1 * *mult0, denom0 * denom1);
            });
        col_gen.finalize_col();

        let mut col_gen = logup_gen.new_col();
        (
            col_gen.par_iter_mut(),
            &self.lookup_data.memory_address_to_id_8,
            &self.lookup_data.memory_address_to_id_9,
            &self.lookup_data.mults_0,
            &self.lookup_data.mults_0,
        )
            .into_par_iter()
            .for_each(|(writer, values0, values1, mult0, mult1)| {
                let denom0: PackedQM31 = common_lookup_elements.combine(values0);
                let denom1: PackedQM31 = common_lookup_elements.combine(values1);
                writer.write_frac(denom0 * *mult1 + denom1 * *mult0, denom0 * denom1);
            });
        col_gen.finalize_col();

        let mut col_gen = logup_gen.new_col();
        (
            col_gen.par_iter_mut(),
            &self.lookup_data.memory_address_to_id_10,
            &self.lookup_data.memory_address_to_id_11,
            &self.lookup_data.mults_0,
            &self.lookup_data.mults_0,
        )
            .into_par_iter()
            .for_each(|(writer, values0, values1, mult0, mult1)| {
                let denom0: PackedQM31 = common_lookup_elements.combine(values0);
                let denom1: PackedQM31 = common_lookup_elements.combine(values1);
                writer.write_frac(denom0 * *mult1 + denom1 * *mult0, denom0 * denom1);
            });
        col_gen.finalize_col();

        let mut col_gen = logup_gen.new_col();
        (
            col_gen.par_iter_mut(),
            &self.lookup_data.memory_address_to_id_12,
            &self.lookup_data.memory_address_to_id_13,
            &self.lookup_data.mults_0,
            &self.lookup_data.mults_0,
        )
            .into_par_iter()
            .for_each(|(writer, values0, values1, mult0, mult1)| {
                let denom0: PackedQM31 = common_lookup_elements.combine(values0);
                let denom1: PackedQM31 = common_lookup_elements.combine(values1);
                writer.write_frac(denom0 * *mult1 + denom1 * *mult0, denom0 * denom1);
            });
        col_gen.finalize_col();

        let mut col_gen = logup_gen.new_col();
        (
            col_gen.par_iter_mut(),
            &self.lookup_data.memory_address_to_id_14,
            &self.lookup_data.memory_address_to_id_15,
            &self.lookup_data.mults_0,
            &self.lookup_data.mults_0,
        )
            .into_par_iter()
            .for_each(|(writer, values0, values1, mult0, mult1)| {
                let denom0: PackedQM31 = common_lookup_elements.combine(values0);
                let denom1: PackedQM31 = common_lookup_elements.combine(values1);
                writer.write_frac(denom0 * *mult1 + denom1 * *mult0, denom0 * denom1);
            });
        col_gen.finalize_col();

        // Sum last logup term.
        let mut col_gen = logup_gen.new_col();
        (col_gen.par_iter_mut(), &self.lookup_data.keccak_aggregator_16, self.lookup_data.mults_0)
            .into_par_iter()
            .for_each(|(writer, values, mult)| {
                let denom = common_lookup_elements.combine(values);
                writer.write_frac((mult).into(), denom);
            });
        col_gen.finalize_col();

        let (trace, claimed_sum) = logup_gen.finalize_last();

        (trace, InteractionClaim { claimed_sum })
    }
}
