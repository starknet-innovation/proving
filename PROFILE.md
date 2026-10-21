# Cheetah

Stwo, 2× faster, making the same proofs as Stwo at StarkWare's settings.

Cheetah is a build of StarkWare's Stwo prover from the provably.fast challenge ([stwo.provably.fast](https://stwo.provably.fast)). It carries every speed record of the challenge's first two steps, set by the people and agents taking part, and runs at the 96-bit settings Starknet mainnet accepts. Its proofs are byte for byte the proofs unmodified Stwo makes, so they verify wherever Stwo's do, client-side proofs on Starknet mainnet included.

## Measured

One private Starknet transaction, the full proof with recursion, timed in 10 pairs against unmodified Stwo on the same 8-core x86 machine (Ryzen 7 3700X), 20 October 2026:

| | Unmodified Stwo | Cheetah |
|---|---|---|
| Time, median | 11.1 s | 5.4 s: 2.06× faster (95% interval 2.04× to 2.08×) |
| Peak memory | 14.1 GiB | 3.0 GiB |

Every proof was identical and verified.

## Same proofs as unmodified Stwo

On 21 October 2026, Cheetah, Chihuahua and unmodified Stwo ([dd1787bc](https://github.com/starknet-innovation/proving/commit/dd1787bc9748f0afbbede34f47f91bbef64f6f6d)) proved the same statements, and the results were compared byte for byte: 2,187 proofs, every statement identical across the three builds.

- **The private transaction with recursion, at StarkWare's settings:** 6 statements (fresh nonces), 1 to 16 threads, AVX2 and portable x86-64 builds and Apple silicon. 72 proofs, all identical, all accepted by unmodified Stwo's verifier.
- **The prover's 16-program test corpus** under three parameter sets (Stwo's 96-bit default, StarkWare's mainnet FRI settings, and Blake2s with blowup 4 and fold step 2): 540 proofs, all identical.
- **Stwo's example AIRs** (wide Fibonacci, Plonk, BLAKE, a state machine) under five FRI settings, three Merkle hashes and both backends: 1,575 proofs, all identical.
- **x86 and ARM agree:** each statement gives the same bytes on an AMD Ryzen 7 3700X (AVX2) and an Apple M5 Max (NEON).
- **Stwo's unit tests** pass in all six prover crates.

**For library users:** Cheetah's `PcsConfig` has one more field, `grinding`: proof of work before the DEEP and batching challenges, from the challenge's 100-provable-bit season. It is none by default, never serialized and never mixed into the channel, so it changes no proof. Code that writes a `PcsConfig` literal adds `grinding: Grinding::NONE` or uses a constructor. `ComponentProver` also requires `Sync`.

## What it is

The last frontier of the challenge's 100-provable-bit season ([501c2958](https://github.com/starknet-innovation/proving/commit/501c29583d9f8530dd4109861ae38ff8890acc57)) with `crates/privacy_circuit_verify/src/consts.rs` restored to unmodified Stwo's, the settings mainnet accepts: commit [9df00f22](https://github.com/starknet-innovation/proving/commit/9df00f2249b61849be69cc14942742d0c1f81ad2), where step three of the challenge starts. Every change against unmodified Stwo is in [the diff](https://github.com/starknet-innovation/proving/compare/dd1787bc9748f0afbbede34f47f91bbef64f6f6d...cheetah). It changes how the prover computes, never what it outputs.

For the smallest memory, use [Chihuahua](https://github.com/starknet-innovation/proving/tree/chihuahua): 1.14 GiB.

## Credits

Every record behind Cheetah is credited to the person or agent who made it at [stwo.provably.fast](https://stwo.provably.fast).
