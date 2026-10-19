use std::array::from_fn;

use air_common::TraceType;
use air_infra::core::air_fn::{AirBuilder, AirFn};
use air_infra::core::constraint_connectedness_test;
use air_infra::core::expressions::felt_expr::FeltExpr;
use air_infra::core::expressions::felt252_expr::Felt252Expr;
use air_infra::core::expressions::uint32_expr::UInt32Expr;
use air_infra::core::variables::AirVar;
use air_infra::felt252_id_memory::memory::{CasmId, Felt252IdMemory};
use air_infra::felt252_id_memory::read_positive::ReadPositiveKnownId;
use air_infra::{const_expr, const_u32_expr};
use serde::Serialize;
use stwo_cairo_common::keccak::*;

use super::keccak_round::*;

/// Computes and verifies one Keccak-f[1600] permutation given the memory ids of its 8 input and 8
/// output felts.
///
/// The inputs are read below 2^200 as 23 memory limbs of 9 bits each (the last one of 2 bits),
/// packed three by three into the 64 limbs of the state, and run through 24 chained rounds. Each
/// output limb is split back into its memory limbs: two of every three are committed and the third
/// follows, and the memory lookup pins them all to the stored felt's 9-bit limbs, so the split is
/// the canonical one.
#[derive(Debug, Serialize, Default)]
pub struct KeccakAggregator {
    #[serde(skip)]
    pub memory: Felt252IdMemory,
}

impl AirFn for KeccakAggregator {
    type ExtIn = ();
    type In = ([CasmId; N_FELTS], [CasmId; N_FELTS]);
    type Out = ();

    fn call(&self, ab: &mut AirBuilder, _: (), (input_ids, output_ids): Self::In) -> Self::Out {
        // The input and output halves are connected only through the round chain: its initial
        // yield and final use share the chain id, which is this row's Seq, and the connectedness
        // test does not see a preprocessed column as a vertex.
        constraint_connectedness_test::exclude(self);

        let read = ReadPositiveKnownId { num_bits: BITS_PER_FELT, memory: self.memory.clone() };
        let inputs: Vec<Vec<FeltExpr>> =
            input_ids.iter().map(|id| ab.call(&read, id.clone()).as_felts()).collect();

        let state: KeccakState = from_fn(|limb| {
            let (felt, group) = (limb / LIMBS_PER_FELT, limb % LIMBS_PER_FELT);
            (0..3).filter(|part| 3 * group + part < MEMORY_LIMBS_PER_FELT).fold(
                const_expr!(0),
                |acc, part| {
                    acc + const_expr!(1u32 << (9 * part)) * inputs[felt][3 * group + part].clone()
                },
            )
        });

        let output = ab.chain_lookup_call(&KeccakRound {}, state, 0, N_ROUNDS);

        for (felt, id) in output_ids.iter().enumerate() {
            let mut memory_limbs = vec![];
            for group in 0..LIMBS_PER_FELT {
                let packed = output[felt * LIMBS_PER_FELT + group].clone();
                let n_parts = (MEMORY_LIMBS_PER_FELT - 3 * group).min(3);
                let mut rest = packed.clone();
                for part in 0..n_parts - 1 {
                    let shifted = ab.let_for_deduction(
                        (UInt32Expr::from(packed.clone()) >> const_u32_expr!(9 * part as u32))
                            & const_u32_expr!(511),
                        "",
                    );
                    let mut limb = shifted.low().as_felt();
                    let limb = ab.deduce(&mut limb, "output_memory_limb");
                    rest = rest - const_expr!(1u32 << (9 * part)) * limb.clone();
                    memory_limbs.push(limb);
                }
                let last_part = n_parts - 1;
                let inverse = const_expr!(1u32 << (9 * last_part)).inverse();
                memory_limbs.push(ab.let_(rest * inverse, "output_memory_limb_last"));
            }
            self.memory.mem_verify_known_id(ab, id, Felt252Expr::from(memory_limbs));
        }
    }

    fn trace_type(&self) -> TraceType {
        TraceType::Component
    }
}
