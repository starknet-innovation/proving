//! A checked cache of the Cairo preprocessed tree's public Merkle boundary (graph lead
//! rgr1_c1b229e4, odinfree).
//!
//! The Cairo preprocessed trace is public and fixed (`CAIRO_PROVER_PARAMS.preprocessed_trace`):
//! its commitment is the same for every statement. Hashing its 2^23-leaf extension took the
//! longer branch of the Cairo leg's first join (beside the base-trace writers). This cache keeps
//! the tree's nodes at a shallow depth, regenerated from the ordinary builder, and leaves the
//! layers below empty: the striped decommitment already regrows every queried stripe and rehashes
//! the leaf blocks under the queried nodes (see `MerkleProverLifted::omitted_leaf_positions`),
//! as the frontier's circuit-tree cache does. A hit requires every live column's values to match
//! the exported fingerprint; any mismatch falls back to the ordinary builder. The columns keep
//! exactly what `CommitmentTreeProver::new` keeps for this tree: stripe 0 of every large column,
//! the whole extension and coefficients of every small one.

mod data;

use std::any::{Any, TypeId};
use std::sync::Arc;

use rayon::prelude::*;
use stwo::core::fields::m31::BaseField;
use stwo::core::channel::MerkleChannel;
use stwo::core::poly::circle::CanonicCoset;
use stwo::core::vcs::blake2_hash::{Blake2sHash, Blake2sHasher};
use stwo::core::vcs_lifted::blake2_merkle::{Blake2sM31MerkleChannel, Blake2sMerkleHasher};
use stwo::prover::backend::BackendForChannel;
use stwo::prover::backend::simd::SimdBackend;
use stwo::prover::mempool::BaseColumnPool;
use stwo::prover::poly::BitReversedOrder;
use stwo::prover::poly::circle::{CircleEvaluation, PolyOps};
use stwo::prover::poly::twiddles::TwiddleTree;
use stwo::prover::vcs_lifted::ops::MerkleOpsLifted;
use stwo::prover::vcs_lifted::prover::MerkleProverLifted;
use stwo::prover::{CommitmentTreeProver, Poly};
use stwo_cairo_common::preprocessed_columns::preprocessed_trace::PreProcessedTrace;

use crate::witness::preprocessed_trace::gen_trace;

pub type CairoTree = CommitmentTreeProver<SimdBackend, Blake2sM31MerkleChannel>;
type Evaluation = CircleEvaluation<SimdBackend, BaseField, BitReversedOrder>;

/// [`cached_cairo_tree`] for a prover generic over its Merkle channel: `None` for any channel but
/// the one the boundary was exported with.
pub fn cached_tree_for<MC: MerkleChannel + 'static>(
    trace: Arc<PreProcessedTrace>,
    log_blowup: u32,
    lifting: u32,
    twiddles: &TwiddleTree<SimdBackend>,
    pool: &BaseColumnPool<SimdBackend>,
) -> Option<CommitmentTreeProver<SimdBackend, MC>>
where
    SimdBackend: BackendForChannel<MC>,
{
    if TypeId::of::<MC>() != TypeId::of::<Blake2sM31MerkleChannel>() {
        return None;
    }
    let tree: Box<dyn Any> = Box::new(cached_cairo_tree(trace, log_blowup, lifting, twiddles, pool)?);
    tree.downcast::<CommitmentTreeProver<SimdBackend, MC>>().ok().map(|tree| *tree)
}

/// The live columns, checked against the export, and the cached nodes, decoded: prepared by
/// [`warm`] beside the twiddles (whose setup leaves workers idle), taken by the Cairo leg.
struct Warm {
    trace: Arc<PreProcessedTrace>,
    evals: Vec<Evaluation>,
    commitment: MerkleProverLifted<SimdBackend, Blake2sMerkleHasher>,
}

static WARM: std::sync::Mutex<Option<Warm>> = std::sync::Mutex::new(None);

/// Checks the live columns against the export and decodes the cached nodes ahead of the Cairo
/// leg, which then only interpolates and extends stripe 0 (see [`cached_cairo_tree`]). Call it
/// after the Pedersen table is warm (`warm_pedersen_pp_trace`): `gen_trace` reads it.
pub fn warm(trace: Arc<PreProcessedTrace>) {
    let checked = checked(trace);
    if let (Some(warm), Ok(mut slot)) = (checked, WARM.lock()) {
        *slot = Some(warm);
    }
}

fn checked(trace: Arc<PreProcessedTrace>) -> Option<Warm> {
    let evals = gen_trace(trace.clone());
    if evals.len() != data::LOG_SIZES.len()
        || evals.iter().zip(data::LOG_SIZES).any(|(e, &s)| e.domain.log_size() != s)
    {
        return None;
    }
    let (fingerprint, commitment) = rayon::join(|| fingerprint(&evals), cached_commitment);
    if fingerprint != data::FINGERPRINT {
        return None;
    }
    Some(Warm { trace, evals, commitment: commitment? })
}

