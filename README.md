# proving (starknet-innovation fork)

A fork of [starkware-libs/proving](https://github.com/starkware-libs/proving) that carries the
provably.fast prover optimizations. See [`CLAUDE.md`](CLAUDE.md) for the repository layout.

## Consuming releases

Releases are cut from the `starknet-<sequencer version>/<frontier>` branches, each a port onto the
upstream rev that sequencer version pins, and tagged `starknet-v<sequencer version>-pf.<n>` (e.g.
`starknet-v0.14.4-pf.1`). Tags are never moved.

[`release.yml`](.github/workflows/release.yml) publishes a release only if, on upstream's
reference PIE, the release's `privacy-prove` produces proofs byte-identical to upstream's
`privacy-prove` at the base rev, and upstream's unpatched `privacy-circuit-verify` accepts them
(see [`.github/release-gate`](.github/release-gate/src/main.rs)). Each release's `manifest.json`
records the source commit, the upstream base rev, the toolchain and the gate's results.

### Cargo

Keep depending on upstream at the base rev and patch `privacy-prove` alone:

```toml
[patch."https://github.com/starkware-libs/proving"]
privacy-prove = { git = "https://github.com/starknet-innovation/proving", tag = "starknet-v0.14.4-pf.1" }
```

- The patch swaps the whole prover stack under `privacy-prove`. `privacy-circuit-verify`, and the
  verifier stack under it, stays upstream's, so proofs are still checked by unmodified upstream
  code.
- The two crates then no longer share types: build upstream's `PrivacyProofOutput` from the
  prover's `proof` and `output_preimage` fields rather than passing the prover's value along.
- The fork's `privacy-prove` installs a `#[global_allocator]` on Unix, so a binary that declares
  its own does not compile.
- Build with the release's toolchain, `nightly-2026-01-15`.

### `stwo-run-and-prove` binaries

Each release has `stwo-run-and-prove-<platform>.tar.gz` for `linux-x86_64`, `linux-arm64` and
`darwin-arm64`, plus `linux-x86_64-znver4` and `linux-x86_64-sapphirerapids`, which are built with
`-C target-cpu` and fail with an illegal instruction on other CPUs. Linux builds need glibc 2.39 or
newer (Ubuntu 24.04+).

```bash
TAG=starknet-v0.14.4-pf.1
PLATFORM=linux-x86_64
BASE=https://github.com/starknet-innovation/proving/releases/download/$TAG
curl -fsSLO "$BASE/stwo-run-and-prove-$PLATFORM.tar.gz"
curl -fsSLO "$BASE/SHA256SUMS"
sha256sum -c --ignore-missing SHA256SUMS  # macOS: shasum -a 256 -c --ignore-missing SHA256SUMS
gh attestation verify "stwo-run-and-prove-$PLATFORM.tar.gz" --repo starknet-innovation/proving
gh attestation verify "stwo-run-and-prove-$PLATFORM.tar.gz" --repo starknet-innovation/proving \
  --predicate-type https://github.com/starknet-innovation/proving/release-manifest/v1
tar -xzf "stwo-run-and-prove-$PLATFORM.tar.gz" stwo-run-and-prove
```

The first check is the build provenance: the file was built by a run of `release.yml` in this
repository. The second checks a statement signed by that run that ties the file to the release's
`manifest.json`, which records the source commit and the gate's results.
