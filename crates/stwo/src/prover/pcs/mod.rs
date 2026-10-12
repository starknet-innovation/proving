use std::array;

use hashbrown::HashMap;
use itertools::Itertools;
#[cfg(feature = "parallel")]
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use tracing::{Level, info, span};

use crate::core::ColumnVec;
use crate::core::channel::{Channel, MerkleChannel};
use crate::core::circle::CirclePoint;
use crate::core::fields::m31::BaseField;
use crate::core::fields::qm31::SecureField;
use crate::core::pcs::quotients::{
    CommitmentSchemeProof, CommitmentSchemeProofAux, ExtendedCommitmentSchemeProof, PointSample,
};
use crate::core::pcs::utils::prepare_preprocessed_query_positions;
use crate::core::pcs::{PcsConfig, TreeSubspan, TreeVec};
use crate::core::poly::circle::{CanonicCoset, CircleDomain};
use crate::core::utils::MaybeOwned;
use crate::core::vcs_lifted::merkle_hasher::MerkleHasherLifted;
use crate::core::vcs_lifted::verifier::ExtendedMerkleDecommitmentLifted;
use crate::prover::air::component_prover::{Poly, Trace, WeightsHashMap};
use crate::prover::backend::{BackendForChannel, Col, Column};
use crate::prover::fri::{FriDecommitResult, FriProver};
use crate::prover::mempool::BaseColumnPool;
use crate::prover::air::component_prover::{SharedTraceSource, source_column};
use crate::prover::pcs::quotient_ops::compute_fri_quotients_with_regrowth;
use crate::prover::poly::BitReversedOrder;
use crate::prover::poly::circle::{CircleCoefficients, CircleEvaluation, PolyOps};
use crate::prover::poly::twiddles::TwiddleTree;
use crate::prover::secure_column::SecureColumnByCoords;
use crate::prover::vcs_lifted::prover::MerkleProverLifted;

pub mod quotient_ops;

/// The prover side of a FRI polynomial commitment scheme. See [super].
pub struct CommitmentSchemeProver<'a, B: BackendForChannel<MC>, MC: MerkleChannel> {
    pub trees: TreeVec<MaybeOwned<'a, CommitmentTreeProver<B, MC>>>,
    pub config: PcsConfig,
    pub twiddles: &'a TwiddleTree<B>,
    pub store_polynomials_coefficients: bool,
    /// Pre-allocated base field column pool for polynomial evaluation during commit.
    pub base_column_pool: MaybeOwned<'a, BaseColumnPool<B>>,
}
impl<'a, B: BackendForChannel<MC>, MC: MerkleChannel> CommitmentSchemeProver<'a, B, MC> {
    /// Creates a new empty commitment scheme prover with the given configuration and twiddles. The
    /// commitment scheme does not store the polynomials coefficients by default.
    pub fn new(config: PcsConfig, twiddles: &'a TwiddleTree<B>) -> Self {
        CommitmentSchemeProver {
            trees: TreeVec::default(),
            config,
            twiddles,
            store_polynomials_coefficients: false,
            base_column_pool: MaybeOwned::Owned(BaseColumnPool::new()),
        }
    }

    pub fn with_memory_pool(
        config: PcsConfig,
        twiddles: &'a TwiddleTree<B>,
        base_column_pool: &'a BaseColumnPool<B>,
    ) -> Self {
        base_column_pool.release_small_idle();
        CommitmentSchemeProver {
            trees: TreeVec::default(),
            config,
            twiddles,
            store_polynomials_coefficients: false,
            base_column_pool: MaybeOwned::Borrowed(base_column_pool),
        }
    }

    /// Sets the commitment scheme to store the polynomials coefficients starting from the next
    /// commit.
    pub const fn set_store_polynomials_coefficients(&mut self) {
        self.store_polynomials_coefficients = true;
    }

    /// Evaluates the given polynomials, commits them into a Merkle tree, mixes the root into
    /// the channel, and appends the resulting tree to the scheme.
    ///
    /// [`PcsConfig::lifting_log_size`] gives the tree its height by index: the first tree
    /// committed is the preprocessed one, every later one a trace tree.
    fn commit(&mut self, polynomials: ColumnVec<CircleCoefficients<B>>, channel: &mut MC::C) {
        let _span = span!(Level::INFO, "Commitment").entered();
        let lifting_log_size = self.config.lifting_log_size(self.trees.len());
        crate::prover::F2_TREE.store(self.trees.len(), std::sync::atomic::Ordering::Relaxed);
        let mut tree = CommitmentTreeProver::new_ex(
            polynomials,
            self.config.fri_config.log_blowup_factor,
            self.twiddles,
            self.store_polynomials_coefficients,
            lifting_log_size,
            &self.base_column_pool,
            true,
            // `M11_PACK_LEGS`: bit 0 packs the circuit leg's base tree (log blowup 2), bit 1 the Cairo
            // leg's (log blowup 3, which the evaluator's large-Cairo guard times on its own).
            if self.trees.len() == 1
                && crate::prover::m11_env("M11_PACK_LEGS", 3)
                    & if self.config.fri_config.log_blowup_factor == 2 { 1 } else { 2 }
                    != 0
            {
                crate::prover::m11_env("M11_PACK", 0) as u32
            } else {
                0
            },
        );
        crate::prover::F2_TREE.store(usize::MAX, std::sync::atomic::Ordering::Relaxed);
        MC::mix_root(channel, tree.commitment.root());
        // Only newly owned ordinary commitments are considered. Externally supplied or borrowed
        // preprocessed trees use commit_tree and retain their existing storage behavior.
        for poly in &mut tree.polynomials {
            poly.evals.values.release_zeroed_pages();
        }
        self.trees.push(MaybeOwned::Owned(tree));
    }

    /// Appends an externally constructed [`CommitmentTreeProver`] to the scheme and mixes its
    /// Merkle root into the channel. Accepts both owned and borrowed trees.
    pub fn commit_tree(
        &mut self,
        tree: MaybeOwned<'a, CommitmentTreeProver<B, MC>>,
        channel: &mut MC::C,
    ) {
        MC::mix_root(channel, tree.commitment.root());
        self.trees.push(tree);
    }

    pub fn tree_builder(&mut self) -> TreeBuilder<'_, 'a, B, MC> {
        TreeBuilder { tree_index: self.trees.len(), commitment_scheme: self, polys: Vec::default() }
    }

    pub fn roots(&self) -> TreeVec<<MC::H as MerkleHasherLifted>::Hash> {
        self.trees.as_ref().map(|tree| tree.commitment.root())
    }

    pub fn polynomials(&self) -> TreeVec<ColumnVec<&Poly<B>>> {
        self.trees.as_ref().map(|tree| tree.polynomials.iter().collect())
    }

    pub fn evaluations(
        &self,
    ) -> TreeVec<ColumnVec<&CircleEvaluation<B, BaseField, BitReversedOrder>>> {
        self.trees.as_ref().map(|tree| tree.polynomials.iter().map(|poly| &poly.evals).collect())
    }

    /// Turns the stripe 0 of every striped column into the column's coefficients, in place (the
    /// interpolation reuses the buffer), so that composition and the out-of-domain samples
    /// extend and evaluate from coefficients instead of regrowing them from stripe 0 at every
    /// use. [`Self::prove_values`] turns them back before the quotients read stripe 0.
    pub fn striped_to_coefficients(&mut self) {
        let _span = span!(Level::INFO, "Striped to coefficients").entered();
        for tree in &mut self.trees.0 {
            if let MaybeOwned::Owned(tree) = tree {
                let to_coefficients = |poly: &mut Poly<B>| {
                    if poly.coeffs.is_none()
                        && !poly.is_sourced()
                        && poly.evals.values.len() < poly.evals.domain.size()
                    {
                        poly.coeffs = Some(poly.take_regrown_coefficients());
                    }
                };
                #[cfg(feature = "parallel")]
                {
                    // SCREEN (r77, n9 LPT2): `R77_LPT2=1` takes the largest columns first, one
                    // column per task.
                    if false {
                        use rayon::iter::{IndexedParallelIterator, IntoParallelIterator, ParallelIterator};
                        let mut polys = tree.polynomials.iter_mut().collect::<Vec<_>>();
                        polys.sort_by_key(|poly| std::cmp::Reverse(poly.evals.domain.log_size()));
                        polys.into_par_iter().with_max_len(1).for_each(|poly| to_coefficients(poly));
                    } else {
                        use rayon::iter::IntoParallelRefMutIterator;
                        tree.polynomials.par_iter_mut().for_each(to_coefficients);
                    }
                }
                #[cfg(not(feature = "parallel"))]
                tree.polynomials.iter_mut().for_each(to_coefficients);
            }
        }
    }

