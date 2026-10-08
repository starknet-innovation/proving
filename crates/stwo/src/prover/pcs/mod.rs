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
use crate::prover::pcs::quotient_ops::compute_fri_quotients;
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
        let mut tree = CommitmentTreeProver::new(
            polynomials,
            self.config.fri_config.log_blowup_factor,
            self.twiddles,
            self.store_polynomials_coefficients,
            lifting_log_size,
            &self.base_column_pool,
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

    pub fn trace(&self) -> Trace<'_, B> {
        let polys = self.polynomials();
        Trace { polys }
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
        assert_eq!(polynomials.len(), sampled_points.len());
        for (tree_index, columns) in polynomials.iter().enumerate() {
            assert_eq!(columns.len(), sampled_points[tree_index].len());
            for (column_index, poly) in columns.iter().enumerate() {
                let log_size = poly.evals.domain.log_size();
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
        print_column_size_histogram::<B, MC>(&columns);
        // Compute oods quotients for boundary constraints on the sampled points.
        let quotients = compute_fri_quotients(
            &columns,
            &samples,
            channel.draw_secure_felt(),
            lifting_log_size,
            self.twiddles,
            self.config.fri_config.log_blowup_factor,
        );

        // Run FRI commitment phase on the oods quotients.
        let fri_prover =
            FriProver::<B, MC>::commit(channel, self.config.fri_config, &quotients, self.twiddles);

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
        if B::STRIPES && can_stripe(&polynomials, log_blowup_factor, lifting_log_size) {
            return Self::new_striped(
                polynomials,
                log_blowup_factor,
                twiddles,
                lifting_log_size,
                base_column_pool,
            );
        }
        let span = span!(Level::INFO, "Extension").entered();
        // A tree that is not striped keeps its coefficients where trees are striped: composition
        // then extends every column from its coefficients (see `compute_composition_polynomial`).
        let polynomials = B::evaluate_polynomials(
            polynomials,
            log_blowup_factor,
            twiddles,
            store_polynomials_coefficients || B::STRIPES,
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
    ) -> Self {
        let _span = span!(Level::INFO, "Striped extension and Merkle").entered();
        let n_stripes = 1usize << log_blowup_factor;
        let stripe_lifting_log_size = lifting_log_size - log_blowup_factor;
        let domains = polynomials
            .iter()
            .map(|poly| CanonicCoset::new(poly.log_size() + log_blowup_factor).circle_domain())
            .collect_vec();
        let prefix = domains.iter().map(|&domain| keeps_prefix(domain, log_blowup_factor)).collect_vec();
        // Prefix columns get their stripe-0 buffer now (the last stripe hashed, which the FFT writes
        // in place). Whole-extension columns are evaluated now.
        let mut evals = polynomials
            .iter()
            .zip(&domains)
            .zip(&prefix)
            .map(|((poly, &domain), &prefix)| {
                if prefix {
                    base_column_pool.take_or_alloc(poly.log_size())
                } else {
                    let buffer = base_column_pool.take_or_alloc(domain.log_size());
                    B::evaluate_into(poly, domain, twiddles, buffer).values
                }
            })
            .collect_vec();
        let mut scratch = polynomials
            .iter()
            .map(|poly| base_column_pool.take_or_alloc(poly.log_size()))
            .collect_vec();
        let mut coeffs = polynomials.into_iter().map(Some).collect_vec();

        // Stripe 0, the only one a prefix column keeps, last and in place.
        let order = (1..n_stripes).chain([0]).collect_vec();
        let mut stripe_layers: Vec<Option<Vec<Col<B, <MC::H as MerkleHasherLifted>::Hash>>>> =
            (0..n_stripes).map(|_| None).collect();
        for &stripe in &order {
            if stripe == 0 {
                // Stripe 0 is written in place, so the prefix columns' scratch is dead: free it
                // first.
                for (i, column) in scratch.iter_mut().enumerate() {
                    if prefix[i] {
                        drop(std::mem::replace(column, Col::<B, BaseField>::zeros(0)));
                    }
                }
                base_column_pool.release_all_idle();
            }
            let fill = |((((poly, &domain), &prefix), eval), column): (
                (((&Option<CircleCoefficients<B>>, &CircleDomain), &bool), &mut Col<B, BaseField>),
                &mut Col<B, BaseField>,
            )| {
                let poly = poly.as_ref().unwrap();
                let len = 1usize << poly.log_size();
                if !prefix {
                    B::copy_block(eval, stripe * len, column, 0, len);
                } else if stripe == 0 {
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
                    .zip(evals.par_iter_mut())
                    .zip(scratch.par_iter_mut())
                    .for_each(fill);
            }
            #[cfg(not(feature = "parallel"))]
            coeffs
                .iter()
                .zip(&domains)
                .zip(&prefix)
                .zip(evals.iter_mut())
                .zip(scratch.iter_mut())
                .for_each(fill);
            let columns = (0..coeffs.len())
                .map(|i| if prefix[i] && stripe == 0 { &evals[i] } else { &scratch[i] })
                .sorted_by_key(|column| column.len())
                .collect_vec();
            stripe_layers[stripe] = Some(B::build_layers_sparse(&columns, stripe_lifting_log_size));
        }
        // A prefix column's coefficients are regrown from stripe 0 where they are needed (the
        // out-of-domain samples, composition and the decommitment), one column or tree at a time.
        let log_sizes = coeffs.iter().map(|poly| poly.as_ref().unwrap().log_size()).collect_vec();
        for (poly, &prefix) in coeffs.iter_mut().zip(&prefix) {
            if prefix {
                drop(poly.take());
            }
        }
        for (&log_size, column) in log_sizes.iter().zip(scratch) {
            if column.len() == 1 << log_size {
                base_column_pool.give_back(log_size, column);
            }
        }
        base_column_pool.release_all_idle();
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
            .map(|((coeffs, domain), values)| {
                let mut evals = CircleEvaluation::new(
                    CanonicCoset::new(values.len().ilog2()).circle_domain(),
                    values,
                );
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
        // The rows of `positions` in one column, read from its stripes as the tree lifts them.
        let read_rows = |poly: &Poly<B>| -> Vec<BaseField> {
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
