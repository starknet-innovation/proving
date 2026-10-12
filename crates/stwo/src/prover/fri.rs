use hashbrown::HashMap;
use itertools::Itertools;
use num_traits::Zero;
use tracing::instrument;

use crate::core::channel::{Channel, MerkleChannel};
use crate::core::fields::m31::BaseField;
use crate::core::fields::qm31::{QM31, SecureField};
use crate::core::fri::{
    ExtendedFriLayerProof, ExtendedFriProof, FriConfig, FriLayerProof, FriLayerProofAux, FriProof,
    FriProofAux,
};
use crate::core::poly::line::LinePoly;
use crate::core::queries::{Queries, draw_queries};
use crate::core::vcs_lifted::merkle_hasher::MerkleHasherLifted;
use crate::core::vcs_lifted::verifier::LOG_PACKED_LEAF_SIZE;
use crate::prover::backend::{Col, Column, ColumnOps};
use crate::prover::line::LineEvaluation;
use crate::prover::poly::BitReversedOrder;
use crate::prover::poly::circle::{PolyOps, SecureEvaluation};
use crate::prover::poly::twiddles::TwiddleTree;
use crate::prover::secure_column::SecureColumnByCoords;
use crate::prover::vcs_lifted::ops::MerkleOpsLifted;
use crate::prover::vcs_lifted::prover::MerkleProverLifted;

pub trait FriOps: ColumnOps<BaseField> + PolyOps + Sized + ColumnOps<SecureField> {
    /// Folds a degree `d` polynomial into a degree `d / 2^k` polynomial, where `k = alphas.len()`.
    ///
    /// For i ∈ [0, k), the i-th fold computes the evaluation of `f_{i+1} = f_{i,0} + alphas[i] *
    /// f_{i,1}` on `E_{i+1} = pi(E_i)`, where:
    /// * `f_i` is the folded polynomial at fold i (`f_0 = eval`), evaluated on line domain `E_i`
    ///   (`E_0 = eval.domain()`).
    /// * `pi(x) = 2x^2 - 1` is the doubling map.
    /// * `f_{i,0}` and `f_{i,1}` are the polynomials determined by the identity `2*f_i(x) =
    ///   f_{i,0}(pi(x)) + x * f_{i,1}(pi(x))`.
    ///
    /// # Panics
    ///
    /// Panics if `alphas` is empty.
    fn fold_line(
        eval: &LineEvaluation<Self>,
        alphas: &[SecureField],
        twiddles: &TwiddleTree<Self>,
    ) -> LineEvaluation<Self>;

    /// Folds and accumulates a degree `d` circle polynomial into a degree `d/2` univariate
    /// polynomial.
    ///
    /// Let `src` be the evaluation of a circle polynomial `f` on a
    /// [`CircleDomain`] `E`. This function computes evaluations of `f' = f0
    /// + alpha * f1` on the x-coordinates of `E` such that `2f(p) = f0(px) + py * f1(px)`. The
    /// evaluations of `f'` are accumulated into `dst` by the formula `dst = dst * alpha^2 + f'`.
    ///
    /// # Panics
    ///
    /// Panics if `src` is not double the length of `dst`.
    ///
    /// [`CircleDomain`]: crate::core::poly::circle::CircleDomain
    // TODO(andrew): Make folding factor generic.
    // TODO(andrew): Fold directly into FRI layer to prevent allocation.
    fn fold_circle_into_line(
        src: &SecureEvaluation<Self, BitReversedOrder>,
        alpha: SecureField,
        twiddles: &TwiddleTree<Self>,
    ) -> LineEvaluation<Self>;

    /// [`Self::fold_circle_into_line`] followed by [`Self::fold_line`] with `line_alphas` (when
    /// not empty). A backend may fuse them so that the intermediate line evaluation, twice the
    /// size of the result per folded layer, is never resident.
    fn fold_circle_into_line_then_lines(
        src: &SecureEvaluation<Self, BitReversedOrder>,
        alpha: SecureField,
        line_alphas: &[SecureField],
        twiddles: &TwiddleTree<Self>,
    ) -> LineEvaluation<Self> {
        let line = Self::fold_circle_into_line(src, alpha, twiddles);
        if line_alphas.is_empty() {
            return line;
        }
        Self::fold_line(&line, line_alphas, twiddles)
    }

    /// SCREEN (w5 Q1603): [`Self::fold_circle_into_line_then_lines`] of the circle evaluation on
    /// `domain` (canonic, `2^log_size` rows) restricted to its rows `[start, start + stripe.len())`
    /// (`stripe` holds them), written to the matching rows of `dst`, the folded line evaluation of
    /// the whole domain. The rows of `dst` are the same field elements the whole fold writes.
    /// Returns the folded line domain.
    #[allow(clippy::too_many_arguments)]
    fn fold_circle_stripe_into_line_then_lines(
        _stripe: &SecureColumnByCoords<Self>,
        _start: usize,
        _domain: crate::core::poly::circle::CircleDomain,
        _alpha: SecureField,
        _line_alphas: &[SecureField],
        _twiddles: &TwiddleTree<Self>,
        _dst: &mut SecureColumnByCoords<Self>,
    ) -> crate::core::poly::line::LineDomain {
        unimplemented!("W5_QPOLY: striped FRI layer 0 is SIMD only")
    }

