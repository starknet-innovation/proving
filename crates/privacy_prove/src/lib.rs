pub mod consts;
#[cfg(test)]
mod tests;

use std::cmp::max;
use std::error::Error;
use std::fs::read_to_string;
use std::rc::Rc;
use std::sync::Arc;

use anyhow::Result;
use cairo_air::flat_claims::FlatClaim;
use cairo_program_runner_lib::types::{
    HashFunc, PrivacySimpleBootloaderInput, SimpleBootloaderInput,
};
use cairo_program_runner_lib::{ProgramInput, Task, TaskSpec, cairo_run_program};
use cairo_vm::vm::runners::cairo_pie::CairoPie;
use circuit_cairo_verifier::verify::{
    CairoVerifierConfig, build_and_fill_cairo_verifier_circuit,
    prepare_cairo_proof_for_circuit_verifier,
};
use circuit_common::finalize::{add_zk_blinding, pad_context};
use circuit_common::preprocessed::{PreprocessedCircuit, PreprocessedColumnSource};
use circuit_prover::prover::{
    prepare_circuit_proof_for_circuit_verifier, prove_circuit_with_precompute,
};
use circuit_serialize::serialize::CircuitSerialize;
use circuit_verifier::verify::CircuitConfig;
use circuits_stark_verifier::proof::ProofConfig;
use itertools::chain;
use privacy_circuit_verify::consts::{CIRCUIT_FRI_CONFIG, CIRCUIT_PCS_CONFIG};
use privacy_circuit_verify::{
    PrivacyProofOutput, Version, compute_privacy_bootloader_output_hash,
    get_cairo_preprocessed_circuit, get_cairo_verifier_config, get_privacy_bootloader_program,
    get_proof_config, get_recursive_circuit_config,
};
use serde_json::from_str;
use starknet_types_core::felt::Felt;
use stwo::core::poly::circle::CanonicCoset;
use stwo::core::utils::MaybeOwned;
use stwo::core::vcs_lifted::blake2_merkle::Blake2sM31MerkleChannel;
use stwo::prover::CommitmentTreeProver;
use stwo::prover::backend::simd::SimdBackend;
use stwo::prover::vcs_lifted::prover::MerkleProverLifted;
#[cfg(unix)]
use stwo::prover::backend::simd::column::LargeBlockCache;
use stwo::prover::mempool::BaseColumnPool;
use stwo::prover::poly::circle::PolyOps;
use stwo::prover::poly::twiddles::TwiddleTree;
use stwo_cairo_adapter::ProverInput;
use stwo_cairo_adapter::adapter::adapt;
use stwo_cairo_common::preprocessed_columns::preprocessed_trace::PreProcessedTrace;
use stwo_cairo_prover::prover::{
    LiftingSizePolicy, prove_cairo, prove_cairo_with_precompute, warm_pedersen_pp_trace,
};
use tempfile::NamedTempFile;
use tracing::{Level, info, span};

use crate::consts::{
    CAIRO_PROVER_PARAMS, CAIRO_RUN_CONFIG, CIRCUIT_STORE_POLYNOMIALS_COEFFICIENTS,
};

/// The precomputes of a prover process.
///
/// The large members (the pool with its buffers, the twiddles, the preprocessed trees and the
/// circuit) live as long as the process and are not freed when the precomputes are dropped: a
/// prover drops them only on its way out, where unmapping several gigabytes buffer by buffer
/// costs tens of milliseconds for nothing, while the operating system reclaims the whole address
/// space at once at exit.
pub struct RecursiveProverPrecomputes {
    pub base_column_pool: std::mem::ManuallyDrop<Arc<BaseColumnPool<SimdBackend>>>,
    pub twiddles: std::mem::ManuallyDrop<Arc<TwiddleTree<SimdBackend>>>,
    pub cairo_preprocessed_trace: Arc<PreProcessedTrace>,
    /// Empty: the Cairo proof builds the tree from `cairo_preprocessed_trace` and frees it before
    /// the circuit proof, which never reads it, so it is not resident under the circuit's peak.
    pub cairo_preprocessed_tree:
        std::mem::ManuallyDrop<CommitmentTreeProver<SimdBackend, Blake2sM31MerkleChannel>>,
    pub cairo_verifier_config: CairoVerifierConfig,
    /// Built on first use, which is after the Cairo proof: its committed evaluations are not
    /// resident while that proof runs, its columns reuse that proof's idle buffers, and the
    /// circuit proof needs them only when it starts.
    pub circuit_preprocessed_tree: std::mem::ManuallyDrop<LazyCommitmentTree>,
    /// Built on first use, after the Cairo proof (see [`LazyPreprocessedCircuit`]).
    pub preprocessed_circuit: std::mem::ManuallyDrop<Arc<LazyPreprocessedCircuit>>,
    pub circuit_config: CircuitConfig,
    pub proof_config: ProofConfig,
}