/// The Cairo preprocessed tree from the checked cache, or `None` (the caller then builds it).
pub fn cached_cairo_tree(
    trace: Arc<PreProcessedTrace>,
    log_blowup: u32,
    lifting: u32,
    twiddles: &TwiddleTree<SimdBackend>,
    pool: &BaseColumnPool<SimdBackend>,
) -> Option<CairoTree> {
    if log_blowup != data::LOG_BLOWUP || lifting != data::LIFTING {
        return None;
    }
    // Only columns warmed from this very trace are taken.
    let warmed = WARM.lock().ok().and_then(|mut slot| slot.take());
    let Warm { evals, commitment, .. } = match warmed {
        Some(warm) if Arc::ptr_eq(&warm.trace, &trace) => warm,
        _ => checked(trace)?,
    };
    let polys = SimdBackend::interpolate_columns(evals, twiddles);
    let polynomials = polys
        .into_par_iter()
        .map(|poly| {
            let log_size = poly.log_size();
            let domain = CanonicCoset::new(log_size + log_blowup).circle_domain();
            if log_size >= 10 {
                // A prefix column of a striped tree: stripe 0 only, no coefficients.
                let mut values = pool.take_or_alloc(log_size);
                SimdBackend::evaluate_stripe_into(&poly, domain, twiddles, 0, &mut values);
                let mut evals = CircleEvaluation::new(
                    CanonicCoset::new(1).circle_domain(),
                    stwo::prover::backend::Column::zeros(2),
                );
                evals.values = values;
                evals.domain = domain;
                Poly::new(None, evals)
            } else {
                let buffer = pool.take_or_alloc(domain.log_size());
                let evals = SimdBackend::evaluate_into(&poly, domain, twiddles, buffer);
                Poly::new(Some(poly), evals)
            }
        })
        .collect();
    Some(CommitmentTreeProver { polynomials, commitment })
}

/// One digest of every column's log size and a sample of its raw M31 words (little-endian, in the
/// stored bit-reversed order): the first 2^10 words and every 2^8-th word after them. The columns
/// are generated from the same code the export ran, and a wrong boundary cannot pass: the proof
/// would commit to another root, which the verifier and the pinned statement reject. The check
/// only has to notice another variant of the trace, and hashing every word held the setup up.
pub fn fingerprint(evals: &[Evaluation]) -> [u8; 32] {
    let digests: Vec<Blake2sHash> = evals
        .par_iter()
        .map(|e| {
            let mut state = Blake2sHasher::new();
            state.update(&e.domain.log_size().to_le_bytes());
            let values = e.values.as_slice();
            let head = values.len().min(1 << 10);
            for value in values[..head].iter().chain(values[head..].iter().step_by(1 << 8)) {
                state.update(&value.0.to_le_bytes());
            }
            state.finalize()
        })
        .collect();
    let mut state = Blake2sHasher::new();
    for d in &digests {
        state.update(&d.0);
    }
    state.finalize().0
}

const BASE85_ALPHABET: &[u8; 85] =
    b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz!#$%&()*+-;<=>?@^_`{|}~";

/// Each byte's base85 digit, or 255 for a byte outside the alphabet.
const BASE85_DIGITS: [u8; 256] = {
    let mut digits = [255_u8; 256];
    let mut digit = 0;
    while digit < 85 {
        digits[BASE85_ALPHABET[digit] as usize] = digit as u8;
        digit += 1;
    }
    digits
};

fn decode_word(encoded: &[u8]) -> Option<[u8; 4]> {
    let mut value = 0_u64;
    for &byte in encoded {
        let digit = BASE85_DIGITS[byte as usize];
        if digit == 255 {
            return None;
        }
        value = value * 85 + digit as u64;
    }
    Some(u32::try_from(value).ok()?.to_le_bytes())
}

fn cached_commitment() -> Option<MerkleProverLifted<SimdBackend, Blake2sMerkleHasher>> {
    let encoded = data::BOUNDARY_BASE85.as_bytes();
    if encoded.len() != (40usize << data::BOUNDARY_DEPTH) {
        return None;
    }
    let boundary = encoded
        .par_chunks_exact(40)
        .map(|chunk| {
            let mut hash = [0_u8; 32];
            for (dst, word) in hash.chunks_exact_mut(4).zip(chunk.chunks_exact(5)) {
                dst.copy_from_slice(&decode_word(word)?);
            }
            Some(Blake2sHash(hash))
        })
        .collect::<Option<Vec<_>>>()?;
    let mut layers = vec![boundary];
    for _ in 0..data::BOUNDARY_DEPTH {
        layers.push(<SimdBackend as MerkleOpsLifted<Blake2sMerkleHasher>>::build_next_layer(
            layers.last().unwrap(),
        ));
    }
    layers.reverse();
    if layers[0][0].0 != data::ROOT {
        return None;
    }
    layers.resize_with(data::LIFTING as usize + 1, Vec::new);
    Some(MerkleProverLifted { layers })
}