    /// Decomposes a FRI-space polynomial into a polynomial inside the fft-space and the
    /// remainder term.
    /// FRI-space: polynomials of total degree n/2.
    /// Based on lemma #12 from the CircleStark paper: f(P) = g(P)+ lambda * alternating(P),
    /// where lambda is the cosset diff of eval, and g is a polynomial in the fft-space.
    fn decompose(
        eval: &SecureEvaluation<Self, BitReversedOrder>,
    ) -> (SecureEvaluation<Self, BitReversedOrder>, SecureField);
}

/// Computes `len` folding alphas derived by repeated squaring: `[alpha, alpha^2, alpha^4, ...]`.
pub fn squared_alpha_powers(alpha: SecureField, len: u32) -> Vec<SecureField> {
    let mut alphas = Vec::with_capacity(len as usize);
    let mut alpha = alpha;
    for _ in 0..len {
        alphas.push(alpha);
        alpha = alpha * alpha;
    }
    alphas
}

pub struct FriDecommitResult<H: MerkleHasherLifted> {
    pub fri_proof: ExtendedFriProof<H>,
    pub query_positions: Vec<usize>,
    pub unsorted_query_locations: Vec<usize>,
}

/// A FRI prover that applies the FRI protocol to prove a set of polynomials are of low degree.
pub struct FriProver<'a, B: FriOps + MerkleOpsLifted<MC::H>, MC: MerkleChannel> {
    config: FriConfig,
    first_layer: FriFirstLayerProver<'a, B, MC::H>,
    inner_layers: Vec<FriInnerLayerProver<B, MC::H>>,
    last_layer_poly: LinePoly,
}
impl<'a, B: FriOps + MerkleOpsLifted<MC::H>, MC: MerkleChannel> FriProver<'a, B, MC> {
    /// Runs the commitment phase of FRI on one circle evaluation over a canonic circle domain.
    ///
    /// # Panics
    ///
    /// Panics if:
    /// * The evaluation is not from a sufficiently low degree circle polynomial.
    /// * The evaluation domain is not a canonic circle domain.
    #[instrument(skip_all)]
    pub fn commit(
        channel: &mut MC::C,
        config: FriConfig,
        column: &'a SecureEvaluation<B, BitReversedOrder>,
        twiddles: &TwiddleTree<B>,
    ) -> Self {
        assert!(column.domain.is_canonic(), "not canonic");

        let first_layer = Self::commit_first_layer(channel, &config, column);
        let (inner_layers, last_layer_evaluation) =
            Self::commit_inner_layers(channel, config, column, twiddles);
        let last_layer_poly = Self::commit_last_layer(channel, config, last_layer_evaluation);

        Self { config, first_layer, inner_layers, last_layer_poly }
    }

