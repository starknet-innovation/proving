//! [`super::VeryPackedM31Dot`] against the reduced field sum it replaces, word for word.

/// Random lanes in `[0, P]` (often `P`, the raw zero, or `P - 1`), 1 to 39 products.
#[test]
fn dot_matches_field_sum() {
    use rand::rngs::SmallRng;
    use rand::{Rng, SeedableRng};

    use super::VeryPackedM31Dot;
    use crate::core::fields::m31::{M31, P};
    use crate::prover::backend::simd::m31::PackedM31;
    use crate::prover::backend::simd::very_packed_m31::VeryPackedM31;

    let lane = |rng: &mut SmallRng| match rng.random_range(0..4) {
        0 => P,
        1 => P - 1,
        _ => rng.random_range(0..=P),
    };
    let mut rng = SmallRng::seed_from_u64(0);
    for n in 1..40 {
        let value = |rng: &mut SmallRng| {
            VeryPackedM31::from_fn(|_| unsafe {
                PackedM31::from_simd_unchecked(std::array::from_fn(|_| lane(rng)).into())
            })
        };
        let init = value(&mut rng);
        let terms: Vec<(VeryPackedM31, M31)> = (0..n)
            .map(|_| (value(&mut rng), M31::from_u32_unchecked(rng.random_range(P - 3..P))))
            .collect();
        let mut acc = VeryPackedM31Dot::new(init);
        acc.add_products(terms[..n / 2].iter().copied());
        acc.add_products(terms[n / 2..].iter().copied());
        let got = acc.reduce();
        let mut want = init;
        for &(v, c) in &terms {
            want += v * VeryPackedM31::broadcast(c);
        }
        // With a product, the reduced field sum's words are canonical too: the same words.
        let raw = |x: VeryPackedM31| x.0.map(|p| p.into_simd().to_array());
        assert!(raw(got).iter().flatten().all(|&x| x < P));
        assert_eq!(raw(got), raw(want));
    }
}

/// Every combination of edge words (`P` is the raw zero), one and two products, word for word.
#[test]
fn dot_edge_words() {
    use super::VeryPackedM31Dot;
    use crate::core::fields::m31::{M31, P};
    use crate::prover::backend::simd::m31::PackedM31;
    use crate::prover::backend::simd::very_packed_m31::VeryPackedM31;

    let edges = [0, 1, 2, P - 2, P - 1, P];
    let packed =
        |w: u32| VeryPackedM31::from_fn(|_| unsafe { PackedM31::from_simd_unchecked([w; 16].into()) });
    for &init in &edges[..5] {
        for &a in &edges {
            for &b in &edges {
                for &ca in &edges[..5] {
                    for &cb in &edges[..5] {
                        let terms = [(packed(a), M31(ca)), (packed(b), M31(cb))];
                        for k in 1..=2 {
                            let mut acc = VeryPackedM31Dot::new(packed(init));
                            acc.add_products(terms[..k].iter().copied());
                            let mut want = packed(init);
                            for &(v, c) in &terms[..k] {
                                want += v * VeryPackedM31::broadcast(c);
                            }
                            let got = acc.reduce();
                            assert_eq!(got.0[1].into_simd(), want.0[1].into_simd());
                        }
                    }
                }
            }
        }
    }
}
