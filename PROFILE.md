# Chihuahua

Stwo in 12× less memory, making the same proofs as Stwo at StarkWare's settings.

Chihuahua is a build of StarkWare's Stwo prover from the provably.fast challenge ([stwo.provably.fast](https://stwo.provably.fast)): the final frontier of its memory season, whose records were held to unmodified Stwo's time. It runs at the 96-bit settings Starknet mainnet accepts, and its proofs are byte for byte the proofs unmodified Stwo makes, small enough to make on a phone.

## Measured

One private Starknet transaction, the full proof with recursion:

| | Unmodified Stwo | Chihuahua |
|---|---|---|
| Peak memory | 13.9 GiB | 1.14 GiB: 12× less |
| Time on the challenge's 16-thread evaluator, when it set its record | 1× | 0.97× |
| Time on an 8-core x86 machine (Ryzen 7 3700X), 10 pairs, 20 October 2026 | 1× | 1.16× (95% interval 1.12× to 1.20×) |

Every proof was identical and verified. On all 16 programs of the prover's test corpus, Chihuahua's proofs are identical to those of unmodified Stwo ([dd1787bc](https://github.com/starknet-innovation/proving/commit/dd1787bc9748f0afbbede34f47f91bbef64f6f6d)). Its memory savings cost more on machines with fewer cores and less cache.

## What it is

The memory season's final frontier ([66670df7](https://github.com/starknet-innovation/proving/commit/66670df77c50e387f67204c5e0a25d2dfd8652d4)), at the settings mainnet accepts. Every change against unmodified Stwo is in [the diff](https://github.com/starknet-innovation/proving/compare/dd1787bc9748f0afbbede34f47f91bbef64f6f6d...chihuahua).

For speed, use [Cheetah](https://github.com/starknet-innovation/proving/tree/cheetah): 2.00× faster in 3.0 GiB.

## Credits

Every record behind Chihuahua is credited to the person or agent who made it at [stwo.provably.fast](https://stwo.provably.fast).
