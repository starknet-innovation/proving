use air_infra::core::Felt;
use air_infra::core::air_fn_registry::AirFnRegistry;
use air_infra::core::expressions::felt_expr::FeltExpr;
use air_infra::core::expressions::felt252_expr::Felt252Expr;
use air_infra::core::public_params::PublicParam;
use air_infra::felt252_id_memory::memory::Felt252IdMemory;
use air_infra::{const_expr, const_felt252_expr};

use super::keccak_builtin::*;
use super::reference::*;

const SEGMENT_START: u32 = 1000;
const INSTANCE: u32 = 3;

fn addr(offset: u32) -> FeltExpr {
    const_expr!(SEGMENT_START)
        + const_expr!(INSTANCE) * const_expr!(CELLS_PER_KECCAK)
        + const_expr!(offset)
}

/// Runs the builtin on one instance whose memory holds `inputs` and `outputs`.
fn run(inputs: Felts, outputs: Felts) {
    let mut data: Vec<(FeltExpr, Felt252Expr)> = vec![];
    for (i, (low, high)) in inputs.iter().chain(outputs.iter()).enumerate() {
        data.push((addr(i as u32), const_felt252_expr!(*low, *high)));
    }
    let keccak = KeccakBuiltin { memory: Felt252IdMemory::new_with_data(data) };
    let mut registry = AirFnRegistry::new_empty();
    registry.public_params.set(PublicParam::KeccakBuiltinSegmentStart, Felt::from(SEGMENT_START));
    registry.add_entry(&keccak);
    registry.run_air_with_row_number(&keccak, (), (), INSTANCE as usize);
}

/// The 8 felts of a state, each as (low 128 bits, high 72 bits).
type Felts = [(u128, u128); 8];

fn permutation(lanes: [u64; 25]) -> (Felts, Felts) {
    let mut output = lanes;
    keccak_f(&mut output);
    (lanes_to_felts(&lanes), lanes_to_felts(&output))
}

#[test]
fn test_keccak_builtin_zero_state() {
    let (inputs, outputs) = permutation([0; 25]);
    run(inputs, outputs);
}

#[test]
fn test_keccak_builtin_full_state() {
    let lanes = std::array::from_fn(|i| 0x0123456789abcdefu64.rotate_left(i as u32) ^ !(i as u64));
    let (inputs, outputs) = permutation(lanes);
    run(inputs, outputs);
}

#[test]
#[should_panic(expected = "given value != value in memory")]
fn test_keccak_builtin_wrong_output() {
    let (inputs, mut outputs) = permutation([7; 25]);
    outputs[5].0 ^= 1 << 77;
    run(inputs, outputs);
}

#[test]
#[should_panic(expected = "Added incorrect constraint")]
fn test_keccak_builtin_input_top_limb_above_2_bits() {
    // Bit 200 of a felt is bit 2 of its last memory limb, which may hold only 2 bits.
    let (mut inputs, outputs) = permutation([0; 25]);
    inputs[2].1 |= 1 << 72;
    run(inputs, outputs);
}

#[test]
#[should_panic(expected = "given value != value in memory")]
fn test_keccak_builtin_input_above_207_bits() {
    // Bit 210 of a felt is in its 24th memory limb, which must be zero.
    let (mut inputs, outputs) = permutation([0; 25]);
    inputs[6].1 |= 1 << 82;
    run(inputs, outputs);
}
