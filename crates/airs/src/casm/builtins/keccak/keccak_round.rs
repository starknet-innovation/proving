use std::array::from_fn;

use air_common::TraceType;
use air_infra::core::air_fn::{AirBuilder, AirFn, ChainRoundAirFn};
use air_infra::core::expressions::felt_expr::FeltExpr;
use air_infra::core::expressions::uint16_expr::UInt16Expr;
use air_infra::core::expressions::uint32_expr::UInt32Expr;
use air_infra::core::variables::{ChainIdVar, RoundNumVar};
use air_infra::{const_expr, const_u16_expr, const_u32_expr};
use serde::Serialize;
use stwo_cairo_common::keccak::*;

/// The Keccak-f[1600] state between rounds: 64 limbs (see `stwo_cairo_common::keccak`).
pub type KeccakState = [FeltExpr; N_LIMBS];

/// One round of Keccak-f[1600] per row, chained 24 times per permutation.
///
/// The row commits the state bit-sliced, as in Plonky3's keccak-air: `c[x][z]`, the column parities
/// of the input state A; `c'[x][z] = c[x][z] ^ d[x][z]`, with theta's term
/// `d[x][z] = c[x-1][z] ^ c[x+1][z-1]`; and `a'[x][y][z]`, theta applied to A. The input limbs are
/// constrained to equal the bits `a' ^ d`, and the parity of `a'` down a column to `c'`, which pins
/// `c` to A's column parities and `a'` to theta(A). Rho and pi are a renaming of `a'`'s bits; chi
/// and iota yield the output limbs, degree 3. Iota only touches 7 bits of lane (0, 0); those chi
/// bits are committed, and the round constant is read from a one-hot selector of the round number.
#[derive(Clone, Debug, Serialize)]
pub struct KeccakRound {}

/// `a ^ b` for bits, as `a + b (1 - 2a)`.
fn xor(a: &FeltExpr, b: &FeltExpr) -> FeltExpr {
    a.clone() + b.clone() * (const_expr!(1) - const_expr!(2) * a.clone())
}

/// `b0 ^ (!b1 & b2)` for bits, the chi step, as `b0 + (1 - b1) b2 (1 - 2 b0)`, degree 3.
fn chi(b0: &FeltExpr, b1: &FeltExpr, b2: &FeltExpr) -> FeltExpr {
    b0.clone()
        + (const_expr!(1) - b1.clone())
            * b2.clone()
            * (const_expr!(1) - const_expr!(2) * b0.clone())
}

/// The sum of `terms` as a balanced tree, so that the expression stays shallow.
fn sum(mut terms: Vec<FeltExpr>) -> FeltExpr {
    match terms.len() {
        0 => const_expr!(0),
        1 => terms.pop().expect("one term"),
        n => {
            let right = terms.split_off(n / 2);
            sum(terms) + sum(right)
        }
    }
}

fn constrain_bit(ab: &mut AirBuilder, bit: &FeltExpr) {
    ab.constrain(bit.clone() * (bit.clone() - const_expr!(1)), "bit");
}

/// Deduces a committed bit from a deduction-only expression whose value is 0 or 1.
fn deduce_bit(ab: &mut AirBuilder, value: UInt16Expr, desc: &str) -> FeltExpr {
    let bit = ab.deduce_air_var(value, desc).as_felt();
    constrain_bit(ab, &bit);
    bit
}

impl AirFn for KeccakRound {
    type ExtIn = ();
    type In = (ChainIdVar, RoundNumVar, KeccakState);
    type Out = (ChainIdVar, RoundNumVar, KeccakState);

    fn trace_type(&self) -> TraceType {
        TraceType::ChainRound
    }

