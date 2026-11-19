//! Stacks for the deep Cairo witness writers (`cube_252`, `partial_ec_mul_window_bits_9/18`).

use std::cell::Cell;

/// Stack size those writers need.
pub const WITNESS_STACK_SIZE: usize = 1024 * 1024 * 8;

thread_local! {
    /// Set on threads with [`WITNESS_STACK_SIZE`] stacks.
    static HAS_WITNESS_STACK: Cell<bool> = const { Cell::new(false) };
}

/// Marks this thread. Call it from the `start_handler` of a [`WITNESS_STACK_SIZE`] pool.
pub fn mark_witness_stack_thread() {
    HAS_WITNESS_STACK.with(|marked| marked.set(true));
}

/// Runs `op` inline on a marked thread, otherwise in a new pool with large enough stacks.
pub fn with_witness_stack<R: Send>(op: impl FnOnce() -> R + Send) -> R {
    if HAS_WITNESS_STACK.with(|marked| marked.get()) {
        return op();
    }
    rayon::ThreadPoolBuilder::new().stack_size(WITNESS_STACK_SIZE).build().unwrap().install(op)
}