type PrecomputedTree = CommitmentTreeProver<SimdBackend, Blake2sM31MerkleChannel>;

/// A committed preprocessed tree that is built the first time it is dereferenced.
pub struct LazyCommitmentTree {
    tree: std::sync::OnceLock<PrecomputedTree>,
    build: std::sync::Mutex<Option<Box<dyn FnOnce() -> PrecomputedTree + Send>>>,
}

impl LazyCommitmentTree {
    pub fn new(build: impl FnOnce() -> PrecomputedTree + Send + 'static) -> Self {
        Self {
            tree: std::sync::OnceLock::new(),
            build: std::sync::Mutex::new(Some(Box::new(build))),
        }
    }
}

impl std::ops::Deref for LazyCommitmentTree {
    type Target = PrecomputedTree;

    fn deref(&self) -> &PrecomputedTree {
        self.tree.get_or_init(|| {
            let build = self.build.lock().unwrap().take().expect("the tree is built once");
            build()
        })
    }
}

/// The circuit's preprocessing, built the first time it is dereferenced: the Cairo proof never
/// reads it, so it is not resident under the Cairo leg's peak, and the circuit proof builds it
/// while the verifier circuit is filled (see `pvfast_workload.rs`).
pub struct LazyPreprocessedCircuit {
    circuit: std::sync::OnceLock<PreprocessedCircuit>,
}

impl LazyPreprocessedCircuit {
    fn new() -> Self {
        Self { circuit: std::sync::OnceLock::new() }
    }
}

impl std::ops::Deref for LazyPreprocessedCircuit {
    type Target = PreprocessedCircuit;

    fn deref(&self) -> &PreprocessedCircuit {
        self.circuit.get_or_init(|| {
            let _span = span!(Level::INFO, "prepare_preprocessed_circuit").entered();
            let cairo_verifier_config =
                get_cairo_verifier_config().expect("the Cairo verifier config was built at setup");
            get_cairo_preprocessed_circuit(&cairo_verifier_config)
        })
    }
}

/// Large blocks come from a cache every thread shares instead of each thread's malloc arena, so
/// the temporaries one worker frees are reused by the others; see [`LargeBlockCache`].
#[cfg(unix)]
#[global_allocator]
static ALLOCATOR: LargeBlockCache = LargeBlockCache;

fn compress_proof(proof_bytes: &[u8]) -> Result<Vec<u8>, Box<dyn Error>> {
    Ok(zstd::encode_all(proof_bytes, 3)?)
}

/// Prepends the serialized current [`Version`] to the compressed proof bytes. The verifier strips
/// this prefix before decompressing (see `split_proof_version`).
fn prepend_version(compressed_proof: Vec<u8>) -> Vec<u8> {
    chain!(Version::current().serialize(), compressed_proof).collect()
}

/// Runs the program and generates a proof for it with params, bootloader and output format suitable
/// for the privacy circuit verifier.
pub fn privacy_prove(pie: CairoPie) -> Result<PrivacyProofOutput, Box<dyn Error>> {
    let _span = span!(Level::INFO, "privacy_prove").entered();

    info!("Run privacy bootloader and get the prover input and output preimage");
    let (prover_input, output_preimage) = run_privacy_bootloader(pie)?;

    info!("Generate the cairo proof");
    let cairo_proof = prove_cairo::<Blake2sM31MerkleChannel>(prover_input, CAIRO_PROVER_PARAMS)?;
    let FlatClaim { component_enable_bits, component_log_sizes: _, public_data: _ } =
        cairo_proof.claim.flatten_claim();

    info!("Prepare the proof for the circuit verifier");
    let (proof, serialized_aux_data) =
        prepare_cairo_proof_for_circuit_verifier(&cairo_proof, &component_enable_bits);

    info!("Serialize and compress the proof and public data");
    let mut proof_bytes: Vec<u8> = vec![];
    proof.serialize(&mut proof_bytes);
    let serialized_aux_bytes: Vec<u8> =
        serialized_aux_data.iter().flat_map(|x| x.0.to_le_bytes()).collect();
    let combined_bytes: Vec<u8> = chain!(serialized_aux_bytes, proof_bytes).collect();
    let compressed = compress_proof(&combined_bytes)?;
    let proof = prepend_version(compressed);

    Ok(PrivacyProofOutput { proof, output_preimage })
}

