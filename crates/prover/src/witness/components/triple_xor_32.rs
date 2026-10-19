// This file was created by the AIR team.

#![allow(unused_parens)]
use cairo_air::components::triple_xor_32::{Claim, InteractionClaim, N_TRACE_COLUMNS};

use crate::witness::components::verify_bitwise_xor_8;
use crate::witness::prelude::*;

pub type InputType = [UInt32; 3];
pub type PackedInputType = [PackedUInt32; 3];

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

        let (trace, lookup_data, sub_component_inputs) =
            write_trace_simd(packed_inputs, n_active_rows, verify_bitwise_xor_8_state);
        for inputs in sub_component_inputs.verify_bitwise_xor_8 {
            add_inputs(verify_bitwise_xor_8_state, &inputs, n_active_rows, 0);
        }
        for inputs in sub_component_inputs.verify_bitwise_xor_8_b {
            add_inputs(verify_bitwise_xor_8_state, &inputs, n_active_rows, 1);
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
}

#[allow(clippy::useless_conversion)]
#[allow(unused_variables)]
#[allow(clippy::double_parens)]
#[allow(non_snake_case)]
fn write_trace_simd(
    inputs: Vec<PackedInputType>,
    n_rows: usize,
    verify_bitwise_xor_8_state: &verify_bitwise_xor_8::ClaimGenerator,
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
    let M31_521092554 = PackedM31::broadcast(M31::from(521092554));
    let M31_990559919 = PackedM31::broadcast(M31::from(990559919));
    let mask8 = std::simd::u32x16::splat(0xFF);
    let enabler_col = Enabler::new(n_rows);

    (
        trace.par_iter_mut(),
        lookup_data.par_iter_mut(),
        sub_component_inputs.par_iter_mut(),
        inputs.into_par_iter(),
    )
        .into_par_iter()
        .enumerate()
        .for_each(
            |(row_index, (row, lookup_data, sub_component_inputs, triple_xor_32_input))| {
                let enabler_col0 = enabler_col.packed_at(row_index);
                *row[0] = enabler_col0;
                let input_limb_0_col1 = triple_xor_32_input[0].low().as_m31();
                *row[1] = input_limb_0_col1;
                let input_limb_1_col2 = triple_xor_32_input[0].high().as_m31();
                *row[2] = input_limb_1_col2;
                let input_limb_2_col3 = triple_xor_32_input[1].low().as_m31();
                *row[3] = input_limb_2_col3;
                let input_limb_3_col4 = triple_xor_32_input[1].high().as_m31();
                *row[4] = input_limb_3_col4;
                let input_limb_4_col5 = triple_xor_32_input[2].low().as_m31();
                *row[5] = input_limb_4_col5;
                let input_limb_5_col6 = triple_xor_32_input[2].high().as_m31();
                *row[6] = input_limb_5_col6;

                let l0 = input_limb_0_col1.into_simd();
                let l1 = input_limb_1_col2.into_simd();
                let l2 = input_limb_2_col3.into_simd();
                let l3 = input_limb_3_col4.into_simd();
                let l4 = input_limb_4_col5.into_simd();
                let l5 = input_limb_5_col6.into_simd();

                let h0 = l0 >> 8;
                let h1 = l1 >> 8;
                let h2 = l2 >> 8;
                let h3 = l3 >> 8;
                let h4 = l4 >> 8;
                let h5 = l5 >> 8;

                let b0 = l0 & mask8;
                let b1 = l1 & mask8;
                let b2 = l2 & mask8;
                let b3 = l3 & mask8;
                let b4 = l4 & mask8;
                let b5 = l5 & mask8;

                let ms_8_bits_col7 = PackedM31::from_u32x16_masked(h0);
                *row[7] = ms_8_bits_col7;
                let ls_8_bits_0 = PackedM31::from_u32x16_masked(b0);

                let ms_8_bits_col8 = PackedM31::from_u32x16_masked(h1);
                *row[8] = ms_8_bits_col8;
                let ls_8_bits_1 = PackedM31::from_u32x16_masked(b1);

                let ms_8_bits_col9 = PackedM31::from_u32x16_masked(h2);
                *row[9] = ms_8_bits_col9;
                let ls_8_bits_2 = PackedM31::from_u32x16_masked(b2);

                let ms_8_bits_col10 = PackedM31::from_u32x16_masked(h3);
                *row[10] = ms_8_bits_col10;
                let ls_8_bits_3 = PackedM31::from_u32x16_masked(b3);

                let ms_8_bits_col11 = PackedM31::from_u32x16_masked(h4);
                *row[11] = ms_8_bits_col11;
                let ls_8_bits_4 = PackedM31::from_u32x16_masked(b4);

                let ms_8_bits_col12 = PackedM31::from_u32x16_masked(h5);
                *row[12] = ms_8_bits_col12;
                let ls_8_bits_5 = PackedM31::from_u32x16_masked(b5);

                let x13 = b0 ^ b2;
                let xor_col13 = PackedM31::from_u32x16_masked(x13);
                *row[13] = xor_col13;
                *sub_component_inputs.verify_bitwise_xor_8[0] = [ls_8_bits_0, ls_8_bits_2, xor_col13];
                *lookup_data.verify_bitwise_xor_8_0 =
                    [M31_112558620, ls_8_bits_0, ls_8_bits_2, xor_col13];

                let x14 = x13 ^ b4;
                let xor_col14 = PackedM31::from_u32x16_masked(x14);
                *row[14] = xor_col14;
                *sub_component_inputs.verify_bitwise_xor_8[1] = [xor_col13, ls_8_bits_4, xor_col14];
                *lookup_data.verify_bitwise_xor_8_1 =
                    [M31_112558620, xor_col13, ls_8_bits_4, xor_col14];

                let x15 = h0 ^ h2;
                let xor_col15 = PackedM31::from_u32x16_masked(x15);
                *row[15] = xor_col15;
                *sub_component_inputs.verify_bitwise_xor_8[2] =
                    [ms_8_bits_col7, ms_8_bits_col9, xor_col15];
                *lookup_data.verify_bitwise_xor_8_2 =
                    [M31_112558620, ms_8_bits_col7, ms_8_bits_col9, xor_col15];

                let x16 = x15 ^ h4;
                let xor_col16 = PackedM31::from_u32x16_masked(x16);
                *row[16] = xor_col16;
                *sub_component_inputs.verify_bitwise_xor_8[3] =
                    [xor_col15, ms_8_bits_col11, xor_col16];
                *lookup_data.verify_bitwise_xor_8_3 =
                    [M31_112558620, xor_col15, ms_8_bits_col11, xor_col16];

                let x17 = b1 ^ b3;
                let xor_col17 = PackedM31::from_u32x16_masked(x17);
                *row[17] = xor_col17;
                *sub_component_inputs.verify_bitwise_xor_8_b[0] =
                    [ls_8_bits_1, ls_8_bits_3, xor_col17];
                *lookup_data.verify_bitwise_xor_8_b_4 =
                    [M31_521092554, ls_8_bits_1, ls_8_bits_3, xor_col17];

                let x18 = x17 ^ b5;
                let xor_col18 = PackedM31::from_u32x16_masked(x18);
                *row[18] = xor_col18;
                *sub_component_inputs.verify_bitwise_xor_8_b[1] =
                    [xor_col17, ls_8_bits_5, xor_col18];
                *lookup_data.verify_bitwise_xor_8_b_5 =
                    [M31_521092554, xor_col17, ls_8_bits_5, xor_col18];

                let x19 = h1 ^ h3;
                let xor_col19 = PackedM31::from_u32x16_masked(x19);
                *row[19] = xor_col19;
                *sub_component_inputs.verify_bitwise_xor_8_b[2] =
                    [ms_8_bits_col8, ms_8_bits_col10, xor_col19];
                *lookup_data.verify_bitwise_xor_8_b_6 =
                    [M31_521092554, ms_8_bits_col8, ms_8_bits_col10, xor_col19];

                let x20 = x19 ^ h5;
                let xor_col20 = PackedM31::from_u32x16_masked(x20);
                *row[20] = xor_col20;
                *sub_component_inputs.verify_bitwise_xor_8_b[3] =
                    [xor_col19, ms_8_bits_col12, xor_col20];
                *lookup_data.verify_bitwise_xor_8_b_7 =
                    [M31_521092554, xor_col19, ms_8_bits_col12, xor_col20];

                let out_low = PackedM31::from_u32x16_masked(x14 | (x16 << 8));
                let out_high = PackedM31::from_u32x16_masked(x18 | (x20 << 8));
                *lookup_data.triple_xor_32_8 = [
                    M31_990559919,
                    input_limb_0_col1,
                    input_limb_1_col2,
                    input_limb_2_col3,
                    input_limb_3_col4,
                    input_limb_4_col5,
                    input_limb_5_col6,
                    out_low,
                    out_high,
                ];
                *lookup_data.mults_0 = enabler_col0;
            },
        );

    (trace, lookup_data, sub_component_inputs)
}