    fn call(&self, ab: &mut AirBuilder, _: (), (chain, round, input): Self::In) -> Self::Out {
        // A one-hot selector of the round number, for the round constant.
        let selectors: [FeltExpr; N_ROUNDS] = from_fn(|i| {
            let is_round = round.clone().eq(const_expr!(i));
            let selector = ab.deduce_air_var(is_round, "round_selector").as_felt();
            constrain_bit(ab, &selector);
            selector
        });
        let one_hot = sum(selectors.to_vec());
        ab.constrain(one_hot - const_expr!(1), "one round selector is set");
        let index = sum((1..N_ROUNDS).map(|i| const_expr!(i) * selectors[i].clone()).collect());
        ab.constrain(index - round.clone(), "the selector is the round number");

        // The input state's bits, for deductions only.
        let a: Vec<UInt16Expr> = (0..1600)
            .map(|k| {
                let (limb, position) = limb_of_bit(k);
                let shifted = ab.let_for_deduction(
                    (UInt32Expr::from(input[limb].clone()) >> const_u32_expr!(position as u32))
                        & const_u32_expr!(1),
                    "",
                );
                shifted.low()
            })
            .collect();
        let a_at = |x: usize, y: usize, z: usize| a[state_bit(x, y, z)].clone();

        // Theta: column parities c, the shifted parities c' and the output a'.
        // Kept on the heap: 2240 expressions overflow a test thread's stack as arrays.
        let c: Vec<Vec<FeltExpr>> = (0..5)
            .map(|x| {
                (0..64)
                    .map(|z| {
                        let parity = a_at(x, 0, z)
                            ^ a_at(x, 1, z)
                            ^ a_at(x, 2, z)
                            ^ a_at(x, 3, z)
                            ^ a_at(x, 4, z);
                        deduce_bit(ab, parity, "c")
                    })
                    .collect()
            })
            .collect();
        let d = |x: usize, z: usize| {
            UInt16Expr::from(c[(x + 4) % 5][z].clone())
                ^ UInt16Expr::from(c[(x + 1) % 5][(z + 63) % 64].clone())
        };
        let c_prime: Vec<Vec<FeltExpr>> = (0..5)
            .map(|x| {
                (0..64)
                    .map(|z| deduce_bit(ab, UInt16Expr::from(c[x][z].clone()) ^ d(x, z), "c_prime"))
                    .collect()
            })
            .collect();
        let a_prime: Vec<Vec<Vec<FeltExpr>>> = (0..5)
            .map(|x| {
                (0..5)
                    .map(|y| {
                        (0..64)
                            .map(|z| deduce_bit(ab, a_at(x, y, z) ^ d(x, z), "a_prime"))
                            .collect()
                    })
                    .collect()
            })
            .collect();

        // Theta's term d[x][z] = c[x-1][z] ^ c[x+1][z-1], degree 2, shared by the column's bits.
        let theta: Vec<Vec<FeltExpr>> = (0..5)
            .map(|x| {
                (0..64)
                    .map(|z| {
                        let term = xor(&c[(x + 4) % 5][z], &c[(x + 1) % 5][(z + 63) % 64]);
                        ab.let_for_constraint(term, "theta_term")
                    })
                    .collect()
            })
            .collect();

        for x in 0..5 {
            for z in 0..64 {
                ab.constrain(
                    c_prime[x][z].clone() - xor(&c[x][z], &theta[x][z]),
                    "c_prime is c xor its theta term",
                );
                let column = sum((0..5).map(|y| a_prime[x][y][z].clone()).collect());
                let diff = ab.let_for_constraint(column - c_prime[x][z].clone(), "");
                ab.constrain(
                    diff.clone() * (diff.clone() - const_expr!(2)) * (diff - const_expr!(4)),
                    "a_prime's column parity is c_prime",
                );
            }
        }

        // The input limbs are the bits a' ^ d (A = theta(A) ^ d).
        for (limb, bits) in bits_of_limbs().iter().enumerate() {
            let value = sum(bits
                .iter()
                .enumerate()
                .map(|(position, &k)| {
                    let (lane, z) = (k / 64, k % 64);
                    let (x, y) = (lane % 5, lane / 5);
                    const_expr!(1u32 << position) * xor(&a_prime[x][y][z], &theta[x][z])
                })
                .collect());
            ab.constrain(input[limb].clone() - value, "input limb is a_prime xor its theta term");
        }

        // Rho and pi: b[y][2x + 3y] is a'[x][y] rotated left by the rho offset.
        let b = |x: usize, y: usize, z: usize| -> FeltExpr {
            // b[x][y] comes from a'[x0][y0] with x = y0 and y = 2 x0 + 3 y0.
            let y0 = x;
            let x0 = (0..5).find(|x0| (2 * x0 + 3 * y0) % 5 == y).expect("pi is a bijection");
            a_prime[x0][y0][(z + 64 - RHO_OFFSETS[x0][y0]) % 64].clone()
        };

        // Chi, and iota on lane (0, 0).
        let round_constant = |j: usize| {
            sum((0..N_ROUNDS)
                .filter(|&i| round_constant_bit(i, j))
                .map(|i| selectors[i].clone())
                .collect())
        };
        let chi_bits: [FeltExpr; IOTA_BITS.len()] = from_fn(|j| {
            let z = IOTA_BITS[j];
            let not_b1 = UInt16Expr::from(b(1, 0, z)) ^ const_u16_expr!(1);
            let value = UInt16Expr::from(b(0, 0, z)) ^ (not_b1 & UInt16Expr::from(b(2, 0, z)));
            let bit = deduce_bit(ab, value, "chi_bit");
            ab.constrain(bit.clone() - chi(&b(0, 0, z), &b(1, 0, z), &b(2, 0, z)), "chi bit");
            bit
        });
        let output_bit = |k: usize| -> FeltExpr {
            let (lane, z) = (k / 64, k % 64);
            let (x, y) = (lane % 5, lane / 5);
            match IOTA_BITS.iter().position(|&iota| lane == 0 && iota == z) {
                Some(j) => xor(&chi_bits[j], &round_constant(j)),
                None => chi(&b(x, y, z), &b((x + 1) % 5, y, z), &b((x + 2) % 5, y, z)),
            }
        };

        let output: KeccakState = {
            let limbs = bits_of_limbs();
            from_fn(|limb| {
                let value = sum(limbs[limb]
                    .iter()
                    .enumerate()
                    .map(|(position, &k)| const_expr!(1u32 << position) * output_bit(k))
                    .collect());
                let column = ab.deduce(&mut value.clone(), "output_limb");
                ab.constrain(column.clone() - value, "output limb is chi and iota");
                column
            })
        };

        (chain, round + const_expr!(1), output)
    }
}

impl ChainRoundAirFn<KeccakState> for KeccakRound {
    fn number_of_chains(&self) -> usize {
        1
    }
}