    /// SCREEN (w5 Q1603): [`Self::commit`] for a quotient kept as its polynomial: `coeffs` holds
    /// the four coordinate coefficient columns (`2^(lifting_log_size - log_blowup_factor)` each).
    /// Layer 0 is never resident: each of its `2^log_blowup_factor` stripes is evaluated, hashed
    /// into the layer-0 tree, and dropped; once the root is mixed, each stripe is evaluated again
    /// and folded into the first line layer. The decommitment evaluates the stripes it reads once
    /// more. Every root, fold and opened value is the one [`Self::commit`] produces.
    pub fn commit_striped(
        channel: &mut MC::C,
        config: FriConfig,
        coeffs: [crate::prover::poly::circle::CircleCoefficients<B>; 4],
        lifting_log_size: u32,
        log_blowup_factor: u32,
        twiddles: &'a TwiddleTree<B>,
    ) -> Self {
        use crate::core::poly::circle::CanonicCoset;
        let domain = CanonicCoset::new(lifting_log_size).circle_domain();
        let stripe_log_size = lifting_log_size - log_blowup_factor;
        let n_stripes = 1usize << log_blowup_factor;
        assert!(config.fold_step > 1 && stripe_log_size > LOG_PACKED_LEAF_SIZE + 5);
        let evaluate = |stripe: usize| -> SecureColumnByCoords<B> {
            let mut columns = SecureColumnByCoords::<B>::zeros(1 << stripe_log_size);
            for (coeffs, column) in coeffs.iter().zip(columns.columns.iter_mut()) {
                B::evaluate_stripe_into(coeffs, domain, twiddles, stripe, column);
            }
            columns
        };
        // Layer 0: the packed-leaf tree of each stripe, concatenated level by level.
        let stripe_packed_log_size = stripe_log_size - LOG_PACKED_LEAF_SIZE;
        let stripe_layers = (0..n_stripes)
            .map(|stripe| {
                let columns = evaluate(stripe);
                let columns: [&Col<B, BaseField>; 4] = std::array::from_fn(|c| &columns.columns[c]);
                B::build_packed_layers(&columns, stripe_packed_log_size)
            })
            .collect_vec();
        let mut layers: Vec<Col<B, <MC::H as MerkleHasherLifted>::Hash>> = (0..=stripe_packed_log_size as usize)
            .map(|level| stripe_layers.iter().flat_map(|layers| layers[level].to_cpu()).collect())
            .collect();
        drop(stripe_layers);
        for _ in stripe_packed_log_size..lifting_log_size - LOG_PACKED_LEAF_SIZE {
            let next = B::build_next_layer(layers.last().unwrap());
            layers.push(next);
        }
        layers.reverse();
        let merkle_tree = MerkleProverLifted { layers };
        MC::mix_root(channel, merkle_tree.root());

        // The circle-to-line fold and the extra line folds, stripe by stripe.
        let folding_alpha = channel.draw_secure_felt();
        let extra_line_folds = config.fold_step.saturating_sub(1);
        let alpha_sq_powers = squared_alpha_powers(folding_alpha * folding_alpha, extra_line_folds);
        let mut values = SecureColumnByCoords::<B>::zeros(1 << (lifting_log_size - 1 - extra_line_folds));
        let mut line_domain = None;
        for stripe in 0..n_stripes {
            let columns = evaluate(stripe);
            line_domain = Some(B::fold_circle_stripe_into_line_then_lines(
                &columns,
                stripe << stripe_log_size,
                domain,
                folding_alpha,
                &alpha_sq_powers,
                twiddles,
                &mut values,
            ));
        }
        let layer_evaluation = LineEvaluation::new(line_domain.unwrap(), values);
        let first_layer = FriFirstLayerProver {
            column: None,
            merkle_tree,
            pack_leaves: true,
            striped: Some(StripedFirstLayer { coeffs, twiddles, domain, log_blowup_factor }),
        };
        let (inner_layers, last_layer_evaluation) =
            Self::commit_inner_from(channel, config, layer_evaluation, twiddles);
        let last_layer_poly = Self::commit_last_layer(channel, config, last_layer_evaluation);
        Self { config, first_layer, inner_layers, last_layer_poly }
    }