    /// Undoes [`Self::striped_to_coefficients`]: a column that holds coefficients and no stripe
    /// gets its stripe 0 back, and drops the coefficients.
    fn coefficients_to_striped(&mut self) {
        let twiddles = self.twiddles;
        let pool = &self.base_column_pool;
        for tree in &mut self.trees.0 {
            if let MaybeOwned::Owned(tree) = tree {
                let to_stripe = |poly: &mut Poly<B>| {
                    if poly.evals.values.len() == 0 && !poly.is_sourced() {
                        let coeffs = poly.coeffs.take().unwrap();
                        let mut stripe = pool.take_or_alloc(coeffs.log_size());
                        B::evaluate_stripe_into(&coeffs, poly.evals.domain, twiddles, 0, &mut stripe);
                        poly.evals.values = stripe;
                        // SCREEN (w5 Q1611): `W5_B2SPOOL=1` hands the coefficient buffer (the same
                        // size as the stripe) to the pool, so the next column's stripe reuses it
                        // instead of the allocator holding both.
                        if crate::prover::m11_env("W5_B2SPOOL", 0) != 0 {
                            pool.give_back(coeffs.log_size(), coeffs.coeffs);
                        }
                    }
                };
                // SCREEN (p12 Q1554): `P12_B2S=<log>` converts the columns of at least that log
                // size one at a time (each FFT is parallel on its own), so that at most one large
                // column has both its coefficients and its stripe resident.
                let b2s_log = crate::prover::m11_env("P12_B2S", 0) as u32;
                #[cfg(feature = "parallel")]
                {
                    use rayon::iter::{IntoParallelRefMutIterator, ParallelIterator};
                    if false {
                        // SCREEN (r77, n9 LPT2): `R77_LPT2=1` takes the largest columns first, one
                        // column per task.
                        use rayon::iter::{IndexedParallelIterator, IntoParallelIterator};
                        let mut polys = tree.polynomials.iter_mut().collect::<Vec<_>>();
                        polys.sort_by_key(|poly| std::cmp::Reverse(poly.evals.domain.log_size()));
                        polys.into_par_iter().with_max_len(1).for_each(|poly| to_stripe(poly));
                    } else if b2s_log > 0 {
                        let is_large = |poly: &Poly<B>| poly.evals.domain.log_size() > b2s_log;
                        tree.polynomials.iter_mut().filter(|p| is_large(p)).for_each(to_stripe);
                        tree.polynomials
                            .par_iter_mut()
                            .filter(|p| !is_large(p))
                            .for_each(to_stripe);
                    } else {
                        tree.polynomials.par_iter_mut().for_each(to_stripe);
                    }
                }
                #[cfg(not(feature = "parallel"))]
                tree.polynomials.iter_mut().for_each(to_stripe);
            }
        }
    }

