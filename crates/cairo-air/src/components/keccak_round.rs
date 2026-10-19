// Written by hand: generated, this component's 3049 constraints over 2402 columns unroll into one
// function that takes LLVM over half an hour to compile and whose debug info overflows rustc's
// stack. The columns, constraints and relation entries are those of the AIR
// `airs::casm::builtins::keccak::keccak_round::KeccakRound`, in the same order; the circuit
// verifier's sample evaluation checks that order against the compiled AIR.

use std::array::from_fn;

use stwo_cairo_common::keccak::{
    IOTA_BITS, N_LIMBS, N_ROUNDS, RHO_OFFSETS, bit_of_limb, limb_len, pi_source, round_constant_bit,
};

use crate::components::prelude::*;

pub const N_TRACE_COLUMNS: usize = 2402;
pub const RELATION_USES_PER_ROW: [RelationUse; 1] =
    [RelationUse { relation_id: "KeccakRound", uses: 1 }];

/// The id the AIR infra derives for the `KeccakRound` relation from its name.
pub const KECCAK_ROUND_RELATION_ID: u32 = 384052509;

pub struct Eval {
    pub claim: Claim,
    pub common_lookup_elements: relations::CommonLookupElements,
}

#[derive(Copy, Clone, Serialize, Deserialize, CairoSerialize, CairoDeserialize)]
pub struct Claim {
    pub log_size: u32,
}
impl Claim {
    pub fn log_sizes(&self) -> TreeVec<Vec<u32>> {
        let trace_log_sizes = vec![self.log_size; N_TRACE_COLUMNS];
        let interaction_log_sizes = vec![self.log_size; SECURE_EXTENSION_DEGREE];
        TreeVec::new(vec![trace_log_sizes, interaction_log_sizes])
    }
}

#[derive(Copy, Clone, Serialize, Deserialize, CairoSerialize, CairoDeserialize)]
pub struct InteractionClaim {
    pub claimed_sum: SecureField,
}

pub type Component = FrameworkComponent<Eval>;

/// `a ^ b` for bits, as `a + b (1 - 2a)`.
fn xor<F>(a: &F, b: &F) -> F
where
    F: Clone + From<M31> + std::ops::Add<Output = F> + std::ops::Sub<Output = F>,
    F: std::ops::Mul<Output = F>,
{
    a.clone() + b.clone() * (F::from(M31::from(1)) - F::from(M31::from(2)) * a.clone())
}

/// `b0 ^ (!b1 & b2)` for bits, the chi step, as `b0 + (1 - b1) b2 (1 - 2 b0)`.
fn chi<F>(b0: &F, b1: &F, b2: &F) -> F
where
    F: Clone + From<M31> + std::ops::Add<Output = F> + std::ops::Sub<Output = F>,
    F: std::ops::Mul<Output = F>,
{
    let one = F::from(M31::from(1));
    b0.clone()
        + (one.clone() - b1.clone()) * b2.clone() * (one - F::from(M31::from(2)) * b0.clone())
}

/// `a * (a - 1)`, zero exactly on bits.
fn bit<F>(a: &F) -> F
where
    F: Clone + From<M31> + std::ops::Sub<Output = F> + std::ops::Mul<Output = F>,
{
    a.clone() * (a.clone() - F::from(M31::from(1)))
}

impl FrameworkEval for Eval {
    fn log_size(&self) -> u32 {
        self.claim.log_size
    }

    fn max_constraint_log_degree_bound(&self) -> u32 {
        self.log_size() + 1
    }