    /// Commits to the first FRI layer.
    fn commit_first_layer(
        channel: &mut MC::C,
        config: &FriConfig,
        column: &'a SecureEvaluation<B, BitReversedOrder>,
    ) -> FriFirstLayerProver<'a, B, MC::H> {
        // The circle-to-line fold is always equal to the config.fold_step.
        // TODO(Leo): consider support for smaller steps.
        let layer = FriFirstLayerProver::new(column, config.fold_step);
        MC::mix_root(channel, layer.merkle_tree.root());
        layer
    }

    /// Builds and commits to the inner FRI layers (all layers except the first and last).
    ///
    /// Returns all inner layers and the evaluation of the last layer.
    fn commit_inner_layers(
        channel: &mut MC::C,
        config: FriConfig,
        column: &SecureEvaluation<B, BitReversedOrder>,
        twiddles: &TwiddleTree<B>,
    ) -> (Vec<FriInnerLayerProver<B, MC::H>>, LineEvaluation<B>) {
        let folding_alpha = channel.draw_secure_felt();

        // The circle-to-line fold and any additional line folds of the first stage, in one pass.
        let extra_line_folds = config.fold_step.saturating_sub(1);
        let alpha_sq_powers = squared_alpha_powers(folding_alpha * folding_alpha, extra_line_folds);
        let layer_evaluation =
            B::fold_circle_into_line_then_lines(column, folding_alpha, &alpha_sq_powers, twiddles);
        Self::commit_inner_from(channel, config, layer_evaluation, twiddles)
    }

    /// The inner layers of [`Self::commit_inner_layers`] from the first line layer on.
    fn commit_inner_from(
        channel: &mut MC::C,
        config: FriConfig,
        layer_evaluation: LineEvaluation<B>,
        twiddles: &TwiddleTree<B>,
    ) -> (Vec<FriInnerLayerProver<B, MC::H>>, LineEvaluation<B>) {
        let mut layers = Vec::new();
        let mut line_log_size = layer_evaluation.domain().log_size();
        let mut layer_evaluation = layer_evaluation;

        let last_layer_log_domain_size = config.last_layer_domain_size().ilog2();
        assert!(
            line_log_size >= last_layer_log_domain_size,
            "The circle-to-line fold results in a smaller line domain than the last layer."
        );
        // If we're already at the last layer, there are no inner layers to compute.
        if line_log_size == last_layer_log_domain_size {
            return (layers, layer_evaluation);
        }
        // While we can, skip `config.fold_step` layers.
        while line_log_size > last_layer_log_domain_size + config.fold_step {
            let layer = FriInnerLayerProver::new(layer_evaluation, config.fold_step);
            MC::mix_root(channel, layer.merkle_tree.root());
            let folding_alpha = channel.draw_secure_felt();
            let alpha_sq_powers = squared_alpha_powers(folding_alpha, config.fold_step);
            layer_evaluation = B::fold_line(&layer.evaluation, &alpha_sq_powers, twiddles);
            layers.push(layer);
            line_log_size -= config.fold_step;
        }

        // Do one last fold (of size 0 < k <= config.fold_step) to reach the correct size.
        let last_fold_step = line_log_size - last_layer_log_domain_size;
        let layer = FriInnerLayerProver::new(layer_evaluation, last_fold_step);
        MC::mix_root(channel, layer.merkle_tree.root());
        let folding_alpha = channel.draw_secure_felt();
        let alpha_sq_powers = squared_alpha_powers(folding_alpha, last_fold_step);
        layer_evaluation = B::fold_line(&layer.evaluation, &alpha_sq_powers, twiddles);
        layers.push(layer);

        (layers, layer_evaluation)
    }

    /// Builds and commits to the last layer.
    ///
    /// The layer is committed to by sending the verifier all the coefficients of the remaining
    /// polynomial.
    ///
    /// # Panics
    ///
    /// Panics if:
    /// * The evaluation domain size exceeds the maximum last layer domain size.
    /// * The evaluation is not of sufficiently low degree.
    fn commit_last_layer(
        channel: &mut MC::C,
        config: FriConfig,
        evaluation: LineEvaluation<B>,
    ) -> LinePoly {
        assert_eq!(evaluation.len(), config.last_layer_domain_size());

        let evaluation = evaluation.to_cpu();
        let mut coeffs = evaluation.interpolate().into_ordered_coefficients();

        let last_layer_degree_bound = 1 << config.log_last_layer_degree_bound;
        let zeros = coeffs.split_off(last_layer_degree_bound);
        assert!(zeros.iter().all(SecureField::is_zero), "invalid degree");

        let last_layer_poly = LinePoly::from_ordered_coefficients(coeffs);
        channel.mix_felts(&last_layer_poly);

        last_layer_poly
    }

    /// Returns a FRI proof and the query positions.
    pub fn decommit(self, channel: &mut MC::C) -> FriDecommitResult<MC::H> {
        let first_layer_log_size = self.first_layer.log_size();
        let unsorted_query_locations =
            draw_queries(channel, first_layer_log_size, self.config.n_queries);
        let queries = Queries::new(&unsorted_query_locations, first_layer_log_size);

        let fri_proof = self.decommit_on_queries(&queries);
        FriDecommitResult {
            fri_proof,
            query_positions: queries.positions,
            unsorted_query_locations,
        }
    }

    /// # Panics
    ///
    /// Panics if the queries were sampled on the wrong domain size.
    pub fn decommit_on_queries(self, queries: &Queries) -> ExtendedFriProof<MC::H> {
        let Self { config, first_layer, inner_layers, last_layer_poly } = self;

        let first_layer_proof = first_layer.decommit(queries, config.fold_step);

        let inner_layer_proofs = inner_layers
            .into_iter()
            .scan(queries.fold(config.fold_step), |layer_queries, layer| {
                let fold_step = layer.fold_step;
                let layer_proof = layer.decommit(layer_queries);
                *layer_queries = layer_queries.fold(fold_step);
                Some(layer_proof)
            })
            .collect_vec();

        let (inner_proofs, inner_layers_aux): (Vec<_>, Vec<_>) =
            inner_layer_proofs.into_iter().map(|p| (p.proof, p.aux)).unzip();

        ExtendedFriProof {
            proof: FriProof {
                first_layer: first_layer_proof.proof,
                inner_layers: inner_proofs,
                last_layer_poly,
            },
            aux: FriProofAux { first_layer: first_layer_proof.aux, inner_layers: inner_layers_aux },
        }
    }
}

/// Commitment to the first FRI layer.
struct FriFirstLayerProver<'a, B: FriOps + MerkleOpsLifted<H>, H: MerkleHasherLifted> {
    column: Option<&'a SecureEvaluation<B, BitReversedOrder>>,
    merkle_tree: MerkleProverLifted<B, H>,
    pack_leaves: bool,
    striped: Option<StripedFirstLayer<'a, B>>,
}

/// SCREEN (w5 Q1603): the first layer of [`FriProver::commit_striped`]: the quotient's coordinate
/// polynomials, from which the stripes the decommitment reads are evaluated again.
struct StripedFirstLayer<'a, B: FriOps> {
    coeffs: [crate::prover::poly::circle::CircleCoefficients<B>; 4],
    twiddles: &'a TwiddleTree<B>,
    domain: crate::core::poly::circle::CircleDomain,
    log_blowup_factor: u32,
}

impl<'a, B: FriOps + MerkleOpsLifted<H>, H: MerkleHasherLifted> FriFirstLayerProver<'a, B, H> {
    fn new(first_layer_column: &'a SecureEvaluation<B, BitReversedOrder>, fold_step: u32) -> Self {
        let pack_leaves =
            first_layer_column.values.len().ilog2() >= LOG_PACKED_LEAF_SIZE && fold_step > 1;
        let log_rows_per_leaf = if pack_leaves { LOG_PACKED_LEAF_SIZE } else { 0 };
        let log_column_size = first_layer_column.values.len().ilog2() - log_rows_per_leaf;
        let merkle_tree = MerkleProverLifted::commit(
            first_layer_column.values.columns.iter().collect_vec(),
            log_column_size,
            log_rows_per_leaf,
        );

        FriFirstLayerProver { column: Some(first_layer_column), merkle_tree, pack_leaves, striped: None }
    }

