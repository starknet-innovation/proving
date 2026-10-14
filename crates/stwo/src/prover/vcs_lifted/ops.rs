use serde::{Deserialize, Serialize};

use crate::core::fields::m31::BaseField;
use crate::core::fields::qm31::SECURE_EXTENSION_DEGREE;
use crate::core::vcs_lifted::merkle_hasher::MerkleHasherLifted;
use crate::core::vcs_lifted::verifier::PACKED_LEAF_SIZE;
use crate::prover::backend::{Col, Column, ColumnOps};

/// Trait for performing Merkle operations on a commitment scheme.
pub trait MerkleOpsLifted<H: MerkleHasherLifted>:
    ColumnOps<BaseField> + ColumnOps<H::Hash> + PackLeavesOps + for<'de> Deserialize<'de> + Serialize
{
    /// Computes the leaves of the lifted Merkle commitment.
    fn build_leaves(columns: &[&Col<Self, BaseField>], lifting_log_size: u32)
    -> Col<Self, H::Hash>;

    /// Given a layer of hashes as input, computes a new layer by hashing pairs
    /// of adjacent elements of the input, as in a standard Merkle tree.
    fn build_next_layer(prev_layer: &Col<Self, H::Hash>) -> Col<Self, H::Hash>;

    /// Computes all the layers of the lifted Merkle commitment, sorted by increasing height: the
    /// leaves, followed by `lifting_log_size` layers, each one built from the previous one. A
    /// backend may override it to compute several layers in a single pass.
    fn build_layers(
        columns: &[&Col<Self, BaseField>],
        lifting_log_size: u32,
    ) -> Vec<Col<Self, H::Hash>> {
        let mut layers = vec![Self::build_leaves(columns, lifting_log_size)];
        for _ in 0..lifting_log_size {
            layers.push(Self::build_next_layer(layers.last().unwrap()));
        }
        layers
    }

    /// Like [`Self::build_layers`], but a backend may leave the lowest layers empty (of length
    /// 0) instead of materializing them: the prover recomputes the few hashes of those layers
    /// that a decommitment needs with [`Self::leaf_hashes_at`] and
    /// [`MerkleHasherLifted::hash_children`]. The layers that are kept are identical to the ones
    /// [`Self::build_layers`] builds.
    fn build_layers_sparse(
        columns: &[&Col<Self, BaseField>],
        lifting_log_size: u32,
    ) -> Vec<Col<Self, H::Hash>> {
        Self::build_layers(columns, lifting_log_size)
    }

    /// Builds packed-leaf layers directly from four coordinate columns where supported.
    fn build_packed_layers(
        columns: &[&Col<Self, BaseField>; SECURE_EXTENSION_DEGREE],
        lifting_log_size: u32,
    ) -> Vec<Col<Self, H::Hash>> {
        let packed = Self::pack_leaves_input(columns);
        Self::build_layers_sparse(&packed.iter().collect::<Vec<_>>(), lifting_log_size)
    }

    /// Rebuilds selected packed leaves from the four retained coordinate columns.
    fn packed_leaf_hashes_at(
        columns: &[&Col<Self, BaseField>],
        lifting_log_size: u32,
        positions: &[usize],
    ) -> Vec<H::Hash> {
        if positions.is_empty() {
            return Vec::new();
        }
        assert_eq!(columns.len(), SECURE_EXTENSION_DEGREE);
        let input_len = columns[0].len();
        assert!(columns.iter().all(|c| c.len() == input_len));
        let log_ratio = lifting_log_size - (input_len / PACKED_LEAF_SIZE).ilog2();
        // Gather lifted positions first, then hash equal-size compact columns. The CPU
        // backend requires at least two rows; repeated padding hashes are discarded.
        let len = positions.len().next_power_of_two().max(2);
        let packed: [Col<Self, BaseField>; SECURE_EXTENSION_DEGREE * PACKED_LEAF_SIZE] =
            core::array::from_fn(|i| {
                let coord = i % SECURE_EXTENSION_DEGREE;
                let offset = i / SECURE_EXTENSION_DEGREE;
                (0..len)
                    .map(|j| {
                        let position = positions[j.min(positions.len() - 1)];
                        let row = (position >> (log_ratio + 1) << 1) + (position & 1);
                        columns[coord].at(row * PACKED_LEAF_SIZE + offset)
                    })
                    .collect()
            });
        let leaves = Self::build_leaves(&packed.iter().collect::<Vec<_>>(), len.ilog2());
        (0..positions.len()).map(|i| leaves.at(i)).collect()
    }

    /// The leaves at `positions` of the tree that [`Self::build_leaves`] builds for `columns`
    /// (sorted increasingly by length, as for [`Self::build_leaves`]) and `lifting_log_size`.
    fn leaf_hashes_at(
        columns: &[&Col<Self, BaseField>],
        lifting_log_size: u32,
        positions: &[usize],
    ) -> Vec<H::Hash> {
        let leaves = Self::build_leaves(columns, lifting_log_size);
        positions.iter().map(|&position| leaves.at(position)).collect()
    }
}

pub trait PackLeavesOps: ColumnOps<BaseField> {
    /// Given a column of QM31s (represented as 4 columns of M31s), reshapes it into 4 columns of
    /// QM31s (represented as 16 columns of M31s). Denoting the input column as [v₀, v₁, v₂, v₃,
    /// ...] where vᵢ ∈ QM31, the output is [[v₀, v₄, v₈, ...], [v₁, v₅, v₉, ...], [v₂, v₆, v₁₀,
    /// ...], [v₃, v₇, v₁₁, ...]].
    fn pack_leaves_input(
        values: &[&Col<Self, BaseField>; SECURE_EXTENSION_DEGREE],
    ) -> [Col<Self, BaseField>; SECURE_EXTENSION_DEGREE * PACKED_LEAF_SIZE];
}