    pub fn trace(&self) -> Trace<'_, B> {
        let polys = self.polynomials();
        Trace { polys, twiddles: Some(self.twiddles) }
    }

    /// Computes the barycentric weights for every (column size, sampled point) pair, on buffers
    /// taken from [`Self::base_column_pool`]. The buffers are returned to the pool by
    /// [`Self::compute_samples()`].
    pub fn build_weights_hash_map(
        &self,
        sampled_points: &TreeVec<ColumnVec<Vec<CirclePoint<SecureField>>>>,
        max_log_size: u32,
    ) -> WeightsHashMap<B> {
        let weights_dashmap = WeightsHashMap::<B>::new();

        self.polynomials().zip_cols(sampled_points).map_cols(|(poly, points)| {
            let compute_weights = |(log_size, point): (u32, CirclePoint<SecureField>)| {
                weights_dashmap.entry((log_size, point)).or_insert_with(|| {
                    let weights_log_size = B::barycentric_log_size(
                        log_size,
                        self.config.fri_config.log_blowup_factor,
                    );
                    let buffer = SecureColumnByCoords {
                        columns: array::from_fn(|_| {
                            self.base_column_pool.take_or_alloc(weights_log_size)
                        }),
                    };
                    CircleEvaluation::<B, BaseField, BitReversedOrder>::barycentric_weights_into(
                        CanonicCoset::new(log_size),
                        point,
                        buffer,
                    )
                });
            };

            let log_size = poly.evals.domain.log_size();
            // For each sample point, compute the weights needed to evaluate the polynomial at
            // the folded sample point.
            // TODO(Leo): the computation `point.repeated_double(max_log_size - log_size)` is
            // likely repeated a bunch of times in a typical flat air. Consider moving it
            // outside the loop.
            #[cfg(not(feature = "parallel"))]
            points.iter().for_each(|&point| {
                compute_weights((log_size, point.repeated_double(max_log_size - log_size)))
            });

            #[cfg(feature = "parallel")]
            points.par_iter().for_each(|&point| {
                compute_weights((log_size, point.repeated_double(max_log_size - log_size)))
            });
        });

        weights_dashmap
    }

    /// Groups coefficient-free columns by domain size and projected sample point, so a backend
    /// can generate a small weight block once and reuse it across every column in the group.
    fn compute_grouped_samples(
        &self,
        sampled_points: &TreeVec<ColumnVec<Vec<CirclePoint<SecureField>>>>,
        lifting_log_size: u32,
    ) -> TreeVec<Vec<Vec<PointSample>>> {
        let polynomials = self.polynomials();
        let mut samples = sampled_points.as_cols_ref().map_cols(|points| {
            points
                .iter()
                .map(|&point| PointSample { point, value: SecureField::default() })
                .collect_vec()
        });
        let mut groups: HashMap<(u32, CirclePoint<SecureField>), Vec<(usize, usize, usize)>> =
            HashMap::new();
        // Columns that store no values are sampled from their source (see below).
        let mut sourced: Vec<(usize, usize)> = vec![];
        // SCREEN (r77, d9x OODSPAR): `R77_OODSPAR=1` evaluates the coefficient columns' samples
        // concurrently (largest first), and the groups too, instead of one at a time.
        let oodspar = cfg!(feature = "parallel") && true;
        let mut jobs = vec![];
        assert_eq!(polynomials.len(), sampled_points.len());
        for (tree_index, columns) in polynomials.iter().enumerate() {
            assert_eq!(columns.len(), sampled_points[tree_index].len());
            for (column_index, poly) in columns.iter().enumerate() {
                let log_size = poly.evals.domain.log_size();
                if poly.is_sourced() {
                    sourced.push((tree_index, column_index));
                    continue;
                }
                for (sample_index, &point) in
                    sampled_points[tree_index][column_index].iter().enumerate()
                {
                    let projected_point = point.repeated_double(lifting_log_size - log_size);
                    if let Some(coefficients) = &poly.coeffs {
                        if oodspar {
                            jobs.push((tree_index, column_index, sample_index, coefficients, projected_point));
                            continue;
                        }
                        samples[tree_index][column_index][sample_index].value =
                            coefficients.eval_at_point(projected_point);
                    } else {
                        groups.entry((log_size, projected_point)).or_default().push((
                            tree_index,
                            column_index,
                            sample_index,
                        ));
                    }
                }
            }
        }
        let log_blowup = self.config.fri_config.log_blowup_factor;
        #[cfg(feature = "parallel")]
        let groups = if oodspar {
            use rayon::iter::{IndexedParallelIterator, IntoParallelRefIterator, ParallelIterator};
            jobs.sort_by_key(|job| std::cmp::Reverse(job.3.log_size()));
            let values: Vec<SecureField> = jobs
                .par_iter()
                .with_max_len(1)
                .map(|&(_, _, _, coefficients, point)| coefficients.eval_at_point(point))
                .collect();
            for (&(tree, column, sample, _, _), value) in jobs.iter().zip(values) {
                samples[tree][column][sample].value = value;
            }
            let groups = groups.into_iter().collect_vec();
            let group_values: Vec<Vec<SecureField>> = groups
                .par_iter()
                .with_max_len(1)
                .map(|((log_size, point), members)| {
                    let evaluations = members
                        .iter()
                        .map(|&(tree, column, _)| &polynomials[tree][column].evals)
                        .collect_vec();
                    B::subdomain_eval_group(CanonicCoset::new(*log_size), log_blowup, *point, &evaluations)
                })
                .collect();
            for ((_, members), values) in groups.into_iter().zip(group_values) {
                assert_eq!(members.len(), values.len());
                for ((tree, column, sample), value) in members.into_iter().zip(values) {
                    samples[tree][column][sample].value = value;
                }
            }
            HashMap::new()
        } else {
            groups
        };
        drop(jobs);
        for ((log_size, point), members) in groups {
            let evaluations = members
                .iter()
                .map(|&(tree, column, _)| &polynomials[tree][column].evals)
                .collect_vec();
            let values = B::subdomain_eval_group(
                CanonicCoset::new(log_size),
                log_blowup,
                point,
                &evaluations,
            );
            assert_eq!(members.len(), values.len());
            for ((tree, column, sample), value) in members.into_iter().zip(values) {
                samples[tree][column][sample].value = value;
            }
        }
        // Columns that store no values are sampled on their trace domain, whose values their
        // source reproduces: by size, one barycentric weight block per sample point, shared by
        // every column of that size, with a bounded batch of columns expanded at a time.
        let mut sourced_by_size: HashMap<u32, Vec<(usize, usize)>> = HashMap::new();
        for (tree, column) in sourced {
            let log_size = polynomials[tree][column].source.as_ref().unwrap().log_size();
            sourced_by_size.entry(log_size).or_default().push((tree, column));
        }
        for (trace_log_size, members) in sourced_by_size {
            let coset = CanonicCoset::new(trace_log_size);
            let project = |tree: usize, column: usize, point: CirclePoint<SecureField>| {
                let log_size = polynomials[tree][column].evals.domain.log_size();
                point.repeated_double(lifting_log_size - log_size)
            };
            let points = members
                .iter()
                .flat_map(|&(tree, column)| {
                    sampled_points[tree][column].iter().map(move |&point| project(tree, column, point))
                })
                .unique_by(|point| (point.x, point.y))
                .collect_vec();
            let weights = points
                .iter()
                .map(|&point| {
                    let buffer = SecureColumnByCoords {
                        columns: array::from_fn(|_| {
                            self.base_column_pool.take_or_alloc(trace_log_size)
                        }),
                    };
                    CircleEvaluation::<B, BaseField, BitReversedOrder>::barycentric_weights_into(
                        coset, point, buffer,
                    )
                })
                .collect_vec();
            // SCREEN (f2 Q1684): `F2_SBATCH=k` expands k sourced columns at once (16).
            for batch in members.chunks(crate::prover::m11_env("F2_SBATCH", SOURCED_BATCH).max(1)) {
                let expand = |&(tree, column): &(usize, usize)| {
                    let source = polynomials[tree][column].source.as_ref().unwrap();
                    CircleEvaluation::<B, BaseField, BitReversedOrder>::new(
                        coset.circle_domain(),
                        source_column::<B>(source.as_ref()),
                    )
                };
                #[cfg(feature = "parallel")]
                let evaluations: Vec<_> = batch.par_iter().map(expand).collect();
                #[cfg(not(feature = "parallel"))]
                let evaluations: Vec<_> = batch.iter().map(expand).collect();
                // Every column of the batch is evaluated at every point of its size in one pass
                // over that point's weights; a column reads only the points it samples.
                let evaluations = evaluations.iter().collect_vec();
                for (point, weights) in points.iter().zip(&weights) {
                    let values = B::barycentric_eval_group(&evaluations, weights);
                    for (&(tree, column), value) in batch.iter().zip(values) {
                        for (sample, &sampled) in
                            samples[tree][column].iter_mut().zip(&sampled_points[tree][column])
                        {
                            if project(tree, column, sampled) == *point {
                                sample.value = value;
                            }
                        }
                    }
                }
            }
            for weight in weights {
                for column in weight.columns {
                    self.base_column_pool.give_back(trace_log_size, column);
                }
            }
        }
        samples
    }

    /// Evaluates the committed polynomials on the sampled points.
    fn compute_samples(
        &self,
        sampled_points: &TreeVec<ColumnVec<Vec<CirclePoint<SecureField>>>>,
        lifting_log_size: u32,
    ) -> TreeVec<Vec<Vec<PointSample>>> {
        let _span =
            span!(Level::INFO, "Evaluate columns out of domain", class = "EvaluateOutOfDomain")
                .entered();

        if !self.store_polynomials_coefficients {
            return self.compute_grouped_samples(sampled_points, lifting_log_size);
        }

        let weights_hash_map = if self.store_polynomials_coefficients {
            None
        } else {
            Some(self.build_weights_hash_map(sampled_points, lifting_log_size))
        };

        // Lambda that evaluates a polynomial on a collection of circle points and returns a vector
        // of point samples.
        let eval_at_points = |(poly, points): (&Poly<B>, &Vec<CirclePoint<SecureField>>)| {
            // A striped column without coefficients regrows them once for all its points.
            let regrown = (poly.coeffs.is_none()
                && weights_hash_map.is_none()
                && poly.evals.values.len() < poly.evals.domain.size())
            .then(|| poly.regrown_coefficients());
            points
                .iter()
                .map(|&point| {
                    let point_at = point.repeated_double(lifting_log_size - poly.evals.domain.log_size());
                    PointSample {
                        point,
                        value: match &regrown {
                            Some(coeffs) => coeffs.eval_at_point(point_at),
                            None => poly.eval_at_point(point_at, weights_hash_map.as_ref()),
                        },
                    }
                })
                .collect_vec()
        };

        #[cfg(not(feature = "parallel"))]
        let samples = self.polynomials().zip_cols(sampled_points).map_cols(eval_at_points);
        #[cfg(feature = "parallel")]
        let samples = self.polynomials().zip_cols(sampled_points).par_map_cols(eval_at_points);

        // Return the weights buffers to the memory pool for reuse.
        if let Some(weights_hash_map) = weights_hash_map {
            for ((log_size, _), weights) in weights_hash_map {
                let weights_log_size =
                    B::barycentric_log_size(log_size, self.config.fri_config.log_blowup_factor);
                for column in weights.columns {
                    self.base_column_pool.give_back(weights_log_size, column);
                }
            }
        }

        samples
    }

    pub fn prove_values(
        mut self,
        sampled_points: TreeVec<ColumnVec<Vec<CirclePoint<SecureField>>>>,
        channel: &mut MC::C,
    ) -> ExtendedCommitmentSchemeProof<MC::H> {
        let lifting_log_size = self.trees.last().unwrap().commitment.layers.len() as u32 - 1;

        // Evaluate polynomials on open points.
        let samples = self.compute_samples(&sampled_points, lifting_log_size);

        // Interpolation weights are complete; quotient and opening paths own their buffers.
        self.base_column_pool.release_all_idle();

        self.coefficients_to_striped();
        if crate::prover::m11_env("W5_B2SPOOL", 0) != 0 {
            // (Q1611: the coefficient buffers left over in the pool go before the quotients.)
            self.base_column_pool.release_all_idle();
        }
        crate::prover::backend::simd::column::trim_heap();
        // Point samples are complete; subsequent quotient and opening paths use evaluations.
        for tree in &mut self.trees.0 {
            if let MaybeOwned::Owned(tree) = tree {
                // A striped tree's small columns keep their coefficients for its decommitment.
                if tree.is_striped() {
                    continue;
                }
                for poly in &mut tree.polynomials {
                    drop(poly.coeffs.take());
                }
            }
        }

        let sampled_values =
            samples.as_cols_ref().map_cols(|x| x.iter().map(|o| o.value).collect());
        channel.mix_felts(&sampled_values.clone().flatten_cols());

        let columns = self.evaluations();
        // Columns that store no values regrow their stripe 0 in bounded batches.
        let polynomials = self.polynomials();
        let quotient_twiddles = self.twiddles;
        let regrow = |tree: usize, column: usize| {
            polynomials[tree][column].stripe0(quotient_twiddles).into_owned()
        };
        print_column_size_histogram::<B, MC>(&columns);
        // Compute oods quotients for boundary constraints on the sampled points.
        let quotients = compute_fri_quotients_with_regrowth(
            &columns,
            Some(&regrow),
            &samples,
            channel.draw_secure_felt(),
            lifting_log_size,
            self.twiddles,
            self.config.fri_config.log_blowup_factor,
        );
        // The quotients' freed accumulations stay in glibc's arenas under FRI and the
        // decommitment unless they are handed back.
        crate::prover::backend::simd::column::trim_heap();

        // Run FRI commitment phase on the oods quotients.
        // SCREEN (w5 Q1603): under `W5_QPOLY` the quotients come back as the coordinate
        // coefficients of the quotient (smaller than the lifting domain), and FRI commits to them
        // stripe by stripe.
        let mut quotients = Some(quotients);
        let mut striped_coeffs = (quotients.as_ref().unwrap().values.len() < 1 << lifting_log_size)
            .then(|| quotients.take().unwrap().values.columns.map(crate::prover::poly::circle::CircleCoefficients::new));
        // A small proof (or one folding a single layer) commits the extended column as before.
        let fri_config = self.config.fri_config;
        let stripe_log_size = lifting_log_size - fri_config.log_blowup_factor;
        if striped_coeffs.is_some()
            && !(fri_config.fold_step > 1
                && stripe_log_size > crate::core::vcs_lifted::verifier::LOG_PACKED_LEAF_SIZE + 5
                && lifting_log_size >= 5 + fri_config.fold_step)
        {
            let domain = CanonicCoset::new(lifting_log_size).circle_domain();
            let columns = striped_coeffs.take().unwrap().map(|c| c.evaluate_with_twiddles(domain, self.twiddles).values);
            quotients = Some(crate::prover::poly::circle::SecureEvaluation::new(domain, SecureColumnByCoords { columns }));
        }
        let fri_prover = match striped_coeffs {
            Some(coeffs) => FriProver::<B, MC>::commit_striped(
                channel,
                self.config.fri_config,
                coeffs,
                lifting_log_size,
                self.config.fri_config.log_blowup_factor,
                self.twiddles,
            ),
            None => FriProver::<B, MC>::commit(
                channel,
                self.config.fri_config,
                quotients.as_ref().unwrap(),
                self.twiddles,
            ),
        };
        crate::prover::backend::simd::column::trim_heap();

        // Proof of work.
        let span1 = span!(Level::INFO, "Grind", class = "Queries POW").entered();
        let proof_of_work = B::grind(channel, self.config.fri_config.pow_bits);
        span1.exit();
        channel.mix_u64(proof_of_work);

        // FRI decommitment phase.
        let FriDecommitResult { fri_proof, query_positions, unsorted_query_locations } =
            fri_prover.decommit(channel);
        // Build the query position tree.
        let preprocessed_query_positions = prepare_preprocessed_query_positions(
            &query_positions,
            lifting_log_size,
            self.trees[0].commitment.layers.len() as u32 - 1,
        );
        let query_positions_tree = TreeVec::new(
            self.trees
                .iter()
                .enumerate()
                .map(|(i, _)| {
                    if i == 0 {
                        preprocessed_query_positions.as_slice()
                    } else {
                        query_positions.as_slice()
                    }
                })
                .collect::<Vec<_>>(),
        );
        // The FRI input column is dead once FRI has decommitted: free it before the striped trees
        // regrow their stripes.
        if let Some(quotients) = quotients {
            for column in quotients.values.columns {
                self.base_column_pool.give_back(lifting_log_size, column);
            }
        }
        self.base_column_pool.release_all_idle();
        let commitments = self.roots();
        let twiddles = self.twiddles;
        let pool: &BaseColumnPool<B> = &self.base_column_pool;
        let log_blowup_factor = self.config.fri_config.log_blowup_factor;
        let pairs = self.trees.as_ref().zip_eq(query_positions_tree).0;
        // SCREEN (r77, d9x PARDEC): `R77_PARDEC=1` decommits the trees concurrently (in order).
        #[cfg(feature = "parallel")]
        let decommitted: Vec<_> = if false {
            use rayon::iter::{IntoParallelIterator, ParallelIterator};
            pairs
                .into_par_iter()
                .map(|(tree, query_positions)| tree.decommit(query_positions, twiddles, pool, log_blowup_factor))
                .collect()
        } else {
            pairs
                .into_iter()
                .map(|(tree, query_positions)| tree.decommit(query_positions, twiddles, pool, log_blowup_factor))
                .collect()
        };
        #[cfg(not(feature = "parallel"))]
        let decommitted: Vec<_> = pairs
            .into_iter()
            .map(|(tree, query_positions)| tree.decommit(query_positions, twiddles, pool, log_blowup_factor))
            .collect();
        let (queried_values, decommitments, aux): (Vec<_>, Vec<_>, Vec<_>) =
            decommitted.into_iter().map(|(v, x)| (v, x.decommitment, x.aux)).multiunzip();

        // Return evaluation buffers to the memory pool for reuse (owned trees only).
        for tree in &mut self.trees.0 {
            if let MaybeOwned::Owned(tree) = tree {
                for poly in tree.polynomials.drain(..) {
                    // A striped column holds only a prefix of its domain.
                    // A column with a source holds no values.
                    if poly.evals.values.is_empty() {
                        continue;
                    }
                    let log_size = poly.evals.values.len().ilog2();
                    self.base_column_pool.give_back(log_size, poly.evals.values);
                }
            }
        }

        self.base_column_pool.release_all_idle();

        ExtendedCommitmentSchemeProof {
            proof: CommitmentSchemeProof {
                commitments,
                sampled_values,
                decommitments: TreeVec(decommitments),
                queried_values: TreeVec(queried_values),
                proof_of_work,
                fri_proof: fri_proof.proof,
                config: self.config,
            },
            aux: CommitmentSchemeProofAux {
                unsorted_query_locations,
                trace_decommitment: TreeVec(aux),
                fri: fri_proof.aux,
            },
        }
    }
}