    fn log_size(&self) -> u32 {
        match (&self.column, &self.striped) {
            (Some(column), _) => column.domain.log_size(),
            (None, Some(striped)) => striped.domain.log_size(),
            _ => unreachable!(),
        }
    }

    /// The decommitment of a [`StripedFirstLayer`]: the rows it reads (the queries' folding cosets
    /// and the packed leaves under the omitted tree layers) are read from the stripes holding
    /// them, evaluated one at a time.
    fn decommit_striped(self, queries: &Queries, fold_step: u32) -> ExtendedFriLayerProof<H> {
        let striped = self.striped.as_ref().unwrap();
        let log_size = striped.domain.log_size();
        assert_eq!(queries.log_domain_size, log_size);
        let stripe_log_size = log_size - striped.log_blowup_factor;
        // Rows of the queries' cosets, then the packed leaves those rows sit in.
        let coset_rows = queries
            .positions
            .iter()
            .map(|&position| position >> fold_step << fold_step)
            .dedup()
            .flat_map(|start| start..start + (1 << fold_step))
            .collect_vec();
        let leaf_positions = coset_rows
            .iter()
            .map(|&row| row >> LOG_PACKED_LEAF_SIZE)
            .dedup()
            .collect_vec();
        let omitted = self.merkle_tree.omitted_leaf_positions(&leaf_positions);
        let mut rows = coset_rows
            .iter()
            .copied()
            .chain(omitted.iter().flat_map(|&leaf| {
                (leaf << LOG_PACKED_LEAF_SIZE)..(leaf + 1) << LOG_PACKED_LEAF_SIZE
            }))
            .collect_vec();
        rows.sort_unstable();
        rows.dedup();
        let mut values: HashMap<usize, [BaseField; 4]> = HashMap::new();
        for (stripe, stripe_rows) in &rows.iter().group_by(|&&row| row >> stripe_log_size) {
            let mut columns = SecureColumnByCoords::<B>::zeros(1 << stripe_log_size);
            for (coeffs, column) in striped.coeffs.iter().zip(columns.columns.iter_mut()) {
                B::evaluate_stripe_into(coeffs, striped.domain, striped.twiddles, stripe, column);
            }
            for &row in stripe_rows {
                let local = row - (stripe << stripe_log_size);
                values.insert(row, std::array::from_fn(|c| columns.columns[c].at(local)));
            }
        }
        let at = |row: usize| QM31::from_m31_array(values[&row]);
        let (decommitment_positions, column_witness, value_map) =
            decommitment_positions_and_witness_evals_at(at, &queries.positions, fold_step);
        let decommitment_positions = decommitment_positions
            .iter()
            .map(|position| position >> LOG_PACKED_LEAF_SIZE)
            .dedup()
            .collect_vec();
        let (_, decommitment) =
            self.merkle_tree.decommit_from(&decommitment_positions, vec![], |leaves| {
                // The leaves rebuilt from compact columns holding only their rows.
                let len = leaves.len().next_power_of_two().max(2);
                let compact: [Col<B, BaseField>; 4] = std::array::from_fn(|c| {
                    (0..len << LOG_PACKED_LEAF_SIZE)
                        .map(|i| {
                            let leaf = leaves[(i >> LOG_PACKED_LEAF_SIZE).min(leaves.len() - 1)];
                            values[&((leaf << LOG_PACKED_LEAF_SIZE) + (i & ((1 << LOG_PACKED_LEAF_SIZE) - 1)))][c]
                        })
                        .collect()
                });
                let compact = compact.iter().collect_vec();
                let positions = (0..leaves.len()).collect_vec();
                B::packed_leaf_hashes_at(&compact, len.ilog2(), &positions)
            });
        let commitment = self.merkle_tree.root();
        ExtendedFriLayerProof {
            proof: FriLayerProof {
                fri_witness: column_witness,
                decommitment: decommitment.decommitment,
                commitment,
            },
            aux: FriLayerProofAux { all_values: vec![value_map], decommitment: decommitment.aux },
        }
    }

