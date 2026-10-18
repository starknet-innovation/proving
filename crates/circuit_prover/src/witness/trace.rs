use std::mem::MaybeUninit;
use std::sync::Arc;

use circuit_common::Qm31OpsTraceGenerator;
use circuit_common::preprocessed::PreProcessedTrace;
use circuit_verifier::circuit_claim::{
    CircuitClaim, CircuitInteractionClaim, CircuitInteractionElements,
};
use circuit_verifier::circuit_components::PerComponent;
use circuits::context::U_VAR_IDX;
use itertools::Itertools;
use rayon::scope;
use stwo::core::channel::MerkleChannel;
use stwo::core::fields::m31::BaseField;
use stwo::core::fields::qm31::QM31;
use stwo::prover::TreeBuilder;
use stwo::prover::backend::BackendForChannel;
use stwo::prover::backend::simd::SimdBackend;
use stwo::prover::mempool::BaseColumnPool;
use stwo::prover::poly::BitReversedOrder;
use stwo::prover::poly::circle::{CircleCoefficients, CircleEvaluation, PolyOps};
use stwo::prover::poly::twiddles::TwiddleTree;

use crate::witness::components::{
    blake_g_gate, eq, m_31_to_u_32, qm31_ops, range_check_16, triple_xor, verify_bitwise_xor_4,
    verify_bitwise_xor_7, verify_bitwise_xor_8, verify_bitwise_xor_9, verify_bitwise_xor_12,
};

/// Interpolates the columns of one component. Columns above 2^16 values (all of a component's
/// columns have one size) are interpolated one after another, each parallel inside, through
/// [`PolyOps::interpolate_pooled`]: the fused out-of-place inverse FFT writes the coefficients
/// without a transposition pass of its own, into the evaluation buffer the previous column gave
/// back. The butterflies, twiddles and last pass are those of the in-place path, so every
/// coefficient is the same. Mechanism: realadii's fused-IFFT interpolation (lead rgr1_90de72d7).
fn interpolate_in_turn(
    columns: Vec<CircleEvaluation<SimdBackend, BaseField, BitReversedOrder>>,
    twiddles: &TwiddleTree<SimdBackend>,
) -> Vec<CircleCoefficients<SimdBackend>> {
    if !columns.first().is_some_and(|column| column.domain.log_size() > 16) {
        return SimdBackend::interpolate_columns(columns, twiddles);
    }
    let pool = BaseColumnPool::<SimdBackend>::new();
    columns.into_iter().map(|column| SimdBackend::interpolate_pooled(column, twiddles, &pool)).collect()
}

pub struct TraceGenerator {
    pub qm31_ops_trace_generator: Qm31OpsTraceGenerator,
}

