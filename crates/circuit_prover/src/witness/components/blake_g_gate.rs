#![allow(unused_parens)]
#![allow(clippy::too_many_arguments)]
use crate::circuit_air::components::blake_g_gate::{Claim, InteractionClaim, N_TRACE_COLUMNS};
use crate::witness::components::prelude::*;
use std::simd::cmp::SimdPartialOrd;
use rayon::slice::ParallelSlice;
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
    let input_addr_a = preprocessed_trace
        .get_column(&PreProcessedColumnId { id: "blake_g_gate_input_addr_a".to_owned() });
    let input_addr_b = preprocessed_trace
        .get_column(&PreProcessedColumnId { id: "blake_g_gate_input_addr_b".to_owned() });
    let input_addr_c = preprocessed_trace
        .get_column(&PreProcessedColumnId { id: "blake_g_gate_input_addr_c".to_owned() });
    let input_addr_d = preprocessed_trace
        .get_column(&PreProcessedColumnId { id: "blake_g_gate_input_addr_d".to_owned() });
    let input_addr_f0 = preprocessed_trace
        .get_column(&PreProcessedColumnId { id: "blake_g_gate_input_addr_f0".to_owned() });
    let input_addr_f1 = preprocessed_trace
        .get_column(&PreProcessedColumnId { id: "blake_g_gate_input_addr_f1".to_owned() });
    let output_addr_a = preprocessed_trace
        .get_column(&PreProcessedColumnId { id: "blake_g_gate_output_addr_a".to_owned() });
    let output_addr_b = preprocessed_trace
        .get_column(&PreProcessedColumnId { id: "blake_g_gate_output_addr_b".to_owned() });
    let output_addr_c = preprocessed_trace
        .get_column(&PreProcessedColumnId { id: "blake_g_gate_output_addr_c".to_owned() });
    let output_addr_d = preprocessed_trace
        .get_column(&PreProcessedColumnId { id: "blake_g_gate_output_addr_d".to_owned() });
    let multiplicity_col = preprocessed_trace
        .get_column(&PreProcessedColumnId { id: "blake_g_gate_multiplicity".to_owned() });

    let inputs = extract_component_inputs(
        &input_addr_a,
        &input_addr_b,
        &input_addr_c,
        &input_addr_d,
        &input_addr_f0,
        &input_addr_f1,
        &output_addr_a,
        &output_addr_b,
        &output_addr_c,
        &output_addr_d,
        context_values,
    );

    let n_rows = inputs.len();
    assert_ne!(n_rows, 0);
    let size = std::cmp::max(n_rows.next_power_of_two(), N_LANES);
    let log_size = size.ilog2();

    // The `usize` address columns are converted (and freed) before the inputs are packed, so the
    // two copies of the inputs never overlap them (a8n Q1463).
    let preprocessed_columns = [
        input_addr_a,
        input_addr_b,
        input_addr_c,
        input_addr_d,
        input_addr_f0,
        input_addr_f1,
        output_addr_a,
        output_addr_b,
        output_addr_c,
        output_addr_d,
        multiplicity_col,
    ]
    .into_iter()
    .map(|col| Col::<SimdBackend, M31>::from_iter(col.iter().map(|&x| M31::from(x))).data)
    .collect_vec();
    let packed_inputs = pack_values(&inputs);
    drop(inputs);

    let (trace, lookup_data) = write_trace_simd(packed_inputs, preprocessed_columns);
    add_narrow_inputs(&lookup_data.verify_bitwise_xor_8_0, |wide| verify_bitwise_xor_8_state.add_packed_inputs(wide, 0));
    add_narrow_inputs(&lookup_data.verify_bitwise_xor_8_1, |wide| verify_bitwise_xor_8_state.add_packed_inputs(wide, 0));
    add_narrow_inputs(&lookup_data.verify_bitwise_xor_8_2, |wide| verify_bitwise_xor_8_state.add_packed_inputs(wide, 0));
    add_narrow_inputs(&lookup_data.verify_bitwise_xor_8_3, |wide| verify_bitwise_xor_8_state.add_packed_inputs(wide, 0));
    add_narrow_inputs(&lookup_data.verify_bitwise_xor_8_4, |wide| verify_bitwise_xor_8_state.add_packed_inputs(wide, 0));
    add_narrow_inputs(&lookup_data.verify_bitwise_xor_8_5, |wide| verify_bitwise_xor_8_state.add_packed_inputs(wide, 0));
    add_narrow_inputs(&lookup_data.verify_bitwise_xor_8_b_0, |wide| verify_bitwise_xor_8_state.add_packed_inputs(wide, 1));
    add_narrow_inputs(&lookup_data.verify_bitwise_xor_8_b_1, |wide| verify_bitwise_xor_8_state.add_packed_inputs(wide, 1));
    add_narrow_inputs(&lookup_data.verify_bitwise_xor_12_0, |wide| verify_bitwise_xor_12_state.add_packed_inputs(wide, 0));
    add_narrow_inputs(&lookup_data.verify_bitwise_xor_12_1, |wide| verify_bitwise_xor_12_state.add_packed_inputs(wide, 0));
    add_narrow_inputs(&lookup_data.verify_bitwise_xor_4_0, |wide| verify_bitwise_xor_4_state.add_packed_inputs(wide, 0));
    add_narrow_inputs(&lookup_data.verify_bitwise_xor_4_1, |wide| verify_bitwise_xor_4_state.add_packed_inputs(wide, 0));
    add_narrow_inputs(&lookup_data.verify_bitwise_xor_9_0, |wide| verify_bitwise_xor_9_state.add_packed_inputs(wide, 0));
    add_narrow_inputs(&lookup_data.verify_bitwise_xor_9_1, |wide| verify_bitwise_xor_9_state.add_packed_inputs(wide, 0));
    add_narrow_inputs(&lookup_data.verify_bitwise_xor_7_0, |wide| verify_bitwise_xor_7_state.add_packed_inputs(wide, 0));
    add_narrow_inputs(&lookup_data.verify_bitwise_xor_7_1, |wide| verify_bitwise_xor_7_state.add_packed_inputs(wide, 0));

    (trace, Claim { log_size }, InteractionClaimGenerator { log_size, lookup_data })
}

