use circuit_common::preprocessed::ColumnRows;

use crate::circuit_air::components::eq::N_TRACE_COLUMNS;
use crate::witness::components::prelude::*;

pub type InputType = [M31; 4];
pub type PackedInputType = [PackedM31; 4];

/// Retrieves the component's inputs from the context values, using the addresses provided in the
/// preprocessed trace.
#[allow(clippy::uninit_vec)]
pub fn extract_component_inputs(
    in0_address: &[usize],
    in1_address: &[usize],
    context_values: &[QM31],
) -> Vec<InputType> {
    let n_rows = in0_address.len();
    assert_eq!(n_rows, in1_address.len());

    let mut inputs = Vec::with_capacity(n_rows);
    unsafe {
        inputs.set_len(n_rows);
    }

    // Sanity check that the inputs are equal
    #[cfg(debug_assertions)]
    (in0_address.par_iter(), in1_address.par_iter()).into_par_iter().for_each(
        |(in0_address, in1_address)| {
            assert_eq!(
                context_values[*in0_address], context_values[*in1_address],
                "Eq gate: in0 and in1 must have equal values"
            );
        },
    );

    (inputs.par_iter_mut(), in0_address.par_iter()).into_par_iter().for_each(
        |(input, in0_address)| {
            *input = context_values[*in0_address].to_m31_array();
        },
    );

    inputs
}

pub fn write_trace(
    context_values: &[QM31],
    preprocessed_trace: &PreProcessedTrace,
) -> (ComponentTrace<N_TRACE_COLUMNS>, u32, LookupData) {
    let column = |id: &str| preprocessed_trace.column_rows(&PreProcessedColumnId { id: id.to_owned() });
    let in0_address = column("eq_in0_address");
    let in1_address = column("eq_in1_address");

    let n_rows = in0_address.len();
    assert_eq!(n_rows, in1_address.len());
    assert_ne!(n_rows, 0);
    assert!(n_rows >= N_LANES);
    assert!(n_rows.is_power_of_two());
    let log_size = n_rows.ilog2();

    // Sanity check that the inputs are equal
    #[cfg(debug_assertions)]
    (0..n_rows).into_par_iter().for_each(|row| {
        assert_eq!(
            context_values[in0_address.at(row)], context_values[in1_address.at(row)],
            "Eq gate: in0 and in1 must have equal values"
        );
    });

    let (trace, lookup_data) = write_trace_simd(context_values, &in0_address);

    (trace, log_size, lookup_data)
}

/// Writes the trace straight from the context values: each packed row gathers its rows' values
/// as [`extract_component_inputs`] would and packs them as `pack_values` would, in place. The
/// addresses are read from the preprocessed columns in place, here and by the interaction trace,
/// which is all the lookup data keeps besides the values (omarespejel's in-place preprocessed
/// reads, rgr1_3f4875e7).
fn write_trace_simd(
    context_values: &[QM31],
    in0_address: &ColumnRows<'_>,
) -> (ComponentTrace<N_TRACE_COLUMNS>, LookupData) {
    let log_n_packed_rows = (in0_address.len() / N_LANES).ilog2();
    let log_size = log_n_packed_rows + LOG_N_LANES;
    let (mut trace, mut lookup_data) = unsafe {
        (
            ComponentTrace::<N_TRACE_COLUMNS>::uninitialized(log_size),
            LookupData::uninitialized(log_n_packed_rows),
        )
    };

    (trace.par_iter_mut(), lookup_data.par_iter_mut())
        .into_par_iter()
        .enumerate()
        .for_each(|(row_index, (row, lookup_data))| {
            let input: PackedInputType = <InputType as Pack>::pack(from_fn(|lane| {
                context_values[in0_address.at(row_index * N_LANES + lane)].to_m31_array()
            }));
            for (column, value) in input.into_iter().enumerate() {
                *row[column] = value;
            }
            *lookup_data.values = input;
        });

    (trace, lookup_data)
}

/// The component's lookup data: the values of its rows. Its two gate tuples are each address
/// with these values.
#[derive(Uninitialized, IterMut, ParIterMut)]
pub struct LookupData {
    values: Vec<[PackedM31; 4]>,
}

pub fn write_interaction_trace(
    log_size: u32,
    lookup_data: LookupData,
    preprocessed_trace: &PreProcessedTrace,
    common_lookup_elements: &relations::CommonLookupElements,
) -> (Vec<CircleEvaluation<SimdBackend, M31, BitReversedOrder>>, SecureField) {
    let mut logup_gen = unsafe { LogupTraceGenerator::uninitialized(log_size) };
    let m31_gate_relation_id = PackedM31::broadcast(M31::from(378353459));
    let column = |id: &str| preprocessed_trace.column_rows(&PreProcessedColumnId { id: id.to_owned() });
    let (in0_address, in1_address) = (column("eq_in0_address"), column("eq_in1_address"));

    // Sum logup terms in pairs, in one pass per chunk of rows (fractions, inverses and the running
    // sum).
    logup_gen.write_frac_cols(1, |_, row| {
        let first = row * N_LANES;
        let [in_col0, in_col1, in_col2, in_col3] = lookup_data.values[row];
        let values0 =
            [m31_gate_relation_id, in0_address.packed_m31_at(first), in_col0, in_col1, in_col2, in_col3];
        let values1 =
            [m31_gate_relation_id, in1_address.packed_m31_at(first), in_col0, in_col1, in_col2, in_col3];
        let denom0: PackedQM31 = common_lookup_elements.combine(&values0);
        let denom1: PackedQM31 = common_lookup_elements.combine(&values1);
        (denom0 + denom1, denom0 * denom1)
    });

    let (trace, claimed_sum) = logup_gen.finalize_last();

    (trace, claimed_sum)
}