    fn decommit(self, queries: &Queries, fold_step: u32) -> ExtendedFriLayerProof<H> {
        if self.striped.is_some() {
            return self.decommit_striped(queries, fold_step);
        }
        let column = self.column.unwrap();
        assert_eq!(queries.log_domain_size, column.domain.log_size());

        let (decommitment_positions, column_witness, value_map) =
            compute_decommitment_positions_and_witness_evals(
                column,
                &queries.positions,
                fold_step,
            );

        let decommitment_positions = if self.pack_leaves {
            decommitment_positions
                .iter()
                .map(|position| position >> LOG_PACKED_LEAF_SIZE)
                .dedup()
                .collect_vec()
        } else {
            decommitment_positions
        };
        // The returned opened values are not used. Retained coordinate columns let both
        // packed and unpacked trees recompute omitted lower layers at decommitment.
        // TODO(Leo): consider adding a method to merkle prover to decommit only the auth paths.
        let columns: Vec<&Col<B, BaseField>> = column.values.columns.iter().collect();
        let (_, decommitment) = if self.pack_leaves {
            self.merkle_tree.decommit_packed(&decommitment_positions, columns)
        } else {
            self.merkle_tree.decommit(&decommitment_positions, columns)
        };
        let commitment = self.merkle_tree.root();

        ExtendedFriLayerProof {
            proof: FriLayerProof {
                fri_witness: column_witness,
                decommitment: decommitment.decommitment,
                commitment,
            },
            aux: FriLayerProofAux { all_values: vec![value_map], decommitment: decommitment.aux },
        }
    }
}

/// A FRI layer comprises of a merkle tree that commits to evaluations of a polynomial.
///
/// The polynomial evaluations are viewed as evaluation of a polynomial on multiple distinct cosets
/// of size two. Each leaf of the merkle tree commits to a single coset evaluation.
// TODO(andrew): Support different step sizes and update docs.
// TODO(andrew): The docs are wrong. Each leaf of the merkle tree commits to a single
// QM31 value. This is inefficient and should be changed.
struct FriInnerLayerProver<B: FriOps + MerkleOpsLifted<H>, H: MerkleHasherLifted> {
    evaluation: LineEvaluation<B>,
    merkle_tree: MerkleProverLifted<B, H>,
    fold_step: u32,
    pack_leaves: bool,
}

impl<B: FriOps + MerkleOpsLifted<H>, H: MerkleHasherLifted> FriInnerLayerProver<B, H> {
    fn new(evaluation: LineEvaluation<B>, fold_step: u32) -> Self {
        let pack_leaves = evaluation.values.len().ilog2() >= LOG_PACKED_LEAF_SIZE && fold_step > 1;
        let log_rows_per_leaf = if pack_leaves { LOG_PACKED_LEAF_SIZE } else { 0 };
        let log_column_size = evaluation.values.len().ilog2() - log_rows_per_leaf;
        let merkle_tree = MerkleProverLifted::commit(
            evaluation.values.columns.iter().collect_vec(),
            log_column_size,
            log_rows_per_leaf,
        );

        FriInnerLayerProver { evaluation, merkle_tree, fold_step, pack_leaves }
    }

    fn decommit(self, queries: &Queries) -> ExtendedFriLayerProof<H> {
        let (decommitment_positions, fri_witness, value_map) =
            compute_decommitment_positions_and_witness_evals(
                &self.evaluation.values,
                queries,
                self.fold_step,
            );

        let decommitment_positions = if self.pack_leaves {
            decommitment_positions
                .iter()
                .map(|position| position >> LOG_PACKED_LEAF_SIZE)
                .dedup()
                .collect_vec()
        } else {
            decommitment_positions
        };
        // Retained coordinate columns supply omitted hashes for either leaf layout.
        let columns: Vec<&Col<B, BaseField>> = self.evaluation.values.columns.iter().collect();
        let (_, decommitment) = if self.pack_leaves {
            self.merkle_tree.decommit_packed(&decommitment_positions, columns)
        } else {
            self.merkle_tree.decommit(&decommitment_positions, columns)
        };
        let commitment = self.merkle_tree.root();

        ExtendedFriLayerProof {
            proof: FriLayerProof {
                fri_witness,
                decommitment: decommitment.decommitment,
                commitment,
            },
            aux: FriLayerProofAux { all_values: vec![value_map], decommitment: decommitment.aux },
        }
    }
}

/// SCREEN (w5 Q1603): [`compute_decommitment_positions_and_witness_evals`] with the column read
/// through `at`.
fn decommitment_positions_and_witness_evals_at(
    at: impl Fn(usize) -> QM31,
    query_positions: &[usize],
    fold_step: u32,
) -> (Vec<usize>, Vec<QM31>, HashMap<usize, QM31>) {
    let mut decommitment_positions = Vec::new();
    let mut witness_evals = Vec::new();
    let mut value_map = HashMap::new();
    for subset_queries in query_positions.chunk_by(|a, b| a >> fold_step == b >> fold_step) {
        let subset_start = (subset_queries[0] >> fold_step) << fold_step;
        let mut subset_queries_iter = subset_queries.iter().peekable();
        for position in subset_start..subset_start + (1 << fold_step) {
            decommitment_positions.push(position);
            let eval = at(position);
            value_map.insert(position, eval);
            if subset_queries_iter.next_if_eq(&&position).is_none() {
                witness_evals.push(eval);
            }
        }
    }
    (decommitment_positions, witness_evals, value_map)
}