/// Helper struct for aggregating polynomials and evaluations for a commitment tree.
pub struct TreeBuilder<'a, 'b, B: BackendForChannel<MC>, MC: MerkleChannel> {
    tree_index: usize,
    commitment_scheme: &'a mut CommitmentSchemeProver<'b, B, MC>,
    polys: ColumnVec<CircleCoefficients<B>>,
}
impl<B: BackendForChannel<MC>, MC: MerkleChannel> TreeBuilder<'_, '_, B, MC> {
    pub fn extend_evals(
        &mut self,
        columns: Vec<CircleEvaluation<B, BaseField, BitReversedOrder>>,
    ) -> TreeSubspan {
        let span = span!(Level::INFO, "Interpolation for commitment").entered();
        let polys = B::interpolate_columns_pooled(
            columns,
            self.commitment_scheme.twiddles,
            &self.commitment_scheme.base_column_pool,
        );
        span.exit();

        self.extend_polys(polys)
    }

    pub fn extend_polys(
        &mut self,
        columns: impl IntoIterator<Item = CircleCoefficients<B>>,
    ) -> TreeSubspan {
        let col_start = self.polys.len();
        self.polys.extend(columns);
        let col_end = self.polys.len();
        TreeSubspan { tree_index: self.tree_index, col_start, col_end }
    }

    pub fn commit(self, channel: &mut MC::C) {
        let _span = span!(Level::INFO, "Commitment").entered();
        self.commitment_scheme.commit(self.polys, channel);
    }
}

/// Prover data for a single commitment tree in a commitment scheme. The commitment scheme allows to
/// commit on a set of polynomials at a time. This corresponds to such a set.
pub struct CommitmentTreeProver<B: BackendForChannel<MC>, MC: MerkleChannel> {
    pub polynomials: ColumnVec<Poly<B>>,
    pub commitment: MerkleProverLifted<B, MC::H>,
}

impl<B: BackendForChannel<MC>, MC: MerkleChannel> CommitmentTreeProver<B, MC> {
    pub fn new(
        polynomials: ColumnVec<CircleCoefficients<B>>,
        log_blowup_factor: u32,
        twiddles: &TwiddleTree<B>,
        store_polynomials_coefficients: bool,
        lifting_log_size: u32,
        base_column_pool: &BaseColumnPool<B>,
    ) -> Self {
        Self::new_ex(
            polynomials,
            log_blowup_factor,
            twiddles,
            store_polynomials_coefficients,
            lifting_log_size,
            base_column_pool,
            false,
            0,
        )
    }

    /// [`Self::new`]; with `keep_coefficients`, a striped tree's prefix columns keep their
    /// coefficients instead of stripe 0 (for a tree the scheme owns, which turns them back into
    /// stripe 0 before the quotients).
    fn new_ex(
        polynomials: ColumnVec<CircleCoefficients<B>>,
        log_blowup_factor: u32,
        twiddles: &TwiddleTree<B>,
        store_polynomials_coefficients: bool,
        lifting_log_size: u32,
        base_column_pool: &BaseColumnPool<B>,
        keep_coefficients: bool,
        pack: u32,
    ) -> Self {
        let n_columns = polynomials.len();
        Self::new_ex_with_sources(
            polynomials,
            log_blowup_factor,
            twiddles,
            store_polynomials_coefficients,
            lifting_log_size,
            base_column_pool,
            keep_coefficients,
            vec![None; n_columns],
            pack,
        )
    }