pub fn prepare_recursive_prover_precomputes()
-> Result<Arc<RecursiveProverPrecomputes>, Box<dyn Error>> {
    let _span = span!(Level::INFO, "prepare_privacy_recursiveprover_precomputes").entered();

    let cairo_verifier_config = get_cairo_verifier_config()?;
    let circuit_config = get_recursive_circuit_config();
    let proof_config = get_proof_config();

    let base_column_pool = BaseColumnPool::<SimdBackend>::new();
    let LiftingSizePolicy::Fixed(cairo_lifting_log_size) = CAIRO_PROVER_PARAMS.lifting_size_policy
    else {
        panic!("Only LiftingSizePolicy::Fixed is supported with a precomputed preprocessed tree");
    };
    let circuit_lifting_log_size = CIRCUIT_PCS_CONFIG.trace_lifting_log_size;

    let max_domain_size = max(cairo_lifting_log_size, circuit_lifting_log_size);

    // The circuit's preprocessing is built on first use, after the Cairo proof.
    let preprocessed_circuit = Arc::new(LazyPreprocessedCircuit::new());
    let (twiddles, cairo_preprocessed_trace, cairo_preprocessed_tree) = {
        // The twiddles and the Cairo preprocessed trace are independent as well.
        let (twiddles, cairo_preprocessed_trace) = rayon::join(
            || {
                info!("Prepare the twiddles");
                SimdBackend::precompute_twiddles(
                    CanonicCoset::new(max_domain_size).circle_domain().half_coset,
                )
            },
            || {
                info!("Prepare the cairo prover preprocessed trace");
                let cairo_preprocessed_trace = Arc::new(
                    CAIRO_PROVER_PARAMS.preprocessed_trace.to_preprocessed_trace(),
                );
                // Warm the Pedersen points table before the Cairo leg's gen_trace reads it.
                warm_pedersen_pp_trace(CAIRO_PROVER_PARAMS.preprocessed_trace);
                cairo_preprocessed_trace
            },
        );
        // The Cairo preprocessed tree is left empty: the Cairo leg builds it, owned, and frees it
        // before the circuit leg, which never reads it (see `prove_cairo_with_precompute`).
        let cairo_preprocessed_tree = PrecomputedTree {
            polynomials: vec![],
            commitment: MerkleProverLifted { layers: vec![] },
        };
        (twiddles, cairo_preprocessed_trace, cairo_preprocessed_tree)
    };

    info!("Prepare the circuit prover preprocessed trace and tree (built on first use)");
    let base_column_pool = Arc::new(base_column_pool);
    let twiddles = Arc::new(twiddles);
    let circuit_preprocessed_tree = {
        let preprocessed_circuit = preprocessed_circuit.clone();
        let twiddles = twiddles.clone();
        let base_column_pool = base_column_pool.clone();
        let preprocessed_lifting_log_size = circuit_config.config.preprocessed_lifting_log_size;
        LazyCommitmentTree::new(move || {
            let _span = span!(Level::INFO, "prepare_circuit_preprocessed_tree").entered();
            let preprocessed_trace = preprocessed_circuit.preprocessed_trace.clone();
            let circuit_preprocessed_trace = preprocessed_trace.get_trace::<SimdBackend>();
            let circuit_preprocessed_trace_polys =
                SimdBackend::interpolate_columns(circuit_preprocessed_trace, &twiddles);
            // The compact preprocessed columns stay resident for the circuit proof: the tree
            // regrows its columns from them instead of keeping a stripe of each extension.
            let sources =
                PreprocessedColumnSource::all(&preprocessed_trace).into_iter().map(Some).collect();
            CommitmentTreeProver::<SimdBackend, Blake2sM31MerkleChannel>::new_with_sources(
                circuit_preprocessed_trace_polys,
                CIRCUIT_FRI_CONFIG.log_blowup_factor,
                &twiddles,
                CIRCUIT_STORE_POLYNOMIALS_COEFFICIENTS,
                preprocessed_lifting_log_size,
                &base_column_pool,
                sources,
            )
        })
    };

    Ok(Arc::new(RecursiveProverPrecomputes {
        base_column_pool: std::mem::ManuallyDrop::new(base_column_pool),
        twiddles: std::mem::ManuallyDrop::new(twiddles),
        cairo_preprocessed_trace,
        cairo_preprocessed_tree: std::mem::ManuallyDrop::new(cairo_preprocessed_tree),
        cairo_verifier_config,
        circuit_preprocessed_tree: std::mem::ManuallyDrop::new(circuit_preprocessed_tree),
        preprocessed_circuit: std::mem::ManuallyDrop::new(preprocessed_circuit),
        circuit_config,
        proof_config,
    }))
}

