use circuits::blake::{blake_g_gate, m31_to_u32, triple_xor};
use circuits::circuit::Circuit;
use circuits::context::{Context, FinalizedContext, Var};
use circuits::eval;
use circuits::ivalue::{IValue, qm31_from_u32s};
use circuits::ops::{add_into, eq};
use rand_chacha::rand_core::{RngCore, SeedableRng};

use crate::N_LANES;

fn pad_qm31_ops(context: &mut Context<impl IValue>, target: usize) {
    let qm31_ops_n_rows = qm31_ops_n_rows(&context.circuit);
    assert!(qm31_ops_n_rows <= target);
    let one = context.one();
    for _ in qm31_ops_n_rows..target {
        // Use 1 + 1 to work around + zero optimization.
        eval!(context, (one) + (one));
    }
}

fn pad_eq(context: &mut Context<impl IValue>, target: usize) {
    let eq_n_rows = context.circuit.eq.len();
    assert!(eq_n_rows <= target);
    let zero = context.zero();
    for _ in eq_n_rows..target {
        circuits::ops::eq(context, zero, zero);
    }
}

fn pad_triple_xor(context: &mut Context<impl IValue>, target: usize) {
    let n_rows = context.circuit.triple_xor.len();
    assert!(n_rows <= target);
    let zero = context.zero();
    for _ in n_rows..target {
        circuits::blake::triple_xor(context, zero, zero, zero);
    }
}

fn pad_m31_to_u32(context: &mut Context<impl IValue>, target: usize) {
    let n_rows = context.circuit.m31_to_u32.len();
    assert!(n_rows <= target);
    let zero = context.zero();
    for _ in n_rows..target {
        circuits::blake::m31_to_u32(context, zero);
    }
}

fn pad_blake_g_gate(context: &mut Context<impl IValue>, target: usize) {
    let n_rows = context.circuit.blake_g_gate.len();
    assert!(n_rows <= target);
    let zero = context.zero();
    for _ in n_rows..target {
        blake_g_gate(context, zero, zero, zero, zero, zero, zero);
    }
}

/// Pads the components to the next power of two of each component.
/// Appends gates to the context so that the resulting AIR components have length equal to a power
/// of two.
// TODO(Gali): Have it under a trait.
// TODO(Ilya): Make it pub(crate).
pub fn pad_context(context: &mut FinalizedContext<impl IValue>) {
    // Padding the components to a power of two.
    let padded_sizes = compute_padded_sizes(context);
    if context.context.releases_on_padding() {
        pad_to_targets_released(context, &padded_sizes);
    } else {
        pad_to_targets(context, &padded_sizes);
    }
    context.release_if_requested();
}

/// [pad_to_targets] for a context released right after padding, which keeps only its values and
/// `n_vars`: each component's first padding gate is added as usual, and the values of the others,
/// identical to the first's (their inputs are the constant zero and one variables), are appended
/// directly. The released values and variable numbering are those of [pad_to_targets].
fn pad_to_targets_released(context: &mut FinalizedContext<impl IValue>, targets: &ComponentSizes) {
    let inner_context = &mut context.context;
    let n_eq = inner_context.circuit.eq.len();
    pad_released(inner_context, n_eq, targets.eq, |context| {
        let zero = context.zero();
        circuits::ops::eq(context, zero, zero);
    });
    let n_qm31_ops = qm31_ops_n_rows(&inner_context.circuit);
    pad_released(inner_context, n_qm31_ops, targets.qm31_ops, |context| {
        let one = context.one();
        // Use 1 + 1 to work around + zero optimization.
        eval!(context, (one) + (one));
    });
    let n_triple_xor = inner_context.circuit.triple_xor.len();
    pad_released(inner_context, n_triple_xor, targets.triple_xor, |context| {
        let zero = context.zero();
        circuits::blake::triple_xor(context, zero, zero, zero);
    });
    let n_m31_to_u32 = inner_context.circuit.m31_to_u32.len();
    pad_released(inner_context, n_m31_to_u32, targets.m31_to_u32, |context| {
        let zero = context.zero();
        circuits::blake::m31_to_u32(context, zero);
    });
    let n_blake_g_gate = inner_context.circuit.blake_g_gate.len();
    pad_released(inner_context, n_blake_g_gate, targets.blake_g_gate, |context| {
        let zero = context.zero();
        blake_g_gate(context, zero, zero, zero, zero, zero, zero);
    });
}

