// This file was created by the AIR team.

#![allow(unused_parens)]
use cairo_air::components::blake_g::{Claim, InteractionClaim, N_TRACE_COLUMNS};

use crate::witness::components::{
    verify_bitwise_xor_4, verify_bitwise_xor_7, verify_bitwise_xor_8, verify_bitwise_xor_9,
    verify_bitwise_xor_12,
};
use crate::witness::prelude::*;

pub type InputType = [UInt32; 6];
pub type PackedInputType = [PackedUInt32; 6];

#[derive(Default)]
pub struct ClaimGenerator {
    pub packed_inputs: Mutex<Vec<PackedInputType>>,
    pub remainder_inputs: Mutex<Vec<InputType>>,
}

impl ClaimGenerator {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn write_trace(
        self,
        verify_bitwise_xor_8_state: &verify_bitwise_xor_8::ClaimGenerator,
        verify_bitwise_xor_12_state: &verify_bitwise_xor_12::ClaimGenerator,
        verify_bitwise_xor_4_state: &verify_bitwise_xor_4::ClaimGenerator,
        verify_bitwise_xor_7_state: &verify_bitwise_xor_7::ClaimGenerator,
        verify_bitwise_xor_9_state: &verify_bitwise_xor_9::ClaimGenerator,
    ) -> (ComponentTrace<N_TRACE_COLUMNS>, Claim, InteractionClaimGenerator) {
        let mut packed_inputs = self.packed_inputs.into_inner().unwrap();
        let remainder_inputs = self.remainder_inputs.into_inner().unwrap();
        let n_active_rows = packed_inputs.len() * N_LANES + remainder_inputs.len();
        add_remainder(&mut packed_inputs, &remainder_inputs);
        assert!(!packed_inputs.is_empty());
        let n_vec_rows = packed_inputs.len();
        let packed_size = n_vec_rows.next_power_of_two();
        let log_size = packed_size.ilog2() + LOG_N_LANES;
        packed_inputs.resize(packed_size, *packed_inputs.first().unwrap());

        let (trace, lookup_data, sub_component_inputs) = write_trace_simd(
            packed_inputs,
            n_active_rows,
            verify_bitwise_xor_8_state,
            verify_bitwise_xor_12_state,
            verify_bitwise_xor_4_state,
            verify_bitwise_xor_7_state,
            verify_bitwise_xor_9_state,
        );
        for inputs in sub_component_inputs.verify_bitwise_xor_8 {
            add_inputs(verify_bitwise_xor_8_state, &inputs, n_active_rows, 0);
        }
        for inputs in sub_component_inputs.verify_bitwise_xor_8_b {
            add_inputs(verify_bitwise_xor_8_state, &inputs, n_active_rows, 1);
        }
        for inputs in sub_component_inputs.verify_bitwise_xor_12 {
            add_inputs(verify_bitwise_xor_12_state, &inputs, n_active_rows, 0);
        }
        for inputs in sub_component_inputs.verify_bitwise_xor_4 {
            add_inputs(verify_bitwise_xor_4_state, &inputs, n_active_rows, 0);
        }
        for inputs in sub_component_inputs.verify_bitwise_xor_7 {
            add_inputs(verify_bitwise_xor_7_state, &inputs, n_active_rows, 0);
        }
        for inputs in sub_component_inputs.verify_bitwise_xor_9 {
            add_inputs(verify_bitwise_xor_9_state, &inputs, n_active_rows, 0);
        }

        (trace, Claim { log_size }, InteractionClaimGenerator { log_size, lookup_data })
    }
}

impl AddInputs for ClaimGenerator {
    type PackedInputType = PackedInputType;
    type InputType = InputType;

    fn add_packed_inputs(&self, inputs: &[PackedInputType], _relation_index: usize) {
        self.packed_inputs.lock().unwrap().extend(inputs);
    }
    fn add_input(&self, input: &InputType, _relation_index: usize) {
        self.remainder_inputs.lock().unwrap().push(*input);
    }
}

