use std::fmt::Debug;

pub use cpu::CpuBackend;

use crate::core::channel::MerkleChannel;
use crate::core::fields::m31::BaseField;
use crate::core::fields::qm31::SecureField;
use crate::core::proof_of_work::GrindOps;
use crate::prover::fri::FriOps;
use crate::prover::lookups::gkr_prover::GkrOps;
use crate::prover::poly::circle::PolyOps;
use crate::prover::vcs_lifted::ops::MerkleOpsLifted;
use crate::prover::{AccumulationOps, QuotientOps};

pub mod cpu;
pub mod simd;

pub trait Backend:
    Copy
    + Clone
    + Debug
    + ColumnOps<BaseField>
    + ColumnOps<SecureField>
    + PolyOps
    + QuotientOps
    + FriOps
    + AccumulationOps
    + GkrOps
{
}

pub trait BackendForChannel<MC: MerkleChannel>:
    Backend + MerkleOpsLifted<MC::H> + GrindOps<MC::C>
{
}

pub trait ColumnOps<T> {
    type Column: Column<T>;
    fn bit_reverse_column(column: &mut Self::Column);
}

pub type Col<B, T> = <B as ColumnOps<T>>::Column;

// TODO(alont): Consider removing the generic parameter and only support BaseField.
pub trait Column<T>: Clone + Debug + FromIterator<T> + Send + Sync {
    /// Creates a new column of zeros with the given length.
    fn zeros(len: usize) -> Self;
    /// Creates a new column of uninitialized values with the given length.
    /// # Safety
    /// The caller must ensure that the column is populated before being used.
    unsafe fn uninitialized(len: usize) -> Self;
    /// Returns a cpu vector of the column.
    fn to_cpu(&self) -> Vec<T>;
    /// Returns the length of the column.
    fn len(&self) -> usize;
    /// Returns true if the column is empty.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
    /// Retrieves the element at the given index.
    fn at(&self, index: usize) -> T;
    /// Sets the element at the given index.
    fn set(&mut self, index: usize, value: T);
    /// Splits the column into two halves.
    fn split_at_mid(self) -> (Self, Self);
    /// Shortens the column to `len` elements while keeping its allocation, and returns whether
    /// it did; a backend that does not support it leaves the column as it was.
    fn truncate(&mut self, _len: usize) -> bool {
        false
    }
    /// Lengthens the column to `len` elements within its existing allocation, the new elements
    /// being uninitialized as after [`Self::uninitialized`] (the caller writes them before reading
    /// them), and returns whether it did; a backend that does not support it leaves the column as
    /// it was.
    fn grow(&mut self, _len: usize) -> bool {
        false
    }
    /// Lets the pages behind the column's spare capacity (past its length) stop being resident
    /// while keeping the allocation, so that a shortened column can still grow back into it; a
    /// backend that does not support it does nothing.
    fn release_spare(&mut self) {}
    /// May discard physical pages containing only initialized literal-zero storage, preserving
    /// every logical value. Backends without proven private anonymous backing do nothing.
    fn release_zeroed_pages(&mut self) {}
}
