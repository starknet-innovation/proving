// This file was created by the AIR team.

use subroutines::read_id::ReadId;

use crate::components::prelude::*;

pub const N_TRACE_COLUMNS: usize = 16;
pub const RELATION_USES_PER_ROW: [RelationUse; 2] = [
    RelationUse { relation_id: "KeccakAggregator", uses: 1 },
    RelationUse { relation_id: "MemoryAddressToId", uses: 16 },
];

pub struct Eval {
    pub claim: Claim,
    pub common_lookup_elements: relations::CommonLookupElements,
    pub keccak_builtin_segment_start: u32,
}

#[derive(Copy, Clone, Serialize, Deserialize, CairoSerialize, CairoDeserialize)]
pub struct Claim {
    pub log_size: u32,
}
impl Claim {
    pub fn log_sizes(&self) -> TreeVec<Vec<u32>> {
        let trace_log_sizes = vec![self.log_size; N_TRACE_COLUMNS];
        let interaction_log_sizes = vec![self.log_size; SECURE_EXTENSION_DEGREE * 9];
        TreeVec::new(vec![trace_log_sizes, interaction_log_sizes])
    }
}

#[derive(Copy, Clone, Serialize, Deserialize, CairoSerialize, CairoDeserialize)]
pub struct InteractionClaim {
    pub claimed_sum: SecureField,
}

pub type Component = FrameworkComponent<Eval>;

impl FrameworkEval for Eval {
    fn log_size(&self) -> u32 {
        self.claim.log_size
    }

    fn max_constraint_log_degree_bound(&self) -> u32 {
        self.log_size() + 1
    }

