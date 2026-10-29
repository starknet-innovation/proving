#[test]
fn regenerated_public_tree_matches_cache_and_changed_live_rows_miss() {
    use std::sync::Arc;

    use circuit_common::preprocessed::PreprocessedColumnSource;
    use privacy_circuit_verify::{get_cairo_preprocessed_circuit, get_cairo_verifier_config};
    use stwo::prover::backend::Column;

    use super::*;

    // Changes a real public source without changing its size, ID, or any other source.
    struct ChangedRow {
        source: SharedTraceSource,
        row: usize,
    }

    impl TraceSource for ChangedRow {
        fn log_size(&self) -> u32 {
            self.source.log_size()
        }

        fn write_values(&self, dst: &mut [BaseField]) {
            self.write_rows(0, dst);
        }

        fn write_rows(&self, first: usize, dst: &mut [BaseField]) {
            self.source.write_rows(first, dst);
            if self.row >= first && self.row - first < dst.len() {
                dst[self.row - first] += BaseField::from_u32_unchecked(1);
            }
        }
    }

    let config = get_cairo_verifier_config().unwrap();
    let circuit = get_cairo_preprocessed_circuit(&config);
    let trace = &circuit.preprocessed_trace;
    let sources = PreprocessedColumnSource::all(trace);
    let ids: Vec<_> = trace.ids().into_iter().map(|id| id.id).collect();
    let twiddles = SimdBackend::precompute_twiddles(
        CanonicCoset::new(data::LIFTING).circle_domain().half_coset,
    );
    let pool = BaseColumnPool::<SimdBackend>::new();
    let cached = cached_tree(
        sources.clone(),
        &ids,
        data::LOG_BLOWUP,
        data::LIFTING,
        &twiddles,
        &pool,
    )
    .expect("the complete live fixed circuit matches its public cache");

    // Construct the reference directly, bypassing the cache-enabled privacy helper.
    // Comparing every upper node prevents a circular test of the cached root alone.
    let coefficients = SimdBackend::interpolate_columns(trace.get_trace(), &twiddles);
    let ordinary = CachedTree::new_with_sources(
        coefficients,
        data::LOG_BLOWUP,
        &twiddles,
        false,
        data::LIFTING,
        &pool,
        sources.iter().cloned().map(Some).collect(),
    );
    assert_eq!(ordinary.commitment.root(), cached.commitment.root());
    assert_eq!(ordinary.commitment.layers.len(), cached.commitment.layers.len());
    for depth in 0..=data::BOUNDARY_DEPTH as usize {
        assert_eq!(cached.commitment.layers[depth], ordinary.commitment.layers[depth]);
    }
    assert_eq!(cached.polynomials.len(), ordinary.polynomials.len());
    let mut small_columns = 0;
    for (cached_poly, ordinary_poly) in cached.polynomials.iter().zip(&ordinary.polynomials) {
        assert_eq!(cached_poly.evals.domain, ordinary_poly.evals.domain);
        assert_eq!(cached_poly.is_sourced(), ordinary_poly.is_sourced());
        if !cached_poly.is_sourced() {
            small_columns += 1;
            assert_eq!(cached_poly.evals.values.to_cpu(), ordinary_poly.evals.values.to_cpu());
            assert_eq!(
                cached_poly.coeffs.as_ref().unwrap().coeffs.to_cpu(),
                ordinary_poly.coeffs.as_ref().unwrap().coeffs.to_cpu(),
            );
        }
    }
    assert_eq!(small_columns, 3, "the three log8 columns keep their ordinary representation");
    drop(ordinary);
    drop(cached);
    pool.release_all_idle();

    // A late column and its last row evade a sampled first-subtree guard. All of
    // these cases keep the complete metadata, requiring content checking to miss.
    let column = sources.len() - 1;
    let last_row = (1usize << sources[column].log_size()) - 1;
    for row in [0, 4095, 4096, last_row] {
        let mut changed = sources.clone();
        changed[column] = Arc::new(ChangedRow { source: sources[column].clone(), row });
        assert!(matches_metadata(&changed, &ids, data::LOG_BLOWUP, data::LIFTING));
        assert!(
            cached_tree(changed, &ids, data::LOG_BLOWUP, data::LIFTING, &twiddles, &pool)
                .is_none(),
            "a changed public source row must select the ordinary builder: {row}",
        );
    }

    // The first two fixed columns share the same size. Swapping only their live
    // contents leaves all metadata intact and must still miss the cache.
    let mut swapped = sources;
    swapped.swap(0, 1);
    assert!(matches_metadata(&swapped, &ids, data::LOG_BLOWUP, data::LIFTING));
    assert!(cached_tree(swapped, &ids, data::LOG_BLOWUP, data::LIFTING, &twiddles, &pool).is_none());
}
