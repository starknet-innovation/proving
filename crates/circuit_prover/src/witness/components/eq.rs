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

/// The trace is written straight from the context values at the in0 addresses (both addresses of
/// an eq gate hold the same value), and the lookup data keeps the two addresses and the four shared
/// values once, 24 bytes per row instead of two 6-element tuples (a8n Q1462, after rukafe0x
/// rgr1_60812844); [`write_interaction_trace`] rebuilds both tuples, relation id first.
pub fn write_trace(
    context_values: &[QM31],
    preprocessed_trace: &PreProcessedTrace,
) -> (ComponentTrace<N_TRACE_COLUMNS>, u32, LookupData) {
    let packed =
        |id: &str| preprocessed_trace.get_packed_column(&PreProcessedColumnId { id: id.to_owned() });
    let in_0 = packed("eq_in0_address");
    let in_1 = packed("eq_in1_address");
    assert_eq!(in_0.len(), in_1.len());
    let n_rows = in_0.len() * N_LANES;
    assert_ne!(n_rows, 0);
    assert!(n_rows.is_power_of_two());
    let log_size = n_rows.ilog2();

    let zero = PackedM31::broadcast(M31::from(0));
    let mut trace = ComponentTrace::<N_TRACE_COLUMNS>::zeroed(log_size);
    let mut values: Vec<[PackedM31; 4]> = vec![[zero; 4]; in_0.len()];
    (trace.par_iter_mut(), values.par_iter_mut(), in_0.par_iter()).into_par_iter().for_each(
        |(row, value, address)| {
            let lanes = address.to_array();
            let mut limbs = [[M31::from(0); N_LANES]; 4];
            for lane in 0..N_LANES {
                let v = context_values[lanes[lane].0 as usize].to_m31_array();
                for k in 0..4 {
                    limbs[k][lane] = v[k];
                }
            }
            let limbs = limbs.map(PackedM31::from_array);
            for k in 0..4 {
                *row[k] = limbs[k];
            }
            *value = limbs;
        },
    );

    (trace, log_size, LookupData { in_0, in_1, values })
}

pub struct LookupData {
    in_0: Vec<PackedM31>,
    in_1: Vec<PackedM31>,
    values: Vec<[PackedM31; 4]>,
}

pub fn write_interaction_trace(
    log_size: u32,
    lookup_data: LookupData,
    common_lookup_elements: &relations::CommonLookupElements,
) -> (Vec<CircleEvaluation<SimdBackend, M31, BitReversedOrder>>, SecureField) {
    let mut logup_gen = unsafe { LogupTraceGenerator::uninitialized(log_size) };

    // Sum logup terms in pairs.
    let mut col_gen = logup_gen.new_col();
    let m31_gate_relation_id = PackedM31::broadcast(M31::from(378353459));
    (col_gen.par_iter_mut(), &lookup_data.in_0, &lookup_data.in_1, &lookup_data.values)
        .into_par_iter()
        .for_each(|(writer, in_0, in_1, v)| {
            let denom0: PackedQM31 = common_lookup_elements
                .combine(&[m31_gate_relation_id, *in_0, v[0], v[1], v[2], v[3]]);
            let denom1: PackedQM31 = common_lookup_elements
                .combine(&[m31_gate_relation_id, *in_1, v[0], v[1], v[2], v[3]]);
            writer.write_frac(denom0 + denom1, denom0 * denom1);
        });
    col_gen.finalize_col();

    let (trace, claimed_sum) = logup_gen.finalize_last();

    (trace, claimed_sum)
}
