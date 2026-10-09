//! Setup work that may run concurrently with a later serial stage.
//!
//! The privacy prover's harness runs its stages one after another: the Cairo proof, then
//! [`build_and_fill_cairo_verifier_circuit`], then the circuit proof. Some setup only the
//! circuit proof reads (its preprocessed columns and tree) is built lazily so that it is not
//! resident while the Cairo proof sets the process's memory peak; left alone, it would then be
//! built serially on first use, between the legs, on an otherwise idle machine. Registering
//! that work here lets the serial circuit fill drain it alongside its own work.
//!
//! Every registered task only forces values that are otherwise built on first use, so running
//! it early, late, on another thread, or not at all leaves every value unchanged.
//!
//! [`build_and_fill_cairo_verifier_circuit`]: ../../circuit_cairo_verifier/fn.build_and_fill_cairo_verifier_circuit.html
use std::sync::Mutex;

type Task = Box<dyn FnOnce() + Send>;

static DEFERRED: Mutex<Vec<Task>> = Mutex::new(Vec::new());

/// Registers setup work that a later stage may run concurrently with its own.
pub fn defer(task: impl FnOnce() + Send + 'static) {
    DEFERRED.lock().unwrap().push(Box::new(task));
}

/// Runs `stage`, and every task registered so far, concurrently; returns `stage`'s result.
pub fn run_with_deferred<R: Send>(stage: impl FnOnce() -> R + Send) -> R {
    let tasks: Vec<Task> = std::mem::take(&mut *DEFERRED.lock().unwrap());
    if tasks.is_empty() {
        return stage();
    }
    let (result, ()) = rayon::join(stage, || tasks.into_iter().for_each(|task| task()));
    result
}