/// Returns a column's merkle tree decommitment positions and the evals the verifier can't
/// deduce from previous computations but requires for decommitment and folding.
///
/// Returns a map from leaf index to its value.
fn compute_decommitment_positions_and_witness_evals(
    column: &SecureColumnByCoords<impl PolyOps>,
    query_positions: &[usize],
    fold_step: u32,
) -> (Vec<usize>, Vec<QM31>, HashMap<usize, QM31>) {
    let mut decommitment_positions = Vec::new();
    let mut witness_evals = Vec::new();
    let mut value_map = HashMap::new();

    // Group queries by the folding coset they reside in.
    for subset_queries in query_positions.chunk_by(|a, b| a >> fold_step == b >> fold_step) {
        let subset_start = (subset_queries[0] >> fold_step) << fold_step;
        let subset_decommitment_positions = subset_start..subset_start + (1 << fold_step);
        let mut subset_queries_iter = subset_queries.iter().peekable();

        for position in subset_decommitment_positions {
            // Add decommitment position.
            decommitment_positions.push(position);

            let eval = column.at(position);
            value_map.insert(position, eval);

            // Only add evals the verifier can't calculate.
            if subset_queries_iter.next_if_eq(&&position).is_none() {
                witness_evals.push(eval);
            }
        }
    }

    (decommitment_positions, witness_evals, value_map)
}

#[cfg(test)]
mod tests {

    use num_traits::One;

    use crate::core::circle::{CirclePointIndex, Coset};
    use crate::core::fields::m31::BaseField;
    use crate::core::fields::qm31::{SECURE_EXTENSION_DEGREE, SecureField};
    use crate::core::fri::FriConfig;
    use crate::core::poly::circle::CircleDomain;
    use crate::core::queries::Queries;
    use crate::core::test_utils::test_channel;
    use crate::core::vcs_lifted::blake2_merkle::Blake2sMerkleChannel;
    use crate::core::vcs_lifted::verifier::PACKED_LEAF_SIZE;
    use crate::prover::backend::cpu::CpuCirclePoly;
    use crate::prover::backend::simd::SimdBackend;
    use crate::prover::backend::{Col, Column, CpuBackend};
    use crate::prover::poly::BitReversedOrder;
    use crate::prover::poly::circle::{PolyOps, SecureEvaluation};
    use crate::prover::secure_column::SecureColumnByCoords;
    use crate::prover::vcs_lifted::ops::PackLeavesOps;

    /// Default blowup factor used for tests.
    const LOG_BLOWUP_FACTOR: u32 = 2;

    type FriProver<'a> = super::FriProver<'a, CpuBackend, Blake2sMerkleChannel>;

    #[test]
    #[should_panic = "invalid degree"]
    fn committing_high_degree_polynomial_fails() {
        const LOG_EXPECTED_BLOWUP_FACTOR: u32 = LOG_BLOWUP_FACTOR;
        const LOG_INVALID_BLOWUP_FACTOR: u32 = LOG_BLOWUP_FACTOR - 1;
        let config = FriConfig::new(0, 2, LOG_EXPECTED_BLOWUP_FACTOR, 3, 1);
        let column = polynomial_evaluation(6, LOG_INVALID_BLOWUP_FACTOR);
        let twiddles = CpuBackend::precompute_twiddles(column.domain.half_coset);

        FriProver::commit(&mut test_channel(), config, &column, &twiddles);
    }

    #[test]
    #[should_panic = "not canonic"]
    fn committing_column_from_invalid_domain_fails() {
        let invalid_domain = CircleDomain::new(Coset::new(CirclePointIndex::generator(), 3));
        assert!(!invalid_domain.is_canonic(), "must be an invalid domain");
        let config = FriConfig::new(0, 2, 2, 3, 1);
        let column = SecureEvaluation::new(
            invalid_domain,
            [SecureField::one(); 1 << 4].into_iter().collect(),
        );
        let twiddles = CpuBackend::precompute_twiddles(column.domain.half_coset);

        FriProver::commit(&mut test_channel(), config, &column, &twiddles);
    }

    /// Returns an evaluation of a random polynomial with degree `2^log_degree`.
    ///
    /// The evaluation domain size is `2^(log_degree + log_blowup_factor)`.
    fn polynomial_evaluation(
        log_degree: u32,
        log_blowup_factor: u32,
    ) -> SecureEvaluation<CpuBackend, BitReversedOrder> {
        let poly = CpuCirclePoly::new(vec![BaseField::one(); 1 << log_degree]);
        let coset = Coset::half_odds(log_degree + log_blowup_factor - 1);
        let domain = CircleDomain::new(coset);
        let values = poly.evaluate(domain);
        SecureEvaluation::new(domain, values.into_iter().map(SecureField::from).collect())
    }

    #[test]
    fn test_fri_commit_decommit_with_jumps() {
        let config = FriConfig::new(0, 2, LOG_BLOWUP_FACTOR, 3, 2);
        let column = polynomial_evaluation(6, LOG_BLOWUP_FACTOR);
        let twiddles = CpuBackend::precompute_twiddles(column.domain.half_coset);

        let prover = FriProver::commit(&mut test_channel(), config, &column, &twiddles);
        let queries = Queries::from_positions(vec![0, 3], 6 + LOG_BLOWUP_FACTOR);
        prover.decommit_on_queries(&queries);
    }

