#!/usr/bin/env bash
# Release gate driver; src/main.rs explains what is compared and why.
#
#   gate.sh build reference <source-dir> <work-dir>
#   gate.sh build candidate <source-dir> <work-dir>
#   gate.sh check <work-dir>
#
# <source-dir> is a full-history checkout of this repository at the release's source commit; it
# must descend from UPSTREAM_BASE_REV. `build` builds the gate crate in <work-dir>/<flavor>/crate
# (reference: upstream as pinned by the base rev's Cargo.lock; candidate: the consumer [patch]
# onto the fork, as pinned by the source's Cargo.lock) and proves upstream's reference PIE in every
# mode into <work-dir>/<flavor>/<mode>/. `check` verifies every proof with the reference build,
# requires the candidate's proofs to be byte-identical to the reference's, and writes
# <work-dir>/gate.json. Any failure exits non-zero.
#
# Env: UPSTREAM_BASE_REV (always), FORK_GIT_URL (build candidate), SOURCE_SHA (check).
set -euo pipefail

GATE_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
UPSTREAM_GIT_URL="https://github.com/starkware-libs/proving"
PIE_PATH="crates/privacy_prove/test_data/privacy_tx_cairo_pie.zip"
MODES=(recursive cairo)
: "${UPSTREAM_BASE_REV:?UPSTREAM_BASE_REV must be set}"

die() { echo "::error::$*" >&2; exit 1; }
sha256() { shasum -a 256 "$1" | cut -d' ' -f1; }

build() {
  local flavor=$1 src work
  src="$(cd "$2" && pwd)"
  mkdir -p "$3" && work="$(cd "$3" && pwd)"
  local crate="$work/$flavor/crate" source_sha expected_source
  source_sha="$(git -C "$src" rev-parse HEAD)"

  grep -q "rev = \"$UPSTREAM_BASE_REV\"" "$GATE_DIR/Cargo.toml" ||
    die "$GATE_DIR/Cargo.toml does not pin upstream at $UPSTREAM_BASE_REV"
  git -C "$src" merge-base --is-ancestor "$UPSTREAM_BASE_REV" "$source_sha" ||
    die "$source_sha does not descend from upstream base $UPSTREAM_BASE_REV"

  rm -rf "${work:?}/$flavor" && mkdir -p "$crate"
  cp -R "$GATE_DIR/Cargo.toml" "$GATE_DIR/src" "$crate/"
  case $flavor in
    reference)
      git -C "$src" show "$UPSTREAM_BASE_REV:Cargo.lock" > "$crate/seed.lock"
      git -C "$src" show "$UPSTREAM_BASE_REV:rust-toolchain.toml" > "$crate/rust-toolchain.toml"
      expected_source="git+$UPSTREAM_GIT_URL?rev=$UPSTREAM_BASE_REV#$UPSTREAM_BASE_REV"
      ;;
    candidate)
      : "${FORK_GIT_URL:?FORK_GIT_URL must be set}"
      git -C "$src" show "$source_sha:Cargo.lock" > "$crate/seed.lock"
      git -C "$src" show "$source_sha:rust-toolchain.toml" > "$crate/rust-toolchain.toml"
      # The same patch the README tells consumers to add, pinned to the commit being released.
      printf '\n[patch."%s"]\nprivacy-prove = { git = "%s", rev = "%s" }\n' \
        "$UPSTREAM_GIT_URL" "$FORK_GIT_URL" "$source_sha" >> "$crate/Cargo.toml"
      expected_source="git+$FORK_GIT_URL?rev=$source_sha#$source_sha"
      ;;
    *) die "unknown flavor $flavor" ;;
  esac
  # Seeding the lockfile makes Cargo keep every crates.io version the seed pins; lockcheck.py
  # then fails the gate if anything resolved outside it.
  cp "$crate/seed.lock" "$crate/Cargo.lock"

  (cd "$crate" && rustc -V && cargo build --release)
  (cd "$crate" && rustc -V) > "$work/$flavor/rustc.txt"
  python3 "$GATE_DIR/lockcheck.py" "$crate/seed.lock" "$crate/Cargo.lock" "$expected_source"
  echo "$source_sha" > "$work/$flavor/source_sha.txt"

  # The reference PIE is upstream's, whichever commit is being released.
  git -C "$src" show "$UPSTREAM_BASE_REV:$PIE_PATH" > "$work/$flavor/pie.zip"
  for mode in "${MODES[@]}"; do
    echo "== $flavor: prove $mode"
    "$crate/target/release/release-gate" prove "$mode" "$work/$flavor/pie.zip" "$work/$flavor/$mode"
  done
}

