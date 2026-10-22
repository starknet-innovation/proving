/// A tree committed as double stripes (stripes 2k and 2k + 1 hashed together, their layers kept
/// whole for the gather) is the tree committed one stripe at a time: every layer and the root.
#[test]
fn double_stripe_tree_matches_single_stripes() {
    use itertools::Itertools;

    use super::*;
    use crate::core::vcs_lifted::blake2_merkle::Blake2sMerkleChannel;
    use crate::prover::backend::simd::SimdBackend;

    type Tree = CommitmentTreeProver<SimdBackend, Blake2sMerkleChannel>;

    let coefficients = |seed: u32, sizes: &[u32]| -> Vec<CircleCoefficients<SimdBackend>> {
        sizes
            .iter()
            .enumerate()
            .map(|(column, &log_size)| {
                CircleCoefficients::new(
                    (0..1 << log_size)
                        .map(|i| BaseField::from(seed + 37 * column as u32 + 13 * i))
                        .collect(),
                )
            })
            .collect()
    };
    let log_blowup_factor = 3;
    for (seed, sizes, lifting_log_size) in [
        (5, vec![10, 8, 10, 4], 13),
        (17, vec![12, 11, 12, 6, 9], 15),
        (31, vec![13, 13], 16),
    ] {
        let twiddles = SimdBackend::precompute_twiddles(
            CanonicCoset::new(lifting_log_size).circle_domain().half_coset,
        );
        let pool = BaseColumnPool::new();
        let make_tree = |keep_pair| {
            Tree::new_ex_with_sources(
                coefficients(seed, &sizes),
                log_blowup_factor,
                &twiddles,
                false,
                lifting_log_size,
                &pool,
                true,
                vec![None; sizes.len()],
                keep_pair,
            )
        };
        let single = make_tree(false);
        let double = make_tree(true);
        assert!(single.is_striped() && double.is_striped(), "sizes {sizes:?}");
        assert_eq!(single.commitment.root(), double.commitment.root(), "sizes {sizes:?}");
        assert_eq!(single.commitment.layers.len(), double.commitment.layers.len());
        for (a, b) in single.commitment.layers.iter().zip_eq(&double.commitment.layers) {
            assert_eq!(a.to_cpu(), b.to_cpu(), "sizes {sizes:?}");
        }
    }
}