    #[test]
    fn test_fri_commit_decommit_with_packed_leaves() {
        for fold_step in 2..=4 {
            let config = FriConfig::new(0, 2, LOG_BLOWUP_FACTOR, 3, fold_step);
            let column = polynomial_evaluation(8, LOG_BLOWUP_FACTOR);
            let twiddles = CpuBackend::precompute_twiddles(column.domain.half_coset);

            let prover = FriProver::commit(&mut test_channel(), config, &column, &twiddles);
            let queries = Queries::from_positions(vec![1, 6, 11], 8 + LOG_BLOWUP_FACTOR);
            prover.decommit_on_queries(&queries);
        }
    }

    #[test]
    fn test_fri_commit_decommit_with_packed_leaves_simd() {
        for fold_step in 2..=4 {
            let config = FriConfig::new(0, 2, LOG_BLOWUP_FACTOR, 3, fold_step);
            let cpu_eval = polynomial_evaluation(8, LOG_BLOWUP_FACTOR);
            let column = SecureEvaluation::new(
                cpu_eval.domain,
                cpu_eval.values.to_vec().into_iter().collect(),
            );
            let twiddles = SimdBackend::precompute_twiddles(column.domain.half_coset);
            let prover = super::FriProver::<'_, SimdBackend, Blake2sMerkleChannel>::commit(
                &mut test_channel(),
                config,
                &column,
                &twiddles,
            );
            let queries = Queries::from_positions(vec![1, 6, 11], 8 + LOG_BLOWUP_FACTOR);
            prover.decommit_on_queries(&queries);
        }
    }

    #[test]
    fn test_fri_commit_decommit_keccak256_cpu() {
        use crate::core::channel::Keccak256Channel;
        use crate::core::vcs_lifted::keccak256_merkle::Keccak256MerkleChannel;

        let config = FriConfig::new(0, 2, LOG_BLOWUP_FACTOR, 3, 2);
        let column = polynomial_evaluation(6, LOG_BLOWUP_FACTOR);
        let twiddles = CpuBackend::precompute_twiddles(column.domain.half_coset);

        let prover = super::FriProver::<'_, CpuBackend, Keccak256MerkleChannel>::commit(
            &mut Keccak256Channel::default(),
            config,
            &column,
            &twiddles,
        );
        let queries = Queries::from_positions(vec![0, 3], 6 + LOG_BLOWUP_FACTOR);
        prover.decommit_on_queries(&queries);
    }

    #[test]
    fn test_fri_commit_decommit_keccak256_simd() {
        use crate::core::channel::Keccak256Channel;
        use crate::core::vcs_lifted::keccak256_merkle::Keccak256MerkleChannel;

        let config = FriConfig::new(0, 2, LOG_BLOWUP_FACTOR, 3, 2);
        let cpu_eval = polynomial_evaluation(8, LOG_BLOWUP_FACTOR);
        let column =
            SecureEvaluation::new(cpu_eval.domain, cpu_eval.values.to_vec().into_iter().collect());
        let twiddles = SimdBackend::precompute_twiddles(column.domain.half_coset);
        let prover = super::FriProver::<'_, SimdBackend, Keccak256MerkleChannel>::commit(
            &mut Keccak256Channel::default(),
            config,
            &column,
            &twiddles,
        );
        let queries = Queries::from_positions(vec![1, 6, 11], 8 + LOG_BLOWUP_FACTOR);
        prover.decommit_on_queries(&queries);
    }

    #[test]
    fn test_pack_leaves_input_simd_matches_cpu() {
        for log_size in 2..8 {
            let values = (0..1 << log_size).map(|i| {
                SecureField::from_m31_array(core::array::from_fn(|coord| {
                    BaseField::from_u32_unchecked((i * SECURE_EXTENSION_DEGREE + coord) as u32)
                }))
            });
            let values = SecureColumnByCoords::<SimdBackend>::from_iter(values);

            let cpu_values = values.to_cpu();
            let cpu_cols: [&Col<CpuBackend, BaseField>; SECURE_EXTENSION_DEGREE] =
                core::array::from_fn(|i| &cpu_values.columns[i]);
            let generic: [Col<CpuBackend, BaseField>; SECURE_EXTENSION_DEGREE * PACKED_LEAF_SIZE] =
                <CpuBackend as PackLeavesOps>::pack_leaves_input(&cpu_cols);
            let simd_cols: [&Col<SimdBackend, BaseField>; SECURE_EXTENSION_DEGREE] =
                core::array::from_fn(|i| &values.columns[i]);
            let simd: [Col<SimdBackend, BaseField>; SECURE_EXTENSION_DEGREE * PACKED_LEAF_SIZE] =
                <SimdBackend as PackLeavesOps>::pack_leaves_input(&simd_cols);

            for col in 0..SECURE_EXTENSION_DEGREE * PACKED_LEAF_SIZE {
                assert_eq!(
                    generic[col].to_cpu(),
                    simd[col].to_cpu(),
                    "mismatch at log_size={log_size}, col={col}"
                );
            }
        }
    }
}
