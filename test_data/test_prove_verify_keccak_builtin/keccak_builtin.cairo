%builtins output pedersen range_check ecdsa bitwise ec_op keccak poseidon range_check96 add_mod mul_mod

from starkware.cairo.common.cairo_builtins import KeccakBuiltin
from starkware.cairo.common.keccak_state import KeccakBuiltinState

// Applies Keccak-f[1600] `n` times through the keccak builtin, each output feeding the next input,
// and returns the final state.
func chain_keccak{keccak_ptr: KeccakBuiltin*}(
    n: felt, state: KeccakBuiltinState
) -> KeccakBuiltinState {
    if (n == 0) {
        return state;
    }
    assert keccak_ptr.input = state;
    let output = keccak_ptr.output;
    let keccak_ptr = keccak_ptr + KeccakBuiltin.SIZE;
    return chain_keccak(n=n - 1, state=output);
}

func main{
    output_ptr: felt*,
    pedersen_ptr,
    range_check_ptr,
    ecdsa_ptr,
    bitwise_ptr,
    ec_op_ptr,
    keccak_ptr: KeccakBuiltin*,
    poseidon_ptr,
    range_check96_ptr,
    add_mod_ptr,
    mul_mod_ptr,
}() {
    alloc_locals;
    // 20 chained permutations, then 1 from the all-zero state: 21 instances, padded to 32.
    let start = KeccakBuiltinState(s0=1, s1=2, s2=3, s3=4, s4=5, s5=6, s6=7, s7=2 ** 200 - 1);
    let final = chain_keccak(n=20, state=start);
    let zero = KeccakBuiltinState(s0=0, s1=0, s2=0, s3=0, s4=0, s5=0, s6=0, s7=0);
    let of_zero = chain_keccak(n=1, state=zero);

    // Keccak-f[1600] of the zero state: its first lane is 0xf1258f7940e1dde7.
    assert [output_ptr] = final.s0;
    assert [output_ptr + 1] = of_zero.s0;
    let output_ptr = output_ptr + 2;
    return ();
}
