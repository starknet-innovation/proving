//! The timed proof body of the whole-proof challenge, yours to edit at
//! `crates/privacy_prove/src/pvfast_workload.rs`. The evaluator's worker (`privacy_prove_stdout.rs`)
//! reads the prepared input, checks its header and owns stdout; it calls `setup` or `prove` here.
//! When a patch has no copy of this file, the evaluator builds this default one.
//!
//! `prove` is StarkWare's recursive privacy prover: the body of
//! `privacy_prove::privacy_recursive_prove` after the bootloader run, production parameters,
//! channel salt 0. It must write exactly the bytes the frontier writes for the same statement:
//! the version prefix, then the zstd-compressed recursive proof. Everything in between, the order
//! of the steps and how long each value lives, is open.
use std::io::Write;

use anyhow::{Result, anyhow, ensure};
use circuit_cairo_verifier::verify::{build_and_fill_cairo_verifier_circuit, prepare_cairo_proof_for_circuit_verifier};
use circuit_common::finalize::{add_zk_blinding, pad_context};
use circuit_prover::prover::{prepare_circuit_proof_for_circuit_verifier, prove_circuit_with_precompute};
use circuit_serialize::serialize::CircuitSerialize;
use privacy_circuit_verify::compute_privacy_bootloader_output_hash;
use privacy_circuit_verify::utils::Version;
use privacy_prove::consts::CAIRO_PROVER_PARAMS;
use privacy_prove::prepare_recursive_prover_precomputes;
use starknet_types_core::felt::Felt;
use stwo::core::utils::MaybeOwned;
use stwo_cairo_adapter::ProverInput;
use stwo_cairo_prover::prover::prove_cairo_with_precompute;

/// Setup-only mode: build the reusable precomputes and nothing else, so the evaluator can
/// report setup apart from per-proof work.
pub fn setup() -> Result<()> {
    prepare_recursive_prover_precomputes().map_err(|e| anyhow!("{e}"))?;
    Ok(())
}

/// Prove mode. `input` is the whole prepared input; `input[witness_start..]` is the
/// `stwo_cairo_adapter::ProverInput` JSON. Writes the version-prefixed compressed proof to `out`.
pub fn prove(input: Vec<u8>, witness_start: usize, output_preimage: &[Felt], out: &mut impl Write) -> Result<()> {
    let precomputes = prepare_recursive_prover_precomputes().map_err(|e| anyhow!("{e}"))?;
    let prover_input: ProverInput = serde_json::from_slice(&input[witness_start..])?;
    drop(input);
    let cairo_proof = prove_cairo_with_precompute(
        &precomputes.base_column_pool,
        &precomputes.twiddles,
        precomputes.cairo_preprocessed_trace.clone(),
        MaybeOwned::Borrowed(&precomputes.cairo_preprocessed_tree),
        prover_input,
        CAIRO_PROVER_PARAMS,
    )
    .map_err(|e| anyhow!("cairo proof failed: {e:?}"))?;
    let component_enable_bits = cairo_proof.claim.flatten_claim().component_enable_bits;
    let zk_blinding_seed = cairo_proof.extended_stark_proof.proof.commitments.0[1].0;
    let (proof, aux) = prepare_cairo_proof_for_circuit_verifier(&cairo_proof, &component_enable_bits);
    // Everything the circuit proof needs from the Cairo proof is in `proof`, `aux` and the seed.
    drop(cairo_proof);
    let output_hash = compute_privacy_bootloader_output_hash(output_preimage);
    // The circuit's preprocessing (built on first use, see `LazyPreprocessedCircuit`) is built
    // while the verifier circuit is filled: the two are independent, and the fill is mostly
    // sequential.
    let (mut context, ()) = rayon::join(
        || build_and_fill_cairo_verifier_circuit(&precomputes.cairo_verifier_config, proof, aux, output_hash),
        || {
            let _ = &**precomputes.preprocessed_circuit;
        },
    );
    ensure!(context.is_circuit_valid(), "Circuit is not valid");
    add_zk_blinding(&mut context, zk_blinding_seed, precomputes.circuit_config.config.fri_config.n_queries);
    pad_context(&mut context);
    // A context that handed its values over at padding (see `FinalizedContext::release_on_padding`)
    // is not read again: the circuit proof takes the released values, so free the rest of it first.
    let released = context.values().is_empty();
    let circuit_proof = if released {
        drop(context);
        prove_circuit_with_precompute(
            &precomputes.base_column_pool,
            &precomputes.twiddles,
            &precomputes.preprocessed_circuit,
            MaybeOwned::Borrowed(&precomputes.circuit_preprocessed_tree),
            &[],
            precomputes.circuit_config.config,
        )
    } else {
        prove_circuit_with_precompute(
            &precomputes.base_column_pool,
            &precomputes.twiddles,
            &precomputes.preprocessed_circuit,
            MaybeOwned::Borrowed(&precomputes.circuit_preprocessed_tree),
            context.values(),
            precomputes.circuit_config.config,
        )
    }
    .map_err(|e| anyhow!("circuit proof failed: {e:?}"))?;
    let (proof_qm31s, _public_data) = prepare_circuit_proof_for_circuit_verifier(circuit_proof);
    let mut proof_bytes: Vec<u8> = vec![];
    proof_qm31s.serialize(&mut proof_bytes);
    let compressed = zstd::encode_all(&proof_bytes[..], 3)?;
    out.write_all(&Version::current().serialize())?;
    out.write_all(&compressed)?;
    Ok(())
}