pub fn privacy_recursive_prove(
    pie: CairoPie,
    precomputes: Arc<RecursiveProverPrecomputes>,
) -> Result<PrivacyProofOutput, Box<dyn Error>> {
    let _span = span!(Level::INFO, "privacy_recursive_prove").entered();

    info!("Run privacy bootloader and get the prover input and output preimage");
    let (prover_input, output_preimage) = run_privacy_bootloader(pie)?;

    info!("Generate the cairo proof");
    let cairo_proof = prove_cairo_with_precompute(
        &precomputes.base_column_pool,
        &precomputes.twiddles,
        precomputes.cairo_preprocessed_trace.clone(),
        MaybeOwned::Borrowed(&precomputes.cairo_preprocessed_tree),
        prover_input,
        CAIRO_PROVER_PARAMS,
    )?;
    let FlatClaim { component_enable_bits, component_log_sizes: _, public_data: _ } =
        cairo_proof.claim.flatten_claim();
    info!("Prepare the cairo proof for the cairo-circuit verifier");
    let (proof, serialized_aux_data) =
        prepare_cairo_proof_for_circuit_verifier(&cairo_proof, &component_enable_bits);

    info!("Build the cairo-circuit verifier context");
    let output_hash = compute_privacy_bootloader_output_hash(&output_preimage);
    let mut context = build_and_fill_cairo_verifier_circuit(
        &precomputes.cairo_verifier_config,
        proof,
        serialized_aux_data,
        output_hash,
    );
    if !context.is_circuit_valid() {
        return Err("Circuit is not valid".into());
    };
    let zk_blinding_seed = cairo_proof.extended_stark_proof.proof.commitments.0[1].0;
    add_zk_blinding(
        &mut context,
        zk_blinding_seed,
        precomputes.circuit_config.config.fri_config.n_queries,
    );
    pad_context(&mut context);
    let context_values = context.values();

    info!("Prove the cairo-circuit verifier");
    let circuit_proof = prove_circuit_with_precompute(
        &precomputes.base_column_pool,
        &precomputes.twiddles,
        &precomputes.preprocessed_circuit,
        MaybeOwned::Borrowed(&precomputes.circuit_preprocessed_tree),
        context_values,
        precomputes.circuit_config.config,
    )?;

    info!("Prepare the circuit proof for the circuit verifier");
    let (proof_qm31s, _public_data) = prepare_circuit_proof_for_circuit_verifier(circuit_proof);

    info!("Serialize and compress the proof");
    let mut proof_bytes: Vec<u8> = vec![];
    proof_qm31s.serialize(&mut proof_bytes);
    let compressed = compress_proof(&proof_bytes)?;
    let proof = prepend_version(compressed);

    Ok(PrivacyProofOutput { proof, output_preimage })
}

fn run_privacy_bootloader(pie: CairoPie) -> Result<(ProverInput, Vec<Felt>), Box<dyn Error>> {
    let _span = span!(Level::INFO, "get_prover_input").entered();

    let output_preimage_file = NamedTempFile::new()?;
    let output_preimage_path = output_preimage_file.path().to_path_buf();
    let pie_task_spec =
        TaskSpec { task: Rc::new(Task::Pie(pie)), program_hash_function: HashFunc::Blake };
    let bootloader_input = PrivacySimpleBootloaderInput {
        simple_bootloader_input: SimpleBootloaderInput {
            fact_topologies_path: None,
            single_page: true,
            tasks: vec![pie_task_spec],
        },
        output_preimage_dump_path: output_preimage_path.clone(),
    };
    let bootloader_program = get_privacy_bootloader_program()?;

    info!("Running the program");
    let runner = cairo_run_program(
        &bootloader_program,
        Some(ProgramInput::Value(Box::new(bootloader_input))),
        CAIRO_RUN_CONFIG,
        None,
    )?;

    info!("Reading the bootloader output preimage");
    let output_preimage_content = read_to_string(&output_preimage_path)?;
    let output_preimage: Vec<Felt> = from_str(&output_preimage_content)?;

    info!("Adapting the runner output for the prover");
    let prover_input = adapt(&runner)?;

    Ok((prover_input, output_preimage))
}