check() {
  local work
  work="$(cd "$1" && pwd)"
  : "${SOURCE_SHA:?SOURCE_SHA must be set}"
  local verifier="$work/reference/crate/target/release/release-gate"
  chmod +x "$verifier"

  [ "$(cat "$work/candidate/source_sha.txt")" = "$SOURCE_SHA" ] ||
    die "candidate proofs are for $(cat "$work/candidate/source_sha.txt"), not $SOURCE_SHA"
  cmp -s "$work/reference/pie.zip" "$work/candidate/pie.zip" || die "the flavors proved different PIEs"

  local modes_json="{}"
  for mode in "${MODES[@]}"; do
    for flavor in reference candidate; do
      echo "== upstream verifier: $flavor $mode proof"
      "$verifier" verify "$mode" "$work/$flavor/$mode/proof.bin" "$work/$flavor/$mode/output_preimage.json" ||
        die "upstream privacy-circuit-verify rejected the $flavor $mode proof"
    done
    for file in proof.bin output_preimage.json; do
      cmp "$work/reference/$mode/$file" "$work/candidate/$mode/$file" ||
        die "$mode $file differs from upstream's (reference $(sha256 "$work/reference/$mode/$file"), candidate $(sha256 "$work/candidate/$mode/$file"))"
    done
    modes_json="$(jq -c \
      --arg mode "$mode" \
      --arg proof_sha256 "$(sha256 "$work/candidate/$mode/proof.bin")" \
      --argjson proof_bytes "$(wc -c < "$work/candidate/$mode/proof.bin" | tr -d ' ')" \
      --arg preimage_sha256 "$(sha256 "$work/candidate/$mode/output_preimage.json")" \
      '.[$mode] = {proof_sha256: $proof_sha256, proof_bytes: $proof_bytes,
                   output_preimage_sha256: $preimage_sha256,
                   byte_identical_to_upstream: true, upstream_verifier: "accepted"}' \
      <<< "$modes_json")"
  done

  jq -n \
    --arg upstream "$UPSTREAM_GIT_URL" \
    --arg base "$UPSTREAM_BASE_REV" \
    --arg source "$SOURCE_SHA" \
    --arg pie "$PIE_PATH" \
    --arg pie_sha256 "$(sha256 "$work/reference/pie.zip")" \
    --arg reference_rustc "$(cat "$work/reference/rustc.txt")" \
    --arg candidate_rustc "$(cat "$work/candidate/rustc.txt")" \
    --argjson modes "$modes_json" \
    '{result: "pass", source_sha: $source, upstream_base_rev: $base,
      reference_pie: {path: $pie, at: $base, sha256: $pie_sha256},
      verifier: "privacy-circuit-verify from \($upstream)@\($base), unpatched",
      rustc: {reference: $reference_rustc, candidate: $candidate_rustc},
      modes: $modes}' > "$work/gate.json"
  cat "$work/gate.json"
}

case "${1:-}" in
  build) [ $# -eq 4 ] || die "usage: gate.sh build <reference|candidate> <source-dir> <work-dir>"; build "$2" "$3" "$4" ;;
  check) [ $# -eq 2 ] || die "usage: gate.sh check <work-dir>"; check "$2" ;;
  *) die "usage: gate.sh build <reference|candidate> <source-dir> <work-dir> | gate.sh check <work-dir>" ;;
esac