/// An XOR lookup tuple `[a, b, a ^ b]` (a, b < 2^16) as one word per lane, `a | b << 16`: a third of
/// the bytes the interaction generator holds from the trace until the interaction trace (Q1010).
fn narrow_tuple([a, b, c]: [PackedM31; 3]) -> u32x16 {
    let (a, b) = (a.into_simd(), b.into_simd());
    debug_assert!(a.simd_lt(u32x16::splat(1 << 16)).all() && b.simd_lt(u32x16::splat(1 << 16)).all());
    debug_assert_eq!(a ^ b, c.into_simd());
    a | (b << 16)
}

/// A gate tuple `[address, low limb, high limb]` kept as the address and one word of the two
/// 16-bit limbs (8 bytes per row, not 12); [`widen_gate`] rebuilds it exactly (a8n Q1459, after
/// rukafe0x rgr1_60812844).
fn narrow_gate([address, low, high]: [PackedM31; 3]) -> [u32x16; 2] {
    let (low, high) = (low.into_simd(), high.into_simd());
    assert!(low.simd_lt(u32x16::splat(1 << 16)).all() && high.simd_lt(u32x16::splat(1 << 16)).all());
    [address.into_simd(), low | (high << 16)]
}

fn widen_gate([address, word]: [u32x16; 2]) -> [PackedM31; 3] {
    // Every lane is below 2^31 - 1 = P, so the reduction is the identity.
    [address, word & u32x16::splat(0xFFFF), word >> 16].map(PackedM31::reduce_simd)
}

/// Counts narrowed tuples in the lookup's multiplicities, widening a chunk at a time.
fn add_narrow_inputs(words: &[u32x16], add: impl Fn(&[[PackedM31; 3]]) + Sync) {
    words.par_chunks(1 << 10).for_each(|chunk| {
        add(&chunk.iter().map(|&w| widen_xor(w)).collect_vec());
    });
}

