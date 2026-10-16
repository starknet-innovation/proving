#![allow(unused_parens)]
#![allow(clippy::too_many_arguments)]
use crate::circuit_air::components::blake_g_gate::{Claim, InteractionClaim, N_TRACE_COLUMNS};
use crate::witness::components::prelude::*;
use crate::witness::components::{
    verify_bitwise_xor_4, verify_bitwise_xor_7, verify_bitwise_xor_8, verify_bitwise_xor_9,
    verify_bitwise_xor_12,
};

pub type InputType = [UInt32; 10];
pub type PackedInputType = [PackedUInt32; 10];

/// Retrieves the component's inputs from the context values, using the addresses provided in the
/// preprocessed trace.
#[allow(clippy::uninit_vec)]
#[allow(clippy::too_many_arguments)]
pub fn extract_component_inputs(
    input_addr_a: &[usize],
    input_addr_b: &[usize],
    input_addr_c: &[usize],
    input_addr_d: &[usize],
    input_addr_f0: &[usize],
    input_addr_f1: &[usize],
    output_addr_a: &[usize],
    output_addr_b: &[usize],
    output_addr_c: &[usize],
    output_addr_d: &[usize],
    context_values: &[QM31],
) -> Vec<InputType> {
    let n_rows = input_addr_a.len();
    assert_eq!(n_rows, input_addr_b.len());
    assert_eq!(n_rows, input_addr_c.len());
    assert_eq!(n_rows, input_addr_d.len());
    assert_eq!(n_rows, input_addr_f0.len());
    assert_eq!(n_rows, input_addr_f1.len());
    assert_eq!(n_rows, output_addr_a.len());
    assert_eq!(n_rows, output_addr_b.len());
    assert_eq!(n_rows, output_addr_c.len());
    assert_eq!(n_rows, output_addr_d.len());

    let mut inputs = Vec::with_capacity(n_rows);
    unsafe {
        inputs.set_len(n_rows);
    }

    (
        inputs.par_iter_mut(),
        input_addr_a.par_iter(),
        input_addr_b.par_iter(),
        input_addr_c.par_iter(),
        input_addr_d.par_iter(),
        input_addr_f0.par_iter(),
        input_addr_f1.par_iter(),
        output_addr_a.par_iter(),
        output_addr_b.par_iter(),
        output_addr_c.par_iter(),
        output_addr_d.par_iter(),
    )
        .into_par_iter()
        .for_each(
            |(
                input,
                &addr_a,
                &addr_b,
                &addr_c,
                &addr_d,
                &addr_f0,
                &addr_f1,
                &out_addr_a,
                &out_addr_b,
                &out_addr_c,
                &out_addr_d,
            )| {
                let qm31_in_addr_a = context_values[addr_a].to_m31_array();
                let qm31_in_addr_b = context_values[addr_b].to_m31_array();
                let qm31_in_addr_c = context_values[addr_c].to_m31_array();
                let qm31_in_addr_d = context_values[addr_d].to_m31_array();
                let qm31_in_addr_f0 = context_values[addr_f0].to_m31_array();
                let qm31_in_addr_f1 = context_values[addr_f1].to_m31_array();
                let qm31_out_addr_a = context_values[out_addr_a].to_m31_array();
                let qm31_out_addr_b = context_values[out_addr_b].to_m31_array();
                let qm31_out_addr_c = context_values[out_addr_c].to_m31_array();
                let qm31_out_addr_d = context_values[out_addr_d].to_m31_array();
                *input = [
                    UInt32::from(qm31_in_addr_a[0].0 | (qm31_in_addr_a[1].0 << 16)),
                    UInt32::from(qm31_in_addr_b[0].0 | (qm31_in_addr_b[1].0 << 16)),
                    UInt32::from(qm31_in_addr_c[0].0 | (qm31_in_addr_c[1].0 << 16)),
                    UInt32::from(qm31_in_addr_d[0].0 | (qm31_in_addr_d[1].0 << 16)),
                    UInt32::from(qm31_in_addr_f0[0].0 | (qm31_in_addr_f0[1].0 << 16)),
                    UInt32::from(qm31_in_addr_f1[0].0 | (qm31_in_addr_f1[1].0 << 16)),
                    UInt32::from(qm31_out_addr_a[0].0 | (qm31_out_addr_a[1].0 << 16)),
                    UInt32::from(qm31_out_addr_b[0].0 | (qm31_out_addr_b[1].0 << 16)),
                    UInt32::from(qm31_out_addr_c[0].0 | (qm31_out_addr_c[1].0 << 16)),
                    UInt32::from(qm31_out_addr_d[0].0 | (qm31_out_addr_d[1].0 << 16)),
                ];
            },
        );

    inputs
}

