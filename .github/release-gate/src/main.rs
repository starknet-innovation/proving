//! Release gate for fork releases of `privacy-prove` and `stwo-run-and-prove`.
//!
//! `.github/workflows/release.yml` builds this crate twice from the same source:
//! - the reference build uses `privacy-prove` and `privacy-circuit-verify` from upstream at the
//!   base rev in `Cargo.toml`;
//! - the candidate build adds the `[patch]` that consumers use, so `privacy-prove` (and with it the
//!   whole prover stack) comes from the fork at the release's source commit, while
//!   `privacy-circuit-verify` stays upstream's.
//!
//! The workflow proves the reference PIE with both builds, requires the proofs and output
//! preimages to be byte-identical, and verifies every proof with the reference build, which has no
//! fork code linked in.

use std::error::Error;
use std::path::Path;
use std::str::FromStr;
use std::time::Instant;
use std::{fmt, fs};

use cairo_vm::vm::runners::cairo_pie::CairoPie;
use privacy_circuit_verify::{PrivacyProofOutput, verify_cairo, verify_recursive_circuit};
use starknet_types_core::felt::Felt;

const USAGE: &str = "usage:
  release-gate prove  <recursive|cairo> <pie.zip> <out-dir>
  release-gate verify <recursive|cairo> <proof.bin> <output_preimage.json>";

/// Which prover/verifier pair to exercise.
#[derive(Clone, Copy)]
enum Mode {
    /// `privacy_recursive_prove` / `verify_recursive_circuit`: what the sequencer's
    /// `starknet_transaction_prover` and `starknet_proof_verifier` call.
    Recursive,
    /// `privacy_prove` / `verify_cairo`: the Cairo proof alone, without the recursion circuit.
    Cairo,
}

impl FromStr for Mode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "recursive" => Ok(Self::Recursive),
            "cairo" => Ok(Self::Cairo),
            _ => Err(format!("unknown mode {s:?}\n{USAGE}")),
        }
    }
}

impl fmt::Display for Mode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Recursive => "recursive",
            Self::Cairo => "cairo",
        })
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.iter().map(String::as_str).collect::<Vec<_>>().as_slice() {
        ["prove", mode, pie, out_dir] => prove(mode.parse()?, Path::new(pie), Path::new(out_dir)),
        ["verify", mode, proof, preimage] => {
            verify(mode.parse()?, Path::new(proof), Path::new(preimage))
        }
        _ => Err(USAGE.into()),
    }
}

/// Proves `pie_path` and writes `proof.bin` and `output_preimage.json` to `out_dir`.
fn prove(mode: Mode, pie_path: &Path, out_dir: &Path) -> Result<(), Box<dyn Error>> {
    let pie = CairoPie::read_zip_file(pie_path)?;

    let start = Instant::now();
    let output = match mode {
        Mode::Recursive => {
            let precomputes = privacy_prove::prepare_recursive_prover_precomputes()?;
            eprintln!("precomputes ready in {:.1?}", start.elapsed());
            privacy_prove::privacy_recursive_prove(pie, precomputes)?
        }
        Mode::Cairo => privacy_prove::privacy_prove(pie)?,
    };
    eprintln!("{mode} proof: {} bytes in {:.1?}", output.proof.len(), start.elapsed());

    let preimage: Vec<String> =
        output.output_preimage.iter().map(|felt| felt.to_fixed_hex_string()).collect();
    fs::create_dir_all(out_dir)?;
    fs::write(out_dir.join("proof.bin"), &output.proof)?;
    fs::write(
        out_dir.join("output_preimage.json"),
        serde_json::to_string_pretty(&preimage)? + "\n",
    )?;
    Ok(())
}

/// Verifies a proof written by [`prove`]; fails unless the verifier accepts it.
fn verify(mode: Mode, proof_path: &Path, preimage_path: &Path) -> Result<(), Box<dyn Error>> {
    let preimage: Vec<String> = serde_json::from_str(&fs::read_to_string(preimage_path)?)?;
    let output_preimage = preimage
        .iter()
        .map(|hex| Felt::from_hex(hex).map_err(|e| format!("bad felt {hex:?}: {e:?}")))
        .collect::<Result<Vec<_>, _>>()?;
    let proof_output = PrivacyProofOutput { proof: fs::read(proof_path)?, output_preimage };

    let start = Instant::now();
    match mode {
        Mode::Recursive => verify_recursive_circuit(&proof_output)?,
        Mode::Cairo => verify_cairo(&proof_output)?,
    }
    eprintln!("{mode} proof verified in {:.1?}", start.elapsed());
    Ok(())
}