pub fn write_trace<MC: MerkleChannel>(
    context_values: &[QM31],
    preprocessed_trace: Arc<PreProcessedTrace>,
    n_outputs: usize,
    tree_builder: &mut TreeBuilder<'_, '_, SimdBackend, MC>,
    trace_generator: &TraceGenerator,
    twiddles: &TwiddleTree<SimdBackend>,
) -> (CircuitClaim, PerComponent<u32>, CircuitInteractionClaimGenerator, std::ops::Range<usize>)
where
    SimdBackend: BackendForChannel<MC>,
{
    let preprocessed_trace_ref = preprocessed_trace.as_ref();

    // Parent scope: eq/qm31_ops traces run as spawns alongside everything else.
    let mut eq_result = MaybeUninit::uninit();
    let mut qm31_ops_result = MaybeUninit::uninit();
    let mut verify_bitwise_xor_8_state = MaybeUninit::uninit();
    let mut verify_bitwise_xor_12_state = MaybeUninit::uninit();
    let mut verify_bitwise_xor_4_state = MaybeUninit::uninit();
    let mut verify_bitwise_xor_7_state = MaybeUninit::uninit();
    let mut verify_bitwise_xor_9_state = MaybeUninit::uninit();
    let mut range_check_16_state = MaybeUninit::uninit();
    let mut verify_bitwise_xor_8_result = MaybeUninit::uninit();
    let mut verify_bitwise_xor_12_result = MaybeUninit::uninit();
    let mut verify_bitwise_xor_4_result = MaybeUninit::uninit();
    let mut verify_bitwise_xor_7_result = MaybeUninit::uninit();
    let mut verify_bitwise_xor_9_result = MaybeUninit::uninit();
    let mut range_check_16_result = MaybeUninit::uninit();
    let mut triple_xor_trace_data = MaybeUninit::uninit();
    let mut triple_xor_claim_icg = MaybeUninit::uninit();
    let mut triple_xor_polys_result = MaybeUninit::uninit();
    let mut blake_g_gate_trace_data = MaybeUninit::uninit();
    let mut blake_g_gate_claim_icg = MaybeUninit::uninit();
    let mut blake_g_gate_polys_result = MaybeUninit::uninit();
    let mut m_31_to_u_32_trace_data = MaybeUninit::uninit();
    let mut m_31_to_u_32_claim_icg = MaybeUninit::uninit();
    let mut m_31_to_u_32_polys_result = MaybeUninit::uninit();
    scope(|s| {
        // Eq and qm31_ops traces (with their interpolation) run first, in a subscope of their own:
        // their temporaries do not overlap the gate components' inputs and lookup data.
        scope(|s| {
        s.spawn(|_| {
            let (trace, log_size) = qm31_ops::write_trace(
                context_values,
                preprocessed_trace_ref,
                &trace_generator.qm31_ops_trace_generator,
            );
            let polys = interpolate_in_turn(trace.to_evals(), twiddles);
            qm31_ops_result.write((polys, log_size));
        });
        s.spawn(|_| {
            let (trace, log_size, lookup_data) =
                eq::write_trace(context_values, preprocessed_trace_ref);
            let polys = interpolate_in_turn(trace.to_evals(), twiddles);
            eq_result.write((polys, log_size, lookup_data));
        });
        });

        // Initialize claim generators in a subscope.
        scope(|s| {
            s.spawn(|_| {
                verify_bitwise_xor_8_state
                    .write(verify_bitwise_xor_8::ClaimGenerator::new(preprocessed_trace.clone()));
            });
            s.spawn(|_| {
                verify_bitwise_xor_12_state
                    .write(verify_bitwise_xor_12::ClaimGenerator::new(preprocessed_trace.clone()));
            });
            s.spawn(|_| {
                verify_bitwise_xor_4_state
                    .write(verify_bitwise_xor_4::ClaimGenerator::new(preprocessed_trace.clone()));
            });
            s.spawn(|_| {
                verify_bitwise_xor_7_state
                    .write(verify_bitwise_xor_7::ClaimGenerator::new(preprocessed_trace.clone()));
            });
            s.spawn(|_| {
                verify_bitwise_xor_9_state
                    .write(verify_bitwise_xor_9::ClaimGenerator::new(preprocessed_trace.clone()));
            });
            s.spawn(|_| {
                range_check_16_state
                    .write(range_check_16::ClaimGenerator::new(preprocessed_trace.clone()));
            });
        });

        // SAFETY: Claim generators were initialized by the subscope above.
        // Move xor/range-check states out of MaybeUninit into owned locals so we can
        // borrow them for blake_gate/blake_g/triple_xor_32, then move them into spawns.
        let verify_bitwise_xor_8_state = unsafe { verify_bitwise_xor_8_state.assume_init() };
        let verify_bitwise_xor_12_state = unsafe { verify_bitwise_xor_12_state.assume_init() };
        let verify_bitwise_xor_4_state = unsafe { verify_bitwise_xor_4_state.assume_init() };
        let verify_bitwise_xor_7_state = unsafe { verify_bitwise_xor_7_state.assume_init() };
        let verify_bitwise_xor_9_state = unsafe { verify_bitwise_xor_9_state.assume_init() };
        let range_check_16_state = unsafe { range_check_16_state.assume_init() };

        // triple_xor gate, m31_to_u32, and blake_g_gate mutate
        // xor/range-check states through shared refs, so they must complete before those
        // states are consumed.
        scope(|s| {
            s.spawn(|_| {
                let (trace, claim, icg) = triple_xor::write_trace(
                    context_values,
                    preprocessed_trace_ref,
                    &verify_bitwise_xor_8_state,
                );
                triple_xor_trace_data.write(trace);
                triple_xor_claim_icg.write((claim, icg));
            });
            s.spawn(|_| {
                let (trace, claim, icg) = m_31_to_u_32::write_trace(
                    context_values,
                    preprocessed_trace_ref,
                    &range_check_16_state,
                );
                m_31_to_u_32_trace_data.write(trace);
                m_31_to_u_32_claim_icg.write((claim, icg));
            });
            s.spawn(|_| {
                let (trace, claim, icg) = blake_g_gate::write_trace(
                    context_values,
                    preprocessed_trace_ref,
                    &verify_bitwise_xor_8_state,
                    &verify_bitwise_xor_12_state,
                    &verify_bitwise_xor_4_state,
                    &verify_bitwise_xor_9_state,
                    &verify_bitwise_xor_7_state,
                );
                blake_g_gate_trace_data.write(trace);
                blake_g_gate_claim_icg.write((claim, icg));
            });
        });

        // SAFETY: The subscope above guarantees triple_xor, m_31_to_u_32, and blake_g_gate have
        // finished mutating xor/range-check states. Interpolations and xor/range-check
        // write_trace calls can now run in parallel.
        s.spawn(|_| {
            let trace = unsafe { triple_xor_trace_data.assume_init() };
            triple_xor_polys_result
                .write(interpolate_in_turn(trace.to_evals(), twiddles));
        });
        s.spawn(|_| {
            let trace = unsafe { m_31_to_u_32_trace_data.assume_init() };
            m_31_to_u_32_polys_result
                .write(interpolate_in_turn(trace.to_evals(), twiddles));
        });
        s.spawn(|_| {
            let trace = unsafe { blake_g_gate_trace_data.assume_init() };
            blake_g_gate_polys_result
                .write(interpolate_in_turn(trace.to_evals(), twiddles));
        });
        s.spawn(|_| {
            let (trace, _claim, interaction_claim_gen) = verify_bitwise_xor_8_state.write_trace();
            let polys = interpolate_in_turn(trace.to_evals(), twiddles);
            verify_bitwise_xor_8_result.write((polys, interaction_claim_gen));
        });
        s.spawn(|_| {
            let (trace, _claim, interaction_claim_gen) = verify_bitwise_xor_12_state.write_trace();
            // verify_bitwise_xor_12 returns Vec<CircleEvaluation> directly, not ComponentTrace.
            let polys = interpolate_in_turn(trace, twiddles);
            verify_bitwise_xor_12_result.write((polys, interaction_claim_gen));
        });
        s.spawn(|_| {
            let (trace, _claim, interaction_claim_gen) = verify_bitwise_xor_4_state.write_trace();
            let polys = interpolate_in_turn(trace.to_evals(), twiddles);
            verify_bitwise_xor_4_result.write((polys, interaction_claim_gen));
        });
        s.spawn(|_| {
            let (trace, _claim, interaction_claim_gen) = verify_bitwise_xor_7_state.write_trace();
            let polys = interpolate_in_turn(trace.to_evals(), twiddles);
            verify_bitwise_xor_7_result.write((polys, interaction_claim_gen));
        });
        s.spawn(|_| {
            let (trace, _claim, interaction_claim_gen) = verify_bitwise_xor_9_state.write_trace();
            let polys = interpolate_in_turn(trace.to_evals(), twiddles);
            verify_bitwise_xor_9_result.write((polys, interaction_claim_gen));
        });
        s.spawn(|_| {
            let (trace, _claim, interaction_claim_gen) = range_check_16_state.write_trace();
            let polys = interpolate_in_turn(trace.to_evals(), twiddles);
            range_check_16_result.write((polys, interaction_claim_gen));
        });
    });

    // SAFETY: All MaybeUninit values were initialized by the scope above.
    let (
        (eq_polys, eq_log_size, eq_lookup_data),
        (qm31_ops_polys, qm31_ops_log_size),
        triple_xor_polys,
        (triple_xor_claim, triple_xor_interaction_claim_gen),
        m_31_to_u_32_polys,
        (m_31_to_u_32_claim, m_31_to_u_32_interaction_claim_gen),
        blake_g_gate_polys,
        (blake_g_gate_claim, blake_g_gate_interaction_claim_gen),
        (verify_bitwise_xor_8_polys, verify_bitwise_xor_8_interaction_claim_gen),
        (verify_bitwise_xor_12_polys, verify_bitwise_xor_12_interaction_claim_gen),
        (verify_bitwise_xor_4_polys, verify_bitwise_xor_4_interaction_claim_gen),
        (verify_bitwise_xor_7_polys, verify_bitwise_xor_7_interaction_claim_gen),
        (verify_bitwise_xor_9_polys, verify_bitwise_xor_9_interaction_claim_gen),
        (range_check_16_polys, range_check_16_interaction_claim_gen),
    ) = unsafe {
        (
            eq_result.assume_init(),
            qm31_ops_result.assume_init(),
            triple_xor_polys_result.assume_init(),
            triple_xor_claim_icg.assume_init(),
            m_31_to_u_32_polys_result.assume_init(),
            m_31_to_u_32_claim_icg.assume_init(),
            blake_g_gate_polys_result.assume_init(),
            blake_g_gate_claim_icg.assume_init(),
            verify_bitwise_xor_8_result.assume_init(),
            verify_bitwise_xor_12_result.assume_init(),
            verify_bitwise_xor_4_result.assume_init(),
            verify_bitwise_xor_7_result.assume_init(),
            verify_bitwise_xor_9_result.assume_init(),
            range_check_16_result.assume_init(),
        )
    };

    let log_sizes = PerComponent {
        eq: eq_log_size,
        qm31_ops: qm31_ops_log_size,
        triple_xor: triple_xor_claim.log_size,
        m_31_to_u_32: m_31_to_u_32_claim.log_size,
        blake_g_gate: blake_g_gate_claim.log_size,
        verify_bitwise_xor_8: crate::circuit_air::components::verify_bitwise_xor_8::LOG_SIZE,
        verify_bitwise_xor_12: crate::circuit_air::components::verify_bitwise_xor_12::LOG_SIZE,
        verify_bitwise_xor_4: crate::circuit_air::components::verify_bitwise_xor_4::LOG_SIZE,
        verify_bitwise_xor_7: crate::circuit_air::components::verify_bitwise_xor_7::LOG_SIZE,
        verify_bitwise_xor_9: crate::circuit_air::components::verify_bitwise_xor_9::LOG_SIZE,
        range_check_16: crate::circuit_air::components::range_check_16::LOG_SIZE,
    };

    let component_trace_polys = [
        eq_polys,
        qm31_ops_polys,
        triple_xor_polys,
        m_31_to_u_32_polys,
        blake_g_gate_polys,
        verify_bitwise_xor_8_polys,
        verify_bitwise_xor_12_polys,
        verify_bitwise_xor_4_polys,
        verify_bitwise_xor_7_polys,
        verify_bitwise_xor_9_polys,
        range_check_16_polys,
    ];
    // The columns of blake_g_gate, the component whose composition reads the most of them.
    let mut blake_g_gate_columns = 0..0;
    let mut qm31_ops_columns = 0..0;
    for (index, polys) in component_trace_polys.into_iter().enumerate() {
        let span = tree_builder.extend_polys(polys);
        match index {
            1 => qm31_ops_columns = span.col_start..span.col_end,
            4 => blake_g_gate_columns = span.col_start..span.col_end,
            _ => {}
        }
    }

    let output_values = ((U_VAR_IDX + 1)..(U_VAR_IDX + 1 + n_outputs))
        .map(|addr| context_values[addr])
        .collect_vec();

    (
        CircuitClaim { output_values },
        log_sizes,
        CircuitInteractionClaimGenerator {
            eq_lookup_data,
            qm31_ops_columns,
            preprocessed_trace: preprocessed_trace.clone(),
            triple_xor: triple_xor_interaction_claim_gen,
            m_31_to_u_32: m_31_to_u_32_interaction_claim_gen,
            blake_g_gate: blake_g_gate_interaction_claim_gen,
            verify_bitwise_xor_8: verify_bitwise_xor_8_interaction_claim_gen,
            verify_bitwise_xor_12: verify_bitwise_xor_12_interaction_claim_gen,
            verify_bitwise_xor_4: verify_bitwise_xor_4_interaction_claim_gen,
            verify_bitwise_xor_7: verify_bitwise_xor_7_interaction_claim_gen,
            verify_bitwise_xor_9: verify_bitwise_xor_9_interaction_claim_gen,
            range_check_16: range_check_16_interaction_claim_gen,
        },
        blake_g_gate_columns,
    )
}