pub fn write_trace(
    context_values: &[QM31],
    preprocessed_trace: &PreProcessedTrace,
    verify_bitwise_xor_8_state: &verify_bitwise_xor_8::ClaimGenerator,
    verify_bitwise_xor_12_state: &verify_bitwise_xor_12::ClaimGenerator,
    verify_bitwise_xor_4_state: &verify_bitwise_xor_4::ClaimGenerator,
    verify_bitwise_xor_9_state: &verify_bitwise_xor_9::ClaimGenerator,
    verify_bitwise_xor_7_state: &verify_bitwise_xor_7::ClaimGenerator,
) -> (ComponentTrace<N_TRACE_COLUMNS>, Claim, InteractionClaimGenerator) {
    // The inputs are gathered per packed row straight from the context values at the
    // preprocessed addresses, read in place: no staged inputs, no packed copy of them and no
    // materialized address columns.
    let columns = [
        "blake_g_gate_input_addr_a",
        "blake_g_gate_input_addr_b",
        "blake_g_gate_input_addr_c",
        "blake_g_gate_input_addr_d",
        "blake_g_gate_input_addr_f0",
        "blake_g_gate_input_addr_f1",
        "blake_g_gate_output_addr_a",
        "blake_g_gate_output_addr_b",
        "blake_g_gate_output_addr_c",
        "blake_g_gate_output_addr_d",
        "blake_g_gate_multiplicity",
    ]
    .map(|id| preprocessed_trace.column_rows(&PreProcessedColumnId { id: id.to_owned() }));

    let n_rows = columns[0].len();
    assert!(columns.iter().all(|column| column.len() == n_rows));
    assert_ne!(n_rows, 0);
    assert!(n_rows.is_multiple_of(N_LANES));
    let size = std::cmp::max(n_rows.next_power_of_two(), N_LANES);
    let log_size = size.ilog2();

    // A row's inputs, as `extract_component_inputs` stages them.
    let input = |row: usize| -> InputType {
        from_fn(|i| {
            let value = context_values[columns[i].at(row)].to_m31_array();
            UInt32::from(value[0].0 | (value[1].0 << 16))
        })
    };

    let (trace, lookup_data) = write_trace_simd(n_rows, input, &columns);
    // The small xor tables count their rows per task from the narrow tuples, every column of a
    // table in one pass (see `count_table_rows`); the 2^24-row xor_12 table keeps its atomic adds.
    let d = &lookup_data;
    verify_bitwise_xor_8_state.add_counted_inputs(
        &[
            (&d.verify_bitwise_xor_8_0[..], 0),
            (&d.verify_bitwise_xor_8_1[..], 0),
            (&d.verify_bitwise_xor_8_2[..], 0),
            (&d.verify_bitwise_xor_8_3[..], 0),
            (&d.verify_bitwise_xor_8_4[..], 0),
            (&d.verify_bitwise_xor_8_5[..], 0),
            (&d.verify_bitwise_xor_8_b_0[..], 1),
            (&d.verify_bitwise_xor_8_b_1[..], 1),
        ],
        Widen::widen,
    );
    verify_bitwise_xor_12_state.add_packed_inputs(&widen_all(&lookup_data.verify_bitwise_xor_12_0), 0);
    verify_bitwise_xor_12_state.add_packed_inputs(&widen_all(&lookup_data.verify_bitwise_xor_12_1), 0);
    verify_bitwise_xor_4_state.add_counted_inputs(
        &[(&d.verify_bitwise_xor_4_0[..], 0), (&d.verify_bitwise_xor_4_1[..], 0)],
        Widen::widen,
    );
    verify_bitwise_xor_9_state.add_counted_inputs(
        &[(&d.verify_bitwise_xor_9_0[..], 0), (&d.verify_bitwise_xor_9_1[..], 0)],
        Widen::widen,
    );
    verify_bitwise_xor_7_state.add_counted_inputs(
        &[(&d.verify_bitwise_xor_7_0[..], 0), (&d.verify_bitwise_xor_7_1[..], 0)],
        Widen::widen,
    );

    (trace, Claim { log_size }, InteractionClaimGenerator { log_size, lookup_data })
}