/// Pads one component from `n_rows` to `target` rows in a context released at padding: one gate
/// as usual, then the values of the other `target - n_rows - 1` gates.
fn pad_released<Value: IValue>(
    context: &mut Context<Value>,
    n_rows: usize,
    target: usize,
    mut gate: impl FnMut(&mut Context<Value>),
) {
    assert!(n_rows <= target);
    if n_rows == target {
        return;
    }
    let before = context.values().len();
    gate(context);
    let pattern = context.values()[before..].to_vec();
    context.append_released_padding_values(&pattern, target - n_rows - 1);
}

/// Pads each component to its target size by appending trivial gates. The target sizes are passed
/// in parameter `targets`.
pub fn pad_to_targets(context: &mut FinalizedContext<impl IValue>, targets: &ComponentSizes) {
    let inner_context = &mut context.context;
    pad_eq(inner_context, targets.eq);
    pad_qm31_ops(inner_context, targets.qm31_ops);
    pad_triple_xor(inner_context, targets.triple_xor);
    pad_m31_to_u32(inner_context, targets.m31_to_u32);
    pad_blake_g_gate(inner_context, targets.blake_g_gate);
}

/// Adds random values to the qm31_ops component via trivial.
/// the padding is
/// x + 0 = x
/// 0 + y = y
/// 0 + z = z
fn qm31_zk_blinding(context: &mut Context<impl IValue>, rng: &mut impl RngCore) {
    let zero = context.zero();
    let value_x = qm31_from_u32s(rng.next_u32(), rng.next_u32(), rng.next_u32(), rng.next_u32());
    let x = context.new_var(IValue::from_qm31(value_x));
    add_into(context, x, zero, x);
    let value_y = qm31_from_u32s(rng.next_u32(), rng.next_u32(), rng.next_u32(), rng.next_u32());
    let y = context.new_var(IValue::from_qm31(value_y));
    add_into(context, zero, y, y);
    let value_z = qm31_from_u32s(rng.next_u32(), rng.next_u32(), rng.next_u32(), rng.next_u32());
    let z = context.new_var(IValue::from_qm31(value_z));
    add_into(context, z, zero, z);
}

/// Adds a random value to the eq component via a trivial `x == x` equality, blinding the eq trace.
///
/// Note that we don't use the guess function here because we want to be able to run this after
/// finalize_guessed_vars.
fn eq_zk_blinding(context: &mut Context<impl IValue>, rng: &mut impl RngCore) {
    let zero = context.zero();
    let value = qm31_from_u32s(rng.next_u32(), rng.next_u32(), rng.next_u32(), rng.next_u32());
    let x = context.new_var(IValue::from_qm31(value));
    // yield x.
    add_into(context, x, zero, x);
    eq(context, x, x);
}

/// Creates a fresh variable holding a random `u32` value, encoded as `(low_u16, high_u16, 0, 0)`,
/// and yields it via a trivial `x + 0 = x` add gate so it can be used as a gate input.
///
/// Note that we don't use the guess function here because we want to be able to run this after
/// finalize_guessed_vars.
fn random_u32_var(context: &mut Context<impl IValue>, rng: &mut impl RngCore) -> Var {
    let zero = context.zero();
    let x = context.new_var(IValue::pack_u32(rng.next_u32()));
    add_into(context, x, zero, x);
    x
}

/// Creates a fresh variable holding a random `M31` value, encoded as `(x, 0, 0, 0)`, and yields it
/// via a trivial `x + 0 = x` add gate so it can be used as a gate input.
///
/// Note that we don't use the guess function here because we want to be able to run this after
/// finalize_guessed_vars.
fn random_m31_var(context: &mut Context<impl IValue>, rng: &mut impl RngCore) -> Var {
    let zero = context.zero();
    let x = context.new_var(IValue::from_qm31(qm31_from_u32s(rng.next_u32(), 0, 0, 0)));
    add_into(context, x, zero, x);
    x
}

/// Adds a random row to the triple_xor component, blinding its trace.
fn triple_xor_zk_blinding(context: &mut Context<impl IValue>, rng: &mut impl RngCore) {
    let a = random_u32_var(context, rng);
    let b = random_u32_var(context, rng);
    let c = random_u32_var(context, rng);
    triple_xor(context, a, b, c);
}

/// Adds a random row to the m31_to_u32 component, blinding its trace.
fn m31_to_u32_zk_blinding(context: &mut Context<impl IValue>, rng: &mut impl RngCore) {
    let input = random_m31_var(context, rng);
    m31_to_u32(context, input);
}

/// Adds a random row to the blake_g_gate component, blinding its trace.
fn blake_g_gate_zk_blinding(context: &mut Context<impl IValue>, rng: &mut impl RngCore) {
    let a = random_u32_var(context, rng);
    let b = random_u32_var(context, rng);
    let c = random_u32_var(context, rng);
    let d = random_u32_var(context, rng);
    let f0 = random_u32_var(context, rng);
    let f1 = random_u32_var(context, rng);
    blake_g_gate(context, a, b, c, d, f0, f1);
}

