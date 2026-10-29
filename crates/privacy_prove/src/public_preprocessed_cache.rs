//! A checked cache of the fixed recursive circuit's public preprocessing.
//!
//! The boundary hashes are public, input-independent values regenerated from the ordinary
//! commitment builder. A hit requires the complete ordered source contents to match, not just
//! the circuit's dimensions or its advertised root. Other sources use the ordinary builder.

mod data;

mod integration_tests;
mod tests;

use rayon::prelude::*;
use stwo::core::fields::m31::BaseField;
use stwo::core::poly::circle::CanonicCoset;
use stwo::core::vcs::blake2_hash::{Blake2sHash, Blake2sHasher};
use stwo::core::vcs_lifted::blake2_merkle::{Blake2sM31MerkleChannel, Blake2sMerkleHasher};
use stwo::prover::backend::simd::SimdBackend;
use stwo::prover::mempool::BaseColumnPool;
use stwo::prover::poly::BitReversedOrder;
use stwo::prover::poly::circle::{CircleEvaluation, PolyOps};
use stwo::prover::poly::twiddles::TwiddleTree;
use stwo::prover::vcs_lifted::ops::MerkleOpsLifted;
use stwo::prover::vcs_lifted::prover::MerkleProverLifted;
use stwo::prover::{CommitmentTreeProver, Poly, SharedTraceSource, TraceSource};

type CachedTree = CommitmentTreeProver<SimdBackend, Blake2sM31MerkleChannel>;

/// Reuses public Merkle boundary nodes only after checking all of their source data.
///
/// The large columns have the same source-backed representation as `new_with_sources`.
/// Small columns retain their full extension and coefficients, as that constructor does.
pub(crate) fn cached_tree(
    sources: Vec<SharedTraceSource>,
    ids: &[String],
    log_blowup: u32,
    lifting: u32,
    twiddles: &TwiddleTree<SimdBackend>,
    pool: &BaseColumnPool<SimdBackend>,
) -> Option<CachedTree> {
    if !matches_metadata(&sources, ids, log_blowup, lifting) {
        return None;
    }
    if !sources
        .par_iter()
        .zip(data::SOURCE_HASHES.par_iter())
        .all(|(source, expected)| fingerprint(source.as_ref()).0 == *expected)
    {
        return None;
    }

    let commitment = cached_commitment()?;
    let polynomials = sources
        .into_iter()
        .map(|source| {
            let log_size = source.log_size();
            let domain = CanonicCoset::new(log_size + log_blowup).circle_domain();
            if log_size >= 10 {
                return Poly::from_source(domain, source);
            }
            let mut values = vec![BaseField::from_u32_unchecked(0); 1 << log_size];
            source.write_values(&mut values);
            let evaluation: CircleEvaluation<SimdBackend, BaseField, BitReversedOrder> =
                CircleEvaluation::new(
                    CanonicCoset::new(log_size).circle_domain(),
                    values.into_iter().collect(),
                );
            let coeffs = evaluation.interpolate_with_twiddles(twiddles);
            let buffer = pool.take_or_alloc(domain.log_size());
            let evals = SimdBackend::evaluate_into(&coeffs, domain, twiddles, buffer);
            Poly::new(Some(coeffs), evals)
        })
        .collect();
    Some(CommitmentTreeProver { polynomials, commitment })
}

fn matches_metadata(
    sources: &[SharedTraceSource],
    ids: &[String],
    log_blowup: u32,
    lifting: u32,
) -> bool {
    log_blowup == data::LOG_BLOWUP
        && lifting == data::LIFTING
        && sources.len() == data::LOG_SIZES.len()
        && ids.len() == data::IDS.len()
        && ids.iter().zip(data::IDS).all(|(actual, expected)| actual == expected)
        && sources
            .iter()
            .zip(data::LOG_SIZES)
            .all(|(source, expected)| source.log_size() == expected)
}

/// The exporter's canonical digest: log size as a little-endian u32, followed by every trace
/// value's raw M31 word as a little-endian u32, in source order. Bounded scratch avoids
/// materializing the large columns just to recognize them.
fn fingerprint(source: &dyn TraceSource) -> Blake2sHash {
    let mut state = Blake2sHasher::new();
    state.update(&source.log_size().to_le_bytes());
    let mut words = vec![BaseField::from_u32_unchecked(0); 1 << 12];
    let mut bytes = vec![0_u8; 4 << 12];
    let size = 1usize << source.log_size();
    for first in (0..size).step_by(words.len()) {
        let len = words.len().min(size - first);
        source.write_rows(first, &mut words[..len]);
        for (dst, word) in bytes[..4 * len].chunks_exact_mut(4).zip(&words[..len]) {
            dst.copy_from_slice(&word.0.to_le_bytes());
        }
        state.update(&bytes[..4 * len]);
    }
    state.finalize()
}

const BASE85_ALPHABET: &[u8; 85] =
    b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz!#$%&()*+-;<=>?@^_`{|}~";
const BASE85_DIGITS: [u8; 256] = {
    let mut digits = [u8::MAX; 256];
    let mut i = 0;
    while i < BASE85_ALPHABET.len() {
        digits[BASE85_ALPHABET[i] as usize] = i as u8;
        i += 1;
    }
    digits
};

/// Five positional base85 digits encode one little-endian u32 hash word.
fn decode_word(encoded: &[u8]) -> Option<[u8; 4]> {
    if encoded.len() != 5 {
        return None;
    }
    let mut value = 0_u64;
    for &byte in encoded {
        let digit = BASE85_DIGITS[usize::from(byte)];
        if digit == u8::MAX {
            return None;
        }
        value = value.checked_mul(85)?.checked_add(u64::from(digit))?;
    }
    Some(u32::try_from(value).ok()?.to_le_bytes())
}

fn decode_boundary(encoded: &[u8]) -> Option<Vec<Blake2sHash>> {
    let expected_nodes = 1usize.checked_shl(data::BOUNDARY_DEPTH)?;
    let expected_chars = expected_nodes.checked_mul(40)?;
    if encoded.len() != expected_chars {
        return None;
    }
    encoded
        .chunks_exact(40)
        .map(|encoded_hash| {
            let mut hash = [0_u8; 32];
            for (dst, word) in hash.chunks_exact_mut(4).zip(encoded_hash.chunks_exact(5)) {
                dst.copy_from_slice(&decode_word(word)?);
            }
            Some(Blake2sHash(hash))
        })
        .collect()
}

fn cached_commitment() -> Option<MerkleProverLifted<SimdBackend, Blake2sMerkleHasher>> {
    let mut layers = vec![decode_boundary(data::BOUNDARY_BASE85.as_bytes())?];
    for _ in 0..data::BOUNDARY_DEPTH {
        layers.push(<SimdBackend as MerkleOpsLifted<Blake2sMerkleHasher>>::build_next_layer(
            layers.last().unwrap(),
        ));
    }
    layers.reverse();
    if layers[0][0].0 != data::ROOT {
        return None;
    }
    // The ordinary decommitment reconstructs the missing lower layers from source rows.
    layers.resize_with(data::LIFTING as usize + 1, Vec::new);
    Some(MerkleProverLifted { layers })
}
