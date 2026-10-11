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
        let mut tree = CommitmentTreeProver::new_ex(
            polynomials,
            self.config.fri_config.log_blowup_factor,
            self.twiddles,
            self.store_polynomials_coefficients,
            lifting_log_size,
            &self.base_column_pool,
            true,
        );
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
                    if poly.coeffs.is_none() && poly.evals.values.len() < poly.evals.domain.size() {
                        poly.coeffs = Some(poly.take_regrown_coefficients());
                    }
                };
                #[cfg(feature = "parallel")]
                {
                    use rayon::iter::IntoParallelRefMutIterator;
                    tree.polynomials.par_iter_mut().for_each(to_coefficients);
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
                    if poly.evals.values.len() == 0 {
                        let coeffs = poly.coeffs.take().unwrap();
                        let mut stripe = pool.take_or_alloc(coeffs.log_size());
                        B::evaluate_stripe_into(&coeffs, poly.evals.domain, twiddles, 0, &mut stripe);
                        poly.evals.values = stripe;
                    }
                };
                #[cfg(feature = "parallel")]
                {
                    use rayon::iter::IntoParallelRefMutIterator;
                    tree.polynomials.par_iter_mut().for_each(to_stripe);
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
            for batch in members.chunks(SOURCED_BATCH) {
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
        let fri_prover =
            FriProver::<B, MC>::commit(channel, self.config.fri_config, &quotients, self.twiddles);
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
        for column in quotients.values.columns {
            self.base_column_pool.give_back(lifting_log_size, column);
        }
        self.base_column_pool.release_all_idle();
        let commitments = self.roots();
        let twiddles = self.twiddles;
        let pool: &BaseColumnPool<B> = &self.base_column_pool;
        let (queried_values, decommitments, aux): (Vec<_>, Vec<_>, Vec<_>) = self
            .trees
            .as_ref()
            .zip_eq(query_positions_tree)
            .map(|(tree, query_positions)| {
                tree.decommit(query_positions, twiddles, pool, self.config.fri_config.log_blowup_factor)
            })
            .0
            .into_iter()
            .map(|(v, x)| (v, x.decommitment, x.aux))
            .multiunzip();

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
        sources: Vec<Option<SharedTraceSource>>,
    ) -> Self {
        let _span = span!(Level::INFO, "Striped extension and Merkle").entered();
        // The witness and interaction writers leave freed temporaries in glibc's arenas: hand
        // them back before the scratch of the commitment is taken.
        crate::prover::backend::simd::column::trim_heap();
        // Each stripe is committed as `2^sub` blocks, so that the scratch of every column is a
        // block, not a stripe. The tree is the same: its leaves are rows.
        let sub = 1u32
            .min(polynomials.iter().map(|poly| poly.log_size()).min().unwrap() - 4);
        let n_stripes = 1usize << (log_blowup_factor + sub);
        let stripe_lifting_log_size = lifting_log_size - log_blowup_factor - sub;
        let domains = polynomials
            .iter()
            .map(|poly| CanonicCoset::new(poly.log_size() + log_blowup_factor).circle_domain())
            .collect_vec();
        let prefix = domains.iter().map(|&domain| keeps_prefix(domain, log_blowup_factor)).collect_vec();
        // A prefix column with a source keeps nothing but the source: every stripe of it, stripe 0
        // included, is hashed from its scratch column.
        let sourced = prefix
            .iter()
            .zip(&sources)
            .map(|(&prefix, source): (&bool, &Option<SharedTraceSource>)| {
                prefix && !keep_coef && source.is_some()
            })
            .collect_vec();
        // With `keep_coef`, a prefix column keeps its coefficients instead of stripe 0 (the same size):
        // composition and the samples read coefficients, and stripe 0 is evaluated back only for
        // the quotients (`coefficients_to_striped`).
        // Prefix columns get their stripe-0 buffer now (the last stripe hashed, which the FFT writes
        // in place). Whole-extension columns are evaluated now.
        let mut evals = polynomials
            .iter()
            .zip(&domains)
            .zip(&prefix)
            .zip(&sourced)
            .map(|(((poly, &domain), &prefix), &sourced)| {
                if (prefix && keep_coef) || sourced {
                    Col::<B, BaseField>::zeros(0)
                } else if prefix {
                    base_column_pool.take_or_alloc(poly.log_size())
                } else {
                    let buffer = base_column_pool.take_or_alloc(domain.log_size());
                    B::evaluate_into(poly, domain, twiddles, buffer).values
                }
            })
            .collect_vec();
        let mut scratch = polynomials
            .iter()
            .map(|poly| base_column_pool.take_or_alloc(poly.log_size() - sub))
            .collect_vec();
        let mut coeffs = polynomials.into_iter().map(Some).collect_vec();

        // Stripe 0, the only one a prefix column keeps, last and in place.
        let n_first = 1usize << sub;
        let order = (n_first..n_stripes).chain(0..n_first).collect_vec();
        let mut stripe_layers: Vec<Option<Vec<Col<B, <MC::H as MerkleHasherLifted>::Hash>>>> =
            (0..n_stripes).map(|_| None).collect();
        for &stripe in &order {
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
            let fill = |(((((poly, &domain), &prefix), &sourced), eval), column): (
                (
                    (((&Option<CircleCoefficients<B>>, &CircleDomain), &bool), &bool),
                    &mut Col<B, BaseField>,
                ),
                &mut Col<B, BaseField>,
            )| {
                let poly = poly.as_ref().unwrap();
                let len = 1usize << (poly.log_size() - sub);
                if !prefix {
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
                use rayon::iter::{IndexedParallelIterator, IntoParallelRefIterator, IntoParallelRefMutIterator};
                coeffs
                    .par_iter()
                    .zip(domains.par_iter())
                    .zip(prefix.par_iter())
                    .zip(sourced.par_iter())
                    .zip(evals.par_iter_mut())
                    .zip(scratch.par_iter_mut())
                    .for_each(fill);
            }
            #[cfg(not(feature = "parallel"))]
            coeffs
                .iter()
                .zip(&domains)
                .zip(&prefix)
                .zip(&sourced)
                .zip(evals.iter_mut())
                .zip(scratch.iter_mut())
                .for_each(fill);
            let columns = (0..coeffs.len())
                .map(|i| {
                    if prefix[i] && !sourced[i] && stripe == 0 && sub == 0 && !keep_coef {
                        &evals[i]
                    } else {
                        &scratch[i]
                    }
                })
                .sorted_by_key(|column| column.len())
                .collect_vec();
            stripe_layers[stripe] = Some(B::build_layers_sparse(&columns, stripe_lifting_log_size));
        }
        // A prefix column's coefficients are regrown from stripe 0 where they are needed (the
        // out-of-domain samples, composition and the decommitment), one column or tree at a time.
        let log_sizes = coeffs.iter().map(|poly| poly.as_ref().unwrap().log_size()).collect_vec();
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
        let stripe_layers = stripe_layers.into_iter().map(Option::unwrap).collect_vec();

        // Level `l` of the tree, up to the stripes' roots, is the concatenation of the stripes'
        // levels `l`, in stripe order.
        let mut layers: Vec<Col<B, <MC::H as MerkleHasherLifted>::Hash>> = (0
            ..=stripe_lifting_log_size as usize)
            .map(|level| {
                stripe_layers.iter().flat_map(|layers| layers[level].to_cpu()).collect()
            })
            .collect();
        drop(stripe_layers);
        for _ in stripe_lifting_log_size..lifting_log_size {
            let next = B::build_next_layer(layers.last().unwrap());
            layers.push(next);
        }
        layers.reverse();

        let polynomials = coeffs
            .into_iter()
            .zip(domains)
            .zip(evals)
            .zip(sources.into_iter().zip(sourced))
            .map(|(((coeffs, domain), values), (source, sourced))| {
                if sourced {
                    return Poly::from_source(domain, source.unwrap());
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
                let mut scratch = base_column_pool.take_or_alloc(log_size);
                let values = B::evaluate_from_prefix(
                    &poly.evals.values,
                    log_size,
                    poly.evals.domain,
                    &targets,
                    &mut scratch,
                );
                base_column_pool.give_back(log_size, scratch);
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
        #[cfg(feature = "parallel")]
        let rows: Vec<Vec<BaseField>> = self.polynomials.par_iter().map(read_rows).collect();
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