#[derive(Uninitialized, IterMut, ParIterMut)]
struct SubComponentInputs {
    verify_bitwise_xor_8: [Vec<verify_bitwise_xor_8::PackedInputType>; 4],
    verify_bitwise_xor_8_b: [Vec<verify_bitwise_xor_8::PackedInputType>; 4],
    verify_bitwise_xor_12: [Vec<verify_bitwise_xor_12::PackedInputType>; 2],
    verify_bitwise_xor_4: [Vec<verify_bitwise_xor_4::PackedInputType>; 2],
    verify_bitwise_xor_7: [Vec<verify_bitwise_xor_7::PackedInputType>; 2],
    verify_bitwise_xor_9: [Vec<verify_bitwise_xor_9::PackedInputType>; 2],
}

#[allow(clippy::useless_conversion)]
#[allow(unused_variables)]
#[allow(clippy::double_parens)]
#[allow(non_snake_case)]
fn write_trace_simd(
    inputs: Vec<PackedInputType>,
    n_rows: usize,
    verify_bitwise_xor_8_state: &verify_bitwise_xor_8::ClaimGenerator,
    verify_bitwise_xor_12_state: &verify_bitwise_xor_12::ClaimGenerator,
    verify_bitwise_xor_4_state: &verify_bitwise_xor_4::ClaimGenerator,
    verify_bitwise_xor_7_state: &verify_bitwise_xor_7::ClaimGenerator,
    verify_bitwise_xor_9_state: &verify_bitwise_xor_9::ClaimGenerator,
) -> (ComponentTrace<N_TRACE_COLUMNS>, LookupData, SubComponentInputs) {
    let log_n_packed_rows = inputs.len().ilog2();
    let log_size = log_n_packed_rows + LOG_N_LANES;
    let (mut trace, mut lookup_data, mut sub_component_inputs) = unsafe {
        (
            ComponentTrace::<N_TRACE_COLUMNS>::uninitialized(log_size),
            LookupData::uninitialized(log_n_packed_rows),
            SubComponentInputs::uninitialized(log_n_packed_rows),
        )
    };

    let M31_112558620 = PackedM31::broadcast(M31::from(112558620));
    let M31_1139985212 = PackedM31::broadcast(M31::from(1139985212));
    let M31_45448144 = PackedM31::broadcast(M31::from(45448144));
    let M31_521092554 = PackedM31::broadcast(M31::from(521092554));
    let M31_62225763 = PackedM31::broadcast(M31::from(62225763));
    let M31_648362599 = PackedM31::broadcast(M31::from(648362599));
    let M31_95781001 = PackedM31::broadcast(M31::from(95781001));
    let mask7 = std::simd::u32x16::splat(0x7F);
    let mask8 = std::simd::u32x16::splat(0xFF);
    let mask12 = std::simd::u32x16::splat(0xFFF);
    let enabler_col = Enabler::new(n_rows);

    (
        trace.par_iter_mut(),
        lookup_data.par_iter_mut(),
        sub_component_inputs.par_iter_mut(),
        inputs.into_par_iter(),
    )
        .into_par_iter()
        .enumerate()
        .for_each(|(row_index, (row, lookup_data, sub_component_inputs, blake_g_input))| {
            let enabler_col0 = enabler_col.packed_at(row_index);
            *row[0] = enabler_col0;
            let input_limb_0_col1 = blake_g_input[0].low().as_m31();
            *row[1] = input_limb_0_col1;
            let input_limb_1_col2 = blake_g_input[0].high().as_m31();
            *row[2] = input_limb_1_col2;
            let input_limb_2_col3 = blake_g_input[1].low().as_m31();
            *row[3] = input_limb_2_col3;
            let input_limb_3_col4 = blake_g_input[1].high().as_m31();
            *row[4] = input_limb_3_col4;
            let input_limb_4_col5 = blake_g_input[2].low().as_m31();
            *row[5] = input_limb_4_col5;
            let input_limb_5_col6 = blake_g_input[2].high().as_m31();
            *row[6] = input_limb_5_col6;
            let input_limb_6_col7 = blake_g_input[3].low().as_m31();
            *row[7] = input_limb_6_col7;
            let input_limb_7_col8 = blake_g_input[3].high().as_m31();
            *row[8] = input_limb_7_col8;
            let input_limb_8_col9 = blake_g_input[4].low().as_m31();
            *row[9] = input_limb_8_col9;
            let input_limb_9_col10 = blake_g_input[4].high().as_m31();
            *row[10] = input_limb_9_col10;
            let input_limb_10_col11 = blake_g_input[5].low().as_m31();
            *row[11] = input_limb_10_col11;
            let input_limb_11_col12 = blake_g_input[5].high().as_m31();
            *row[12] = input_limb_11_col12;

            // Triple Sum 32 (1).
            let triple_sum_32_output_tmp_4da06_3 =
                blake_g_input[0] + blake_g_input[1] + blake_g_input[4];
            let triple_sum32_res_limb_0_col13 = triple_sum_32_output_tmp_4da06_3.low().as_m31();
            *row[13] = triple_sum32_res_limb_0_col13;
            let triple_sum32_res_limb_1_col14 = triple_sum_32_output_tmp_4da06_3.high().as_m31();
            *row[14] = triple_sum32_res_limb_1_col14;

            // Xor Rot 32 R 16.
            let ts0_lo = triple_sum32_res_limb_0_col13.into_simd();
            let ts0_hi = triple_sum32_res_limb_1_col14.into_simd();
            let in3_lo = input_limb_6_col7.into_simd();
            let in3_hi = input_limb_7_col8.into_simd();

            let h15 = ts0_lo >> 8;
            let l15 = ts0_lo & mask8;
            let ms_8_bits_col15 = PackedM31::from_u32x16_masked(h15);
            *row[15] = ms_8_bits_col15;
            let ls_8_bits_5 = PackedM31::from_u32x16_masked(l15);

            let h16 = ts0_hi >> 8;
            let l16 = ts0_hi & mask8;
            let ms_8_bits_col16 = PackedM31::from_u32x16_masked(h16);
            *row[16] = ms_8_bits_col16;
            let ls_8_bits_7 = PackedM31::from_u32x16_masked(l16);

            let h17 = in3_lo >> 8;
            let l17 = in3_lo & mask8;
            let ms_8_bits_col17 = PackedM31::from_u32x16_masked(h17);
            *row[17] = ms_8_bits_col17;
            let ls_8_bits_9 = PackedM31::from_u32x16_masked(l17);

            let h18 = in3_hi >> 8;
            let l18 = in3_hi & mask8;
            let ms_8_bits_col18 = PackedM31::from_u32x16_masked(h18);
            *row[18] = ms_8_bits_col18;
            let ls_8_bits_11 = PackedM31::from_u32x16_masked(l18);

            let x19 = l15 ^ l17;
            let xor_col19 = PackedM31::from_u32x16_masked(x19);
            *row[19] = xor_col19;
            *sub_component_inputs.verify_bitwise_xor_8[0] = [ls_8_bits_5, ls_8_bits_9, xor_col19];
            *lookup_data.verify_bitwise_xor_8_0 =
                [M31_112558620, ls_8_bits_5, ls_8_bits_9, xor_col19];

            let x20 = h15 ^ h17;
            let xor_col20 = PackedM31::from_u32x16_masked(x20);
            *row[20] = xor_col20;
            *sub_component_inputs.verify_bitwise_xor_8[1] =
                [ms_8_bits_col15, ms_8_bits_col17, xor_col20];
            *lookup_data.verify_bitwise_xor_8_1 =
                [M31_112558620, ms_8_bits_col15, ms_8_bits_col17, xor_col20];

            let x21 = l16 ^ l18;
            let xor_col21 = PackedM31::from_u32x16_masked(x21);
            *row[21] = xor_col21;
            *sub_component_inputs.verify_bitwise_xor_8_b[0] =
                [ls_8_bits_7, ls_8_bits_11, xor_col21];
            *lookup_data.verify_bitwise_xor_8_b_2 =
                [M31_521092554, ls_8_bits_7, ls_8_bits_11, xor_col21];

            let x22 = h16 ^ h18;
            let xor_col22 = PackedM31::from_u32x16_masked(x22);
            *row[22] = xor_col22;
            *sub_component_inputs.verify_bitwise_xor_8_b[1] =
                [ms_8_bits_col16, ms_8_bits_col18, xor_col22];
            *lookup_data.verify_bitwise_xor_8_b_3 =
                [M31_521092554, ms_8_bits_col16, ms_8_bits_col18, xor_col22];

            let rot16_lo_u = x21 | (x22 << 8);
            let rot16_hi_u = x19 | (x20 << 8);
            let rot16_lo_m31 = PackedM31::from_u32x16_masked(rot16_lo_u);
            let rot16_hi_m31 = PackedM31::from_u32x16_masked(rot16_hi_u);
            let xor_rot_32_r_16_output_tmp_4da06_21 =
                PackedUInt32::from_limbs([rot16_lo_m31, rot16_hi_m31]);

            // Triple Sum 32 (2).
            let triple_sum_32_output_tmp_4da06_25 =
                blake_g_input[2] + xor_rot_32_r_16_output_tmp_4da06_21;
            let triple_sum32_res_limb_0_col23 = triple_sum_32_output_tmp_4da06_25.low().as_m31();
            *row[23] = triple_sum32_res_limb_0_col23;
            let triple_sum32_res_limb_1_col24 = triple_sum_32_output_tmp_4da06_25.high().as_m31();
            *row[24] = triple_sum32_res_limb_1_col24;

            // Xor Rot 32 R 12.
            let in1_lo = input_limb_2_col3.into_simd();
            let in1_hi = input_limb_3_col4.into_simd();
            let ts1_lo = triple_sum32_res_limb_0_col23.into_simd();
            let ts1_hi = triple_sum32_res_limb_1_col24.into_simd();

            let h25 = in1_lo >> 12;
            let l25 = in1_lo & mask12;
            let ms_4_bits_col25 = PackedM31::from_u32x16_masked(h25);
            *row[25] = ms_4_bits_col25;
            let ls_12_bits_27 = PackedM31::from_u32x16_masked(l25);

            let h26 = in1_hi >> 12;
            let l26 = in1_hi & mask12;
            let ms_4_bits_col26 = PackedM31::from_u32x16_masked(h26);
            *row[26] = ms_4_bits_col26;
            let ls_12_bits_29 = PackedM31::from_u32x16_masked(l26);

            let h27 = ts1_lo >> 12;
            let l27 = ts1_lo & mask12;
            let ms_4_bits_col27 = PackedM31::from_u32x16_masked(h27);
            *row[27] = ms_4_bits_col27;
            let ls_12_bits_31 = PackedM31::from_u32x16_masked(l27);

            let h28 = ts1_hi >> 12;
            let l28 = ts1_hi & mask12;
            let ms_4_bits_col28 = PackedM31::from_u32x16_masked(h28);
            *row[28] = ms_4_bits_col28;
            let ls_12_bits_33 = PackedM31::from_u32x16_masked(l28);

            let x29 = l25 ^ l27;
            let xor_col29 = PackedM31::from_u32x16_masked(x29);
            *row[29] = xor_col29;
            *sub_component_inputs.verify_bitwise_xor_12[0] =
                [ls_12_bits_27, ls_12_bits_31, xor_col29];
            *lookup_data.verify_bitwise_xor_12_4 =
                [M31_648362599, ls_12_bits_27, ls_12_bits_31, xor_col29];

            let x30 = h25 ^ h27;
            let xor_col30 = PackedM31::from_u32x16_masked(x30);
            *row[30] = xor_col30;
            *sub_component_inputs.verify_bitwise_xor_4[0] =
                [ms_4_bits_col25, ms_4_bits_col27, xor_col30];
            *lookup_data.verify_bitwise_xor_4_5 =
                [M31_45448144, ms_4_bits_col25, ms_4_bits_col27, xor_col30];

            let x31 = l26 ^ l28;
            let xor_col31 = PackedM31::from_u32x16_masked(x31);
            *row[31] = xor_col31;
            *sub_component_inputs.verify_bitwise_xor_12[1] =
                [ls_12_bits_29, ls_12_bits_33, xor_col31];
            *lookup_data.verify_bitwise_xor_12_6 =
                [M31_648362599, ls_12_bits_29, ls_12_bits_33, xor_col31];

            let x32 = h26 ^ h28;
            let xor_col32 = PackedM31::from_u32x16_masked(x32);
            *row[32] = xor_col32;
            *sub_component_inputs.verify_bitwise_xor_4[1] =
                [ms_4_bits_col26, ms_4_bits_col28, xor_col32];
            *lookup_data.verify_bitwise_xor_4_7 =
                [M31_45448144, ms_4_bits_col26, ms_4_bits_col28, xor_col32];

            let rot12_lo_u = x30 | (x31 << 4);
            let rot12_hi_u = x32 | (x29 << 4);
            let rot12_lo_m31 = PackedM31::from_u32x16_masked(rot12_lo_u);
            let rot12_hi_m31 = PackedM31::from_u32x16_masked(rot12_hi_u);
            let xor_rot_32_r_12_output_tmp_4da06_43 =
                PackedUInt32::from_limbs([rot12_lo_m31, rot12_hi_m31]);

            // Triple Sum 32 (3).
            let triple_sum_32_output_tmp_4da06_47 = triple_sum_32_output_tmp_4da06_3
                + xor_rot_32_r_12_output_tmp_4da06_43
                + blake_g_input[5];
            let triple_sum32_res_limb_0_col33 = triple_sum_32_output_tmp_4da06_47.low().as_m31();
            *row[33] = triple_sum32_res_limb_0_col33;
            let triple_sum32_res_limb_1_col34 = triple_sum_32_output_tmp_4da06_47.high().as_m31();
            *row[34] = triple_sum32_res_limb_1_col34;

            // Xor Rot 32 R 8.
            let ts2_lo = triple_sum32_res_limb_0_col33.into_simd();
            let ts2_hi = triple_sum32_res_limb_1_col34.into_simd();

            let h35 = ts2_lo >> 8;
            let l35 = ts2_lo & mask8;
            let ms_8_bits_col35 = PackedM31::from_u32x16_masked(h35);
            *row[35] = ms_8_bits_col35;
            let ls_8_bits_49 = PackedM31::from_u32x16_masked(l35);

            let h36 = ts2_hi >> 8;
            let l36 = ts2_hi & mask8;
            let ms_8_bits_col36 = PackedM31::from_u32x16_masked(h36);
            *row[36] = ms_8_bits_col36;
            let ls_8_bits_51 = PackedM31::from_u32x16_masked(l36);

            // Note: rot16_lo_u = x21 | (x22 << 8) so its high 8 bits are x22 (xor_col22) and low 8 bits are x21 (xor_col21).
            let h37 = x22;
            let l37 = x21;
            let ms_8_bits_col37 = xor_col22;
            *row[37] = ms_8_bits_col37;
            let ls_8_bits_53 = xor_col21;

            // Note: rot16_hi_u = x19 | (x20 << 8) so its high 8 bits are x20 (xor_col20) and low 8 bits are x19 (xor_col19).
            let h38 = x20;
            let l38 = x19;
            let ms_8_bits_col38 = xor_col20;
            *row[38] = ms_8_bits_col38;
            let ls_8_bits_55 = xor_col19;

            let x39 = l35 ^ l37;
            let xor_col39 = PackedM31::from_u32x16_masked(x39);
            *row[39] = xor_col39;
            *sub_component_inputs.verify_bitwise_xor_8[2] = [ls_8_bits_49, ls_8_bits_53, xor_col39];
            *lookup_data.verify_bitwise_xor_8_8 =
                [M31_112558620, ls_8_bits_49, ls_8_bits_53, xor_col39];

            let x40 = h35 ^ h37;
            let xor_col40 = PackedM31::from_u32x16_masked(x40);
            *row[40] = xor_col40;
            *sub_component_inputs.verify_bitwise_xor_8[3] =
                [ms_8_bits_col35, ms_8_bits_col37, xor_col40];
            *lookup_data.verify_bitwise_xor_8_9 =
                [M31_112558620, ms_8_bits_col35, ms_8_bits_col37, xor_col40];

            let x41 = l36 ^ l38;
            let xor_col41 = PackedM31::from_u32x16_masked(x41);
            *row[41] = xor_col41;
            *sub_component_inputs.verify_bitwise_xor_8_b[2] =
                [ls_8_bits_51, ls_8_bits_55, xor_col41];
            *lookup_data.verify_bitwise_xor_8_b_10 =
                [M31_521092554, ls_8_bits_51, ls_8_bits_55, xor_col41];

            let x42 = h36 ^ h38;
            let xor_col42 = PackedM31::from_u32x16_masked(x42);
            *row[42] = xor_col42;
            *sub_component_inputs.verify_bitwise_xor_8_b[3] =
                [ms_8_bits_col36, ms_8_bits_col38, xor_col42];
            *lookup_data.verify_bitwise_xor_8_b_11 =
                [M31_521092554, ms_8_bits_col36, ms_8_bits_col38, xor_col42];

            let rot8_lo_m31 = PackedM31::from_u32x16_masked(x40 | (x41 << 8));
            let rot8_hi_m31 = PackedM31::from_u32x16_masked(x42 | (x39 << 8));
            let xor_rot_32_r_8_output_tmp_4da06_65 =
                PackedUInt32::from_limbs([rot8_lo_m31, rot8_hi_m31]);

            // Triple Sum 32 (4).
            let triple_sum_32_output_tmp_4da06_69 =
                triple_sum_32_output_tmp_4da06_25 + xor_rot_32_r_8_output_tmp_4da06_65;
            let triple_sum32_res_limb_0_col43 = triple_sum_32_output_tmp_4da06_69.low().as_m31();
            *row[43] = triple_sum32_res_limb_0_col43;
            let triple_sum32_res_limb_1_col44 = triple_sum_32_output_tmp_4da06_69.high().as_m31();
            *row[44] = triple_sum32_res_limb_1_col44;

            // Xor Rot 32 R 7.
            let ts3_lo = triple_sum32_res_limb_0_col43.into_simd();
            let ts3_hi = triple_sum32_res_limb_1_col44.into_simd();

            let h45 = rot12_lo_u >> 7;
            let l45 = rot12_lo_u & mask7;
            let ms_9_bits_col45 = PackedM31::from_u32x16_masked(h45);
            *row[45] = ms_9_bits_col45;
            let ls_7_bits_71 = PackedM31::from_u32x16_masked(l45);

            let h46 = rot12_hi_u >> 7;
            let l46 = rot12_hi_u & mask7;
            let ms_9_bits_col46 = PackedM31::from_u32x16_masked(h46);
            *row[46] = ms_9_bits_col46;
            let ls_7_bits_73 = PackedM31::from_u32x16_masked(l46);

            let h47 = ts3_lo >> 7;
            let l47 = ts3_lo & mask7;
            let ms_9_bits_col47 = PackedM31::from_u32x16_masked(h47);
            *row[47] = ms_9_bits_col47;
            let ls_7_bits_75 = PackedM31::from_u32x16_masked(l47);

            let h48 = ts3_hi >> 7;
            let l48 = ts3_hi & mask7;
            let ms_9_bits_col48 = PackedM31::from_u32x16_masked(h48);
            *row[48] = ms_9_bits_col48;
            let ls_7_bits_77 = PackedM31::from_u32x16_masked(l48);

            let x49 = l45 ^ l47;
            let xor_col49 = PackedM31::from_u32x16_masked(x49);
            *row[49] = xor_col49;
            *sub_component_inputs.verify_bitwise_xor_7[0] = [ls_7_bits_71, ls_7_bits_75, xor_col49];
            *lookup_data.verify_bitwise_xor_7_12 =
                [M31_62225763, ls_7_bits_71, ls_7_bits_75, xor_col49];

            let x50 = h45 ^ h47;
            let xor_col50 = PackedM31::from_u32x16_masked(x50);
            *row[50] = xor_col50;
            *sub_component_inputs.verify_bitwise_xor_9[0] =
                [ms_9_bits_col45, ms_9_bits_col47, xor_col50];
            *lookup_data.verify_bitwise_xor_9_13 =
                [M31_95781001, ms_9_bits_col45, ms_9_bits_col47, xor_col50];

            let x51 = l46 ^ l48;
            let xor_col51 = PackedM31::from_u32x16_masked(x51);
            *row[51] = xor_col51;
            *sub_component_inputs.verify_bitwise_xor_7[1] = [ls_7_bits_73, ls_7_bits_77, xor_col51];
            *lookup_data.verify_bitwise_xor_7_14 =
                [M31_62225763, ls_7_bits_73, ls_7_bits_77, xor_col51];

            let x52 = h46 ^ h48;
            let xor_col52 = PackedM31::from_u32x16_masked(x52);
            *row[52] = xor_col52;
            *sub_component_inputs.verify_bitwise_xor_9[1] =
                [ms_9_bits_col46, ms_9_bits_col48, xor_col52];
            *lookup_data.verify_bitwise_xor_9_15 =
                [M31_95781001, ms_9_bits_col46, ms_9_bits_col48, xor_col52];

            let rot7_lo_m31 = PackedM31::from_u32x16_masked(x50 | (x51 << 9));
            let rot7_hi_m31 = PackedM31::from_u32x16_masked(x52 | (x49 << 9));

            *lookup_data.blake_g_16 = [
                M31_1139985212,
                input_limb_0_col1,
                input_limb_1_col2,
                input_limb_2_col3,
                input_limb_3_col4,
                input_limb_4_col5,
                input_limb_5_col6,
                input_limb_6_col7,
                input_limb_7_col8,
                input_limb_8_col9,
                input_limb_9_col10,
                input_limb_10_col11,
                input_limb_11_col12,
                triple_sum32_res_limb_0_col33,
                triple_sum32_res_limb_1_col34,
                rot7_lo_m31,
                rot7_hi_m31,
                triple_sum32_res_limb_0_col43,
                triple_sum32_res_limb_1_col44,
                rot8_lo_m31,
                rot8_hi_m31,
            ];
            *lookup_data.mults_0 = enabler_col0;
        });

    (trace, lookup_data, sub_component_inputs)
}

