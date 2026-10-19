#![allow(unused_parens)]
#![allow(clippy::too_many_arguments)]
use crate::circuit_air::components::triple_xor::{Claim, InteractionClaim, N_TRACE_COLUMNS};
use crate::witness::components::prelude::*;
use crate::witness::components::verify_bitwise_xor_8;

pub type InputType = [UInt32; 4];
pub type PackedInputType = [PackedUInt32; 4];

/// Retrieves the component's inputs from the context values, using the addresses provided in the
/// preprocessed trace.
#[allow(clippy::uninit_vec)]
pub fn extract_component_inputs(
    input_addr_col_0: &[usize],
    input_addr_col_1: &[usize],
    input_addr_col_2: &[usize],
    output_addr_col: &[usize],
    context_values: &[QM31],
) -> Vec<InputType> {
    let n_rows = input_addr_col_0.len();
    assert_eq!(n_rows, input_addr_col_1.len());
    assert_eq!(n_rows, input_addr_col_2.len());
    assert_eq!(n_rows, output_addr_col.len());

    (
        input_addr_col_0.par_iter(),
        input_addr_col_1.par_iter(),
        input_addr_col_2.par_iter(),
        output_addr_col.par_iter(),
    )
        .into_par_iter()
        .map(|(&addr_0, &addr_1, &addr_2, &out_addr)| {
            let qm31_in_addr_0 = context_values[addr_0].to_m31_array();
            let qm31_in_addr_1 = context_values[addr_1].to_m31_array();
            let qm31_in_addr_2 = context_values[addr_2].to_m31_array();
            let qm31_out_addr = context_values[out_addr].to_m31_array();
            [
                UInt32::from(qm31_in_addr_0[0].0 | (qm31_in_addr_0[1].0 << 16)),
                UInt32::from(qm31_in_addr_1[0].0 | (qm31_in_addr_1[1].0 << 16)),
                UInt32::from(qm31_in_addr_2[0].0 | (qm31_in_addr_2[1].0 << 16)),
                UInt32::from(qm31_out_addr[0].0 | (qm31_out_addr[1].0 << 16)),
            ]
        })
        .collect()
}

