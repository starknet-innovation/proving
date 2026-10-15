#[derive(Debug, Default, PartialEq)]
pub struct Stats {
    pub equals: usize,
    pub add: usize,
    pub sub: usize,
    pub mul: usize,
    /// The number of inversions. Note that each inversion also increments [Self::mul],
    /// [Self::guess] and [Self::equals] by 1.
    pub inv: usize,
    /// The number of divisions. Note that each division also increments [Self::mul],
    /// [Self::guess] and [Self::equals] by 1. The caller must ensure the divisor is non-zero.
    pub div: usize,
    pub pointwise_mul: usize,
    pub guess: usize,
    pub blake_updates: usize,
    /// The number of inputs to permutation gates.
    pub permutation_inputs: usize,
    pub outputs: usize,
    pub triple_xor: usize,
    pub m31_to_u32: usize,
}

impl Stats {
    /// Adds the counts of `other` (a piece of [`Context::build_in_parallel`]).
    ///
    /// [`Context::build_in_parallel`]: crate::context::Context::build_in_parallel
    pub fn add_assign(&mut self, other: &Stats) {
        let Stats {
            equals,
            add,
            sub,
            mul,
            inv,
            div,
            pointwise_mul,
            guess,
            blake_updates,
            permutation_inputs,
            outputs,
            triple_xor,
            m31_to_u32,
        } = other;
        self.equals += equals;
        self.add += add;
        self.sub += sub;
        self.mul += mul;
        self.inv += inv;
        self.div += div;
        self.pointwise_mul += pointwise_mul;
        self.guess += guess;
        self.blake_updates += blake_updates;
        self.permutation_inputs += permutation_inputs;
        self.outputs += outputs;
        self.triple_xor += triple_xor;
        self.m31_to_u32 += m31_to_u32;
    }
}