pub struct CircuitInteractionClaimGenerator {
    pub eq_lookup_data: eq::LookupData,
    /// The base tree's columns of qm31_ops, which its interaction write reads back.
    pub qm31_ops_columns: std::ops::Range<usize>,
    pub preprocessed_trace: Arc<PreProcessedTrace>,
    pub triple_xor: triple_xor::InteractionClaimGenerator,
    pub m_31_to_u_32: m_31_to_u_32::InteractionClaimGenerator,
    pub blake_g_gate: blake_g_gate::InteractionClaimGenerator,
    pub verify_bitwise_xor_8: verify_bitwise_xor_8::InteractionClaimGenerator,
    pub verify_bitwise_xor_12: verify_bitwise_xor_12::InteractionClaimGenerator,
    pub verify_bitwise_xor_4: verify_bitwise_xor_4::InteractionClaimGenerator,
    pub verify_bitwise_xor_7: verify_bitwise_xor_7::InteractionClaimGenerator,
    pub verify_bitwise_xor_9: verify_bitwise_xor_9::InteractionClaimGenerator,
    pub range_check_16: range_check_16::InteractionClaimGenerator,
}

pub fn write_interaction_trace<MC: MerkleChannel>(
    component_log_sizes: &PerComponent<u32>,
    circuit_interaction_claim_generator: CircuitInteractionClaimGenerator,
    context_values: &[QM31],
    first_permutation_row: usize,
    tree_builder: &mut TreeBuilder<'_, '_, SimdBackend, MC>,
    interaction_elements: &CircuitInteractionElements,
    twiddles: &TwiddleTree<SimdBackend>,
) -> CircuitInteractionClaim
where
    SimdBackend: BackendForChannel<MC>,
{
    // Extract log sizes before parallel section.
    let eq_log_size = component_log_sizes.eq;
    let qm31_ops_log_size = component_log_sizes.qm31_ops;

    // Write all interaction traces in parallel, including interpolation. The slots below start
    // default-initialized; each spawned task writes into its own disjoint field. Destructuring
    // `circuit_interaction_claim_generator` below forces every task to run, so no field is left
    // at its default.
    let mut all_polys = PerComponent::default();
    let mut claimed_sums = PerComponent::default();
    let CircuitInteractionClaimGenerator {
        eq_lookup_data,
        qm31_ops_columns: _,
        preprocessed_trace,
        triple_xor,
        m_31_to_u_32,
        blake_g_gate,
        verify_bitwise_xor_8,
        verify_bitwise_xor_12,
        verify_bitwise_xor_4,
        verify_bitwise_xor_7,
        verify_bitwise_xor_9,
        range_check_16,
    } = circuit_interaction_claim_generator;
    // The blake gate's interaction trace, the largest, is written beside the other components'
    // (each writer is parallel inside too): with the gate tuples kept narrow, their temporaries fit
    // together under the proof's peak.
    let ((blake_polys, blake_sum), ()) = rayon::join(
        || {
            let (trace, claim) =
                blake_g_gate.write_interaction_trace(&interaction_elements.common_lookup_elements);
            (interpolate_in_turn(trace, twiddles), claim.claimed_sum)
        },
        || rayon::scope(|s| {
        s.spawn(|_| {
            let (trace, claimed_sum) = eq::write_interaction_trace(
                eq_log_size,
                eq_lookup_data,
                &preprocessed_trace,
                &interaction_elements.common_lookup_elements,
            );
            all_polys.eq = interpolate_in_turn(trace, twiddles);
            claimed_sums.eq = claimed_sum;
        });
        s.spawn(|_| {
            let (trace, claimed_sum) = qm31_ops::write_interaction_trace(
                qm31_ops_log_size,
                context_values,
                first_permutation_row,
                &preprocessed_trace,
                &interaction_elements.common_lookup_elements,
            );
            all_polys.qm31_ops = interpolate_in_turn(trace, twiddles);
            claimed_sums.qm31_ops = claimed_sum;
        });
        s.spawn(|_| {
            let (trace, claim) =
                triple_xor.write_interaction_trace(&interaction_elements.common_lookup_elements);
            all_polys.triple_xor = interpolate_in_turn(trace, twiddles);
            claimed_sums.triple_xor = claim.claimed_sum;
        });
        s.spawn(|_| {
            let (trace, claim) =
                m_31_to_u_32.write_interaction_trace(&interaction_elements.common_lookup_elements);
            all_polys.m_31_to_u_32 = interpolate_in_turn(trace, twiddles);
            claimed_sums.m_31_to_u_32 = claim.claimed_sum;
        });
        s.spawn(|_| {
            let (trace, claim) = verify_bitwise_xor_8
                .write_interaction_trace(&interaction_elements.common_lookup_elements);
            all_polys.verify_bitwise_xor_8 = interpolate_in_turn(trace, twiddles);
            claimed_sums.verify_bitwise_xor_8 = claim.claimed_sum;
        });
        s.spawn(|_| {
            let (trace, claim) = verify_bitwise_xor_12
                .write_interaction_trace(&interaction_elements.common_lookup_elements);
            all_polys.verify_bitwise_xor_12 = interpolate_in_turn(trace, twiddles);
            claimed_sums.verify_bitwise_xor_12 = claim.claimed_sum;
        });
        s.spawn(|_| {
            let (trace, claim) = verify_bitwise_xor_4
                .write_interaction_trace(&interaction_elements.common_lookup_elements);
            all_polys.verify_bitwise_xor_4 = interpolate_in_turn(trace, twiddles);
            claimed_sums.verify_bitwise_xor_4 = claim.claimed_sum;
        });
        s.spawn(|_| {
            let (trace, claim) = verify_bitwise_xor_7
                .write_interaction_trace(&interaction_elements.common_lookup_elements);
            all_polys.verify_bitwise_xor_7 = interpolate_in_turn(trace, twiddles);
            claimed_sums.verify_bitwise_xor_7 = claim.claimed_sum;
        });
        s.spawn(|_| {
            let (trace, claim) = verify_bitwise_xor_9
                .write_interaction_trace(&interaction_elements.common_lookup_elements);
            all_polys.verify_bitwise_xor_9 = interpolate_in_turn(trace, twiddles);
            claimed_sums.verify_bitwise_xor_9 = claim.claimed_sum;
        });
        s.spawn(|_| {
            let (trace, claim) = range_check_16
                .write_interaction_trace(&interaction_elements.common_lookup_elements);
            all_polys.range_check_16 = interpolate_in_turn(trace, twiddles);
            claimed_sums.range_check_16 = claim.claimed_sum;
        });
    }),
    );
    all_polys.blake_g_gate = blake_polys;
    claimed_sums.blake_g_gate = blake_sum;

    // Commit the interaction columns.
    for polys in all_polys.into_array() {
        tree_builder.extend_polys(polys);
    }

    CircuitInteractionClaim { claimed_sums }
}