#[derive(Uninitialized, IterMut, ParIterMut)]
struct LookupData {
    verify_bitwise_xor_8_0: Vec<[PackedM31; 4]>,
    verify_bitwise_xor_8_1: Vec<[PackedM31; 4]>,
    verify_bitwise_xor_8_b_2: Vec<[PackedM31; 4]>,
    verify_bitwise_xor_8_b_3: Vec<[PackedM31; 4]>,
    verify_bitwise_xor_12_4: Vec<[PackedM31; 4]>,
    verify_bitwise_xor_4_5: Vec<[PackedM31; 4]>,
    verify_bitwise_xor_12_6: Vec<[PackedM31; 4]>,
    verify_bitwise_xor_4_7: Vec<[PackedM31; 4]>,
    verify_bitwise_xor_8_8: Vec<[PackedM31; 4]>,
    verify_bitwise_xor_8_9: Vec<[PackedM31; 4]>,
    verify_bitwise_xor_8_b_10: Vec<[PackedM31; 4]>,
    verify_bitwise_xor_8_b_11: Vec<[PackedM31; 4]>,
    verify_bitwise_xor_7_12: Vec<[PackedM31; 4]>,
    verify_bitwise_xor_9_13: Vec<[PackedM31; 4]>,
    verify_bitwise_xor_7_14: Vec<[PackedM31; 4]>,
    verify_bitwise_xor_9_15: Vec<[PackedM31; 4]>,
    blake_g_16: Vec<[PackedM31; 21]>,
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
            &self.lookup_data.verify_bitwise_xor_8_0,
            &self.lookup_data.verify_bitwise_xor_8_1,
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
            &self.lookup_data.verify_bitwise_xor_8_b_2,
            &self.lookup_data.verify_bitwise_xor_8_b_3,
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
            &self.lookup_data.verify_bitwise_xor_12_4,
            &self.lookup_data.verify_bitwise_xor_4_5,
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
            &self.lookup_data.verify_bitwise_xor_12_6,
            &self.lookup_data.verify_bitwise_xor_4_7,
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
            &self.lookup_data.verify_bitwise_xor_8_8,
            &self.lookup_data.verify_bitwise_xor_8_9,
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
            &self.lookup_data.verify_bitwise_xor_8_b_10,
            &self.lookup_data.verify_bitwise_xor_8_b_11,
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
            &self.lookup_data.verify_bitwise_xor_7_12,
            &self.lookup_data.verify_bitwise_xor_9_13,
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
            &self.lookup_data.verify_bitwise_xor_7_14,
            &self.lookup_data.verify_bitwise_xor_9_15,
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
        (col_gen.par_iter_mut(), &self.lookup_data.blake_g_16, self.lookup_data.mults_0)
            .into_par_iter()
            .for_each(|(writer, values, mult)| {
                let denom = common_lookup_elements.combine(values);
                writer.write_frac((-mult).into(), denom);
            });
        col_gen.finalize_col();

        let (trace, claimed_sum) = logup_gen.finalize_last();

        (trace, InteractionClaim { claimed_sum })
    }
}