/// Adds ZK blinding to the circuit by adding random rows to all the witness components: qm31_ops,
/// eq, triple_xor, m31_to_u32, and blake_g_gate. The fresh input variables are yielded through
/// trivial `x + 0 = x` add gates in the qm31_ops component.
pub fn add_zk_blinding(
    context: &mut FinalizedContext<impl IValue>,
    seed_bytes: [u8; 32],
    n_padding: usize,
) {
    let context = &mut context.context;
    let mut rng = rand_chacha::ChaCha20Rng::from_seed(seed_bytes);
    for _ in 0..n_padding {
        qm31_zk_blinding(context, &mut rng);
        eq_zk_blinding(context, &mut rng);
        triple_xor_zk_blinding(context, &mut rng);
        m31_to_u32_zk_blinding(context, &mut rng);
        blake_g_gate_zk_blinding(context, &mut rng);
    }
}

/// Helper struct containing the sizes of the AIR components of a circuit.
#[derive(Debug, Clone, PartialEq)]
pub struct ComponentSizes {
    pub eq: usize,
    pub qm31_ops: usize,
    pub m31_to_u32: usize,
    pub triple_xor: usize,
    pub blake_g_gate: usize,
}

impl ComponentSizes {
    /// Applies `f` to every component's size.
    pub fn map(&self, f: impl Fn(usize) -> usize) -> ComponentSizes {
        ComponentSizes {
            eq: f(self.eq),
            qm31_ops: f(self.qm31_ops),
            m31_to_u32: f(self.m31_to_u32),
            triple_xor: f(self.triple_xor),
            blake_g_gate: f(self.blake_g_gate),
        }
    }

    /// Combines two sets of sizes componentwise with `f`.
    pub fn zip(&self, other: &ComponentSizes, f: impl Fn(usize, usize) -> usize) -> ComponentSizes {
        ComponentSizes {
            eq: f(self.eq, other.eq),
            qm31_ops: f(self.qm31_ops, other.qm31_ops),
            m31_to_u32: f(self.m31_to_u32, other.m31_to_u32),
            triple_xor: f(self.triple_xor, other.triple_xor),
            blake_g_gate: f(self.blake_g_gate, other.blake_g_gate),
        }
    }

    /// The component-wise maximum of two sets of sizes. Padding several circuits to this shared
    /// target is what lets a single AIR verify executions of any of them.
    pub fn elementwise_max(&self, other: &ComponentSizes) -> ComponentSizes {
        self.zip(other, std::cmp::max)
    }
}

// Prints the sizes and the log base 2 rounded up.
impl std::fmt::Display for ComponentSizes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&format!(
            "eq={:>2} (log: {}),  qm31_ops={:>2} (log: {}), m31_to_u32={:>2} (log: {}), \
             triple_xor={:>2} (log: {}), blake_g_gate={:>2} (log: {})",
            self.eq,
            self.eq.next_power_of_two().ilog2(),
            self.qm31_ops,
            self.qm31_ops.next_power_of_two().ilog2(),
            self.m31_to_u32,
            self.m31_to_u32.next_power_of_two().ilog2(),
            self.triple_xor,
            self.triple_xor.next_power_of_two().ilog2(),
            self.blake_g_gate,
            self.blake_g_gate.next_power_of_two().ilog2(),
        ))
    }
}

/// The raw (non-padded) row count of each AIR component, i.e. [`compute_padded_sizes`] before its
/// power-of-two rounding.
pub fn raw_component_sizes(context: &FinalizedContext<impl IValue>) -> ComponentSizes {
    let circuit = &context.context.circuit;
    ComponentSizes {
        eq: circuit.eq.len(),
        qm31_ops: qm31_ops_n_rows(circuit),
        m31_to_u32: circuit.m31_to_u32.len(),
        triple_xor: circuit.triple_xor.len(),
        blake_g_gate: circuit.blake_g_gate.len(),
    }
}

pub fn compute_padded_sizes(context: &FinalizedContext<impl IValue>) -> ComponentSizes {
    raw_component_sizes(context).map(padded_size)
}

/// Computes the number of rows in the qm31_ops AIR component.
pub fn qm31_ops_n_rows(circuit: &Circuit) -> usize {
    circuit.add.len()
        + circuit.sub.len()
        + circuit.mul.len()
        + circuit.pointwise_mul.len()
        + circuit.permutation.iter().map(|p| p.inputs.len() + p.outputs.len()).sum::<usize>()
}

pub fn padded_size(n_rows: usize) -> usize {
    std::cmp::max(n_rows.next_power_of_two(), N_LANES)
}
