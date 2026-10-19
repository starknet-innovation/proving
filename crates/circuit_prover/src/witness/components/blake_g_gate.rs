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

    (
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
        .map(
            |(
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
                [
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
                ]
            },
        )
        .collect()
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

    let (trace, lookup_data) = write_trace_simd(n_rows, context_values, &columns);
    // The small xor tables count their rows per task from the narrow tuples, every column of a
    // table in one pass (see `count_table_rows`); the 2^24-row xor_12 table keeps its atomic adds,
    // from the narrow tuples widened in turn.
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
    verify_bitwise_xor_12_state.add_widened_inputs(
        [&d.verify_bitwise_xor_12_0[..], &d.verify_bitwise_xor_12_1[..]],
        Widen::widen,
    );
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
    context_values: &[QM31],
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

    let mask16 = u32x16::splat(0xFFFF);
    let mask12 = u32x16::splat(0xFFF);
    let mask9 = u32x16::splat(0x1FF);
    let mask8 = u32x16::splat(0xFF);
    let mask7 = u32x16::splat(0x7F);

    (trace.par_iter_mut(), lookup_data.par_iter_mut())
        .into_par_iter()
        .enumerate()
        .for_each(|(row_index, (row, lookup_data))| {
            let first = row_index * N_LANES;
            let addr_arrays: [[u32; N_LANES]; 10] =
                from_fn(|i| columns[i].packed_m31_at(first).into_simd().to_array());
            let blake_g_gate_multiplicity = columns[10].packed_m31_at(first);
            let w: [u32x16; 10] = from_fn(|i| {
                u32x16::from_array(addr_arrays[i].map(|addr| {
                    let value = context_values[addr as usize].0;
                    value.0.0 | (value.1.0 << 16)
                }))
            });

            let a_lo = w[0] & mask16;
            let a_hi = w[0] >> 16;
            let b_lo = w[1] & mask16;
            let b_hi = w[1] >> 16;
            let c_lo = w[2] & mask16;
            let c_hi = w[2] >> 16;
            let d_lo = w[3] & mask16;
            let d_hi = w[3] >> 16;
            let f0_lo = w[4] & mask16;
            let f0_hi = w[4] >> 16;
            let f1_lo = w[5] & mask16;
            let f1_hi = w[5] >> 16;
            let a_tag_lo = w[6] & mask16;
            let a_tag_hi = w[6] >> 16;
            let b_tag_lo = w[7] & mask16;
            let b_tag_hi = w[7] >> 16;
            let c_tag_lo = w[8] & mask16;
            let c_tag_hi = w[8] >> 16;
            let d_tag_lo = w[9] & mask16;
            let d_tag_hi = w[9] >> 16;

            *row[0] = simd_to_m31(a_lo);
            *row[1] = simd_to_m31(a_hi);
            *row[2] = simd_to_m31(b_lo);
            *row[3] = simd_to_m31(b_hi);
            *row[4] = simd_to_m31(c_lo);
            *row[5] = simd_to_m31(c_hi);
            *row[6] = simd_to_m31(d_lo);
            *row[7] = simd_to_m31(d_hi);
            *row[8] = simd_to_m31(f0_lo);
            *row[9] = simd_to_m31(f0_hi);
            *row[10] = simd_to_m31(f1_lo);
            *row[11] = simd_to_m31(f1_hi);
            *row[12] = simd_to_m31(a_tag_lo);
            *row[13] = simd_to_m31(a_tag_hi);
            *row[14] = simd_to_m31(b_tag_lo);
            *row[15] = simd_to_m31(b_tag_hi);
            *row[16] = simd_to_m31(c_tag_lo);
            *row[17] = simd_to_m31(c_tag_hi);
            *row[18] = simd_to_m31(d_tag_lo);
            *row[19] = simd_to_m31(d_tag_hi);

            // Triple Sum 32.
            let sum0 = w[0] + w[1] + w[4];
            let sum0_lo = sum0 & mask16;
            let sum0_hi = sum0 >> 16;
            *row[20] = simd_to_m31(sum0_lo);
            *row[21] = simd_to_m31(sum0_hi);

            // Xor Rot 32 R 16.
            let col22 = sum0_lo >> 8;
            let s0_lo_8 = sum0_lo & mask8;
            *row[22] = simd_to_m31(col22);

            let col23 = sum0_hi >> 8;
            let s0_hi_8 = sum0_hi & mask8;
            *row[23] = simd_to_m31(col23);

            let col24 = d_lo >> 8;
            let d_lo_8 = d_lo & mask8;
            *row[24] = simd_to_m31(col24);

            let col25 = d_hi >> 8;
            let d_hi_8 = d_hi & mask8;
            *row[25] = simd_to_m31(col25);

            let col26 = s0_lo_8 ^ d_lo_8;
            *row[26] = simd_to_m31(col26);
            *lookup_data.verify_bitwise_xor_8_0 = NarrowTriple::new([s0_lo_8, d_lo_8, col26]);

            let col27 = col22 ^ col24;
            *row[27] = simd_to_m31(col27);
            *lookup_data.verify_bitwise_xor_8_1 = NarrowTriple::new([col22, col24, col27]);

            let col28 = s0_hi_8 ^ d_hi_8;
            *row[28] = simd_to_m31(col28);
            *lookup_data.verify_bitwise_xor_8_b_0 = NarrowTriple::new([s0_hi_8, d_hi_8, col28]);

            let col29 = col23 ^ col25;
            *row[29] = simd_to_m31(col29);
            *lookup_data.verify_bitwise_xor_8_b_1 = NarrowTriple::new([col23, col25, col29]);

            let rot16_lo = col28 | (col29 << 8);
            let rot16_hi = col26 | (col27 << 8);
            let rot16 = rot16_lo | (rot16_hi << 16);

            // Triple Sum 32.
            let sum1 = w[2] + rot16;
            let sum1_lo = sum1 & mask16;
            let sum1_hi = sum1 >> 16;
            *row[30] = simd_to_m31(sum1_lo);
            *row[31] = simd_to_m31(sum1_hi);

            // Xor Rot 32 R 12.
            let col32 = b_lo >> 12;
            let b_lo_12 = b_lo & mask12;
            *row[32] = simd_to_m31(col32);

            let col33 = b_hi >> 12;
            let b_hi_12 = b_hi & mask12;
            *row[33] = simd_to_m31(col33);

            let col34 = sum1_lo >> 12;
            let s1_lo_12 = sum1_lo & mask12;
            *row[34] = simd_to_m31(col34);

            let col35 = sum1_hi >> 12;
            let s1_hi_12 = sum1_hi & mask12;
            *row[35] = simd_to_m31(col35);

            let col36 = b_lo_12 ^ s1_lo_12;
            *row[36] = simd_to_m31(col36);
            *lookup_data.verify_bitwise_xor_12_0 = NarrowTriple::new([b_lo_12, s1_lo_12, col36]);

            let col37 = col32 ^ col34;
            *row[37] = simd_to_m31(col37);
            *lookup_data.verify_bitwise_xor_4_0 = NarrowTriple::new([col32, col34, col37]);

            let col38 = b_hi_12 ^ s1_hi_12;
            *row[38] = simd_to_m31(col38);
            *lookup_data.verify_bitwise_xor_12_1 = NarrowTriple::new([b_hi_12, s1_hi_12, col38]);

            let col39 = col33 ^ col35;
            *row[39] = simd_to_m31(col39);
            *lookup_data.verify_bitwise_xor_4_1 = NarrowTriple::new([col33, col35, col39]);

            let rot12_lo = col37 | (col38 << 4);
            let rot12_hi = col39 | (col36 << 4);

            // Verify Xor Rot 32 R 8.
            let col40 = a_tag_lo >> 8;
            let a_tag_lo_8 = a_tag_lo & mask8;
            *row[40] = simd_to_m31(col40);

            let col41 = a_tag_hi >> 8;
            let a_tag_hi_8 = a_tag_hi & mask8;
            *row[41] = simd_to_m31(col41);

            let col42 = col29;
            let rot16_lo_8 = col28;
            *row[42] = simd_to_m31(col42);

            let col43 = col27;
            let rot16_hi_8 = col26;
            *row[43] = simd_to_m31(col43);

            let col44 = d_tag_lo >> 8;
            let d_tag_lo_8 = d_tag_lo & mask8;
            *row[44] = simd_to_m31(col44);

            let col45 = d_tag_hi >> 8;
            let d_tag_hi_8 = d_tag_hi & mask8;
            *row[45] = simd_to_m31(col45);

            *lookup_data.verify_bitwise_xor_8_2 = NarrowTriple::new([col40, col42, d_tag_lo_8]);
            *lookup_data.verify_bitwise_xor_8_3 = NarrowTriple::new([a_tag_hi_8, rot16_hi_8, col44]);
            *lookup_data.verify_bitwise_xor_8_4 = NarrowTriple::new([col41, col43, d_tag_hi_8]);
            *lookup_data.verify_bitwise_xor_8_5 = NarrowTriple::new([a_tag_lo_8, rot16_lo_8, col45]);

            // Verify Xor Rot 32 R 7.
            let col46 = rot12_lo >> 7;
            let rot12_lo_7 = rot12_lo & mask7;
            *row[46] = simd_to_m31(col46);

            let col47 = rot12_hi >> 7;
            let rot12_hi_7 = rot12_hi & mask7;
            *row[47] = simd_to_m31(col47);

            let col48 = c_tag_lo >> 7;
            let c_tag_lo_7 = c_tag_lo & mask7;
            *row[48] = simd_to_m31(col48);

            let col49 = c_tag_hi >> 7;
            let c_tag_hi_7 = c_tag_hi & mask7;
            *row[49] = simd_to_m31(col49);

            let col50 = b_tag_lo >> 9;
            let b_tag_lo_9 = b_tag_lo & mask9;
            *row[50] = simd_to_m31(col50);

            let col51 = b_tag_hi >> 9;
            let b_tag_hi_9 = b_tag_hi & mask9;
            *row[51] = simd_to_m31(col51);

            *lookup_data.verify_bitwise_xor_9_0 = NarrowTriple::new([col46, col48, b_tag_lo_9]);
            *lookup_data.verify_bitwise_xor_7_0 = NarrowTriple::new([rot12_hi_7, c_tag_hi_7, col50]);
            *lookup_data.verify_bitwise_xor_9_1 = NarrowTriple::new([col47, col49, b_tag_hi_9]);
            *lookup_data.verify_bitwise_xor_7_1 = NarrowTriple::new([rot12_lo_7, c_tag_lo_7, col51]);

            *lookup_data.gate_0 = NarrowGate::new(addr_arrays[0], a_lo, a_hi);
            *lookup_data.gate_1 = NarrowGate::new(addr_arrays[1], b_lo, b_hi);
            *lookup_data.gate_2 = NarrowGate::new(addr_arrays[2], c_lo, c_hi);
            *lookup_data.gate_3 = NarrowGate::new(addr_arrays[3], d_lo, d_hi);
            *lookup_data.gate_4 = NarrowGate::new(addr_arrays[4], f0_lo, f0_hi);
            *lookup_data.gate_5 = NarrowGate::new(addr_arrays[5], f1_lo, f1_hi);
            *lookup_data.gate_6 = NarrowGate::new(addr_arrays[6], a_tag_lo, a_tag_hi);
            *lookup_data.gate_7 = NarrowGate::new(addr_arrays[7], b_tag_lo, b_tag_hi);
            *lookup_data.gate_8 = NarrowGate::new(addr_arrays[8], c_tag_lo, c_tag_hi);
            *lookup_data.gate_9 = NarrowGate::new(addr_arrays[9], d_tag_lo, d_tag_hi);
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

/// Wraps a `u32x16` whose lanes are already in `0..P` as a [`PackedM31`].
#[inline(always)]
fn simd_to_m31(value: u32x16) -> PackedM31 {
    unsafe { PackedM31::from_simd_unchecked(value) }
}

/// The lanes of `value`, which are below 2^16.
#[inline(always)]
fn narrow_lanes(value: u32x16) -> [u16; N_LANES] {
    value.cast::<u16>().to_array()
}

#[inline(always)]
fn widen_lanes(lanes: [u16; N_LANES]) -> PackedM31 {
    simd_to_m31(Simd::from_array(lanes).cast())
}

impl NarrowGate {
    #[inline(always)]
    fn new(address: [u32; N_LANES], low: u32x16, high: u32x16) -> Self {
        Self(address, [narrow_lanes(low), narrow_lanes(high)])
    }
}

impl NarrowTriple {
    #[inline(always)]
    fn new(values: [u32x16; 3]) -> Self {
        Self(values.map(narrow_lanes))
    }
}

/// The tuple as it was written.
trait Widen {
    fn widen(&self) -> [PackedM31; 3];
}

impl Widen for NarrowGate {
    #[inline(always)]
    fn widen(&self) -> [PackedM31; 3] {
        [simd_to_m31(Simd::from_array(self.0)), widen_lanes(self.1[0]), widen_lanes(self.1[1])]
    }
}

impl Widen for NarrowTriple {
    #[inline(always)]
    fn widen(&self) -> [PackedM31; 3] {
        self.0.map(widen_lanes)
    }
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
