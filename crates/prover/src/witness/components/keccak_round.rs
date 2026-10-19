// Written by hand, as the component's constraints are (see `cairo_air::components::keccak_round`):
// each row runs one round on 64-bit lanes and writes the 2402 columns in the AIR's order.

use cairo_air::components::keccak_round::{
    Claim, InteractionClaim, KECCAK_ROUND_RELATION_ID, N_TRACE_COLUMNS,
};
use stwo_cairo_common::keccak::{
    IOTA_BITS, N_LIMBS, N_ROUNDS, keccak_round, lanes_to_limbs, limbs_to_lanes,
};

use crate::witness::prelude::*;

pub type InputType = (M31, M31, [M31; N_LIMBS]);
pub type PackedInputType = (PackedM31, PackedM31, [PackedM31; N_LIMBS]);

// The first column of each group of columns, in the AIR's order.
const ENABLER: usize = 0;
const CHAIN: usize = 1;
const ROUND: usize = 2;
const INPUT: usize = 3;
const SELECTORS: usize = INPUT + N_LIMBS;
const C: usize = SELECTORS + N_ROUNDS;
const C_PRIME: usize = C + 5 * 64;
const A_PRIME: usize = C_PRIME + 5 * 64;
const CHI_BITS: usize = A_PRIME + 25 * 64;
const OUTPUT: usize = CHI_BITS + IOTA_BITS.len();
const _: () = assert!(OUTPUT + N_LIMBS == N_TRACE_COLUMNS);

#[derive(Default)]
pub struct ClaimGenerator {
    pub packed_inputs: Mutex<Vec<PackedInputType>>,
    pub remainder_inputs: Mutex<Vec<InputType>>,
}

impl ClaimGenerator {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn write_trace(
        self,
    ) -> (ComponentTrace<N_TRACE_COLUMNS>, Claim, InteractionClaimGenerator) {
        let mut packed_inputs = self.packed_inputs.into_inner().unwrap();
        let remainder_inputs = self.remainder_inputs.into_inner().unwrap();
        let n_active_rows = packed_inputs.len() * N_LANES + remainder_inputs.len();
        add_remainder(&mut packed_inputs, &remainder_inputs);
        assert!(!packed_inputs.is_empty());
        let n_vec_rows = packed_inputs.len();
        let packed_size = n_vec_rows.next_power_of_two();
        let log_size = packed_size.ilog2() + LOG_N_LANES;
        packed_inputs.resize(packed_size, *packed_inputs.first().unwrap());

        let (trace, lookup_data) = write_trace_simd(packed_inputs, n_active_rows);

        (trace, Claim { log_size }, InteractionClaimGenerator { log_size, lookup_data })
    }
}

impl AddInputs for ClaimGenerator {
    type PackedInputType = PackedInputType;
    type InputType = InputType;

    fn add_packed_inputs(&self, inputs: &[PackedInputType], _relation_index: usize) {
        self.packed_inputs.lock().unwrap().extend(inputs);
    }
    fn add_input(&self, input: &InputType, _relation_index: usize) {
        self.remainder_inputs.lock().unwrap().push(*input);
    }
}

/// Writes the columns of the round `(chain, round, state)` into `column(index, value)`, all but
/// the enabler.
fn write_row(input: &InputType, mut column: impl FnMut(usize, u32)) {
    let (chain, round, state) = input;
    let round_index = round.0 as usize;
    let values = keccak_round(&limbs_to_lanes(&state.map(|limb| limb.0)), round_index);
    let bit = |word: u64, z: usize| ((word >> z) & 1) as u32;

    column(CHAIN, chain.0);
    column(ROUND, round.0);
    for (i, limb) in state.iter().enumerate() {
        column(INPUT + i, limb.0);
    }
    for i in 0..N_ROUNDS {
        column(SELECTORS + i, u32::from(i == round_index));
    }
    for x in 0..5 {
        for z in 0..64 {
            column(C + 64 * x + z, bit(values.c[x], z));
            column(C_PRIME + 64 * x + z, bit(values.c_prime[x], z));
            for y in 0..5 {
                column(A_PRIME + 320 * x + 64 * y + z, bit(values.a_prime[x + 5 * y], z));
            }
        }
    }
    for (j, &z) in IOTA_BITS.iter().enumerate() {
        column(CHI_BITS + j, bit(values.chi_lane_0, z));
    }
    for (i, limb) in lanes_to_limbs(&values.output).into_iter().enumerate() {
        column(OUTPUT + i, limb);
    }
}

