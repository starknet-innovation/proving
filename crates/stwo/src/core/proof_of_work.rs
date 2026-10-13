use serde::{Deserialize, Serialize};

use crate::core::channel::Channel;

pub trait GrindOps<C: Channel> {
    /// Searches for a nonce s.t. mixing it to the channel makes the digest have `pow_bits` leading
    /// zero bits.
    fn grind(channel: &C, pow_bits: u32) -> u64;
}

/// Proof of work before two more challenges than the queries': the out-of-domain point (DEEP) and
/// the random coefficient that batches the quotients for FRI. Each costs a prover `2^bits` hashes
/// per attempt at that challenge, which is what lifts those soundness terms to 100 provable bits
/// at the privacy statement's parameters (ethereum/soundcalc, unique decoding). Zero bits is the
/// protocol without them, unchanged.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Grinding {
    pub deep_bits: u32,
    pub batching_bits: u32,
}

impl Grinding {
    pub const NONE: Self = Self { deep_bits: 0, batching_bits: 0 };
}

/// Mixes into the channel the smallest nonce that gives its digest `bits` leading zero bits. The
/// nonce is a function of the transcript, so prover and verifier derive the same one and no proof
/// carries it; finding it costs about `2^bits` hashes on either side. Nothing when `bits` is zero.
pub fn grind_smallest<C: Channel>(channel: &mut C, bits: u32) {
    if bits == 0 {
        return;
    }
    let nonce = (0u64..).find(|&nonce| channel.verify_pow_nonce(bits, nonce)).unwrap();
    channel.mix_u64(nonce);
}
