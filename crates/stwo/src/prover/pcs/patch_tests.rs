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

/// The tail reads a column that gave up its coefficients for its stripe pair from stripe 0 once
/// the pair is dropped, and a sourced column whose stripes 0 and 1 were evaluated from those: the
/// opening (every sampled value, the quotients through the FRI proof, every queried value and
/// every decommitment) is the one of the same columns kept as coefficients or regrown from their
/// sources, and it verifies.
#[test]
fn stripe0_and_evaluated_pairs_open_as_regrown_columns() {
    use itertools::Itertools;

    use super::*;
    use crate::core::channel::Blake2sChannel;
    use crate::core::circle::SECURE_FIELD_CIRCLE_GEN;
    use crate::core::fri::FriConfig;
    use crate::core::pcs::CommitmentSchemeVerifier;
    use crate::core::vcs_lifted::blake2_merkle::Blake2sMerkleChannel;
    use crate::prover::air::component_prover::TraceSource;
    use crate::prover::backend::simd::SimdBackend;

    struct ValuesSource(Vec<BaseField>);
    impl TraceSource for ValuesSource {
        fn log_size(&self) -> u32 {
            self.0.len().ilog2()
        }
        fn write_values(&self, dst: &mut [BaseField]) {
            dst.copy_from_slice(&self.0);
        }
    }
    let coefficients = |seed: u32, sizes: &[u32]| -> Vec<CircleCoefficients<SimdBackend>> {
        sizes
            .iter()
            .enumerate()
            .map(|(column, &log_size)| {
                CircleCoefficients::new(
                    (0..1u32 << log_size)
                        .map(|i| BaseField::from((seed + 37 * column as u32 + 13 * i) % 1999))
                        .collect(),
                )
            })
            .collect()
    };
    let (trace_log_size, log_blowup_factor) = (13, 2);
    let config = PcsConfig::from_fri_and_trace_size(
        FriConfig::new(0, 0, log_blowup_factor, 6, 1),
        trace_log_size,
    );
    let lifting_log_size = trace_log_size + log_blowup_factor;
    let twiddles = SimdBackend::precompute_twiddles(
        CanonicCoset::new(lifting_log_size + 1).circle_domain().half_coset,
    );
    let (preprocessed_sizes, base_sizes, last_sizes) = ([13, 11, 6], [13, 12, 10, 12, 5], [12, 12]);
    let points = [SECURE_FIELD_CIRCLE_GEN.mul(1 << 40 | 977), SECURE_FIELD_CIRCLE_GEN.mul(1 << 50 | 31)];
    let sampled_points = TreeVec(
        [preprocessed_sizes.len(), base_sizes.len(), last_sizes.len()]
            .iter()
            .map(|&n| (0..n).map(|i| points[..1 + i % 2].to_vec()).collect_vec())
            .collect_vec(),
    );
    let open = |release: bool, evaluate_pairs: bool| {
        let pool = BaseColumnPool::new();
        let channel = &mut Blake2sChannel::default();
        let mut scheme = CommitmentSchemeProver::<SimdBackend, Blake2sMerkleChannel>::new(
            config, &twiddles,
        );
        // The preprocessed tree: two sourced columns and a small one.
        let polys = coefficients(3, &preprocessed_sizes);
        let sources = polys
            .iter()
            .map(|poly| -> Option<SharedTraceSource> {
                (poly.log_size() >= 10).then(|| {
                    let domain = CanonicCoset::new(poly.log_size()).circle_domain();
                    std::sync::Arc::new(ValuesSource(
                        poly.evaluate_with_twiddles(domain, &twiddles).values.to_cpu(),
                    )) as SharedTraceSource
                })
            })
            .collect_vec();
        let tree = CommitmentTreeProver::new_with_sources(
            polys, log_blowup_factor, &twiddles, false, lifting_log_size, &pool, sources,
        );
        assert!(tree.polynomials[0].is_sourced() && tree.polynomials[1].is_sourced());
        if evaluate_pairs {
            for poly in tree.polynomials.iter().filter(|poly| poly.wants_pair()) {
                let _ = poly.pair(&twiddles);
            }
        }
        scheme.commit_tree(MaybeOwned::Owned(tree), channel);
        // A trace tree that keeps stripe pairs, with or without its coefficients.
        scheme.keep_stripe_pair = true;
        scheme.release_pair_coefficients = release;
        let mut builder = scheme.tree_builder();
        builder.extend_polys(coefficients(11, &base_sizes));
        builder.commit(channel);
        scheme.keep_stripe_pair = false;
        scheme.release_pair_coefficients = false;
        assert_eq!(scheme.trees[1].polynomials.iter().any(|poly| poly.pair_only), release);
        scheme.drop_stripe_pairs();
        let mut builder = scheme.tree_builder();
        builder.extend_polys(coefficients(29, &last_sizes));
        builder.commit(channel);
        let regrown = scheme.trees.iter().flat_map(|tree| &tree.polynomials).filter(|poly| {
            poly.coeffs.is_none() && !poly.regrows_from_source() && poly.resident_evals().values.len() > 0
        });
        let n_resident = regrown.count();
        let proof = scheme.prove_values(sampled_points.clone(), channel).proof;
        (format!("{proof:?}"), proof, n_resident)
    };
    let (reference, proof, n_reference) = open(false, false);
    assert_eq!(n_reference, 0);
    for (release, evaluate_pairs) in [(true, false), (false, true), (true, true)] {
        let (opening, _, n_resident) = open(release, evaluate_pairs);
        // Four large base columns keep stripe 0, two preprocessed ones their evaluated pairs.
        assert_eq!(n_resident, 4 * release as usize + 2 * evaluate_pairs as usize);
        assert_eq!(opening, reference, "release {release}, evaluated pairs {evaluate_pairs}");
    }
    let channel = &mut Blake2sChannel::default();
    let mut verifier = CommitmentSchemeVerifier::<Blake2sMerkleChannel>::new(config);
    for (root, sizes) in proof.commitments.iter().zip([&preprocessed_sizes[..], &base_sizes, &last_sizes]) {
        verifier.commit(*root, sizes, channel);
    }
    verifier.verify_values(sampled_points, proof, channel).unwrap();
}
