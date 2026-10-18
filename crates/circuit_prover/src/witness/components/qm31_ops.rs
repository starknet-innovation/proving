use num_traits::Zero;
use rayon::slice::{ParallelSlice, ParallelSliceMut};

use circuit_common::preprocessed::ColumnRows;

use crate::circuit_air::components::qm_31_ops::N_TRACE_COLUMNS;
use crate::witness::components::prelude::*;

pub type InputType = [[M31; 4]; 3];
pub type PackedInputType = [[PackedM31; 4]; 3];

/// Retrieves the component's inputs from the context values, using the addresses provided in the
/// preprocessed trace.
#[allow(clippy::uninit_vec)]
pub fn extract_component_inputs(
    in0_address: &[usize],
    in1_address: &[usize],
    out_address: &[usize],
    context_values: &[QM31],
    trace_generator: &Qm31OpsTraceGenerator,
) -> Vec<InputType> {
    let first_permutation_row = trace_generator.first_permutation_row;
    let n_total_rows = in0_address.len();
    assert_eq!(n_total_rows, in1_address.len());
    assert_eq!(n_total_rows, out_address.len());
    assert!(first_permutation_row <= n_total_rows);

    let mut inputs = Vec::with_capacity(n_total_rows);
    unsafe {
        inputs.set_len(n_total_rows);
    }

    // Handle non-permutation and permutation rows in parallel.
    let (non_perm_inputs, perm_inputs) = inputs.split_at_mut(first_permutation_row);
    let (non_perm_in0, _) = in0_address.split_at(first_permutation_row);
    let (non_perm_in1, perm_in1) = in1_address.split_at(first_permutation_row);
    let (non_perm_out, perm_out) = out_address.split_at(first_permutation_row);

    rayon::join(
        || {
            // Handle non-permutation rows.
            (non_perm_inputs, non_perm_in0, non_perm_in1, non_perm_out).into_par_iter().for_each(
                |(input, in0_address, in1_address, out_address)| {
                    *input = [in0_address, in1_address, out_address]
                        .map(|address| context_values[*address].to_m31_array());
                },
            );
        },
        || {
            // Handle permutation rows in chunks of 2. The in0 address is always 0.
            let zero = QM31::zero().to_m31_array();
            (perm_inputs.par_chunks_mut(2), perm_in1.par_chunks(2), perm_out.par_chunks(2))
                .into_par_iter()
                .for_each(|(input_chunk, in1_chunk, out_chunk)| {
                    let input_value = context_values[in1_chunk[0]].to_m31_array();
                    input_chunk[0] = [zero, input_value, input_value];

                    let output_value = context_values[out_chunk[1]].to_m31_array();
                    input_chunk[1] = [zero, output_value, output_value];
                });
        },
    );

    inputs
}

pub fn write_trace(
    context_values: &[QM31],
    preprocessed_trace: &PreProcessedTrace,
    trace_generator: &Qm31OpsTraceGenerator,
) -> (ComponentTrace<N_TRACE_COLUMNS>, ComponentLogSize) {
    // Only the addresses are read, in place: the multiplicities are read by the interaction trace
    // and the four flag columns are not part of this component's base trace (its AIR reads them
    // from the preprocessed tree).
    let column = |id: &str| preprocessed_trace.column_rows(&PreProcessedColumnId { id: id.to_owned() });
    let in0_address = column("qm31_ops_in0_address");
    let in1_address = column("qm31_ops_in1_address");
    let out_address = column("qm31_ops_out_address");

    let n_rows = in0_address.len();
    assert_eq!(n_rows, in1_address.len());
    assert_eq!(n_rows, out_address.len());
    assert_ne!(n_rows, 0);
    assert!(n_rows >= N_LANES);
    assert!(n_rows.is_power_of_two());
    let first_permutation_row = trace_generator.first_permutation_row;
    assert!(first_permutation_row <= n_rows);
    // The permutation rows come in pairs (see `extract_component_inputs`).
    assert!((n_rows - first_permutation_row).is_multiple_of(2));
    let log_size = n_rows.ilog2();

    let trace = write_trace_simd(
        context_values,
        [&in0_address, &in1_address, &out_address],
        first_permutation_row,
    );

    (trace, log_size)
}

