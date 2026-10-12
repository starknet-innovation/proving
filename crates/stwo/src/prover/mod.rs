use thiserror::Error;
use tracing::{Level, info, instrument, span};

use crate::core::channel::{Channel, MerkleChannel};
use crate::core::circle::CirclePoint;
use crate::core::fields::qm31::{SECURE_EXTENSION_DEGREE, SecureField};
use crate::core::pcs::utils::InvalidLiftingLogSizeError;
use crate::core::proof::{ExtendedStarkProof, StarkProof};
use crate::core::verifier::PREPROCESSED_TRACE_IDX;
use crate::prover::backend::BackendForChannel;

mod air;
pub use air::component_prover::{
    ComponentProver, ComponentProvers, Poly, SharedTraceSource, Trace, TraceSource, source_column,
};
pub use air::{AccumulationOps, ColumnAccumulator, DomainEvaluationAccumulator, EvaluationMode};
pub mod pcs;
pub use pcs::quotient_ops::QuotientOps;
pub use pcs::{CommitmentSchemeProver, CommitmentTreeProver, TreeBuilder};
pub mod backend;
pub mod channel;
pub mod fri;
pub mod line;
pub mod lookups;
pub mod mempool;
pub mod poly;
pub mod secure_column;
pub mod vcs;
pub mod vcs_lifted;

/// The screened configuration of the m11 knobs (frozen; any other name takes its default).
pub fn m11_env(name: &'static str, default: usize) -> usize {
    match name {
        "A8N_FOR" => 1,
        "A8N_PLAN1" => 1,
        "A8N_QMW" => 1,
        "A8N_QSER" => 1,
        "A8N_REGROW1" => 1,
        "A8N_REGROW1_MIB" => 64,
        "F2_STILE" => 2,
        "F2_SUBT" => 2,
        "F2_SUPER" => 4,
        "F2_SUPERMIB" => 8,
        "M11_PACK" => 2,
        "M11_PACK_LEGS" => 1,
        "M11_REGROW" => 4,
        "M11_SUB" => 3,
        "M11_SUBMIN" => 2,
        "P12_ADDINTO" => 1,
        "P12_B2S" => 19,
        "P12_EAGER" => 1,
        "P12_QIN" => 1,
        "R77_DIRECT" => 1,
        "R77_ISEQI" => 1,
        "R77_OODSPAR" => 1,
        "W5_B2SPOOL" => 1,
        "W5_CSUB" => 2,
        "W5_FOLDTLS" => 1,
        "W5_PACKPAR" => 4,
        "W5_QPOLY" => 1,
        "W5_RFOLD" => 1,
        "W5_SHIFTWIN" => 1,
        _ => default,
    }
}

/// SCREEN (w5 Q1612): bytes held by the committed trees at the start of `prove_ex` (see there).
pub static W5_HELD: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
/// SCREEN (f2 Q1674): the index of the tree the commitment scheme is committing (`usize::MAX`
/// outside `commit`), for per-tree commit splits (`F2_SUBT`).
pub static F2_TREE: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(usize::MAX);

/// Returns the allocator's free pages to the system between large phases.
pub fn trim_heap() {
    crate::prover::backend::simd::column::trim_heap();
}

pub fn prove<B: BackendForChannel<MC>, MC: MerkleChannel>(
    components: &[&dyn ComponentProver<B>],
    channel: &mut MC::C,
    commitment_scheme: CommitmentSchemeProver<'_, B, MC>,
) -> Result<StarkProof<MC::H>, ProvingError> {
    Ok(prove_ex(components, channel, commitment_scheme, false)?.proof)
}

