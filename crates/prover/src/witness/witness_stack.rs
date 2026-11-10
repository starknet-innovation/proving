//! Stacks for the deep Cairo witness writers (`cube_252`, `partial_ec_mul_window_bits_9/18`).

use std::cell::Cell;

/// Stack size those writers need, or witness generation may overflow the stack.
pub const WITNESS_STACK_SIZE: usize = 1024 * 1024 * 8;

thread_local! {
    /// Set on the threads of a pool built with [`WITNESS_STACK_SIZE`] stacks.
    static HAS_WITNESS_STACK: Cell<bool> = const { Cell::new(false) };
}

/// Marks the calling thread as running on a [`WITNESS_STACK_SIZE`] stack: call it from the
/// `start_handler` of a rayon pool built with `stack_size(WITNESS_STACK_SIZE)`.
pub fn mark_witness_stack_thread() {
    HAS_WITNESS_STACK.with(|marked| marked.set(true));
}

/// Runs a deep witness writer where its stack is large enough: inline on a marked thread (its
/// parallel work stays in that thread's pool), otherwise in a pool of such stacks built for it, as
/// the writers always did. Inline, the writers no longer start and drop a pool of one thread per
/// core beside the busy workers of the proof's own pool.
pub fn with_witness_stack<R: Send>(op: impl FnOnce() -> R + Send) -> R {
    if HAS_WITNESS_STACK.with(|marked| marked.get()) {
        return op();
    }
    rayon::ThreadPoolBuilder::new().stack_size(WITNESS_STACK_SIZE).build().unwrap().install(op)
}