/// Writes the trace straight from the context values: each packed row gathers the operands of its
/// rows exactly as [`extract_component_inputs`] would, packs them as `pack_values` would, and
/// writes them in place, instead of collecting every row's operands into a vector and packing it
/// into another (two copies of the 2^23-row component, 384 MiB each). The addresses are read from
/// the preprocessed columns in place (omarespejel's in-place preprocessed reads, rgr1_3f4875e7).
fn write_trace_simd(
    context_values: &[QM31],
    [in0_address, in1_address, out_address]: [&ColumnRows<'_>; 3],
    first_permutation_row: usize,
) -> ComponentTrace<N_TRACE_COLUMNS> {
    let log_n_packed_rows = (in0_address.len() / N_LANES).ilog2();
    let log_size = log_n_packed_rows + LOG_N_LANES;
    let mut trace = unsafe { ComponentTrace::<N_TRACE_COLUMNS>::uninitialized(log_size) };

    let zero = QM31::zero().to_m31_array();
    // The operands of a row, as `extract_component_inputs` takes them: a permutation row (from
    // `first_permutation_row` on, in pairs) holds `[0, v, v]` with `v` the value at its pair's
    // first in1 address on the pair's first row and at its second out address on the second.
    let operands = |row: usize| -> InputType {
        if row < first_permutation_row {
            [in0_address.at(row), in1_address.at(row), out_address.at(row)]
                .map(|address| context_values[address].to_m31_array())
        } else {
            let address = if (row - first_permutation_row).is_multiple_of(2) {
                in1_address.at(row)
            } else {
                out_address.at(row)
            };
            let value = context_values[address].to_m31_array();
            [zero, value, value]
        }
    };

    trace.par_iter_mut().enumerate().for_each(|(row_index, row)| {
        let packed: PackedInputType =
            <InputType as Pack>::pack(from_fn(|lane| operands(row_index * N_LANES + lane)));
        for column in 0..N_TRACE_COLUMNS {
            *row[column] = packed[column / 4][column % 4];
        }
    });

    trace
}

/// The component keeps no lookup data: its gate tuples are the preprocessed addresses and the
/// operands of its rows, gathered again from the context values at those addresses exactly as
/// [`write_trace_simd`] gathers them for the trace (the committed columns hold the same values,
/// which the previous version read back from the tree, re-evaluating their 12 columns), and its
/// multiplicities the preprocessed `qm31_ops_mults`, all read in place (omarespejel,
/// rgr1_3f4875e7).
pub fn write_interaction_trace(
    log_size: u32,
    context_values: &[QM31],
    first_permutation_row: usize,
    preprocessed_trace: &PreProcessedTrace,
    common_lookup_elements: &relations::CommonLookupElements,
) -> (Vec<CircleEvaluation<SimdBackend, M31, BitReversedOrder>>, ClaimedSum) {
    let mut logup_gen = unsafe { LogupTraceGenerator::uninitialized(log_size) };
    let tag = PackedM31::broadcast(M31::from(relations::GATE_RELATION_ID));
    let column = |id: &str| preprocessed_trace.column_rows(&PreProcessedColumnId { id: id.to_owned() });
    let [in0_address, in1_address, out_address, mults] = [
        "qm31_ops_in0_address",
        "qm31_ops_in1_address",
        "qm31_ops_out_address",
        "qm31_ops_mults",
    ]
    .map(column);
    let zero = QM31::zero().to_m31_array();
    // The operands `index` (in0, in1, out) of a row, as `write_trace_simd` takes them.
    let operand = |row: usize, index: usize| -> [M31; 4] {
        if row < first_permutation_row {
            let address = [&in0_address, &in1_address, &out_address][index].at(row);
            context_values[address].to_m31_array()
        } else if index == 0 {
            zero
        } else {
            let address = if (row - first_permutation_row).is_multiple_of(2) {
                in1_address.at(row)
            } else {
                out_address.at(row)
            };
            context_values[address].to_m31_array()
        }
    };
    let tuple = |address: &ColumnRows<'_>, index: usize, row: usize| {
        let first = row * N_LANES;
        let [a, b, c, d] = <[M31; 4] as Pack>::pack(from_fn(|lane| operand(first + lane, index)));
        [tag, address.packed_m31_at(first), a, b, c, d]
    };

    // Sum logup terms in pairs, in one pass per column (fractions, inverses and the running sum
    // per chunk of rows).
    logup_gen.write_frac_cols(1, |_, row| {
        let denom0: PackedQM31 = common_lookup_elements.combine(&tuple(&in0_address, 0, row));
        let denom1: PackedQM31 = common_lookup_elements.combine(&tuple(&in1_address, 1, row));
        (denom0 + denom1, denom0 * denom1)
    });

    // Sum last logup term.
    logup_gen.write_frac_cols(1, |_, row| {
        let denom = common_lookup_elements.combine(&tuple(&out_address, 2, row));
        (-PackedQM31::one() * mults.packed_m31_at(row * N_LANES), denom)
    });

    let (trace, claimed_sum) = logup_gen.finalize_last();

    (trace, claimed_sum)
}
