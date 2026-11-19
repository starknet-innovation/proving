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
use circuit_cairo_verifier::verify::{
    fill_cairo_verifier_context, new_cairo_verifier_context, prepare_cairo_proof_for_circuit_verifier,
};
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
    // Before this: worker start and input read, outside the proof.
    eprintln!("pvfast-phase setup");
    // The proof runs on a worker of rayon's pool, so that its parallel steps start from inside
    // the pool instead of each being injected into it from this thread (the sequential steps,
    // the interaction writers among them, waited on every injection).
    // Freed large blocks wait for reuse while the live memory and the idle blocks take at most
    // 3000 MiB (more only keeps idle blocks resident): phases reuse freed blocks, not fresh pages.
    #[cfg(unix)]
    stwo::prover::backend::simd::column::LargeBlockCache::set_idle_budget(3000 << 20);
    // Stacks for the deep witness writers, so they run here (see `with_witness_stack`).
    use stwo_cairo_prover::witness::witness_stack::{WITNESS_STACK_SIZE, mark_witness_stack_thread};
    let pool = rayon::ThreadPoolBuilder::new()
        .stack_size(WITNESS_STACK_SIZE)
        .start_handler(|_| mark_witness_stack_thread())
        .build()
        .map_err(|e| anyhow!("{e}"))?;
    let (version, compressed) =
        pool.install(|| rayon::scope(|_| prove_to_bytes(input, witness_start, output_preimage)))?;
    out.write_all(&version)?;
    out.write_all(&compressed)?;
    Ok(())
}

/// Pre-faults `ranges` (`MADV_POPULATE_WRITE`, Linux only) in 2 MiB chunks, in writer order, until
/// `done`. Never reads or writes the memory, so the writer may run beside it.
fn populate_ahead(ranges: &[(usize, usize)], done: &std::sync::atomic::AtomicBool) {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use rayon::iter::{IntoParallelIterator, ParallelIterator};
    if !cfg!(target_os = "linux") {
        return;
    }
    const CHUNK: usize = 2 << 20;
    let mut chunks: Vec<(u64, usize, usize)> = vec![];
    for &(start, len) in ranges {
        for offset in (0..len).step_by(CHUNK) {
            let position = (((offset as u128) << 32) / len as u128) as u64;
            chunks.push((position, start + offset, CHUNK.min(len - offset)));
        }
    }
    chunks.sort_unstable_by_key(|chunk| chunk.0);
    let next = AtomicUsize::new(0);
    (0..rayon::current_num_threads()).into_par_iter().for_each(|_| loop {
        let i = next.fetch_add(1, Ordering::Relaxed);
        if i >= chunks.len() || done.load(Ordering::Relaxed) {
            return;
        }
        stwo::prover::backend::simd::column::populate_range(chunks[i].1, chunks[i].2);
    });
}

