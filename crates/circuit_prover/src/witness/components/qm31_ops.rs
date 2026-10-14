use num_traits::Zero;
use rayon::slice::{ParallelSlice, ParallelSliceMut};

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
) -> (ComponentTrace<N_TRACE_COLUMNS>, ComponentLogSize, LookupData) {
    let add_flag =
        preprocessed_trace.get_column(&PreProcessedColumnId { id: "qm31_ops_add_flag".to_owned() });
    let sub_flag =
        preprocessed_trace.get_column(&PreProcessedColumnId { id: "qm31_ops_sub_flag".to_owned() });
    let mul_flag =
        preprocessed_trace.get_column(&PreProcessedColumnId { id: "qm31_ops_mul_flag".to_owned() });
    let pointwise_mul_flag = preprocessed_trace
        .get_column(&PreProcessedColumnId { id: "qm31_ops_pointwise_mul_flag".to_owned() });
    let in0_address = preprocessed_trace
        .get_column(&PreProcessedColumnId { id: "qm31_ops_in0_address".to_owned() });
    let in1_address = preprocessed_trace
        .get_column(&PreProcessedColumnId { id: "qm31_ops_in1_address".to_owned() });
    let out_address = preprocessed_trace
        .get_column(&PreProcessedColumnId { id: "qm31_ops_out_address".to_owned() });
    let mults =
        preprocessed_trace.get_column(&PreProcessedColumnId { id: "qm31_ops_mults".to_owned() });

    let inputs = extract_component_inputs(
        &in0_address,
        &in1_address,
        &out_address,
        context_values,
        trace_generator,
    );

    let n_rows = inputs.len();
    assert_ne!(n_rows, 0);
    assert!(n_rows >= N_LANES);
    assert!(n_rows.is_power_of_two());
    let log_size = n_rows.ilog2();

    let packed_inputs = pack_values(&inputs);
    drop(inputs);

    let preprocessed_columns = [
        add_flag,
        sub_flag,
        mul_flag,
        pointwise_mul_flag,
        in0_address,
        in1_address,
        out_address,
        mults,
    ]
    .into_iter()
    .map(|col| crate::witness::utils::pack_usize_column(&col))
    .collect_vec();

    let (trace, lookup_data) = write_trace_simd(packed_inputs, preprocessed_columns);

    (trace, log_size, lookup_data)
}

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

    let [
        add_flag,
        sub_flag,
        mul_flag,
        pointwise_mul_flag,
        in0_address,
        in1_address,
        out_address,
        mults,
    ] = preprocessed_columns.try_into().unwrap();

    (trace.par_iter_mut(), lookup_data.par_iter_mut(), inputs.into_par_iter())
        .into_par_iter()
        .enumerate()
        .for_each(|(row_index, (row, lookup_data, qm_31_ops_input))| {
            let _add_flag = add_flag[row_index];
            let _sub_flag = sub_flag[row_index];
            let _mul_flag = mul_flag[row_index];
            let _pointwise_mul_flag = pointwise_mul_flag[row_index];
            let in0_address = in0_address[row_index];
            let in1_address = in1_address[row_index];
            let out_address = out_address[row_index];
            let mults = mults[row_index];

            let in0_col0 = qm_31_ops_input[0][0];
            *row[0] = in0_col0;
            let in0_col1 = qm_31_ops_input[0][1];
            *row[1] = in0_col1;
            let in0_col2 = qm_31_ops_input[0][2];
            *row[2] = in0_col2;
            let in0_col3 = qm_31_ops_input[0][3];
            *row[3] = in0_col3;
            let in1_col4 = qm_31_ops_input[1][0];
            *row[4] = in1_col4;
            let in1_col5 = qm_31_ops_input[1][1];
            *row[5] = in1_col5;
            let in1_col6 = qm_31_ops_input[1][2];
            *row[6] = in1_col6;
            let in1_col7 = qm_31_ops_input[1][3];
            *row[7] = in1_col7;
            let out_col8 = qm_31_ops_input[2][0];
            *row[8] = out_col8;
            let out_col9 = qm_31_ops_input[2][1];
            *row[9] = out_col9;
            let out_col10 = qm_31_ops_input[2][2];
            *row[10] = out_col10;
            let out_col11 = qm_31_ops_input[2][3];
            *row[11] = out_col11;
            // The gate tuples are the addresses and these trace columns: the interaction write
            // reads them back from the committed tree (see `write_interaction_trace`).
            let _ = (in0_address, in1_address, out_address);
            *lookup_data.mults = mults;
        });

    (trace, lookup_data)
}