#[derive(Uninitialized, IterMut, ParIterMut)]
struct LookupData {
    verify_bitwise_xor_8_0: Vec<[PackedM31; 4]>,
    verify_bitwise_xor_8_1: Vec<[PackedM31; 4]>,
    verify_bitwise_xor_8_2: Vec<[PackedM31; 4]>,
    verify_bitwise_xor_8_3: Vec<[PackedM31; 4]>,
    verify_bitwise_xor_8_b_4: Vec<[PackedM31; 4]>,
    verify_bitwise_xor_8_b_5: Vec<[PackedM31; 4]>,
    verify_bitwise_xor_8_b_6: Vec<[PackedM31; 4]>,
    verify_bitwise_xor_8_b_7: Vec<[PackedM31; 4]>,
    triple_xor_32_8: Vec<[PackedM31; 9]>,
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
            &self.lookup_data.verify_bitwise_xor_8_2,
            &self.lookup_data.verify_bitwise_xor_8_3,
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
            &self.lookup_data.verify_bitwise_xor_8_b_4,
            &self.lookup_data.verify_bitwise_xor_8_b_5,
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
            &self.lookup_data.verify_bitwise_xor_8_b_6,
            &self.lookup_data.verify_bitwise_xor_8_b_7,
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
        (col_gen.par_iter_mut(), &self.lookup_data.triple_xor_32_8, self.lookup_data.mults_0)
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