fn write_trace_simd(
    inputs: Vec<PackedInputType>,
    n_rows: usize,
) -> (ComponentTrace<N_TRACE_COLUMNS>, LookupData) {
    let log_n_packed_rows = inputs.len().ilog2();
    let log_size = log_n_packed_rows + LOG_N_LANES;
    let (mut trace, mut lookup_data) = unsafe {
        (
            ComponentTrace::<N_TRACE_COLUMNS>::uninitialized(log_size),
            LookupData::uninitialized(log_n_packed_rows),
        )
    };
    let enabler_col = Enabler::new(n_rows);
    let relation_id = PackedM31::broadcast(M31::from(KECCAK_ROUND_RELATION_ID));

    (trace.par_iter_mut(), lookup_data.par_iter_mut(), inputs.into_par_iter())
        .into_par_iter()
        .enumerate()
        .for_each(|(row_index, (mut row, lookup_data, keccak_round_input))| {
            let mut lanes = vec![[0u32; N_LANES]; N_TRACE_COLUMNS];
            for (lane, input) in keccak_round_input.unpack().iter().enumerate() {
                write_row(input, |index, value| lanes[index][lane] = value);
            }
            let packed: Vec<PackedM31> =
                lanes.iter().map(|values| PackedM31::from_array(values.map(M31::from))).collect();
            let enabler = enabler_col.packed_at(row_index);
            for (index, cell) in row.iter_mut().enumerate() {
                **cell = if index == ENABLER { enabler } else { packed[index] };
            }

            let (chain, round) = (packed[CHAIN], packed[ROUND]);
            *lookup_data.keccak_round_0 = std::array::from_fn(|i| match i {
                0 => relation_id,
                1 => chain,
                2 => round,
                _ => packed[INPUT + i - 3],
            });
            *lookup_data.keccak_round_1 = std::array::from_fn(|i| match i {
                0 => relation_id,
                1 => chain,
                2 => round + PackedM31::one(),
                _ => packed[OUTPUT + i - 3],
            });
            *lookup_data.mults_0 = enabler;
        });

    (trace, lookup_data)
}

#[derive(Uninitialized, IterMut, ParIterMut)]
struct LookupData {
    keccak_round_0: Vec<[PackedM31; 3 + N_LIMBS]>,
    keccak_round_1: Vec<[PackedM31; 3 + N_LIMBS]>,
    mults_0: Vec<PackedM31>,
}

pub struct InteractionClaimGenerator {
    log_size: u32,
    lookup_data: LookupData,
}
impl InteractionClaimGenerator {
    pub fn write_interaction_trace(
        self,
        common_lookup_elements: &relations::CommonLookupElements,
    ) -> (Vec<CircleEvaluation<SimdBackend, M31, BitReversedOrder>>, InteractionClaim) {
        let mut logup_gen = unsafe { LogupTraceGenerator::uninitialized(self.log_size) };

        // Sum logup terms in pairs.
        let mut col_gen = logup_gen.new_col();
        (
            col_gen.par_iter_mut(),
            &self.lookup_data.keccak_round_0,
            &self.lookup_data.keccak_round_1,
            &self.lookup_data.mults_0,
            &self.lookup_data.mults_0,
        )
            .into_par_iter()
            .for_each(|(writer, values0, values1, mult0, mult1)| {
                let denom0: PackedQM31 = common_lookup_elements.combine(values0);
                let denom1: PackedQM31 = common_lookup_elements.combine(values1);
                writer.write_frac(denom1 * *mult0 - denom0 * *mult1, denom0 * denom1);
            });
        col_gen.finalize_col();

        let (trace, claimed_sum) = logup_gen.finalize_last();

        (trace, InteractionClaim { claimed_sum })
    }
}