    /// [`Self::new`] for a tree whose columns' trace values can be reproduced on demand: a
    /// striped column with a source in `sources` keeps nothing but the source (no stripe and no
    /// coefficients), and is regrown from it where it is read. A tree that is not striped ignores
    /// the sources.
    pub fn new_with_sources(
        polynomials: ColumnVec<CircleCoefficients<B>>,
        log_blowup_factor: u32,
        twiddles: &TwiddleTree<B>,
        store_polynomials_coefficients: bool,
        lifting_log_size: u32,
        base_column_pool: &BaseColumnPool<B>,
        sources: Vec<Option<SharedTraceSource>>,
    ) -> Self {
        Self::new_ex_with_sources(
            polynomials,
            log_blowup_factor,
            twiddles,
            store_polynomials_coefficients,
            lifting_log_size,
            base_column_pool,
            false,
            sources,
            0,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn new_ex_with_sources(
        polynomials: ColumnVec<CircleCoefficients<B>>,
        log_blowup_factor: u32,
        twiddles: &TwiddleTree<B>,
        store_polynomials_coefficients: bool,
        lifting_log_size: u32,
        base_column_pool: &BaseColumnPool<B>,
        keep_coefficients: bool,
        sources: Vec<Option<SharedTraceSource>>,
        pack: u32,
    ) -> Self {
        assert_eq!(sources.len(), polynomials.len());
        if B::STRIPES && can_stripe(&polynomials, log_blowup_factor, lifting_log_size) {
            return Self::new_striped(
                polynomials,
                log_blowup_factor,
                twiddles,
                lifting_log_size,
                base_column_pool,
                keep_coefficients,
                sources,
                pack,
            );
        }
        let span = span!(Level::INFO, "Extension").entered();
        // A tree that is not striped keeps its coefficients where trees are striped: composition
        // then extends every column from its coefficients (see `compute_composition_polynomial`).
        let polynomials = B::evaluate_polynomials(
            polynomials,
            log_blowup_factor,
            twiddles,
            // Only a blowup that can stripe (>= 2) needs them: a blowup-1 proof never stripes and
            // composes from its committed prefix.
            store_polynomials_coefficients || (B::STRIPES && log_blowup_factor >= 2),
            base_column_pool,
        );
        span.exit();

        let _span = span!(Level::INFO, "Merkle").entered();
        let tree = MerkleProverLifted::commit(
            polynomials.iter().map(|poly: &Poly<B>| &poly.evals.values).collect(),
            lifting_log_size,
            0,
        );

        CommitmentTreeProver { polynomials, commitment: tree }
    }

    /// Commits the extension of `polynomials` one subdomain stripe at a time. The extension of a
    /// column on its domain is `2^log_blowup_factor` independent FFTs, one per subdomain of the
    /// coefficients' size, each writing one contiguous block of the bit-reversed evaluation.
    /// Stripe `k` of every column is computed into a scratch column, and its Merkle subtree is
    /// built from the scratch columns; the subtrees are the lowest layers of the tree. A column
    /// keeps only stripe 0, its evaluation on the first subdomain, which determines it (small
    /// columns keep their whole extension and their coefficients); everything else is regrown
    /// from stripe 0 where it is read. No column's full extension is ever resident.
    fn new_striped(
        polynomials: ColumnVec<CircleCoefficients<B>>,
        log_blowup_factor: u32,
        twiddles: &TwiddleTree<B>,
        lifting_log_size: u32,
        base_column_pool: &BaseColumnPool<B>,
        keep_coef: bool,
        mut sources: Vec<Option<SharedTraceSource>>,
        pack_mode: u32,
    ) -> Self {
        let _span = span!(Level::INFO, "Striped extension and Merkle").entered();
        // The witness and interaction writers leave freed temporaries in glibc's arenas: hand
        // them back before the scratch of the commitment is taken.
        crate::prover::backend::simd::column::trim_heap();
        // Each stripe is committed as `2^sub` blocks, so that the scratch of every column is a
        // block, not a stripe. The tree is the same: its leaves are rows.
        // SCREEN (r77, n9 CSUB0): `R77_CSUB0=1` commits the Cairo trees (log blowup != 2) in whole
        // stripes.
        let csub0 = log_blowup_factor != 2 && false;
        // SCREEN (w5 Q1616): `W5_CSUB=n+1` commits the Cairo trees (log blowup != 2) as `2^n` blocks per
        // stripe, whatever `M11_SUB` gives the circuit trees (only the circuit commit needs the split).
        // SCREEN (f2 Q1661): `F2_SUBK=s+1` is the same for the Cairo leg (log blowup 3), found
        // independently (z1-s1m: SUB3 costs ~1 s of Cairo folds under windows that do not bind).
        // SCREEN (f2 Q1674): `F2_SUBT=mask` gives `M11_SUB` only to the circuit leg's trees whose
        // index is in `mask` (bit 1 = base trace); its other trees commit at sub 1.
        let csub = crate::prover::m11_env("W5_CSUB", 0);
        let subt = crate::prover::m11_env("F2_SUBT", 0);
        let tree = crate::prover::F2_TREE.load(std::sync::atomic::Ordering::Relaxed);
        let leg_sub = match crate::prover::m11_env("F2_SUBK", 0) {
            k if k > 0 && log_blowup_factor == 3 => k - 1,
            _ if csub > 0 && log_blowup_factor != 2 => csub - 1,
            _ if subt != 0 && log_blowup_factor == 2 && (tree >= 64 || (subt >> tree) & 1 == 0) => 1,
            _ => crate::prover::m11_env("M11_SUB", 1),
        };
        let sub = (if csub0 { 0u32 } else { leg_sub as u32 })
            .min(polynomials.iter().map(|poly| poly.log_size()).min().unwrap() - crate::prover::m11_env("M11_SUBMIN", 4) as u32);
        // SCREEN (r77, d9s LPT): `R77_LPT=1` fills the largest columns first, one column per task.
        let lpt = false;
        // SCREEN (r77, d9s MULTI): `R77_MULTI=k`: a tree with fewer columns than threads fills and
        // hashes `k` stripes at once, each into its own scratch columns.
        let multi =
            1;
        let n_stripes = 1usize << (log_blowup_factor + sub);
        let stripe_lifting_log_size = lifting_log_size - log_blowup_factor - sub;
        let domains = polynomials
            .iter()
            .map(|poly| CanonicCoset::new(poly.log_size() + log_blowup_factor).circle_domain())
            .collect_vec();
        let prefix = domains.iter().map(|&domain| keeps_prefix(domain, log_blowup_factor)).collect_vec();
        // SCREEN (m11, Q1405): a kept-coefficient prefix column whose trace values fit in 16 bits
        // keeps them packed, as its source, instead of its coefficients.
        // Mode 1 packs before the stripes (its transients add to the commit window); mode 2 (Q1406)
        // after them, one column at a time, each column's coefficients dropped once it is packed.
        let pack = (pack_mode == 1 || pack_mode == 3) && keep_coef;
        // Mode 3 (Q1407): packs before the stripes like mode 1, but one column at a time, each
        // column's coefficients dropped once packed; the stripes regrow them from the source.
        let mut polynomials = polynomials.into_iter().map(Some).collect_vec();
        if pack_mode == 3 && keep_coef {
            for ((poly, &prefix), source) in polynomials.iter_mut().zip(&prefix).zip(sources.iter_mut()) {
                if prefix && source.is_none() {
                    *source = pack_column(poly.as_ref().unwrap(), twiddles);
                    if source.is_some() {
                        *poly = None;
                    }
                }
            }
        }
        let log_sizes = domains.iter().map(|d| d.log_size() - log_blowup_factor).collect_vec();
        if pack_mode == 1 && keep_coef {
            let polynomials = polynomials.iter().map(|p| p.clone().unwrap()).collect_vec();
            pack_sources(&polynomials, &prefix, &mut sources, twiddles);
        }
        // A prefix column with a source keeps nothing but the source: every stripe of it, stripe 0
        // included, is hashed from its scratch column.
        let sourced = prefix
            .iter()
            .zip(&sources)
            .map(|(&prefix, source): (&bool, &Option<SharedTraceSource>)| {
                prefix && (!keep_coef || pack) && source.is_some()
            })
            .collect_vec();
        // With `keep_coef`, a prefix column keeps its coefficients instead of stripe 0 (the same size):
        // composition and the samples read coefficients, and stripe 0 is evaluated back only for
        // the quotients (`coefficients_to_striped`).
        // Z1_S0 (Q1580): with `sub > 0`, a prefix column's stripe 0 is evaluated whole when the
        // stripe-0 blocks start, its coefficients are dropped right away and its blocks are copied
        // from it, so coefficients, stripe 0 and scratch are never all resident at once.
        let s0 = sub > 0 && !keep_coef && crate::prover::m11_env("Z1_S0", 0) != 0;
        // Prefix columns get their stripe-0 buffer now (the last stripe hashed, which the FFT writes
        // in place). Whole-extension columns are evaluated now.
        let mut evals = polynomials
            .iter()
            .zip(&domains)
            .zip(&prefix)
            .zip(&sourced)
            .zip(&log_sizes)
            .map(|((((poly, &domain), &prefix), &sourced), &log_size)| {
                if (prefix && keep_coef) || sourced || (s0 && prefix) {
                    Col::<B, BaseField>::zeros(0)
                } else if prefix {
                    base_column_pool.take_or_alloc(log_size)
                } else {
                    let buffer = base_column_pool.take_or_alloc(domain.log_size());
                    B::evaluate_into(poly.as_ref().unwrap(), domain, twiddles, buffer).values
                }
            })
            .collect_vec();
        let mut scratch = log_sizes
            .iter()
            // Q1409: a column whose block is below one packed word (16 rows) gets a 16-row scratch.
            .map(|&log_size| base_column_pool.take_or_alloc((log_size - sub).max(LOG_N_LANES_PCS)))
            .collect_vec();
        if false {
            // Z1 census: what the striped commit holds while it hashes.
            let mib = |b: usize| b as f64 / (1u64 << 20) as f64;
            let sbytes: usize = scratch.iter().map(|c| c.len() * 4).sum();
            let ebytes: usize = evals.iter().map(|c| c.len() * 4).sum();
            let cbytes: usize =
                polynomials.iter().flatten().map(|p| (1usize << p.log_size()) * 4).sum();
            let mut hist = std::collections::BTreeMap::<u32, usize>::new();
            for &l in &log_sizes {
                *hist.entry(l).or_default() += 1;
            }
            eprintln!(
                "Z1C commit n={} lift={} sub={} keep_coef={} sourced={} prefix={} scratch={:.1}MiB evals={:.1}MiB coeffs={:.1}MiB hist={:?}",
                log_sizes.len(), lifting_log_size, sub, keep_coef,
                sourced.iter().filter(|&&s| s).count(), prefix.iter().filter(|&&p| p).count(),
                mib(sbytes), mib(ebytes), mib(cbytes), hist
            );
        }
        let mut coeffs = polynomials;
        let sflags: Vec<(bool, Option<SharedTraceSource>)> =
            sourced.iter().copied().zip(sources.iter().cloned()).collect();

        // Stripe 0, the only one a prefix column keeps, last and in place.
        let n_first = 1usize << sub;
        let order = (n_first..n_stripes).chain(0..n_first).collect_vec();
        let mut stripe_layers: Vec<Option<Vec<Col<B, <MC::H as MerkleHasherLifted>::Hash>>>> =
            (0..n_stripes).map(|_| None).collect();
        let mut done = vec![false; n_stripes];
        #[cfg(feature = "parallel")]
        if multi > 1 && coeffs.len() < rayon::current_num_threads() {
            use rayon::iter::{IndexedParallelIterator, IntoParallelIterator, IntoParallelRefIterator, IntoParallelRefMutIterator, ParallelIterator};
            // Only stripes that write no column's resident stripe 0.
            let safe = order
                .iter()
                .copied()
                .filter(|&stripe| keep_coef || (sub > 0 && stripe >= n_first))
                .collect_vec();
            for group in safe.chunks(multi) {
                let sets = group
                    .iter()
                    .map(|_| {
                        coeffs
                            .iter()
                            .map(|poly| base_column_pool.take_or_alloc(poly.as_ref().unwrap().log_size() - sub))
                            .collect_vec()
                    })
                    .collect_vec();
                let results = group
                    .par_iter()
                    .zip(sets.into_par_iter())
                    .map(|(&stripe, mut set)| {
                        set.par_iter_mut().enumerate().for_each(|(i, column)| {
                            let poly = coeffs[i].as_ref().unwrap();
                            let len = 1usize << (poly.log_size() - sub);
                            if !prefix[i] {
                                B::copy_block(&evals[i], stripe * len, column, 0, len);
                            } else {
                                B::evaluate_block_into(poly, domains[i], twiddles, poly.log_size() - sub, stripe, column);
                            }
                        });
                        let layers = {
                            let columns = set.iter().sorted_by_key(|column| column.len()).collect_vec();
                            B::build_layers_sparse(&columns, stripe_lifting_log_size)
                        };
                        (stripe, layers, set)
                    })
                    .collect::<Vec<_>>();
                for (stripe, layers, set) in results {
                    stripe_layers[stripe] = Some(layers);
                    done[stripe] = true;
                    for (poly, column) in coeffs.iter().zip(set) {
                        base_column_pool.give_back(poly.as_ref().unwrap().log_size() - sub, column);
                    }
                }
            }
        }
        #[cfg(not(feature = "parallel"))]
        let _ = multi;
        // SCREEN (1bm Q1531, Q1511): each stripe's levels are copied into the tree's levels as
        // the stripe is hashed, so the levels are never held twice.
        let in_place = crate::prover::m11_env("Q1531_INPLACE", 0) != 0;
        let mut levels: Vec<Vec<<MC::H as MerkleHasherLifted>::Hash>> = vec![];
        for &stripe in &order {
            if done[stripe] {
                continue;
            }
            if stripe == 0 && sub == 0 && !keep_coef {
                // Stripe 0 is written in place, so the prefix columns' scratch is dead: free it
                // first.
                for (i, column) in scratch.iter_mut().enumerate() {
                    if prefix[i] && !sourced[i] {
                        drop(std::mem::replace(column, Col::<B, BaseField>::zeros(0)));
                    }
                }
                base_column_pool.release_all_idle();
            }
            if s0 && stripe == 0 {
                // Q1580: stripe 0 whole, then the coefficients go (see `s0`).
                let first = |((((poly, eval), &domain), &prefix), &(sourced, _)): (
                    (((&mut Option<CircleCoefficients<B>>, &mut Col<B, BaseField>), &CircleDomain), &bool),
                    &(bool, Option<SharedTraceSource>),
                )| {
                    if prefix && !sourced {
                        let coeffs = poly.take().unwrap();
                        let mut buffer = base_column_pool.take_or_alloc(coeffs.log_size());
                        B::evaluate_stripe_into(&coeffs, domain, twiddles, 0, &mut buffer);
                        drop(coeffs);
                        *eval = buffer;
                    }
                };
                #[cfg(feature = "parallel")]
                {
                    use rayon::iter::{IndexedParallelIterator, IntoParallelRefIterator, IntoParallelRefMutIterator, ParallelIterator};
                    coeffs
                        .par_iter_mut()
                        .zip(evals.par_iter_mut())
                        .zip(domains.par_iter())
                        .zip(prefix.par_iter())
                        .zip(sflags.par_iter())
                        .for_each(first);
                }
                #[cfg(not(feature = "parallel"))]
                coeffs.iter_mut().zip(evals.iter_mut()).zip(&domains).zip(&prefix).zip(&sflags).for_each(first);
            }
            let fill = |(((((poly, &domain), &prefix), sflag), eval), column): (
                (
                    (((&Option<CircleCoefficients<B>>, &CircleDomain), &bool), &(bool, Option<SharedTraceSource>)),
                    &mut Col<B, BaseField>,
                ),
                &mut Col<B, BaseField>,
            )| {
                if s0 && prefix && !sflag.0 && stripe < n_first {
                    // Q1580: a block of the stripe 0 evaluated above.
                    let len = eval.len() >> sub;
                    B::copy_block(eval, stripe * len, column, 0, len);
                    return;
                }
                // A mode-3 packed column regrows its coefficients from its source for each stripe.
                let regrown;
                let poly = match poly {
                    Some(poly) => poly,
                    None => {
                        let source = sflag.1.as_ref().unwrap();
                        let trace = CanonicCoset::new(source.log_size()).circle_domain();
                        regrown = CircleEvaluation::<B, BaseField, BitReversedOrder>::new(
                            trace,
                            crate::prover::air::component_prover::source_column::<B>(source.as_ref()),
                        )
                        .interpolate_with_twiddles(twiddles);
                        &regrown
                    }
                };
                let sourced = sflag.0;
                let len = 1usize << (poly.log_size() - sub);
                if !prefix && len < 1 << LOG_N_LANES_PCS {
                    // Q1409: the block lifted to 16 rows as the tree lifts a column
                    // (`dup[j] = block[(j >> (k + 1) << 1) + (j & 1)]`, `k` the log ratio): every
                    // leaf reads the same value of it as of the block, so the hashes are the same,
                    // and the tree keeps the packed-word leaves of the fused hasher.
                    let k = LOG_N_LANES_PCS - len.ilog2();
                    for j in 0..1usize << LOG_N_LANES_PCS {
                        column.set(j, eval.at(stripe * len + (j >> (k + 1) << 1) + (j & 1)));
                    }
                } else if !prefix {
                    B::copy_block(eval, stripe * len, column, 0, len);
                } else if keep_coef {
                    B::evaluate_block_into(poly, domain, twiddles, poly.log_size() - sub, stripe, column);
                } else if sub > 0 {
                    B::evaluate_block_into(poly, domain, twiddles, poly.log_size() - sub, stripe, column);
                    if stripe < n_first && !sourced {
                        B::copy_block(column, 0, eval, stripe * len, len);
                    }
                } else if stripe == 0 && !sourced {
                    B::evaluate_stripe_into(poly, domain, twiddles, 0, eval);
                } else {
                    B::evaluate_stripe_into(poly, domain, twiddles, stripe, column);
                }
            };
            #[cfg(feature = "parallel")]
            {
                use rayon::iter::{IndexedParallelIterator, IntoParallelIterator, IntoParallelRefIterator, IntoParallelRefMutIterator};
                if lpt {
                    // SCREEN (r77, d9s LPT): the largest columns first, one column per task.
                    let mut items = coeffs
                        .iter()
                        .zip(domains.iter())
                        .zip(prefix.iter())
                        .zip(sflags.iter())
                        .zip(evals.iter_mut())
                        .zip(scratch.iter_mut())
                        .collect_vec();
                    items.sort_by_key(|(((((poly, _), _), _), _), _)| {
                        std::cmp::Reverse(poly.as_ref().unwrap().log_size())
                    });
                    items.into_par_iter().with_max_len(1).for_each(fill);
                } else {
                    coeffs
                        .par_iter()
                        .zip(domains.par_iter())
                        .zip(prefix.par_iter())
                        .zip(sflags.par_iter())
                        .zip(evals.par_iter_mut())
                        .zip(scratch.par_iter_mut())
                        .for_each(fill);
                }
            }
            #[cfg(not(feature = "parallel"))]
            coeffs
                .iter()
                .zip(&domains)
                .zip(&prefix)
                .zip(&sflags)
                .zip(evals.iter_mut())
                .zip(scratch.iter_mut())
                .for_each(fill);
            // Sorted by the columns' own sizes (a lifted block of Q1409 keeps its place).
            let columns = (0..coeffs.len())
                .sorted_by_key(|&i| log_sizes[i])
                .map(|i| {
                    if prefix[i] && !sourced[i] && stripe == 0 && sub == 0 && !keep_coef {
                        &evals[i]
                    } else {
                        &scratch[i]
                    }
                })
                .collect_vec();
            let mut built = B::build_layers_sparse(&columns, stripe_lifting_log_size);
            crate::prover::vcs_lifted::prover::q1530_drop_levels(&mut built);
            if in_place {
                if levels.is_empty() {
                    levels = built.iter().map(|layer| Vec::with_capacity(layer.len() * n_stripes)).collect();
                    levels.iter_mut().zip(&built).for_each(|(level, layer)| {
                        level.resize(layer.len() * n_stripes, Default::default())
                    });
                }
                for (level, layer) in levels.iter_mut().zip(built) {
                    let len = layer.len();
                    level[stripe * len..(stripe + 1) * len].copy_from_slice(&layer.to_cpu());
                }
            } else {
                stripe_layers[stripe] = Some(built);
            }
            if crate::prover::m11_env("M11_STRIPE_TRIM", 0) != 0 {
                crate::prover::backend::simd::column::trim_heap();
            }
        }
        crate::prover::backend::simd::circle::w5_clear_fold_buffers();
        // A prefix column's coefficients are regrown from stripe 0 where they are needed (the
        // out-of-domain samples, composition and the decommitment), one column or tree at a time.

        for (poly, &prefix) in coeffs.iter_mut().zip(&prefix) {
            if prefix && !keep_coef {
                drop(poly.take());
            }
        }
        for (&log_size, column) in log_sizes.iter().zip(scratch) {
            if column.len() == 1 << (log_size - sub) {
                base_column_pool.give_back(log_size - sub, column);
            }
        }
        base_column_pool.release_all_idle();
        crate::prover::backend::simd::column::trim_heap();
        // Level `l` of the tree, up to the stripes' roots, is the concatenation of the stripes'
        // levels `l`, in stripe order.
        let mut layers: Vec<Col<B, <MC::H as MerkleHasherLifted>::Hash>> = if in_place {
            levels.into_iter().map(|level| level.into_iter().collect()).collect()
        } else {
            let stripe_layers = stripe_layers.into_iter().map(Option::unwrap).collect_vec();
            (0..=stripe_lifting_log_size as usize)
                .map(|level| {
                    stripe_layers.iter().flat_map(|layers| layers[level].to_cpu()).collect()
                })
                .collect()
        };
        for _ in stripe_lifting_log_size..lifting_log_size {
            let next = B::build_next_layer(layers.last().unwrap());
            layers.push(next);
        }
        layers.reverse();

        // SCREEN (w5 Q1605): `W5_PACKPAR=k` packs the mode-2 columns `k` at a time in parallel
        // (each column's coefficients dropped once it is packed) instead of one after another.
        let packpar = crate::prover::m11_env("W5_PACKPAR", 0);
        let mut prepacked: Vec<Option<Option<SharedTraceSource>>> = vec![None; coeffs.len()];
        #[cfg(feature = "parallel")]
        if pack_mode == 2 && keep_coef && packpar > 0 {
            use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
            let eligible = (0..coeffs.len())
                .filter(|&i| !sourced[i] && evals[i].len() == 0 && coeffs[i].is_some())
                .collect_vec();
            for chunk in eligible.chunks(packpar) {
                let packed: Vec<Option<SharedTraceSource>> = chunk
                    .par_iter()
                    .map(|&i| pack_column(coeffs[i].as_ref().unwrap(), twiddles))
                    .collect();
                for (&i, packed) in chunk.iter().zip(packed) {
                    if packed.is_some() {
                        coeffs[i] = None;
                    }
                    prepacked[i] = Some(packed);
                }
            }
        }
        #[cfg(not(feature = "parallel"))]
        let _ = packpar;
        let polynomials = coeffs
            .into_iter()
            .zip(domains)
            .zip(evals)
            .zip(sources.into_iter().zip(sourced))
            .zip(prepacked)
            .map(|((((coeffs, domain), values), (source, sourced)), prepacked)| {
                if sourced {
                    return Poly::from_source(domain, source.unwrap());
                }
                let tried = prepacked.is_some();
                if let Some(Some(packed)) = prepacked {
                    return Poly::from_source(domain, packed);
                }
                if pack_mode == 2 && keep_coef && values.len() == 0 && !tried {
                    if let Some(packed) = coeffs.as_ref().and_then(|c| pack_column(c, twiddles)) {
                        drop(coeffs);
                        return Poly::from_source(domain, packed);
                    }
                }
                // (A column that kept its coefficients holds no evaluation yet.)
                let mut evals = CircleEvaluation::new(
                    CanonicCoset::new(1).circle_domain(),
                    Col::<B, BaseField>::zeros(2),
                );
                evals.values = values;
                evals.domain = domain;
                Poly::new(coeffs, evals)
            })
            .collect();
        CommitmentTreeProver { polynomials, commitment: MerkleProverLifted { layers } }
    }

    /// Whether the tree was committed by [`Self::new_striped`] with a column that keeps only a
    /// prefix of its extension.
    pub fn is_striped(&self) -> bool {
        self.polynomials.iter().any(|poly| poly.evals.values.len() < poly.evals.domain.size())
    }

    /// Decommits the merkle tree on the given query positions.
    /// Returns the values at the queried positions and the decommitment.
    /// The queries are given as a mapping from the log size of the layer size to the queried
    /// positions on each column of that size.
    fn decommit(
        &self,
        queries: &[usize],
        twiddles: &TwiddleTree<B>,
        base_column_pool: &BaseColumnPool<B>,
        log_blowup_factor: u32,
    ) -> (ColumnVec<Vec<BaseField>>, ExtendedMerkleDecommitmentLifted<MC::H>) {
        if self.is_striped() {
            return self.decommit_striped(queries, twiddles, base_column_pool, log_blowup_factor);
        }
        let eval_vec = self.polynomials.iter().map(|poly| &poly.evals.values).collect_vec();
        self.commitment.decommit(queries, eval_vec)
    }

    /// [`Self::decommit`] for a striped tree. Column by column, the coefficients and every stripe
    /// that holds a queried position or a leaf the decommitment rehashes are regrown, and only
    /// the rows read there are kept; the leaves are then hashed from those rows.
    fn decommit_striped(
        &self,
        queries: &[usize],
        twiddles: &TwiddleTree<B>,
        base_column_pool: &BaseColumnPool<B>,
        log_blowup_factor: u32,
    ) -> (ColumnVec<Vec<BaseField>>, ExtendedMerkleDecommitmentLifted<MC::H>) {
        let _span = span!(Level::INFO, "Striped decommit").entered();
        let lifting_log_size = self.commitment.layers.len() as u32 - 1;
        let stripe_log_size = lifting_log_size - log_blowup_factor;
        let leaf_positions = self.commitment.omitted_leaf_positions(queries);
        let positions = queries.iter().chain(&leaf_positions).copied().collect_vec();
        let stripes = positions
            .iter()
            .map(|position| position >> stripe_log_size)
            .sorted()
            .dedup()
            .collect_vec();
        let rows_of = |log_size: u32, stripe: usize| -> Vec<usize> {
            let shift = stripe_log_size - log_size;
            positions
                .iter()
                .filter(|&&position| position >> stripe_log_size == stripe)
                .map(|&position| {
                    let local = position - (stripe << stripe_log_size);
                    (local >> (shift + 1) << 1) + (local & 1)
                })
                .sorted()
                .dedup()
                .collect_vec()
        };
        // TILE evaluates a striped column's non-resident rows straight from its
        // stripe 0 (TILE, with the Q722 weighted-sum tail), without regrowing its coefficients.
        // The rows of `positions` in one column, read from its stripes as the tree lifts them.
        let read_rows = |poly: &Poly<B>| -> Vec<BaseField> {
            if poly.coeffs.is_none()
                && !poly.is_sourced()
                && poly.evals.values.len() < poly.evals.domain.size()
            {
                let log_size = poly.evals.domain.log_size() - log_blowup_factor;
                let resident = poly.evals.values.len() >> log_size;
                let targets = stripes
                    .iter()
                    .filter(|&&stripe| stripe >= resident)
                    .map(|&stripe| (stripe, rows_of(log_size, stripe)))
                    .collect_vec();
                // SCREEN (f2 Q1660): `F2_STILE=2` reads the prefix tile by tile (no scratch).
                let values = if crate::prover::m11_env("F2_STILE", 0) >= 2 {
                    B::evaluate_from_prefix_stream(&poly.evals.values, log_size, poly.evals.domain, &targets)
                } else {
                    let mut scratch = base_column_pool.take_or_alloc(log_size);
                    let values = B::evaluate_from_prefix(
                        &poly.evals.values,
                        log_size,
                        poly.evals.domain,
                        &targets,
                        &mut scratch,
                    );
                    base_column_pool.give_back(log_size, scratch);
                    values
                };
                if let Some(values) = values {
                    let len = 1usize << log_size;
                    let shift = stripe_log_size - log_size;
                    return positions
                        .iter()
                        .map(|&position| {
                            let stripe = position >> stripe_log_size;
                            let local = position - (stripe << stripe_log_size);
                            let row = (local >> (shift + 1) << 1) + (local & 1);
                            if stripe < resident {
                                poly.evals.values.at(stripe * len + row)
                            } else {
                                let t = targets.iter().position(|(s, _)| *s == stripe).unwrap();
                                values[t][targets[t].1.binary_search(&row).unwrap()]
                            }
                        })
                        .collect();
                }
            }
            // SCREEN (f2 Q1653): `F2_STILE=1` evaluates a sourced column's rows with TILE straight
            // from its trace values (any circle domain of the column's size determines it), with
            // no coefficients and no stripe FFTs.
            if poly.is_sourced() && crate::prover::m11_env("F2_STILE", 0) != 0 {
                let source = poly.source.as_ref().unwrap();
                let log_size = source.log_size();
                if log_size + log_blowup_factor == poly.evals.domain.log_size() {
                    let targets = stripes
                        .iter()
                        .map(|&stripe| (stripe, rows_of(log_size, stripe)))
                        .collect_vec();
                    // SCREEN (f2 Q1658): `F2_STILE=2` streams the trace values tile by tile.
                    let out = if crate::prover::m11_env("F2_STILE", 0) >= 2 {
                        B::evaluate_from_fill(
                            &|start, dst| source.write_range(start, dst),
                            log_size,
                            CanonicCoset::new(log_size).circle_domain(),
                            poly.evals.domain,
                            &targets,
                        )
                    } else {
                        let values =
                            crate::prover::air::component_prover::source_column::<B>(source.as_ref());
                        let mut scratch = base_column_pool.take_or_alloc(log_size);
                        let out = B::evaluate_from_source(
                            &values,
                            log_size,
                            CanonicCoset::new(log_size).circle_domain(),
                            poly.evals.domain,
                            &targets,
                            &mut scratch,
                        );
                        base_column_pool.give_back(log_size, scratch);
                        out
                    };
                    if let Some(out) = out {
                        let shift = stripe_log_size - log_size;
                        return positions
                            .iter()
                            .map(|&position| {
                                let stripe = position >> stripe_log_size;
                                let local = position - (stripe << stripe_log_size);
                                let row = (local >> (shift + 1) << 1) + (local & 1);
                                let t = targets.iter().position(|(s, _)| *s == stripe).unwrap();
                                out[t][targets[t].1.binary_search(&row).unwrap()]
                            })
                            .collect();
                    }
                }
            }
            let regrown = poly.coeffs.is_none().then(|| poly.regrown_coefficients());
            let coeffs = poly.coeffs.as_ref().or(regrown.as_ref()).unwrap();
            let len = 1usize << coeffs.log_size();
            let shift = stripe_log_size - coeffs.log_size();
            let mut column = base_column_pool.take_or_alloc(coeffs.log_size());
            let mut rows = vec![BaseField::from(0); positions.len()];
            for &stripe in &stripes {
                if (stripe + 1) * len <= poly.evals.values.len() {
                    B::copy_block(&poly.evals.values, stripe * len, &mut column, 0, len);
                } else {
                    B::evaluate_stripe_into(coeffs, poly.evals.domain, twiddles, stripe, &mut column);
                }
                for (row, &position) in rows.iter_mut().zip(&positions) {
                    if position >> stripe_log_size == stripe {
                        let local = position - (stripe << stripe_log_size);
                        *row = column.at((local >> (shift + 1) << 1) + (local & 1));
                    }
                }
            }
            base_column_pool.give_back(coeffs.log_size(), column);
            rows
        };
        // SCREEN (a8n Q1458): `A8N_PLAN1=1` reads first, in parallel, one prefix-path column of
        // every (domain size, coset), so that each prefix plan is built once (rukafe0x
        // rgr1_c5ae9f6b); the other columns then find their plan cached. Same rows, same order.
        #[cfg(feature = "parallel")]
        let rows: Vec<Vec<BaseField>> = if crate::prover::m11_env("A8N_PLAN1", 0) != 0 {
            // f2: under `F2_STILE`, a sourced column reads through a TILE plan too (Q1653).
            let stile = crate::prover::m11_env("F2_STILE", 0) != 0;
            let prefix_path = |poly: &Poly<B>| {
                (poly.coeffs.is_none()
                    && !poly.is_sourced()
                    && poly.evals.values.len() < poly.evals.domain.size())
                    || (stile && poly.is_sourced())
            };
            let mut seen = std::collections::HashSet::new();
            let first = self
                .polynomials
                .iter()
                .enumerate()
                .filter(|(_, poly)| prefix_path(poly))
                .filter(|(_, poly)| {
                    let domain = poly.evals.domain;
                    seen.insert((domain.log_size(), domain.half_coset.initial_index.0))
                })
                .map(|(i, _)| i)
                .collect_vec();
            let mut rows: Vec<Option<Vec<BaseField>>> = vec![None; self.polynomials.len()];
            let first_rows: Vec<Vec<BaseField>> =
                first.par_iter().map(|&i| read_rows(&self.polynomials[i])).collect();
            for (&i, r) in first.iter().zip(first_rows) {
                rows[i] = Some(r);
            }
            let rest: Vec<(usize, Vec<BaseField>)> = rayon::iter::IntoParallelIterator::into_par_iter(0..self.polynomials.len())
                .filter(|i| rows[*i].is_none())
                .map(|i| (i, read_rows(&self.polynomials[i])))
                .collect();
            for (i, r) in rest {
                rows[i] = Some(r);
            }
            rows.into_iter().map(Option::unwrap).collect()
        } else {
            self.polynomials.par_iter().map(read_rows).collect()
        };
        #[cfg(not(feature = "parallel"))]
        let rows: Vec<Vec<BaseField>> = self.polynomials.iter().map(read_rows).collect();
        base_column_pool.release_all_idle();

        let queried_values = rows.iter().map(|rows| rows[..queries.len()].to_vec()).collect_vec();
        // A leaf hashes its row column by column in increasing size, 16 values of one size per
        // update, as `MerkleOpsLifted::build_leaves` does.
        let by_size = rows
            .iter()
            .zip(&self.polynomials)
            .sorted_by_key(|(_, poly)| poly.evals.domain.log_size())
            .collect_vec();
        let leaves: HashMap<usize, <MC::H as MerkleHasherLifted>::Hash> = leaf_positions
            .iter()
            .enumerate()
            .map(|(i, &position)| {
                let mut hasher = MC::H::default();
                for (_, group) in &by_size.iter().group_by(|(_, poly)| poly.evals.domain.log_size()) {
                    for chunk in &group.chunks(16) {
                        let values = chunk.map(|(rows, _)| rows[queries.len() + i]).collect_vec();
                        hasher.update_leaf(&values);
                    }
                }
                (position, hasher.finalize())
            })
            .collect();
        self.commitment.decommit_from(queries, queried_values, |positions| {
            positions.iter().map(|position| leaves[position]).collect()
        })
    }
}

/// The most columns without stored values whose trace values are expanded at once when they are
/// sampled out of domain.
const SOURCED_BATCH: usize = 16;

/// Whether a column of `domain` keeps only stripe 0 of its extension in a striped
/// tree. Small columns keep it whole: some CPU paths read their whole extension.
/// Log of the rows of one packed word: a block column below it is lifted to it (Q1409).
const LOG_N_LANES_PCS: u32 = 4;

fn keeps_prefix(domain: CircleDomain, log_blowup_factor: u32) -> bool {
    domain.log_size() >= log_blowup_factor + 10
}

/// Whether [`CommitmentTreeProver::new_striped`] applies: a blowup of at least 4 (a prefix of
/// half the extension saves nothing below it), a stripe of every column at least one packed
/// word, and a column large enough to keep only a prefix.
fn can_stripe<B: PolyOps>(
    polynomials: &[CircleCoefficients<B>],
    log_blowup_factor: u32,
    lifting_log_size: u32,
) -> bool {
    log_blowup_factor >= 2
        && lifting_log_size >= log_blowup_factor + 10
        && !polynomials.is_empty()
        && polynomials.iter().all(|poly| {
            poly.log_size() >= 4 && poly.log_size() + log_blowup_factor <= lifting_log_size
        })
        && polynomials.iter().any(|poly| {
            keeps_prefix(
                CanonicCoset::new(poly.log_size() + log_blowup_factor).circle_domain(),
                log_blowup_factor,
            )
        })
}

fn print_column_size_histogram<B: BackendForChannel<MC>, MC: MerkleChannel>(
    columns_per_tree: &TreeVec<ColumnVec<&CircleEvaluation<B, BaseField, BitReversedOrder>>>,
) {
    let mut log_size_histogram = HashMap::new();
    for columns in columns_per_tree.iter() {
        for column in columns {
            *log_size_histogram.entry(column.domain.log_size()).or_insert(0) += 1;
        }
    }
    for (log_size, count) in log_size_histogram {
        info!("Log size {log_size}: {count}");
    }
}

/// SCREEN (m11, Q1405): gives every prefix column without a source whose values on its trace
/// domain fit in 16 bits a [`PackedTrace`] source.
fn pack_sources<B: crate::prover::backend::Backend>(
    polynomials: &[CircleCoefficients<B>],
    prefix: &[bool],
    sources: &mut [Option<SharedTraceSource>],
    twiddles: &TwiddleTree<B>,
) {
    let pack = |((poly, &prefix), source): ((&CircleCoefficients<B>, &bool), &mut Option<SharedTraceSource>)| {
        if !prefix || source.is_some() {
            return;
        }
        *source = pack_column(poly, twiddles);
    };
    #[cfg(feature = "parallel")]
    {
        use rayon::iter::{IndexedParallelIterator, IntoParallelRefIterator, IntoParallelRefMutIterator, ParallelIterator};
        polynomials.par_iter().zip(prefix.par_iter()).zip(sources.par_iter_mut()).for_each(pack);
    }
    #[cfg(not(feature = "parallel"))]
    polynomials.iter().zip(prefix).zip(sources.iter_mut()).for_each(pack);
}

/// SCREEN (m11): a [`PackedTrace`] source of `poly`'s values on its trace domain, if they fit 16 bits.
fn pack_column<B: crate::prover::backend::Backend>(
    poly: &CircleCoefficients<B>,
    twiddles: &TwiddleTree<B>,
) -> Option<SharedTraceSource> {
    let domain = CanonicCoset::new(poly.log_size()).circle_domain();
    let values = poly.evaluate_with_twiddles(domain, twiddles).values.to_cpu();
    crate::prover::air::component_prover::PackedTrace::pack(poly.log_size(), &values)
        .map(|packed| std::sync::Arc::new(packed) as SharedTraceSource)
}