#[allow(clippy::useless_conversion)]
#[allow(unused_variables)]
#[allow(clippy::double_parens)]
#[allow(non_snake_case)]
fn write_trace_simd(
    n_rows: usize,
    input: impl Fn(usize) -> InputType + Sync,
    columns: &[circuit_common::preprocessed::ColumnRows<'_>; 11],
) -> (ComponentTrace<N_TRACE_COLUMNS>, LookupData) {
    let log_n_packed_rows = (n_rows / N_LANES).ilog2();
    let log_size = log_n_packed_rows + LOG_N_LANES;
    let (mut trace, mut lookup_data) = unsafe {
        (
            ComponentTrace::<N_TRACE_COLUMNS>::uninitialized(log_size),
            LookupData::uninitialized(log_n_packed_rows),
        )
    };

    let M31_128 = PackedM31::broadcast(M31::from(128));
    let M31_16 = PackedM31::broadcast(M31::from(16));
    let M31_256 = PackedM31::broadcast(M31::from(256));
    let M31_4096 = PackedM31::broadcast(M31::from(4096));
    let M31_512 = PackedM31::broadcast(M31::from(512));
    let UInt16_12 = PackedUInt16::broadcast(UInt16::from(12));
    let UInt16_7 = PackedUInt16::broadcast(UInt16::from(7));
    let UInt16_8 = PackedUInt16::broadcast(UInt16::from(8));
    let UInt16_9 = PackedUInt16::broadcast(UInt16::from(9));
    let UInt32_0 = PackedUInt32::broadcast(UInt32::from(0));
    (trace.par_iter_mut(), lookup_data.par_iter_mut())
        .into_par_iter()
        .enumerate()
        .for_each(|(row_index, (row, lookup_data))| {
            let first = row_index * N_LANES;
            let blake_g_gate_input: PackedInputType =
                crate::witness::utils::packed_input(&input, first);
            let [
                blake_g_gate_input_addr_a,
                blake_g_gate_input_addr_b,
                blake_g_gate_input_addr_c,
                blake_g_gate_input_addr_d,
                blake_g_gate_input_addr_f0,
                blake_g_gate_input_addr_f1,
                blake_g_gate_output_addr_a,
                blake_g_gate_output_addr_b,
                blake_g_gate_output_addr_c,
                blake_g_gate_output_addr_d,
                blake_g_gate_multiplicity,
            ] = crate::witness::utils::packed_column_rows(columns, first);
            let input_a_limb_0_col0 = blake_g_gate_input[0].low().as_m31();
            *row[0] = input_a_limb_0_col0;
            let input_a_limb_1_col1 = blake_g_gate_input[0].high().as_m31();
            *row[1] = input_a_limb_1_col1;
            let input_b_limb_0_col2 = blake_g_gate_input[1].low().as_m31();
            *row[2] = input_b_limb_0_col2;
            let input_b_limb_1_col3 = blake_g_gate_input[1].high().as_m31();
            *row[3] = input_b_limb_1_col3;
            let input_c_limb_0_col4 = blake_g_gate_input[2].low().as_m31();
            *row[4] = input_c_limb_0_col4;
            let input_c_limb_1_col5 = blake_g_gate_input[2].high().as_m31();
            *row[5] = input_c_limb_1_col5;
            let input_d_limb_0_col6 = blake_g_gate_input[3].low().as_m31();
            *row[6] = input_d_limb_0_col6;
            let input_d_limb_1_col7 = blake_g_gate_input[3].high().as_m31();
            *row[7] = input_d_limb_1_col7;
            let input_f0_limb_0_col8 = blake_g_gate_input[4].low().as_m31();
            *row[8] = input_f0_limb_0_col8;
            let input_f0_limb_1_col9 = blake_g_gate_input[4].high().as_m31();
            *row[9] = input_f0_limb_1_col9;
            let input_f1_limb_0_col10 = blake_g_gate_input[5].low().as_m31();
            *row[10] = input_f1_limb_0_col10;
            let input_f1_limb_1_col11 = blake_g_gate_input[5].high().as_m31();
            *row[11] = input_f1_limb_1_col11;
            let input_a_tag_limb_0_col12 = blake_g_gate_input[6].low().as_m31();
            *row[12] = input_a_tag_limb_0_col12;
            let input_a_tag_limb_1_col13 = blake_g_gate_input[6].high().as_m31();
            *row[13] = input_a_tag_limb_1_col13;
            let input_b_tag_limb_0_col14 = blake_g_gate_input[7].low().as_m31();
            *row[14] = input_b_tag_limb_0_col14;
            let input_b_tag_limb_1_col15 = blake_g_gate_input[7].high().as_m31();
            *row[15] = input_b_tag_limb_1_col15;
            let input_c_tag_limb_0_col16 = blake_g_gate_input[8].low().as_m31();
            *row[16] = input_c_tag_limb_0_col16;
            let input_c_tag_limb_1_col17 = blake_g_gate_input[8].high().as_m31();
            *row[17] = input_c_tag_limb_1_col17;
            let input_d_tag_limb_0_col18 = blake_g_gate_input[9].low().as_m31();
            *row[18] = input_d_tag_limb_0_col18;
            let input_d_tag_limb_1_col19 = blake_g_gate_input[9].high().as_m31();
            *row[19] = input_d_tag_limb_1_col19;

            // Triple Sum 32.

            let triple_sum32_res_tmp_754f3_0 =
                (((blake_g_gate_input[0]) + (blake_g_gate_input[1])) + (blake_g_gate_input[4]));
            let triple_sum32_res_limb_0_col20 = triple_sum32_res_tmp_754f3_0.low().as_m31();
            *row[20] = triple_sum32_res_limb_0_col20;
            let triple_sum32_res_limb_1_col21 = triple_sum32_res_tmp_754f3_0.high().as_m31();
            *row[21] = triple_sum32_res_limb_1_col21;
            let triple_sum_32_output_tmp_754f3_3 = triple_sum32_res_tmp_754f3_0;

            // Xor Rot 32 R 16.

            // Split 16 Low Part Size 8.

            let ms_8_bits_tmp_754f3_4 = ((triple_sum_32_output_tmp_754f3_3.low()) >> (UInt16_8));
            let ms_8_bits_col22 = ms_8_bits_tmp_754f3_4.as_m31();
            *row[22] = ms_8_bits_col22;
            let split_16_low_part_size_8_output_tmp_754f3_5 = [
                ((triple_sum32_res_limb_0_col20) - ((ms_8_bits_col22) * (M31_256))),
                ms_8_bits_col22,
            ];

            // Split 16 Low Part Size 8.

            let ms_8_bits_tmp_754f3_6 = ((triple_sum_32_output_tmp_754f3_3.high()) >> (UInt16_8));
            let ms_8_bits_col23 = ms_8_bits_tmp_754f3_6.as_m31();
            *row[23] = ms_8_bits_col23;
            let split_16_low_part_size_8_output_tmp_754f3_7 = [
                ((triple_sum32_res_limb_1_col21) - ((ms_8_bits_col23) * (M31_256))),
                ms_8_bits_col23,
            ];

            // Split 16 Low Part Size 8.

            let ms_8_bits_tmp_754f3_8 = ((blake_g_gate_input[3].low()) >> (UInt16_8));
            let ms_8_bits_col24 = ms_8_bits_tmp_754f3_8.as_m31();
            *row[24] = ms_8_bits_col24;
            let split_16_low_part_size_8_output_tmp_754f3_9 =
                [((input_d_limb_0_col6) - ((ms_8_bits_col24) * (M31_256))), ms_8_bits_col24];

            // Split 16 Low Part Size 8.

            let ms_8_bits_tmp_754f3_10 = ((blake_g_gate_input[3].high()) >> (UInt16_8));
            let ms_8_bits_col25 = ms_8_bits_tmp_754f3_10.as_m31();
            *row[25] = ms_8_bits_col25;
            let split_16_low_part_size_8_output_tmp_754f3_11 =
                [((input_d_limb_1_col7) - ((ms_8_bits_col25) * (M31_256))), ms_8_bits_col25];

            // Bitwise Xor Num Bits 8.

            let xor_tmp_754f3_12 =
                ((PackedUInt16::from_m31(split_16_low_part_size_8_output_tmp_754f3_5[0]))
                    ^ (PackedUInt16::from_m31(split_16_low_part_size_8_output_tmp_754f3_9[0])));
            let xor_col26 = xor_tmp_754f3_12.as_m31();
            *row[26] = xor_col26;
            *lookup_data.verify_bitwise_xor_8_0 = NarrowTriple::new([
                split_16_low_part_size_8_output_tmp_754f3_5[0],
                split_16_low_part_size_8_output_tmp_754f3_9[0],
                xor_col26,
            ]);

            // Bitwise Xor Num Bits 8.

            let xor_tmp_754f3_14 = ((PackedUInt16::from_m31(ms_8_bits_col22))
                ^ (PackedUInt16::from_m31(ms_8_bits_col24)));
            let xor_col27 = xor_tmp_754f3_14.as_m31();
            *row[27] = xor_col27;
            *lookup_data.verify_bitwise_xor_8_1 = NarrowTriple::new([ms_8_bits_col22, ms_8_bits_col24, xor_col27]);

            // Bitwise Xor Num Bits 8 B.

            let xor_tmp_754f3_16 =
                ((PackedUInt16::from_m31(split_16_low_part_size_8_output_tmp_754f3_7[0]))
                    ^ (PackedUInt16::from_m31(split_16_low_part_size_8_output_tmp_754f3_11[0])));
            let xor_col28 = xor_tmp_754f3_16.as_m31();
            *row[28] = xor_col28;
            *lookup_data.verify_bitwise_xor_8_b_0 = NarrowTriple::new([
                split_16_low_part_size_8_output_tmp_754f3_7[0],
                split_16_low_part_size_8_output_tmp_754f3_11[0],
                xor_col28,
            ]);

            // Bitwise Xor Num Bits 8 B.

            let xor_tmp_754f3_18 = ((PackedUInt16::from_m31(ms_8_bits_col23))
                ^ (PackedUInt16::from_m31(ms_8_bits_col25)));
            let xor_col29 = xor_tmp_754f3_18.as_m31();
            *row[29] = xor_col29;
            *lookup_data.verify_bitwise_xor_8_b_1 = NarrowTriple::new([ms_8_bits_col23, ms_8_bits_col25, xor_col29]);

            let xor_rot_16_output_tmp_754f3_20 = PackedUInt32::from_limbs([
                ((xor_col28) + ((xor_col29) * (M31_256))),
                ((xor_col26) + ((xor_col27) * (M31_256))),
            ]);
            let xor_rot_32_r_16_output_tmp_754f3_21 = xor_rot_16_output_tmp_754f3_20;

            // Triple Sum 32.

            let triple_sum32_res_tmp_754f3_22 =
                (((blake_g_gate_input[2]) + (xor_rot_32_r_16_output_tmp_754f3_21)) + (UInt32_0));
            let triple_sum32_res_limb_0_col30 = triple_sum32_res_tmp_754f3_22.low().as_m31();
            *row[30] = triple_sum32_res_limb_0_col30;
            let triple_sum32_res_limb_1_col31 = triple_sum32_res_tmp_754f3_22.high().as_m31();
            *row[31] = triple_sum32_res_limb_1_col31;
            let triple_sum_32_output_tmp_754f3_25 = triple_sum32_res_tmp_754f3_22;

            // Xor Rot 32 R 12.

            // Split 16 Low Part Size 12.

            let ms_4_bits_tmp_754f3_26 = ((blake_g_gate_input[1].low()) >> (UInt16_12));
            let ms_4_bits_col32 = ms_4_bits_tmp_754f3_26.as_m31();
            *row[32] = ms_4_bits_col32;
            let split_16_low_part_size_12_output_tmp_754f3_27 =
                [((input_b_limb_0_col2) - ((ms_4_bits_col32) * (M31_4096))), ms_4_bits_col32];

            // Split 16 Low Part Size 12.

            let ms_4_bits_tmp_754f3_28 = ((blake_g_gate_input[1].high()) >> (UInt16_12));
            let ms_4_bits_col33 = ms_4_bits_tmp_754f3_28.as_m31();
            *row[33] = ms_4_bits_col33;
            let split_16_low_part_size_12_output_tmp_754f3_29 =
                [((input_b_limb_1_col3) - ((ms_4_bits_col33) * (M31_4096))), ms_4_bits_col33];

            // Split 16 Low Part Size 12.

            let ms_4_bits_tmp_754f3_30 = ((triple_sum_32_output_tmp_754f3_25.low()) >> (UInt16_12));
            let ms_4_bits_col34 = ms_4_bits_tmp_754f3_30.as_m31();
            *row[34] = ms_4_bits_col34;
            let split_16_low_part_size_12_output_tmp_754f3_31 = [
                ((triple_sum32_res_limb_0_col30) - ((ms_4_bits_col34) * (M31_4096))),
                ms_4_bits_col34,
            ];

            // Split 16 Low Part Size 12.

            let ms_4_bits_tmp_754f3_32 =
                ((triple_sum_32_output_tmp_754f3_25.high()) >> (UInt16_12));
            let ms_4_bits_col35 = ms_4_bits_tmp_754f3_32.as_m31();
            *row[35] = ms_4_bits_col35;
            let split_16_low_part_size_12_output_tmp_754f3_33 = [
                ((triple_sum32_res_limb_1_col31) - ((ms_4_bits_col35) * (M31_4096))),
                ms_4_bits_col35,
            ];

            // Bitwise Xor Num Bits 12.

            let xor_tmp_754f3_34 =
                ((PackedUInt16::from_m31(split_16_low_part_size_12_output_tmp_754f3_27[0]))
                    ^ (PackedUInt16::from_m31(split_16_low_part_size_12_output_tmp_754f3_31[0])));
            let xor_col36 = xor_tmp_754f3_34.as_m31();
            *row[36] = xor_col36;
            *lookup_data.verify_bitwise_xor_12_0 = NarrowTriple::new([
                split_16_low_part_size_12_output_tmp_754f3_27[0],
                split_16_low_part_size_12_output_tmp_754f3_31[0],
                xor_col36,
            ]);

            // Bitwise Xor Num Bits 4.

            let xor_tmp_754f3_36 = ((PackedUInt16::from_m31(ms_4_bits_col32))
                ^ (PackedUInt16::from_m31(ms_4_bits_col34)));
            let xor_col37 = xor_tmp_754f3_36.as_m31();
            *row[37] = xor_col37;
            *lookup_data.verify_bitwise_xor_4_0 = NarrowTriple::new([ms_4_bits_col32, ms_4_bits_col34, xor_col37]);

            // Bitwise Xor Num Bits 12.

            let xor_tmp_754f3_38 =
                ((PackedUInt16::from_m31(split_16_low_part_size_12_output_tmp_754f3_29[0]))
                    ^ (PackedUInt16::from_m31(split_16_low_part_size_12_output_tmp_754f3_33[0])));
            let xor_col38 = xor_tmp_754f3_38.as_m31();
            *row[38] = xor_col38;
            *lookup_data.verify_bitwise_xor_12_1 = NarrowTriple::new([
                split_16_low_part_size_12_output_tmp_754f3_29[0],
                split_16_low_part_size_12_output_tmp_754f3_33[0],
                xor_col38,
            ]);

            // Bitwise Xor Num Bits 4.

            let xor_tmp_754f3_40 = ((PackedUInt16::from_m31(ms_4_bits_col33))
                ^ (PackedUInt16::from_m31(ms_4_bits_col35)));
            let xor_col39 = xor_tmp_754f3_40.as_m31();
            *row[39] = xor_col39;
            *lookup_data.verify_bitwise_xor_4_1 = NarrowTriple::new([ms_4_bits_col33, ms_4_bits_col35, xor_col39]);

            let xor_rot_12_output_tmp_754f3_42 = PackedUInt32::from_limbs([
                ((xor_col37) + ((xor_col38) * (M31_16))),
                ((xor_col39) + ((xor_col36) * (M31_16))),
            ]);
            let xor_rot_32_r_12_output_tmp_754f3_43 = xor_rot_12_output_tmp_754f3_42;

            // Verify Xor Rot 32 R 8.

            // Split 16 Low Part Size 8.

            let ms_8_bits_tmp_754f3_46 = ((blake_g_gate_input[6].low()) >> (UInt16_8));
            let ms_8_bits_col40 = ms_8_bits_tmp_754f3_46.as_m31();
            *row[40] = ms_8_bits_col40;
            let split_16_low_part_size_8_output_tmp_754f3_47 =
                [((input_a_tag_limb_0_col12) - ((ms_8_bits_col40) * (M31_256))), ms_8_bits_col40];

            // Split 16 Low Part Size 8.

            let ms_8_bits_tmp_754f3_48 = ((blake_g_gate_input[6].high()) >> (UInt16_8));
            let ms_8_bits_col41 = ms_8_bits_tmp_754f3_48.as_m31();
            *row[41] = ms_8_bits_col41;
            let split_16_low_part_size_8_output_tmp_754f3_49 =
                [((input_a_tag_limb_1_col13) - ((ms_8_bits_col41) * (M31_256))), ms_8_bits_col41];

            // Split 16 Low Part Size 8.

            let ms_8_bits_tmp_754f3_50 =
                ((xor_rot_32_r_16_output_tmp_754f3_21.low()) >> (UInt16_8));
            let ms_8_bits_col42 = ms_8_bits_tmp_754f3_50.as_m31();
            *row[42] = ms_8_bits_col42;
            let split_16_low_part_size_8_output_tmp_754f3_51 = [
                ((xor_rot_16_output_tmp_754f3_20.low().as_m31()) - ((ms_8_bits_col42) * (M31_256))),
                ms_8_bits_col42,
            ];

            // Split 16 Low Part Size 8.

            let ms_8_bits_tmp_754f3_52 =
                ((xor_rot_32_r_16_output_tmp_754f3_21.high()) >> (UInt16_8));
            let ms_8_bits_col43 = ms_8_bits_tmp_754f3_52.as_m31();
            *row[43] = ms_8_bits_col43;
            let split_16_low_part_size_8_output_tmp_754f3_53 = [
                ((xor_rot_16_output_tmp_754f3_20.high().as_m31())
                    - ((ms_8_bits_col43) * (M31_256))),
                ms_8_bits_col43,
            ];

            // Split 16 Low Part Size 8.

            let ms_8_bits_tmp_754f3_54 = ((blake_g_gate_input[9].low()) >> (UInt16_8));
            let ms_8_bits_col44 = ms_8_bits_tmp_754f3_54.as_m31();
            *row[44] = ms_8_bits_col44;
            let split_16_low_part_size_8_output_tmp_754f3_55 =
                [((input_d_tag_limb_0_col18) - ((ms_8_bits_col44) * (M31_256))), ms_8_bits_col44];

            // Split 16 Low Part Size 8.

            let ms_8_bits_tmp_754f3_56 = ((blake_g_gate_input[9].high()) >> (UInt16_8));
            let ms_8_bits_col45 = ms_8_bits_tmp_754f3_56.as_m31();
            *row[45] = ms_8_bits_col45;
            let split_16_low_part_size_8_output_tmp_754f3_57 =
                [((input_d_tag_limb_1_col19) - ((ms_8_bits_col45) * (M31_256))), ms_8_bits_col45];
            *lookup_data.verify_bitwise_xor_8_2 = NarrowTriple::new([ms_8_bits_col40, ms_8_bits_col42, split_16_low_part_size_8_output_tmp_754f3_55[0]]);
            *lookup_data.verify_bitwise_xor_8_3 = NarrowTriple::new([
                split_16_low_part_size_8_output_tmp_754f3_49[0],
                split_16_low_part_size_8_output_tmp_754f3_53[0],
                ms_8_bits_col44,
            ]);
            *lookup_data.verify_bitwise_xor_8_4 = NarrowTriple::new([ms_8_bits_col41, ms_8_bits_col43, split_16_low_part_size_8_output_tmp_754f3_57[0]]);
            *lookup_data.verify_bitwise_xor_8_5 = NarrowTriple::new([
                split_16_low_part_size_8_output_tmp_754f3_47[0],
                split_16_low_part_size_8_output_tmp_754f3_51[0],
                ms_8_bits_col45,
            ]);

            // Verify Xor Rot 32 R 7.

            // Split 16 Low Part Size 7.

            let ms_9_bits_tmp_754f3_60 =
                ((xor_rot_32_r_12_output_tmp_754f3_43.low()) >> (UInt16_7));
            let ms_9_bits_col46 = ms_9_bits_tmp_754f3_60.as_m31();
            *row[46] = ms_9_bits_col46;
            let split_16_low_part_size_7_output_tmp_754f3_61 = [
                ((xor_rot_12_output_tmp_754f3_42.low().as_m31()) - ((ms_9_bits_col46) * (M31_128))),
                ms_9_bits_col46,
            ];

            // Split 16 Low Part Size 7.

            let ms_9_bits_tmp_754f3_62 =
                ((xor_rot_32_r_12_output_tmp_754f3_43.high()) >> (UInt16_7));
            let ms_9_bits_col47 = ms_9_bits_tmp_754f3_62.as_m31();
            *row[47] = ms_9_bits_col47;
            let split_16_low_part_size_7_output_tmp_754f3_63 = [
                ((xor_rot_12_output_tmp_754f3_42.high().as_m31())
                    - ((ms_9_bits_col47) * (M31_128))),
                ms_9_bits_col47,
            ];

            // Split 16 Low Part Size 7.

            let ms_9_bits_tmp_754f3_64 = ((blake_g_gate_input[8].low()) >> (UInt16_7));
            let ms_9_bits_col48 = ms_9_bits_tmp_754f3_64.as_m31();
            *row[48] = ms_9_bits_col48;
            let split_16_low_part_size_7_output_tmp_754f3_65 =
                [((input_c_tag_limb_0_col16) - ((ms_9_bits_col48) * (M31_128))), ms_9_bits_col48];

            // Split 16 Low Part Size 7.

            let ms_9_bits_tmp_754f3_66 = ((blake_g_gate_input[8].high()) >> (UInt16_7));
            let ms_9_bits_col49 = ms_9_bits_tmp_754f3_66.as_m31();
            *row[49] = ms_9_bits_col49;
            let split_16_low_part_size_7_output_tmp_754f3_67 =
                [((input_c_tag_limb_1_col17) - ((ms_9_bits_col49) * (M31_128))), ms_9_bits_col49];

            // Split 16 Low Part Size 9.

            let ms_7_bits_tmp_754f3_68 = ((blake_g_gate_input[7].low()) >> (UInt16_9));
            let ms_7_bits_col50 = ms_7_bits_tmp_754f3_68.as_m31();
            *row[50] = ms_7_bits_col50;
            let split_16_low_part_size_9_output_tmp_754f3_69 =
                [((input_b_tag_limb_0_col14) - ((ms_7_bits_col50) * (M31_512))), ms_7_bits_col50];

            // Split 16 Low Part Size 9.

            let ms_7_bits_tmp_754f3_70 = ((blake_g_gate_input[7].high()) >> (UInt16_9));
            let ms_7_bits_col51 = ms_7_bits_tmp_754f3_70.as_m31();
            *row[51] = ms_7_bits_col51;
            let split_16_low_part_size_9_output_tmp_754f3_71 =
                [((input_b_tag_limb_1_col15) - ((ms_7_bits_col51) * (M31_512))), ms_7_bits_col51];
            *lookup_data.verify_bitwise_xor_9_0 = NarrowTriple::new([ms_9_bits_col46, ms_9_bits_col48, split_16_low_part_size_9_output_tmp_754f3_69[0]]);
            *lookup_data.verify_bitwise_xor_7_0 = NarrowTriple::new([
                split_16_low_part_size_7_output_tmp_754f3_63[0],
                split_16_low_part_size_7_output_tmp_754f3_67[0],
                ms_7_bits_col50,
            ]);
            *lookup_data.verify_bitwise_xor_9_1 = NarrowTriple::new([ms_9_bits_col47, ms_9_bits_col49, split_16_low_part_size_9_output_tmp_754f3_71[0]]);
            *lookup_data.verify_bitwise_xor_7_1 = NarrowTriple::new([
                split_16_low_part_size_7_output_tmp_754f3_61[0],
                split_16_low_part_size_7_output_tmp_754f3_65[0],
                ms_7_bits_col51,
            ]);

            *lookup_data.gate_0 = NarrowGate::new([blake_g_gate_input_addr_a, input_a_limb_0_col0, input_a_limb_1_col1]);
            *lookup_data.gate_1 = NarrowGate::new([blake_g_gate_input_addr_b, input_b_limb_0_col2, input_b_limb_1_col3]);
            *lookup_data.gate_2 = NarrowGate::new([blake_g_gate_input_addr_c, input_c_limb_0_col4, input_c_limb_1_col5]);
            *lookup_data.gate_3 = NarrowGate::new([blake_g_gate_input_addr_d, input_d_limb_0_col6, input_d_limb_1_col7]);
            *lookup_data.gate_4 = NarrowGate::new([blake_g_gate_input_addr_f0, input_f0_limb_0_col8, input_f0_limb_1_col9]);
            *lookup_data.gate_5 = NarrowGate::new([blake_g_gate_input_addr_f1, input_f1_limb_0_col10, input_f1_limb_1_col11]);
            *lookup_data.gate_6 = NarrowGate::new([blake_g_gate_output_addr_a, input_a_tag_limb_0_col12, input_a_tag_limb_1_col13]);
            *lookup_data.gate_7 = NarrowGate::new([blake_g_gate_output_addr_b, input_b_tag_limb_0_col14, input_b_tag_limb_1_col15]);
            *lookup_data.gate_8 = NarrowGate::new([blake_g_gate_output_addr_c, input_c_tag_limb_0_col16, input_c_tag_limb_1_col17]);
            *lookup_data.gate_9 = NarrowGate::new([blake_g_gate_output_addr_d, input_d_tag_limb_0_col18, input_d_tag_limb_1_col19]);
            *lookup_data.mults_0 = blake_g_gate_multiplicity;
        });

    (trace, lookup_data)
}

#[derive(Uninitialized, IterMut, ParIterMut)]
struct LookupData {
    gate_0: Vec<NarrowGate>,
    gate_1: Vec<NarrowGate>,
    gate_2: Vec<NarrowGate>,
    gate_3: Vec<NarrowGate>,
    gate_4: Vec<NarrowGate>,
    gate_5: Vec<NarrowGate>,
    gate_6: Vec<NarrowGate>,
    gate_7: Vec<NarrowGate>,
    gate_8: Vec<NarrowGate>,
    gate_9: Vec<NarrowGate>,
    verify_bitwise_xor_12_0: Vec<NarrowTriple>,
    verify_bitwise_xor_12_1: Vec<NarrowTriple>,
    verify_bitwise_xor_4_0: Vec<NarrowTriple>,
    verify_bitwise_xor_4_1: Vec<NarrowTriple>,
    verify_bitwise_xor_7_0: Vec<NarrowTriple>,
    verify_bitwise_xor_7_1: Vec<NarrowTriple>,
    verify_bitwise_xor_8_0: Vec<NarrowTriple>,
    verify_bitwise_xor_8_1: Vec<NarrowTriple>,
    verify_bitwise_xor_8_2: Vec<NarrowTriple>,
    verify_bitwise_xor_8_3: Vec<NarrowTriple>,
    verify_bitwise_xor_8_4: Vec<NarrowTriple>,
    verify_bitwise_xor_8_5: Vec<NarrowTriple>,
    verify_bitwise_xor_8_b_0: Vec<NarrowTriple>,
    verify_bitwise_xor_8_b_1: Vec<NarrowTriple>,
    verify_bitwise_xor_9_0: Vec<NarrowTriple>,
    verify_bitwise_xor_9_1: Vec<NarrowTriple>,
    mults_0: Vec<PackedM31>,
}

/// A gate lookup tuple of a packed row, `[address, low limb, high limb]`, as a u32 address and
/// two u16 limbs (every limb of a u32 word is below 2^16): 8 bytes a row instead of 12. The lookup
/// data is resident from the witness write through the base commit (its 26 tuples were 312 of
/// every row's 316 bytes, about 0.3 GB): narrow gate tuples are rukafe0x's (rgr1_60812844).
#[derive(Clone, Copy)]
struct NarrowGate([u32; N_LANES], [[u16; N_LANES]; 2]);

/// A xor lookup tuple of a packed row, three values of at most 12 bits, as u16s: 6 bytes a row.
#[derive(Clone, Copy)]
struct NarrowTriple([[u16; N_LANES]; 3]);

/// The lanes of `value`, which are below 2^16.
fn narrow_lanes(value: PackedM31) -> [u16; N_LANES] {
    value.to_array().map(|value| u16::try_from(value.0).expect("a lookup value of at most 16 bits"))
}

fn widen_lanes(lanes: [u16; N_LANES]) -> PackedM31 {
    PackedM31::from_array(lanes.map(|lane| M31::from(u32::from(lane))))
}

impl NarrowGate {
    fn new([address, low, high]: [PackedM31; 3]) -> Self {
        Self(address.to_array().map(|address| address.0), [narrow_lanes(low), narrow_lanes(high)])
    }
}

impl NarrowTriple {
    fn new(values: [PackedM31; 3]) -> Self {
        Self(values.map(narrow_lanes))
    }
}

/// The tuple as it was written.
trait Widen {
    fn widen(&self) -> [PackedM31; 3];
}

impl Widen for NarrowGate {
    fn widen(&self) -> [PackedM31; 3] {
        [PackedM31::from_array(self.0.map(M31::from)), widen_lanes(self.1[0]), widen_lanes(self.1[1])]
    }
}

impl Widen for NarrowTriple {
    fn widen(&self) -> [PackedM31; 3] {
        self.0.map(widen_lanes)
    }
}

/// The tuples of a column, as written, for the xor states to read.
fn widen_all(column: &[NarrowTriple]) -> Vec<[PackedM31; 3]> {
    column.par_iter().map(Widen::widen).collect()
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
        let mut lookup_data = self.lookup_data;

        // The interaction columns are written one at a time (each one adds its fractions to the
        // previous one, as one pass over all of them does), and each column's two lookup arrays
        // are freed once it is written: the lookup data (about 0.3 GB) no longer sits under the
        // whole interaction trace (about 0.2 GB), which was the circuit's interaction peak.
        for column in 0..13 {
            let lookup_data_ref = &lookup_data;
            logup_gen.write_frac_cols(1, |_, row| {
                let lookup_data = lookup_data_ref;
                let (values0, values1, tag0, tag1) = match column {
                    0 => (
                        lookup_data.verify_bitwise_xor_8_0[row].widen(),
                        lookup_data.verify_bitwise_xor_8_1[row].widen(),
                        relations::VERIFY_BITWISE_XOR_8_RELATION_ID,
                        relations::VERIFY_BITWISE_XOR_8_RELATION_ID,
                    ),
                    1 => (
                        lookup_data.verify_bitwise_xor_8_b_0[row].widen(),
                        lookup_data.verify_bitwise_xor_8_b_1[row].widen(),
                        relations::VERIFY_BITWISE_XOR_8_B_RELATION_ID,
                        relations::VERIFY_BITWISE_XOR_8_B_RELATION_ID,
                    ),
                    2 => (
                        lookup_data.verify_bitwise_xor_12_0[row].widen(),
                        lookup_data.verify_bitwise_xor_4_0[row].widen(),
                        relations::VERIFY_BITWISE_XOR_12_RELATION_ID,
                        relations::VERIFY_BITWISE_XOR_4_RELATION_ID,
                    ),
                    3 => (
                        lookup_data.verify_bitwise_xor_12_1[row].widen(),
                        lookup_data.verify_bitwise_xor_4_1[row].widen(),
                        relations::VERIFY_BITWISE_XOR_12_RELATION_ID,
                        relations::VERIFY_BITWISE_XOR_4_RELATION_ID,
                    ),
                    4 => (
                        lookup_data.verify_bitwise_xor_8_2[row].widen(),
                        lookup_data.verify_bitwise_xor_8_3[row].widen(),
                        relations::VERIFY_BITWISE_XOR_8_RELATION_ID,
                        relations::VERIFY_BITWISE_XOR_8_RELATION_ID,
                    ),
                    5 => (
                        lookup_data.verify_bitwise_xor_8_4[row].widen(),
                        lookup_data.verify_bitwise_xor_8_5[row].widen(),
                        relations::VERIFY_BITWISE_XOR_8_RELATION_ID,
                        relations::VERIFY_BITWISE_XOR_8_RELATION_ID,
                    ),
                    6 => (
                        lookup_data.verify_bitwise_xor_9_0[row].widen(),
                        lookup_data.verify_bitwise_xor_7_0[row].widen(),
                        relations::VERIFY_BITWISE_XOR_9_RELATION_ID,
                        relations::VERIFY_BITWISE_XOR_7_RELATION_ID,
                    ),
                    7 => (
                        lookup_data.verify_bitwise_xor_9_1[row].widen(),
                        lookup_data.verify_bitwise_xor_7_1[row].widen(),
                        relations::VERIFY_BITWISE_XOR_9_RELATION_ID,
                        relations::VERIFY_BITWISE_XOR_7_RELATION_ID,
                    ),
                    8 => (
                        lookup_data.gate_0[row].widen(),
                        lookup_data.gate_1[row].widen(),
                        relations::GATE_RELATION_ID,
                        relations::GATE_RELATION_ID,
                    ),
                    9 => (
                        lookup_data.gate_2[row].widen(),
                        lookup_data.gate_3[row].widen(),
                        relations::GATE_RELATION_ID,
                        relations::GATE_RELATION_ID,
                    ),
                    10 => (
                        lookup_data.gate_4[row].widen(),
                        lookup_data.gate_5[row].widen(),
                        relations::GATE_RELATION_ID,
                        relations::GATE_RELATION_ID,
                    ),
                    11 => (
                        lookup_data.gate_6[row].widen(),
                        lookup_data.gate_7[row].widen(),
                        relations::GATE_RELATION_ID,
                        relations::GATE_RELATION_ID,
                    ),
                    12 => (
                        lookup_data.gate_8[row].widen(),
                        lookup_data.gate_9[row].widen(),
                        relations::GATE_RELATION_ID,
                        relations::GATE_RELATION_ID,
                    ),
                    _ => unreachable!(),
                };
                // Relation tags are constant per column; materialize them only at combination.
                let denom0: PackedQM31 = common_lookup_elements.combine(&[
                    PackedM31::broadcast(M31::from(tag0)),
                    values0[0],
                    values0[1],
                    values0[2],
                ]);
                let denom1: PackedQM31 = common_lookup_elements.combine(&[
                    PackedM31::broadcast(M31::from(tag1)),
                    values1[0],
                    values1[1],
                    values1[2],
                ]);
                let numerator = if column < 11 {
                    denom0 + denom1
                } else {
                    -(denom0 + denom1) * lookup_data.mults_0[row]
                };
                (numerator, denom0 * denom1)
            });
            if column < 8 {
                let released = match column {
                    0 => [&mut lookup_data.verify_bitwise_xor_8_0, &mut lookup_data.verify_bitwise_xor_8_1],
                    1 => [&mut lookup_data.verify_bitwise_xor_8_b_0, &mut lookup_data.verify_bitwise_xor_8_b_1],
                    2 => [&mut lookup_data.verify_bitwise_xor_12_0, &mut lookup_data.verify_bitwise_xor_4_0],
                    3 => [&mut lookup_data.verify_bitwise_xor_12_1, &mut lookup_data.verify_bitwise_xor_4_1],
                    4 => [&mut lookup_data.verify_bitwise_xor_8_2, &mut lookup_data.verify_bitwise_xor_8_3],
                    5 => [&mut lookup_data.verify_bitwise_xor_8_4, &mut lookup_data.verify_bitwise_xor_8_5],
                    6 => [&mut lookup_data.verify_bitwise_xor_9_0, &mut lookup_data.verify_bitwise_xor_7_0],
                    _ => [&mut lookup_data.verify_bitwise_xor_9_1, &mut lookup_data.verify_bitwise_xor_7_1],
                };
                for values in released {
                    drop(std::mem::take(values));
                }
            } else {
                let released = match column {
                    8 => [&mut lookup_data.gate_0, &mut lookup_data.gate_1],
                    9 => [&mut lookup_data.gate_2, &mut lookup_data.gate_3],
                    10 => [&mut lookup_data.gate_4, &mut lookup_data.gate_5],
                    11 => [&mut lookup_data.gate_6, &mut lookup_data.gate_7],
                    _ => [&mut lookup_data.gate_8, &mut lookup_data.gate_9],
                };
                for values in released {
                    drop(std::mem::take(values));
                }
            }
        }
        drop(lookup_data);

        let (trace, claimed_sum) = logup_gen.finalize_last();

        (trace, InteractionClaim { claimed_sum })
    }
}
