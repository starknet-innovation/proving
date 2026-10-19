use std::array::from_fn;

use air_common::TraceType;
use air_infra::casm_state::CasmAddress;
use air_infra::const_expr;
use air_infra::core::air_fn::{AirBuilder, AirFn};
use air_infra::core::expressions::felt_expr::FeltExpr;
use air_infra::core::public_params::PublicParam;
use air_infra::felt252_id_memory::memory::{CasmId, Felt252IdMemory};
use air_infra::felt252_id_memory::read_id::ReadId;
use air_infra::seq::Seq;
use serde::Serialize;
use stwo_cairo_common::keccak::*;

use super::keccak_aggregator::*;

/// Each Keccak operation consists of 16 cells: 8 input felts and 8 output felts, each below 2^200.
pub const CELLS_PER_KECCAK: u32 = 16;

/// The builtin enforces that
///   KeccakF1600(mem[addr : addr + 8]) = mem[addr + 8 : addr + 16]
/// where addr = KeccakBuiltinSegmentStart + Seq * 16, and the 1600-bit state is the little-endian
/// concatenation of the 8 felts, 200 bits each (as the Cairo VM's keccak builtin defines it).
#[derive(Debug, Serialize, Default)]
pub struct KeccakBuiltin {
    #[serde(skip)]
    pub memory: Felt252IdMemory,
}

impl AirFn for KeccakBuiltin {
    type ExtIn = ();
    type In = ();
    type Out = ();

    fn call(&self, ab: &mut AirBuilder, _: (), _: ()) -> Self::Out {
        let instance_num = ab.call_external_table(&Seq {});
        let segment_start = ab.get_public_param(PublicParam::KeccakBuiltinSegmentStart);
        let instance_addr =
            ab.let_(instance_num * const_expr!(CELLS_PER_KECCAK) + segment_start, "instance_addr");

        let read_id = |ab: &mut AirBuilder, offset: usize, desc: &str| -> CasmId {
            let address = CasmAddress::new(instance_addr.clone() + const_expr!(offset), desc);
            ab.call(&ReadId { memory: self.memory.clone() }, address)
        };
        let input_ids: [CasmId; N_FELTS] = from_fn(|i| read_id(ab, i, &format!("input_{i}")));
        let output_ids: [CasmId; N_FELTS] =
            from_fn(|i| read_id(ab, N_FELTS + i, &format!("output_{i}")));

        ab.lookup_call(
            &KeccakAggregator { memory: self.memory.clone() },
            (),
            (input_ids, output_ids),
        );
    }

    fn trace_type(&self) -> TraceType {
        TraceType::Builtin
    }
}