/// The version prefix and the compressed recursive proof.
fn prove_to_bytes(input: Vec<u8>, witness_start: usize, output_preimage: &[Felt]) -> Result<(Vec<u8>, Vec<u8>)> {
    // The input is parsed beside the precomputes: the two are independent.
    let (precomputes, prover_input) = rayon::join(
        || {
            let precomputes = prepare_recursive_prover_precomputes().map_err(|e| anyhow!("{e}"));
            eprintln!("pvfast-phase setup_wait");
            precomputes
        },
        || {
            let parsed = stwo_cairo_adapter::json::prover_input_from_slice(&input[witness_start..]);
            eprintln!("pvfast-phase setup_parsed");
            parsed
        },
    );
    let precomputes = precomputes?;
    eprintln!("pvfast-phase cairo_leg");
    // Hold heap trims until the fill: the Cairo leg stays below the peak.
    #[cfg(unix)]
    stwo::prover::backend::simd::column::hold_trims(true);
    let prover_input: ProverInput = prover_input?;
    drop(input);
    // The circuit's preprocessing and its committed tree (both built on first use, see
    // `LazyPreprocessedCircuit`) are independent of the Cairo proof and are needed only by the
    // circuit proof: they are built beside the Cairo proof, whose witness windows leave workers
    // idle, and the tree, when it outlasts the Cairo proof, beside the verifier-circuit fill (which
    // is mostly sequential) instead of ahead of it.
    let (filled, ()) = rayon::join(
        || -> Result<_> {
            let cairo_proof = prove_cairo_with_precompute(
                &precomputes.base_column_pool,
                &precomputes.twiddles,
                precomputes.cairo_preprocessed_trace.clone(),
                MaybeOwned::Borrowed(&precomputes.cairo_preprocessed_tree),
                prover_input,
                CAIRO_PROVER_PARAMS,
            );
            let cairo_proof = cairo_proof.map_err(|e| anyhow!("cairo proof failed: {e:?}"))?;
            eprintln!("pvfast-phase cairo_to_circuit");
            let component_enable_bits = cairo_proof.claim.flatten_claim().component_enable_bits;
            let zk_blinding_seed = cairo_proof.extended_stark_proof.proof.commitments.0[1].0;
            let (proof, aux) =
                prepare_cairo_proof_for_circuit_verifier(&cairo_proof, &component_enable_bits);
            // Everything the circuit proof needs from the Cairo proof is in `proof`, `aux` and the
            // seed.
            drop(cairo_proof);
            let output_hash = compute_privacy_bootloader_output_hash(output_preimage);
            eprintln!("pvfast-phase verifier_fill");
            // The circuit's variable count is known from its preprocessing (built beside the Cairo
            // proof): the fill reserves its values once.
            #[cfg(unix)]
            stwo::prover::backend::simd::column::hold_trims(false);
            let (context, ()) = rayon::join(
                || {
                    let mut context = new_cairo_verifier_context(
                        precomputes.preprocessed_circuit.n_vars,
                        &precomputes.preprocessed_circuit.gate_counts,
                    );
                    // The fill is single-threaded: idle workers pre-fault its pages.
                    let ranges = context.reserved_ranges();
                    let done = std::sync::atomic::AtomicBool::new(false);
                    rayon::join(
                        || {
                            let context = fill_cairo_verifier_context(
                                context,
                                &precomputes.cairo_verifier_config,
                                proof,
                                aux,
                                output_hash,
                            );
                            done.store(true, std::sync::atomic::Ordering::Relaxed);
                            context
                        },
                        || populate_ahead(&ranges, &done),
                    )
                    .0
                },
                || {
                    // The single-threaded trim runs beside the pair evaluations.
                    rayon::join(
                        || {
                            #[cfg(unix)]
                            stwo::prover::backend::simd::column::trim_heap_now();
                        },
                        || {
                            // Public columns: compute their `Poly::pair` beside the fill.
                            use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
                            let tree = &**precomputes.circuit_preprocessed_tree;
                            let twiddles: &stwo::prover::poly::twiddles::TwiddleTree<_> =
                                &precomputes.twiddles;
                            tree.polynomials.par_iter().filter(|poly| poly.wants_pair()).for_each(
                                |poly| {
                                    let _ = poly.pair(twiddles);
                                },
                            );
                        },
                    );
                },
            );
            Ok((context, zk_blinding_seed))
        },
        || {
            let _ = &**precomputes.circuit_preprocessed_tree;
        },
    );
    let (mut context, zk_blinding_seed) = filled?;
    eprintln!("pvfast-phase validity_check");
    ensure!(context.is_circuit_valid(), "Circuit is not valid");
    eprintln!("pvfast-phase blinding_padding");
    add_zk_blinding(&mut context, zk_blinding_seed, precomputes.circuit_config.config.fri_config.n_queries);
    pad_context(&mut context);
    eprintln!("pvfast-phase circuit_leg");
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
    eprintln!("pvfast-phase serialize");
    let (proof_qm31s, _public_data) = prepare_circuit_proof_for_circuit_verifier(circuit_proof);
    let mut proof_bytes: Vec<u8> = vec![];
    proof_qm31s.serialize(&mut proof_bytes);
    let compressed = zstd::encode_all(&proof_bytes[..], 3)?;
    Ok((Version::current().serialize().to_vec(), compressed))
}
