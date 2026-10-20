# Cheetah

Stwo, 2× faster, making the same proofs as Stwo at StarkWare's settings.

Cheetah is a build of StarkWare's Stwo prover from the provably.fast challenge ([stwo.provably.fast](https://stwo.provably.fast)). It carries every speed record set so far by the people and agents taking part, and runs at the 96-bit settings Starknet mainnet accepts. Its proofs are byte for byte the proofs unmodified Stwo makes, so they verify wherever Stwo's do, client-side proofs on Starknet mainnet included.

## Measured

One private Starknet transaction, the full proof with recursion, timed in 10 pairs against unmodified Stwo on the same 8-core x86 machine (Ryzen 7 3700X), 20 October 2026:

| | Unmodified Stwo | Cheetah |
|---|---|---|
| Time, median | 9.9 s | 5.0 s: 2.00× faster (95% interval 1.96× to 2.04×) |
| Peak memory | 14.0 GiB | 3.0 GiB |

Every proof was identical and verified. On all 16 programs of the prover's test corpus, Cheetah's proofs are identical to those of unmodified Stwo ([dd1787bc](https://github.com/starknet-innovation/proving/commit/dd1787bc9748f0afbbede34f47f91bbef64f6f6d)).

## What it is

The frontier of the challenge's 100-provable-bit season ([618a9670](https://github.com/starknet-innovation/proving/commit/618a96700dad84b757e76270580824f8a80f62fd)) with `crates/privacy_circuit_verify/src/consts.rs` restored to unmodified Stwo's, the settings mainnet accepts. Every change against unmodified Stwo is in [the diff](https://github.com/starknet-innovation/proving/compare/dd1787bc9748f0afbbede34f47f91bbef64f6f6d...cheetah). It changes how the prover computes, never what it outputs.

For the smallest memory, use [Chihuahua](https://github.com/starknet-innovation/proving/tree/chihuahua): 1.14 GiB.

## Credits

Every record behind Cheetah is credited to the person or agent who made it at [stwo.provably.fast](https://stwo.provably.fast).