    fn evaluate<E: EvalAtRow>(&self, mut eval: E) -> E {
        let constant = |value: u32| E::F::from(M31::from(value));

        let enabler = eval.next_trace_mask();
        let chain = eval.next_trace_mask();
        let round = eval.next_trace_mask();
        let input: [E::F; N_LIMBS] = from_fn(|_| eval.next_trace_mask());
        let selectors: [E::F; N_ROUNDS] = from_fn(|_| eval.next_trace_mask());
        // Indexed [x][z], then [x][y][z].
        let c: [[E::F; 64]; 5] = from_fn(|_| from_fn(|_| eval.next_trace_mask()));
        let c_prime: [[E::F; 64]; 5] = from_fn(|_| from_fn(|_| eval.next_trace_mask()));
        let a_prime: [[[E::F; 64]; 5]; 5] =
            from_fn(|_| from_fn(|_| from_fn(|_| eval.next_trace_mask())));
        let chi_bits: [E::F; IOTA_BITS.len()] = from_fn(|_| eval.next_trace_mask());
        let output: [E::F; N_LIMBS] = from_fn(|_| eval.next_trace_mask());

        eval.add_constraint(enabler.clone() * enabler.clone() - enabler.clone());

        // The round constant comes from a one-hot selector of the round number.
        for selector in &selectors {
            eval.add_constraint(bit(selector));
        }
        let one_hot = selectors.iter().cloned().reduce(|acc, s| acc + s).unwrap();
        eval.add_constraint(one_hot - constant(1));
        let index = (1..N_ROUNDS)
            .map(|i| constant(i as u32) * selectors[i].clone())
            .reduce(|acc, term| acc + term)
            .unwrap();
        eval.add_constraint(index - round.clone());

        for bits in c.iter().chain(&c_prime).chain(a_prime.iter().flatten()) {
            for b in bits {
                eval.add_constraint(bit(b));
            }
        }

        // Theta: d[x][z] = c[x-1][z] ^ c[x+1][z-1]; c' = c ^ d, and a' down a column has parity c'.
        let theta: [[E::F; 64]; 5] = from_fn(|x| {
            from_fn(|z| {
                eval.add_intermediate(xor(&c[(x + 4) % 5][z], &c[(x + 1) % 5][(z + 63) % 64]))
            })
        });
        for x in 0..5 {
            for z in 0..64 {
                eval.add_constraint(c_prime[x][z].clone() - xor(&c[x][z], &theta[x][z]));
                let column =
                    (0..5).map(|y| a_prime[x][y][z].clone()).reduce(|acc, b| acc + b).unwrap();
                let diff = eval.add_intermediate(column - c_prime[x][z].clone());
                eval.add_constraint(
                    diff.clone() * (diff.clone() - constant(2)) * (diff - constant(4)),
                );
            }
        }

        // The input limbs are the bits a' ^ d.
        for (limb, input_limb) in input.iter().enumerate() {
            let value = (0..limb_len(limb))
                .map(|position| {
                    let k = bit_of_limb(limb, position);
                    let (lane, z) = (k / 64, k % 64);
                    let (x, y) = (lane % 5, lane / 5);
                    constant(1 << position) * xor(&a_prime[x][y][z], &theta[x][z])
                })
                .reduce(|acc, term| acc + term)
                .unwrap();
            eval.add_constraint(input_limb.clone() - value);
        }

        // Rho and pi: bit z of lane (x, y) is bit z - rho of a'[x0][y0].
        let b = |x: usize, y: usize, z: usize| -> E::F {
            let (x0, y0) = pi_source(x, y);
            a_prime[x0][y0][(z + 64 - RHO_OFFSETS[x0][y0]) % 64].clone()
        };

        // Chi, and iota on the 7 bits of lane (0, 0) that a round constant can set.
        for (j, chi_bit) in chi_bits.iter().enumerate() {
            let z = IOTA_BITS[j];
            eval.add_constraint(bit(chi_bit));
            eval.add_constraint(chi_bit.clone() - chi(&b(0, 0, z), &b(1, 0, z), &b(2, 0, z)));
        }
        let round_constant = |j: usize| {
            (0..N_ROUNDS)
                .filter(|&i| round_constant_bit(i, j))
                .map(|i| selectors[i].clone())
                .reduce(|acc, s| acc + s)
                .unwrap()
        };
        for (limb, output_limb) in output.iter().enumerate() {
            let value = (0..limb_len(limb))
                .map(|position| {
                    let k = bit_of_limb(limb, position);
                    let (lane, z) = (k / 64, k % 64);
                    let (x, y) = (lane % 5, lane / 5);
                    let out = match IOTA_BITS.iter().position(|&iota| lane == 0 && iota == z) {
                        Some(j) => xor(&chi_bits[j], &round_constant(j)),
                        None => chi(&b(x, y, z), &b((x + 1) % 5, y, z), &b((x + 2) % 5, y, z)),
                    };
                    constant(1 << position) * out
                })
                .reduce(|acc, term| acc + term)
                .unwrap();
            eval.add_constraint(output_limb.clone() - value);
        }

        let relation_id = constant(KECCAK_ROUND_RELATION_ID);
        let input_tuple: Vec<E::F> =
            [relation_id.clone(), chain.clone(), round.clone()].into_iter().chain(input).collect();
        eval.add_to_relation(RelationEntry::new(
            &self.common_lookup_elements,
            E::EF::from(enabler.clone()),
            &input_tuple,
        ));
        let output_tuple: Vec<E::F> =
            [relation_id, chain, round + constant(1)].into_iter().chain(output).collect();
        eval.add_to_relation(RelationEntry::new(
            &self.common_lookup_elements,
            -E::EF::from(enabler),
            &output_tuple,
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
    fn keccak_round_constraints_regression() {
        let mut rng = SmallRng::seed_from_u64(0);
        let eval = Eval {
            claim: Claim { log_size: 4 },
            common_lookup_elements: relations::CommonLookupElements::dummy(),
        };
        let expr_eval = eval.evaluate(ExprEvaluator::new());
        let assignment = expr_eval.random_assignment();

        let mut sum = QM31::zero();
        for c in expr_eval.constraints {
            sum += c.assign(&assignment) * rng.random::<QM31>();
        }

        constraints_regression_test_values::KECCAK_ROUND.assert_debug_eq(&sum);
    }
}