    #[allow(unused_parens)]
    #[allow(clippy::double_parens)]
    #[allow(non_snake_case)]
    fn evaluate<E: EvalAtRow>(&self, mut eval: E) -> E {
        let M31_1 = E::F::from(M31::from(1));
        let M31_10 = E::F::from(M31::from(10));
        let M31_11 = E::F::from(M31::from(11));
        let M31_12 = E::F::from(M31::from(12));
        let M31_13 = E::F::from(M31::from(13));
        let M31_14 = E::F::from(M31::from(14));
        let M31_15 = E::F::from(M31::from(15));
        let M31_16 = E::F::from(M31::from(16));
        let M31_1973115366 = E::F::from(M31::from(1973115366));
        let M31_2 = E::F::from(M31::from(2));
        let M31_3 = E::F::from(M31::from(3));
        let M31_4 = E::F::from(M31::from(4));
        let M31_5 = E::F::from(M31::from(5));
        let M31_6 = E::F::from(M31::from(6));
        let M31_7 = E::F::from(M31::from(7));
        let M31_8 = E::F::from(M31::from(8));
        let M31_9 = E::F::from(M31::from(9));
        let seq = eval.get_preprocessed_column(Seq::new(self.log_size()).id());
        let input_0_id_col0 = eval.next_trace_mask();
        let input_1_id_col1 = eval.next_trace_mask();
        let input_2_id_col2 = eval.next_trace_mask();
        let input_3_id_col3 = eval.next_trace_mask();
        let input_4_id_col4 = eval.next_trace_mask();
        let input_5_id_col5 = eval.next_trace_mask();
        let input_6_id_col6 = eval.next_trace_mask();
        let input_7_id_col7 = eval.next_trace_mask();
        let output_0_id_col8 = eval.next_trace_mask();
        let output_1_id_col9 = eval.next_trace_mask();
        let output_2_id_col10 = eval.next_trace_mask();
        let output_3_id_col11 = eval.next_trace_mask();
        let output_4_id_col12 = eval.next_trace_mask();
        let output_5_id_col13 = eval.next_trace_mask();
        let output_6_id_col14 = eval.next_trace_mask();
        let output_7_id_col15 = eval.next_trace_mask();

        let instance_addr_tmp_24554_0 = eval.add_intermediate(
            ((seq.clone() * M31_16.clone())
                + E::F::from(M31::from(self.keccak_builtin_segment_start))),
        );
        ReadId::evaluate(
            [instance_addr_tmp_24554_0.clone()],
            M31_1.clone(),
            input_0_id_col0.clone(),
            &self.common_lookup_elements,
            &mut eval,
        );
        ReadId::evaluate(
            [(instance_addr_tmp_24554_0.clone() + M31_1.clone())],
            M31_1.clone(),
            input_1_id_col1.clone(),
            &self.common_lookup_elements,
            &mut eval,
        );
        ReadId::evaluate(
            [(instance_addr_tmp_24554_0.clone() + M31_2.clone())],
            M31_1.clone(),
            input_2_id_col2.clone(),
            &self.common_lookup_elements,
            &mut eval,
        );
        ReadId::evaluate(
            [(instance_addr_tmp_24554_0.clone() + M31_3.clone())],
            M31_1.clone(),
            input_3_id_col3.clone(),
            &self.common_lookup_elements,
            &mut eval,
        );
        ReadId::evaluate(
            [(instance_addr_tmp_24554_0.clone() + M31_4.clone())],
            M31_1.clone(),
            input_4_id_col4.clone(),
            &self.common_lookup_elements,
            &mut eval,
        );
        ReadId::evaluate(
            [(instance_addr_tmp_24554_0.clone() + M31_5.clone())],
            M31_1.clone(),
            input_5_id_col5.clone(),
            &self.common_lookup_elements,
            &mut eval,
        );
        ReadId::evaluate(
            [(instance_addr_tmp_24554_0.clone() + M31_6.clone())],
            M31_1.clone(),
            input_6_id_col6.clone(),
            &self.common_lookup_elements,
            &mut eval,
        );
        ReadId::evaluate(
            [(instance_addr_tmp_24554_0.clone() + M31_7.clone())],
            M31_1.clone(),
            input_7_id_col7.clone(),
            &self.common_lookup_elements,
            &mut eval,
        );
        ReadId::evaluate(
            [(instance_addr_tmp_24554_0.clone() + M31_8.clone())],
            M31_1.clone(),
            output_0_id_col8.clone(),
            &self.common_lookup_elements,
            &mut eval,
        );
        ReadId::evaluate(
            [(instance_addr_tmp_24554_0.clone() + M31_9.clone())],
            M31_1.clone(),
            output_1_id_col9.clone(),
            &self.common_lookup_elements,
            &mut eval,
        );
        ReadId::evaluate(
            [(instance_addr_tmp_24554_0.clone() + M31_10.clone())],
            M31_1.clone(),
            output_2_id_col10.clone(),
            &self.common_lookup_elements,
            &mut eval,
        );
        ReadId::evaluate(
            [(instance_addr_tmp_24554_0.clone() + M31_11.clone())],
            M31_1.clone(),
            output_3_id_col11.clone(),
            &self.common_lookup_elements,
            &mut eval,
        );
        ReadId::evaluate(
            [(instance_addr_tmp_24554_0.clone() + M31_12.clone())],
            M31_1.clone(),
            output_4_id_col12.clone(),
            &self.common_lookup_elements,
            &mut eval,
        );
        ReadId::evaluate(
            [(instance_addr_tmp_24554_0.clone() + M31_13.clone())],
            M31_1.clone(),
            output_5_id_col13.clone(),
            &self.common_lookup_elements,
            &mut eval,
        );
        ReadId::evaluate(
            [(instance_addr_tmp_24554_0.clone() + M31_14.clone())],
            M31_1.clone(),
            output_6_id_col14.clone(),
            &self.common_lookup_elements,
            &mut eval,
        );
        ReadId::evaluate(
            [(instance_addr_tmp_24554_0.clone() + M31_15.clone())],
            M31_1.clone(),
            output_7_id_col15.clone(),
            &self.common_lookup_elements,
            &mut eval,
        );
        eval.add_to_relation(RelationEntry::new(
            &self.common_lookup_elements,
            E::EF::from(M31_1.clone()),
            &[
                M31_1973115366.clone(),
                input_0_id_col0.clone(),
                input_1_id_col1.clone(),
                input_2_id_col2.clone(),
                input_3_id_col3.clone(),
                input_4_id_col4.clone(),
                input_5_id_col5.clone(),
                input_6_id_col6.clone(),
                input_7_id_col7.clone(),
                output_0_id_col8.clone(),
                output_1_id_col9.clone(),
                output_2_id_col10.clone(),
                output_3_id_col11.clone(),
                output_4_id_col12.clone(),
                output_5_id_col13.clone(),
                output_6_id_col14.clone(),
                output_7_id_col15.clone(),
            ],
        ));

        eval.finalize_logup_in_pairs();
        eval
    }
}

#[cfg(test)]
mod tests {
    use num_traits::Zero;
    use rand::rngs::SmallRng;
    use rand::{Rng, SeedableRng};
    use stwo::core::fields::qm31::QM31;
    use stwo_constraint_framework::expr::ExprEvaluator;

    use super::*;

    #[test]
    fn keccak_builtin_constraints_regression() {
        let mut rng = SmallRng::seed_from_u64(0);
        let eval = Eval {
            claim: Claim { log_size: 4 },
            common_lookup_elements: relations::CommonLookupElements::dummy(),
            keccak_builtin_segment_start: rng.random::<u32>(),
        };
        let expr_eval = eval.evaluate(ExprEvaluator::new());
        let assignment = expr_eval.random_assignment();

        let mut sum = QM31::zero();
        for c in expr_eval.constraints {
            sum += c.assign(&assignment) * rng.random::<QM31>();
        }

        constraints_regression_test_values::KECCAK_BUILTIN.assert_debug_eq(&sum);
    }
}