#[instrument(skip_all)]
pub fn prove_ex<B: BackendForChannel<MC>, MC: MerkleChannel>(
    components: &[&dyn ComponentProver<B>],
    channel: &mut MC::C,
    mut commitment_scheme: CommitmentSchemeProver<'_, B, MC>,
    include_all_preprocessed_columns: bool,
) -> Result<ExtendedStarkProof<MC::H>, ProvingError> {
    let n_preprocessed_columns = commitment_scheme.trees[PREPROCESSED_TRACE_IDX].polynomials.len();
    let component_provers =
        ComponentProvers { components: components.to_vec(), n_preprocessed_columns };
    commitment_scheme.striped_to_coefficients();
    // SCREEN (w5 Q1612): the bytes the committed trees hold (coefficients and values; sources not
    // counted), the deterministic stand-in for the resident set that `W5_COMPEST` budgets with.
    use crate::prover::backend::Column;
    let held: usize = commitment_scheme
        .trees
        .iter()
        .flat_map(|tree| tree.polynomials.iter())
        .map(|poly| 4 * (poly.coeffs.as_ref().map_or(0, |c| c.coeffs.len()) + poly.evals.values.len()))
        .sum();
    W5_HELD.store(held, std::sync::atomic::Ordering::Relaxed);
    if false {
        eprintln!("Z1I {} pool idle {:.1} MiB", "composition", commitment_scheme.base_column_pool.idle_bytes() as f64 / 1048576.0);
    }
    if m11_env("Z1_REL", 0) != 0 {
        // Z1_REL (Q1583): idle pool buffers and freed arena pages go back before the window.
        commitment_scheme.base_column_pool.release_all_idle();
        trim_heap();
    }
    let trace = commitment_scheme.trace();

    // Evaluate and commit on composition polynomial.
    let random_coeff = channel.draw_secure_felt();

    let span = span!(Level::INFO, "Composition", class = "Composition").entered();
    let span1 =
        span!(Level::INFO, "Generation", class = "CompositionPolynomialGeneration").entered();

    let composition_poly = component_provers.compute_composition_polynomial(
        random_coeff,
        &trace,
        commitment_scheme.twiddles,
        commitment_scheme.config.fri_config.log_blowup_factor,
    );
    span1.exit();
    crate::prover::backend::simd::circle::w5_clear_fold_buffers();

    // Commit on the Composition Polynomial by splitting its coeffs to two polynomialsof degree
    // half the size of the original polynomial, and commit on each half separately.
    let mut tree_builder = commitment_scheme.tree_builder();
    let (left_comp_poly_half, right_comp_poly_half) = composition_poly.split_at_mid();

    tree_builder.extend_polys(left_comp_poly_half.into_coordinate_polys());
    tree_builder.extend_polys(right_comp_poly_half.into_coordinate_polys());
    tree_builder.commit(channel);
    span.exit();
    // Composition's freed block temporaries stay in glibc's arenas under the samples and the
    // quotients unless they are handed back.
    crate::prover::backend::simd::column::trim_heap();

    // Draw OODS point.
    let oods_point = CirclePoint::<SecureField>::get_random_point(channel);

    // The composition tree was just committed like any other trace tree, so its height is
    // `trace_lifting_log_size` (see `CommitmentSchemeProver::commit`) — the height every tree
    // the queries are answered on was lifted to.
    let lifting_log_size =
        commitment_scheme.trees.last().unwrap().commitment.layers.len() as u32 - 1;
    if include_all_preprocessed_columns {
        // If all the preprocessed columns are included, the lifting log size must be greater than
        // or equal to the preprocessed log size.
        let preprocessed_log_size =
            commitment_scheme.trees[PREPROCESSED_TRACE_IDX].commitment.layers.len() as u32 - 1;
        if lifting_log_size < preprocessed_log_size {
            Err(InvalidLiftingLogSizeError {
                lifting_log_size,
                min_lifting_log_size: preprocessed_log_size,
            })?;
        }
    }
    let max_log_degree_bound =
        lifting_log_size - commitment_scheme.config.fri_config.log_blowup_factor;

    // Get mask sample points relative to oods point.
    let mut sample_points = component_provers.components().mask_points(
        oods_point,
        max_log_degree_bound,
        include_all_preprocessed_columns,
    );

    // Add the composition polynomial mask points.
    sample_points.push(vec![vec![oods_point]; 2 * SECURE_EXTENSION_DEGREE]);

    // Prove the trace and composition OODS values, and retrieve them.
    if false {
        eprintln!("Z1I {} pool idle {:.1} MiB", "prove_values", commitment_scheme.base_column_pool.idle_bytes() as f64 / 1048576.0);
    }
    if m11_env("Z1_REL", 0) != 0 {
        // Z1_REL (Q1583): idle pool buffers and freed arena pages go back before the window.
        commitment_scheme.base_column_pool.release_all_idle();
        trim_heap();
    }
    let commitment_scheme_proof = commitment_scheme.prove_values(sample_points, channel);
    // This leg's striped caches are dead once it has decommitted.
    crate::prover::backend::simd::circle::clear_stripe_caches();
    let proof = StarkProof(commitment_scheme_proof.proof);
    info!(proof_size_estimate = proof.size_estimate());

    // Evaluate composition polynomial at OODS point and check that it matches the trace OODS
    // values. This is a sanity check.
    if proof.extract_composition_oods_eval(oods_point, max_log_degree_bound).unwrap()
        != component_provers.components().eval_composition_polynomial_at_point(
            oods_point,
            &proof.sampled_values,
            random_coeff,
            max_log_degree_bound,
        )
    {
        return Err(ProvingError::ConstraintsNotSatisfied);
    }

    Ok(ExtendedStarkProof { proof, aux: commitment_scheme_proof.aux })
}

#[derive(Clone, Copy, Debug, Error)]
pub enum ProvingError {
    #[error("Constraints not satisfied.")]
    ConstraintsNotSatisfied,
    #[error(transparent)]
    InvalidLiftingLogSize(#[from] crate::core::pcs::utils::InvalidLiftingLogSizeError),
    #[error(transparent)]
    InvalidCanonicCosetLogSize(#[from] crate::core::poly::circle::InvalidCanonicCosetLogSize),
}