/// The component's lookup data: its multiplicities. Its gate tuples (each address with the four
/// coordinates of its value) are the preprocessed addresses and the component's trace columns,
/// which [`write_interaction_trace`] reads back from the committed tree instead of their copy
/// being kept through the base commit (realadii's qm31_ops read-back, rgr1_3c4bf108).
#[derive(Uninitialized, IterMut, ParIterMut)]
pub struct LookupData {
    mults: Vec<PackedM31>,
}

/// `read_trace_columns(range)` reads back the component's trace columns `range` (the coordinates
/// of `in_0`, `in_1` and `out` are columns 0 to 12), as committed: the eight of the inputs for
/// the first logup column, then the four of the output for the second.
pub fn write_interaction_trace(
    log_size: u32,
    lookup_data: LookupData,
    read_trace_columns: &dyn Fn(std::ops::Range<usize>) -> Vec<Col<SimdBackend, M31>>,
    preprocessed_trace: &PreProcessedTrace,
    common_lookup_elements: &relations::CommonLookupElements,
) -> (Vec<CircleEvaluation<SimdBackend, M31, BitReversedOrder>>, ClaimedSum) {
    let mut logup_gen = unsafe { LogupTraceGenerator::uninitialized(log_size) };
    let tag = PackedM31::broadcast(M31::from(relations::GATE_RELATION_ID));
    let address = |id: &str| {
        let column = preprocessed_trace.get_column(&PreProcessedColumnId { id: id.to_owned() });
        Col::<SimdBackend, M31>::from_iter(column.iter().map(|&x| M31::from(x))).data
    };
    let [in0_address, in1_address, out_address] =
        ["qm31_ops_in0_address", "qm31_ops_in1_address", "qm31_ops_out_address"].map(address);
    let tuple = |columns: &[Col<SimdBackend, M31>], address: &[PackedM31], first: usize, row: usize| {
        [
            address[row],
            columns[first].data[row],
            columns[first + 1].data[row],
            columns[first + 2].data[row],
            columns[first + 3].data[row],
        ]
    };
    let rows = 0..in0_address.len();
    let inputs = read_trace_columns(0..8);

    // Sum logup terms in pairs.
    let mut col_gen = logup_gen.new_col();
    (col_gen.par_iter_mut(), rows.clone().into_par_iter()).into_par_iter().for_each(
        |(writer, row)| {
            let (values0, values1) =
                (tuple(&inputs, &in0_address, 0, row), tuple(&inputs, &in1_address, 4, row));
            let denom0: PackedQM31 = common_lookup_elements
                .combine(&[tag, values0[0], values0[1], values0[2], values0[3], values0[4]]);
            let denom1: PackedQM31 = common_lookup_elements
                .combine(&[tag, values1[0], values1[1], values1[2], values1[3], values1[4]]);
            writer.write_frac(denom0 + denom1, denom0 * denom1);
        },
    );
    col_gen.finalize_col();
    drop(inputs);

    // Sum last logup term.
    let outputs = read_trace_columns(8..12);
    let mut col_gen = logup_gen.new_col();
    (col_gen.par_iter_mut(), rows.into_par_iter(), lookup_data.mults).into_par_iter().for_each(
        |(writer, row, mults)| {
            let values = tuple(&outputs, &out_address, 0, row);
            let denom = common_lookup_elements
                .combine(&[tag, values[0], values[1], values[2], values[3], values[4]]);
            writer.write_frac(-PackedQM31::one() * mults, denom);
        },
    );
    col_gen.finalize_col();

    let (trace, claimed_sum) = logup_gen.finalize_last();

    (trace, claimed_sum)
}
