//! Pre-allocated memory pools for the proving pipeline.
//!
//! The [`BaseColumnPool`] manages reusable [`Col<B, BaseField>`] buffers for polynomial evaluation,
//! avoiding repeated allocation/deallocation of large column buffers during proving.

use std::sync::atomic::{AtomicU32, Ordering};

use dashmap::DashMap;

use crate::core::fields::m31::BaseField;
use crate::prover::backend::{Col, Column, ColumnOps};

/// A pool of pre-allocated [`Col<B, BaseField>`] buffers, organized by log_size.
///
/// Used to avoid repeated allocation of evaluation buffers during polynomial commitment.
pub struct BaseColumnPool<B: ColumnOps<BaseField>> {
    /// Map from log_size -> stack of available buffers.
    pools: DashMap<u32, Vec<Col<B, BaseField>>>,
    /// The largest log size requested so far: fresh buffers of nearly that size are allocated
    /// with room to grow to the next size, so that later larger requests can reuse them.
    largest_log_size: AtomicU32,
}

impl<B: ColumnOps<BaseField>> BaseColumnPool<B> {
    /// Creates a new empty base column pool.
    pub fn new() -> Self {
        Self { pools: DashMap::new(), largest_log_size: AtomicU32::new(0) }
    }

    /// Pre-allocates `count` zero-initialized buffers of size `1 << log_size`.
    pub fn reserve(&self, log_size: u32, count: usize) {
        let mut pool = self.pools.entry(log_size).or_default();
        for _ in 0..count {
            pool.push(Col::<B, BaseField>::zeros(1 << log_size));
        }
    }

    /// Takes a buffer from the pool for the given `log_size`.
    ///
    /// # Panics
    ///
    /// Panics if no buffer of the requested size is available.
    pub fn take(&self, log_size: u32) -> Col<B, BaseField> {
        self.pools.get_mut(&log_size).and_then(|mut pool| pool.pop()).unwrap_or_else(|| {
            panic!("BaseColumnPool: no buffer available for log_size={log_size}")
        })
    }

    /// Takes a buffer of exactly `1 << log_size` elements from the pool, if one is available.
    pub fn try_take(&self, log_size: u32) -> Option<Col<B, BaseField>> {
        self.pools.get_mut(&log_size).and_then(|mut pool| pool.pop())
    }

    /// Takes a buffer from the pool, or allocates a new uninitialized one if none is available.
    ///
    /// Without a buffer of the exact size, the smallest larger idle buffer is shortened to the
    /// requested size (where the backend supports it): its pages are already resident, which the
    /// pages of a fresh allocation are not.
    pub fn take_or_alloc(&self, log_size: u32) -> Col<B, BaseField> {
        let largest = self.largest_log_size.fetch_max(log_size, Ordering::Relaxed).max(log_size);
        if let Some(buffer) = self.try_take(log_size) {
            return buffer;
        }
        let larger = self
            .pools
            .iter()
            .filter(|entry| *entry.key() > log_size && !entry.value().is_empty())
            .map(|entry| *entry.key())
            .min();
        if let Some(larger) = larger {
            if let Some(mut buffer) = self.try_take(larger) {
                if buffer.truncate(1 << log_size) {
                    // The tail of a larger buffer was resident; give those pages back to the
                    // kernel instead of carrying them through the rest of the proof.
                    buffer.release_spare();
                    return buffer;
                }
                self.give_back(larger, buffer);
            }
        }
        // A smaller idle buffer allocated with room to grow (see below), or shortened from a
        // larger one, is grown back: its pages are resident or reserved, and nothing is copied.
        for smaller in (log_size.saturating_sub(3)..log_size).rev() {
            if let Some(mut pool) = self.pools.get_mut(&smaller) {
                for i in 0..pool.len() {
                    if pool[i].grow(1 << log_size) {
                        return pool.swap_remove(i);
                    }
                }
            }
        }
        // Fresh buffers close to the largest size seen are allocated with room for the next
        // size: the capacity is only reserved, its pages are not touched before they are used.
        let capacity_log_size =
            if log_size + 3 >= largest { (log_size + 1).min(largest) } else { log_size };
        unsafe {
            let mut buffer = Col::<B, BaseField>::uninitialized(1 << capacity_log_size);
            if capacity_log_size > log_size && !buffer.truncate(1 << log_size) {
                buffer = Col::<B, BaseField>::uninitialized(1 << log_size);
            }
            buffer
        }
    }

    /// Drops the idle buffers that are more than two log sizes below the largest size requested
    /// so far, and keeps the larger ones.
    ///
    /// Called when a proof starts on a pool another proof has filled: the large buffers are the
    /// ones worth keeping resident, since the new proof's largest columns take them as they are
    /// or grow into them, while the many small and mid-size ones hold gigabytes that it rarely
    /// asks for and that are cheap to allocate again.
    pub fn release_small_idle(&self) {
        let largest = self.largest_log_size.load(Ordering::Relaxed);
        let keep_from = largest.saturating_sub(2);
        for mut entry in self.pools.iter_mut() {
            if *entry.key() < keep_from {
                entry.value_mut().clear();
            }
        }
    }

    /// Drops every idle buffer: nothing of the finished proof stays resident while the next one
    /// builds its context; its columns are allocated afresh, on huge pages.
    pub fn release_all_idle(&self) {
        for mut entry in self.pools.iter_mut() {
            entry.value_mut().clear();
        }
    }

    /// Returns a buffer to the pool. The caller is responsible for ensuring the buffer's log_size
    /// matches.
    pub fn give_back(&self, log_size: u32, buf: Col<B, BaseField>) {
        debug_assert_eq!(buf.len(), 1 << log_size);
        self.pools.entry(log_size).or_default().push(buf);
    }
}

impl<B: ColumnOps<BaseField>> Default for BaseColumnPool<B> {
    fn default() -> Self {
        Self::new()
    }
}