fn widen_xor(w: u32x16) -> [PackedM31; 3] {
    let (a, b) = (w & u32x16::splat(0xFFFF), w >> 16);
    // a, b < 2^16 < P, so the reduction is the identity.
    [a, b, a ^ b].map(PackedM31::reduce_simd)
}

#[allow(clippy::useless_conversion)]
#[allow(unused_variables)]
#[allow(clippy::double_parens)]
#[allow(non_snake_case)]
fn write_trace_simd(
    inputs: Vec<PackedInputType>,
    preprocessed_columns: Vec<Vec<PackedM31>>,
) -> (ComponentTrace<N_TRACE_COLUMNS>, LookupData) {
    let log_n_packed_rows = inputs.len().ilog2();
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
    ]: [Vec<PackedM31>; 11] = preprocessed_columns.try_into().unwrap();

    (trace.par_iter_mut(), lookup_data.par_iter_mut(), inputs.into_par_iter())
        .into_par_iter()
        .enumerate()
        .for_each(|(row_index, (row, lookup_data, blake_g_gate_input))| {
            let blake_g_gate_input_addr_a = blake_g_gate_input_addr_a[row_index];
            let blake_g_gate_input_addr_b = blake_g_gate_input_addr_b[row_index];
            let blake_g_gate_input_addr_c = blake_g_gate_input_addr_c[row_index];
            let blake_g_gate_input_addr_d = blake_g_gate_input_addr_d[row_index];
            let blake_g_gate_input_addr_f0 = blake_g_gate_input_addr_f0[row_index];
            let blake_g_gate_input_addr_f1 = blake_g_gate_input_addr_f1[row_index];
            let blake_g_gate_output_addr_a = blake_g_gate_output_addr_a[row_index];
            let blake_g_gate_output_addr_b = blake_g_gate_output_addr_b[row_index];
            let blake_g_gate_output_addr_c = blake_g_gate_output_addr_c[row_index];
            let blake_g_gate_output_addr_d = blake_g_gate_output_addr_d[row_index];
            let blake_g_gate_multiplicity = blake_g_gate_multiplicity[row_index];
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
            *lookup_data.verify_bitwise_xor_8_0 = narrow_tuple([
                split_16_low_part_size_8_output_tmp_754f3_5[0],
                split_16_low_part_size_8_output_tmp_754f3_9[0],
                xor_col26,
            ]);

            // Bitwise Xor Num Bits 8.

            let xor_tmp_754f3_14 = ((PackedUInt16::from_m31(ms_8_bits_col22))
                ^ (PackedUInt16::from_m31(ms_8_bits_col24)));
            let xor_col27 = xor_tmp_754f3_14.as_m31();
            *row[27] = xor_col27;
            *lookup_data.verify_bitwise_xor_8_1 = narrow_tuple([ms_8_bits_col22, ms_8_bits_col24, xor_col27]);

            // Bitwise Xor Num Bits 8 B.

            let xor_tmp_754f3_16 =
                ((PackedUInt16::from_m31(split_16_low_part_size_8_output_tmp_754f3_7[0]))
                    ^ (PackedUInt16::from_m31(split_16_low_part_size_8_output_tmp_754f3_11[0])));
            let xor_col28 = xor_tmp_754f3_16.as_m31();
            *row[28] = xor_col28;
            *lookup_data.verify_bitwise_xor_8_b_0 = narrow_tuple([
                split_16_low_part_size_8_output_tmp_754f3_7[0],
                split_16_low_part_size_8_output_tmp_754f3_11[0],
                xor_col28,
            ]);

            // Bitwise Xor Num Bits 8 B.

            let xor_tmp_754f3_18 = ((PackedUInt16::from_m31(ms_8_bits_col23))
                ^ (PackedUInt16::from_m31(ms_8_bits_col25)));
            let xor_col29 = xor_tmp_754f3_18.as_m31();
            *row[29] = xor_col29;
            *lookup_data.verify_bitwise_xor_8_b_1 = narrow_tuple([ms_8_bits_col23, ms_8_bits_col25, xor_col29]);

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
            *lookup_data.verify_bitwise_xor_12_0 = narrow_tuple([
                split_16_low_part_size_12_output_tmp_754f3_27[0],
                split_16_low_part_size_12_output_tmp_754f3_31[0],
                xor_col36,
            ]);

            // Bitwise Xor Num Bits 4.

            let xor_tmp_754f3_36 = ((PackedUInt16::from_m31(ms_4_bits_col32))
                ^ (PackedUInt16::from_m31(ms_4_bits_col34)));
            let xor_col37 = xor_tmp_754f3_36.as_m31();
            *row[37] = xor_col37;
            *lookup_data.verify_bitwise_xor_4_0 = narrow_tuple([ms_4_bits_col32, ms_4_bits_col34, xor_col37]);

            // Bitwise Xor Num Bits 12.

            let xor_tmp_754f3_38 =
                ((PackedUInt16::from_m31(split_16_low_part_size_12_output_tmp_754f3_29[0]))
                    ^ (PackedUInt16::from_m31(split_16_low_part_size_12_output_tmp_754f3_33[0])));
            let xor_col38 = xor_tmp_754f3_38.as_m31();
            *row[38] = xor_col38;
            *lookup_data.verify_bitwise_xor_12_1 = narrow_tuple([
                split_16_low_part_size_12_output_tmp_754f3_29[0],
                split_16_low_part_size_12_output_tmp_754f3_33[0],
                xor_col38,
            ]);

            // Bitwise Xor Num Bits 4.

            let xor_tmp_754f3_40 = ((PackedUInt16::from_m31(ms_4_bits_col33))
                ^ (PackedUInt16::from_m31(ms_4_bits_col35)));
            let xor_col39 = xor_tmp_754f3_40.as_m31();
            *row[39] = xor_col39;
            *lookup_data.verify_bitwise_xor_4_1 = narrow_tuple([ms_4_bits_col33, ms_4_bits_col35, xor_col39]);

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
            *lookup_data.verify_bitwise_xor_8_2 =
                narrow_tuple([ms_8_bits_col40, ms_8_bits_col42, split_16_low_part_size_8_output_tmp_754f3_55[0]]);
            *lookup_data.verify_bitwise_xor_8_3 = narrow_tuple([
                split_16_low_part_size_8_output_tmp_754f3_49[0],
                split_16_low_part_size_8_output_tmp_754f3_53[0],
                ms_8_bits_col44,
            ]);
            *lookup_data.verify_bitwise_xor_8_4 =
                narrow_tuple([ms_8_bits_col41, ms_8_bits_col43, split_16_low_part_size_8_output_tmp_754f3_57[0]]);
            *lookup_data.verify_bitwise_xor_8_5 = narrow_tuple([
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
            *lookup_data.verify_bitwise_xor_9_0 =
                narrow_tuple([ms_9_bits_col46, ms_9_bits_col48, split_16_low_part_size_9_output_tmp_754f3_69[0]]);
            *lookup_data.verify_bitwise_xor_7_0 = narrow_tuple([
                split_16_low_part_size_7_output_tmp_754f3_63[0],
                split_16_low_part_size_7_output_tmp_754f3_67[0],
                ms_7_bits_col50,
            ]);
            *lookup_data.verify_bitwise_xor_9_1 =
                narrow_tuple([ms_9_bits_col47, ms_9_bits_col49, split_16_low_part_size_9_output_tmp_754f3_71[0]]);
            *lookup_data.verify_bitwise_xor_7_1 = narrow_tuple([
                split_16_low_part_size_7_output_tmp_754f3_61[0],
                split_16_low_part_size_7_output_tmp_754f3_65[0],
                ms_7_bits_col51,
            ]);

            *lookup_data.gate_0 =
                narrow_gate([blake_g_gate_input_addr_a, input_a_limb_0_col0, input_a_limb_1_col1]);
            *lookup_data.gate_1 =
                narrow_gate([blake_g_gate_input_addr_b, input_b_limb_0_col2, input_b_limb_1_col3]);
            *lookup_data.gate_2 =
                narrow_gate([blake_g_gate_input_addr_c, input_c_limb_0_col4, input_c_limb_1_col5]);
            *lookup_data.gate_3 =
                narrow_gate([blake_g_gate_input_addr_d, input_d_limb_0_col6, input_d_limb_1_col7]);
            *lookup_data.gate_4 =
                narrow_gate([blake_g_gate_input_addr_f0, input_f0_limb_0_col8, input_f0_limb_1_col9]);
            *lookup_data.gate_5 =
                narrow_gate([blake_g_gate_input_addr_f1, input_f1_limb_0_col10, input_f1_limb_1_col11]);
            *lookup_data.gate_6 =
                narrow_gate([blake_g_gate_output_addr_a, input_a_tag_limb_0_col12, input_a_tag_limb_1_col13]);
            *lookup_data.gate_7 =
                narrow_gate([blake_g_gate_output_addr_b, input_b_tag_limb_0_col14, input_b_tag_limb_1_col15]);
            *lookup_data.gate_8 =
                narrow_gate([blake_g_gate_output_addr_c, input_c_tag_limb_0_col16, input_c_tag_limb_1_col17]);
            *lookup_data.gate_9 =
                narrow_gate([blake_g_gate_output_addr_d, input_d_tag_limb_0_col18, input_d_tag_limb_1_col19]);
            *lookup_data.mults_0 = blake_g_gate_multiplicity;
        });

    (trace, lookup_data)
}

#[derive(Uninitialized, IterMut, ParIterMut)]
struct LookupData {
    gate_0: Vec<[u32x16; 2]>,
    gate_1: Vec<[u32x16; 2]>,
    gate_2: Vec<[u32x16; 2]>,
    gate_3: Vec<[u32x16; 2]>,
    gate_4: Vec<[u32x16; 2]>,
    gate_5: Vec<[u32x16; 2]>,
    gate_6: Vec<[u32x16; 2]>,
    gate_7: Vec<[u32x16; 2]>,
    gate_8: Vec<[u32x16; 2]>,
    gate_9: Vec<[u32x16; 2]>,
    verify_bitwise_xor_12_0: Vec<u32x16>,
    verify_bitwise_xor_12_1: Vec<u32x16>,
    verify_bitwise_xor_4_0: Vec<u32x16>,
    verify_bitwise_xor_4_1: Vec<u32x16>,
    verify_bitwise_xor_7_0: Vec<u32x16>,
    verify_bitwise_xor_7_1: Vec<u32x16>,
    verify_bitwise_xor_8_0: Vec<u32x16>,
    verify_bitwise_xor_8_1: Vec<u32x16>,
    verify_bitwise_xor_8_2: Vec<u32x16>,
    verify_bitwise_xor_8_3: Vec<u32x16>,
    verify_bitwise_xor_8_4: Vec<u32x16>,
    verify_bitwise_xor_8_5: Vec<u32x16>,
    verify_bitwise_xor_8_b_0: Vec<u32x16>,
    verify_bitwise_xor_8_b_1: Vec<u32x16>,
    verify_bitwise_xor_9_0: Vec<u32x16>,
    verify_bitwise_xor_9_1: Vec<u32x16>,
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

        // Generate the interaction columns within one row-chunk pass.
        logup_gen.write_frac_cols(13, |column, row| {
            let (values0, values1, tag0, tag1) = match column {
                0 => (
                    widen_xor(self.lookup_data.verify_bitwise_xor_8_0[row]),
                    widen_xor(self.lookup_data.verify_bitwise_xor_8_1[row]),
                    relations::VERIFY_BITWISE_XOR_8_RELATION_ID,
                    relations::VERIFY_BITWISE_XOR_8_RELATION_ID,
                ),
                1 => (
                    widen_xor(self.lookup_data.verify_bitwise_xor_8_b_0[row]),
                    widen_xor(self.lookup_data.verify_bitwise_xor_8_b_1[row]),
                    relations::VERIFY_BITWISE_XOR_8_B_RELATION_ID,
                    relations::VERIFY_BITWISE_XOR_8_B_RELATION_ID,
                ),
                2 => (
                    widen_xor(self.lookup_data.verify_bitwise_xor_12_0[row]),
                    widen_xor(self.lookup_data.verify_bitwise_xor_4_0[row]),
                    relations::VERIFY_BITWISE_XOR_12_RELATION_ID,
                    relations::VERIFY_BITWISE_XOR_4_RELATION_ID,
                ),
                3 => (
                    widen_xor(self.lookup_data.verify_bitwise_xor_12_1[row]),
                    widen_xor(self.lookup_data.verify_bitwise_xor_4_1[row]),
                    relations::VERIFY_BITWISE_XOR_12_RELATION_ID,
                    relations::VERIFY_BITWISE_XOR_4_RELATION_ID,
                ),
                4 => (
                    widen_xor(self.lookup_data.verify_bitwise_xor_8_2[row]),
                    widen_xor(self.lookup_data.verify_bitwise_xor_8_3[row]),
                    relations::VERIFY_BITWISE_XOR_8_RELATION_ID,
                    relations::VERIFY_BITWISE_XOR_8_RELATION_ID,
                ),
                5 => (
                    widen_xor(self.lookup_data.verify_bitwise_xor_8_4[row]),
                    widen_xor(self.lookup_data.verify_bitwise_xor_8_5[row]),
                    relations::VERIFY_BITWISE_XOR_8_RELATION_ID,
                    relations::VERIFY_BITWISE_XOR_8_RELATION_ID,
                ),
                6 => (
                    widen_xor(self.lookup_data.verify_bitwise_xor_9_0[row]),
                    widen_xor(self.lookup_data.verify_bitwise_xor_7_0[row]),
                    relations::VERIFY_BITWISE_XOR_9_RELATION_ID,
                    relations::VERIFY_BITWISE_XOR_7_RELATION_ID,
                ),
                7 => (
                    widen_xor(self.lookup_data.verify_bitwise_xor_9_1[row]),
                    widen_xor(self.lookup_data.verify_bitwise_xor_7_1[row]),
                    relations::VERIFY_BITWISE_XOR_9_RELATION_ID,
                    relations::VERIFY_BITWISE_XOR_7_RELATION_ID,
                ),
                8 => (
                    widen_gate(self.lookup_data.gate_0[row]),
                    widen_gate(self.lookup_data.gate_1[row]),
                    relations::GATE_RELATION_ID,
                    relations::GATE_RELATION_ID,
                ),
                9 => (
                    widen_gate(self.lookup_data.gate_2[row]),
                    widen_gate(self.lookup_data.gate_3[row]),
                    relations::GATE_RELATION_ID,
                    relations::GATE_RELATION_ID,
                ),
                10 => (
                    widen_gate(self.lookup_data.gate_4[row]),
                    widen_gate(self.lookup_data.gate_5[row]),
                    relations::GATE_RELATION_ID,
                    relations::GATE_RELATION_ID,
                ),
                11 => (
                    widen_gate(self.lookup_data.gate_6[row]),
                    widen_gate(self.lookup_data.gate_7[row]),
                    relations::GATE_RELATION_ID,
                    relations::GATE_RELATION_ID,
                ),
                12 => (
                    widen_gate(self.lookup_data.gate_8[row]),
                    widen_gate(self.lookup_data.gate_9[row]),
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
                -(denom0 + denom1) * self.lookup_data.mults_0[row]
            };
            (numerator, denom0 * denom1)
        });

        let (trace, claimed_sum) = logup_gen.finalize_last();

        (trace, InteractionClaim { claimed_sum })
    }
}