pub fn write_trace(
    context_values: &[QM31],
    preprocessed_trace: &PreProcessedTrace,
    verify_bitwise_xor_8_state: &verify_bitwise_xor_8::ClaimGenerator,
) -> (ComponentTrace<N_TRACE_COLUMNS>, Claim, InteractionClaimGenerator) {
    // The inputs are gathered per packed row straight from the context values at the
    // preprocessed addresses, read in place: no staged inputs, no packed copy of them and no
    // materialized address columns.
    let columns = [
        "triple_xor_input_addr_0",
        "triple_xor_input_addr_1",
        "triple_xor_input_addr_2",
        "triple_xor_output_addr",
        "triple_xor_multiplicity",
    ]
    .map(|id| preprocessed_trace.column_rows(&PreProcessedColumnId { id: id.to_owned() }));

    let n_rows = columns[0].len();
    assert!(columns[..4].iter().all(|column| column.len() == n_rows));
    assert_eq!(columns[4].len(), n_rows);
    assert_ne!(n_rows, 0);
    assert!(n_rows.is_multiple_of(N_LANES));
    let size = std::cmp::max(n_rows.next_power_of_two(), N_LANES);
    let log_size = size.ilog2();

    let (trace, lookup_data) = write_trace_simd(n_rows, context_values, &columns);
    // The xor_8 rows are counted per task straight from the lookup tuples, the eight columns in
    // one pass (see `count_table_rows`), instead of from a staged copy of them with atomic adds
    // (omarespejel, rgr1_3f4875e7).
    let d = &lookup_data;
    verify_bitwise_xor_8_state.add_counted_inputs(
        &[
            (&d.verify_bitwise_xor_8_0[..], 0),
            (&d.verify_bitwise_xor_8_1[..], 0),
            (&d.verify_bitwise_xor_8_2[..], 0),
            (&d.verify_bitwise_xor_8_3[..], 0),
            (&d.verify_bitwise_xor_8_4[..], 0),
            (&d.verify_bitwise_xor_8_5[..], 0),
            (&d.verify_bitwise_xor_8_6[..], 0),
            (&d.verify_bitwise_xor_8_7[..], 0),
        ],
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
    columns: &[circuit_common::preprocessed::ColumnRows<'_>; 5],
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
    let mask8 = u32x16::splat(0xFF);

    (trace.par_iter_mut(), lookup_data.par_iter_mut())
        .into_par_iter()
        .enumerate()
        .for_each(|(row_index, (row, lookup_data))| {
            let first = row_index * N_LANES;
            let addr_arrays: [[u32; N_LANES]; 4] =
                from_fn(|i| columns[i].packed_m31_at(first).into_simd().to_array());
            let mults = columns[4].packed_m31_at(first);
            let w: [u32x16; 4] = from_fn(|i| {
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
            let out_lo = w[3] & mask16;
            let out_hi = w[3] >> 16;

            *row[0] = simd_to_m31(a_lo);
            *row[1] = simd_to_m31(a_hi);
            *row[2] = simd_to_m31(b_lo);
            *row[3] = simd_to_m31(b_hi);
            *row[4] = simd_to_m31(c_lo);
            *row[5] = simd_to_m31(c_hi);
            *row[6] = simd_to_m31(out_lo);
            *row[7] = simd_to_m31(out_hi);

            let col8 = a_lo >> 8;
            let a_lo_8 = a_lo & mask8;
            *row[8] = simd_to_m31(col8);

            let col9 = a_hi >> 8;
            let a_hi_8 = a_hi & mask8;
            *row[9] = simd_to_m31(col9);

            let col10 = b_lo >> 8;
            let b_lo_8 = b_lo & mask8;
            *row[10] = simd_to_m31(col10);

            let col11 = b_hi >> 8;
            let b_hi_8 = b_hi & mask8;
            *row[11] = simd_to_m31(col11);

            let col12 = c_lo >> 8;
            let c_lo_8 = c_lo & mask8;
            *row[12] = simd_to_m31(col12);

            let col13 = c_hi >> 8;
            let c_hi_8 = c_hi & mask8;
            *row[13] = simd_to_m31(col13);

            let col14 = out_lo >> 8;
            let out_lo_8 = out_lo & mask8;
            *row[14] = simd_to_m31(col14);

            let col15 = out_hi >> 8;
            let out_hi_8 = out_hi & mask8;
            *row[15] = simd_to_m31(col15);

            let col16 = a_lo_8 ^ b_lo_8;
            *row[16] = simd_to_m31(col16);
            *lookup_data.verify_bitwise_xor_8_0 = NarrowTriple::new([a_lo_8, b_lo_8, col16]);

            let col17 = col8 ^ col10;
            *row[17] = simd_to_m31(col17);
            *lookup_data.verify_bitwise_xor_8_1 = NarrowTriple::new([col8, col10, col17]);

            let col18 = a_hi_8 ^ b_hi_8;
            *row[18] = simd_to_m31(col18);
            *lookup_data.verify_bitwise_xor_8_2 = NarrowTriple::new([a_hi_8, b_hi_8, col18]);

            let col19 = col9 ^ col11;
            *row[19] = simd_to_m31(col19);
            *lookup_data.verify_bitwise_xor_8_3 = NarrowTriple::new([col9, col11, col19]);
            *lookup_data.verify_bitwise_xor_8_4 = NarrowTriple::new([col16, c_lo_8, out_lo_8]);
            *lookup_data.verify_bitwise_xor_8_5 = NarrowTriple::new([col17, col12, col14]);
            *lookup_data.verify_bitwise_xor_8_6 = NarrowTriple::new([col18, c_hi_8, out_hi_8]);
            *lookup_data.verify_bitwise_xor_8_7 = NarrowTriple::new([col19, col13, col15]);

            *lookup_data.gate_0 = NarrowGate::new(addr_arrays[0], a_lo, a_hi);
            *lookup_data.gate_1 = NarrowGate::new(addr_arrays[1], b_lo, b_hi);
            *lookup_data.gate_2 = NarrowGate::new(addr_arrays[2], c_lo, c_hi);
            *lookup_data.gate_3 = NarrowGate::new(addr_arrays[3], out_lo, out_hi);
            *lookup_data.mults_0 = mults;
        });
    (trace, lookup_data)
}

#[derive(Uninitialized, IterMut, ParIterMut)]
struct LookupData {
    gate_0: Vec<NarrowGate>,
    gate_1: Vec<NarrowGate>,
    gate_2: Vec<NarrowGate>,
    gate_3: Vec<NarrowGate>,
    verify_bitwise_xor_8_0: Vec<NarrowTriple>,
    verify_bitwise_xor_8_1: Vec<NarrowTriple>,
    verify_bitwise_xor_8_2: Vec<NarrowTriple>,
    verify_bitwise_xor_8_3: Vec<NarrowTriple>,
    verify_bitwise_xor_8_4: Vec<NarrowTriple>,
    verify_bitwise_xor_8_5: Vec<NarrowTriple>,
    verify_bitwise_xor_8_6: Vec<NarrowTriple>,
    verify_bitwise_xor_8_7: Vec<NarrowTriple>,
    mults_0: Vec<PackedM31>,
}

#[derive(Clone, Copy)]
struct NarrowGate([u32; N_LANES], [[u16; N_LANES]; 2]);

#[derive(Clone, Copy)]
struct NarrowTriple([[u16; N_LANES]; 3]);

#[inline(always)]
fn simd_to_m31(value: u32x16) -> PackedM31 {
    unsafe { PackedM31::from_simd_unchecked(value) }
}

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

        for column in 0..6 {
            let lookup_data_ref = &lookup_data;
            logup_gen.write_frac_cols(1, |_, row| {
                let lookup_data = lookup_data_ref;
                let (values0, values1, tag) = match column {
                    0 => (
                        lookup_data.verify_bitwise_xor_8_0[row].widen(),
                        lookup_data.verify_bitwise_xor_8_1[row].widen(),
                        relations::VERIFY_BITWISE_XOR_8_RELATION_ID,
                    ),
                    1 => (
                        lookup_data.verify_bitwise_xor_8_2[row].widen(),
                        lookup_data.verify_bitwise_xor_8_3[row].widen(),
                        relations::VERIFY_BITWISE_XOR_8_RELATION_ID,
                    ),
                    2 => (
                        lookup_data.verify_bitwise_xor_8_4[row].widen(),
                        lookup_data.verify_bitwise_xor_8_5[row].widen(),
                        relations::VERIFY_BITWISE_XOR_8_RELATION_ID,
                    ),
                    3 => (
                        lookup_data.verify_bitwise_xor_8_6[row].widen(),
                        lookup_data.verify_bitwise_xor_8_7[row].widen(),
                        relations::VERIFY_BITWISE_XOR_8_RELATION_ID,
                    ),
                    4 => (
                        lookup_data.gate_0[row].widen(),
                        lookup_data.gate_1[row].widen(),
                        relations::GATE_RELATION_ID,
                    ),
                    5 => (
                        lookup_data.gate_2[row].widen(),
                        lookup_data.gate_3[row].widen(),
                        relations::GATE_RELATION_ID,
                    ),
                    _ => unreachable!(),
                };
                let tag_m31 = PackedM31::broadcast(M31::from(tag));
                let denom0: PackedQM31 =
                    common_lookup_elements.combine(&[tag_m31, values0[0], values0[1], values0[2]]);
                let denom1: PackedQM31 =
                    common_lookup_elements.combine(&[tag_m31, values1[0], values1[1], values1[2]]);
                let numerator = if column < 5 {
                    denom0 + denom1
                } else {
                    denom1 - denom0 * lookup_data.mults_0[row]
                };
                (numerator, denom0 * denom1)
            });
            if column < 4 {
                let released = match column {
                    0 => [&mut lookup_data.verify_bitwise_xor_8_0, &mut lookup_data.verify_bitwise_xor_8_1],
                    1 => [&mut lookup_data.verify_bitwise_xor_8_2, &mut lookup_data.verify_bitwise_xor_8_3],
                    2 => [&mut lookup_data.verify_bitwise_xor_8_4, &mut lookup_data.verify_bitwise_xor_8_5],
                    _ => [&mut lookup_data.verify_bitwise_xor_8_6, &mut lookup_data.verify_bitwise_xor_8_7],
                };
                for values in released {
                    drop(std::mem::take(values));
                }
            } else {
                let released = match column {
                    4 => [&mut lookup_data.gate_0, &mut lookup_data.gate_1],
                    _ => [&mut lookup_data.gate_2, &mut lookup_data.gate_3],
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
