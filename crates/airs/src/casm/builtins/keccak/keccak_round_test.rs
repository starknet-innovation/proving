use std::array::from_fn;

use air_infra::const_expr;
use air_infra::core::air_fn_registry::AirFnRegistry;
use air_infra::core::expressions::felt_expr::FeltExpr;
use air_infra::core::variables::AsProverType;
use stwo_cairo_common::keccak::*;

use super::keccak_round::*;
use super::reference::*;

fn pseudo_random_lanes(seed: u64) -> [u64; 25] {
    let mut x = seed.wrapping_mul(0x9e3779b97f4a7c15) | 1;
    from_fn(|_| {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        x
    })
}

fn run_round(lanes: [u64; 25], round: usize) -> [u64; 25] {
    let air_fn = KeccakRound {};
    let (registry, _) = AirFnRegistry::new(&air_fn);
    let limbs = lanes_to_limbs(&lanes);
    let state: KeccakState = from_fn(|i| const_expr!(limbs[i]));
    let (_, (_, next_round, output)) =
        registry.run_air(&air_fn, (), (const_expr!(0), const_expr!(round), state));
    assert_eq!(next_round.calc(), (round + 1).to_string());
    let output: [u32; N_LIMBS] = from_fn(|i| output[i].calc().parse().unwrap());
    limbs_to_lanes(&output)
}

#[test]
fn test_keccak_round_matches_reference() {
    for (seed, round) in [(1, 0), (2, 1), (3, 7), (4, 13), (5, 23)] {
        let lanes = pseudo_random_lanes(seed);
        let mut expected = lanes;
        reference_round(&mut expected, round);
        assert_eq!(run_round(lanes, round), expected, "round {round}");
    }
}

#[test]
fn test_keccak_permutation_by_rounds() {
    let mut lanes = [0u64; 25];
    for round in 0..N_ROUNDS {
        lanes = run_round(lanes, round);
    }
    assert_eq!(lanes[0], 0xf1258f7940e1dde7);
}

#[allow(dead_code)]
fn unused(_: FeltExpr) {}
