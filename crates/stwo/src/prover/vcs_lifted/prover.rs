use hashbrown::HashMap;
use itertools::Itertools;
#[cfg(feature = "parallel")]
use rayon::prelude::*;
use tracing::{Level, span};

use super::ops::MerkleOpsLifted;
use crate::core::ColumnVec;
use crate::core::fields::m31::BaseField;
use crate::core::fields::qm31::SECURE_EXTENSION_DEGREE;
use crate::core::vcs_lifted::hasher::Hasher;
use crate::core::vcs_lifted::merkle_hasher::MerkleHasherLifted;
use crate::core::vcs_lifted::verifier::{
    ExtendedMerkleDecommitmentLifted, MerkleDecommitmentLifted, MerkleDecommitmentLiftedAux,
};
use crate::parallel_iter;
use crate::prover::backend::{Col, Column};

/// Represents the prover side of a Merkle commitment scheme.
#[derive(Debug)]
pub struct MerkleProverLifted<B: MerkleOpsLifted<H>, H: MerkleHasherLifted> {
    /// Layers of the Merkle tree, sorted by increasing length.
    /// The first layer is a column of length 1, containing the root commitment.
    pub layers: Vec<Col<B, H::Hash>>,
}

impl<B: MerkleOpsLifted<H>, H: MerkleHasherLifted> MerkleProverLifted<B, H> {
    /// Commits to columns.
    /// Columns must be of power of 2 sizes, not necessarily sorted by length.
    ///
    /// # Arguments
    ///
    /// * `columns` - A vector of references to columns.
    ///
    /// # Returns
    ///
    /// A new instance of `MerkleProverLifted` with the committed layers.
    pub fn commit(
        columns: Vec<&Col<B, BaseField>>,
        lifting_log_size: u32,
        log_rows_per_leaf: u32,
    ) -> Self {
        let _span = span!(Level::TRACE, "Merkle", class = "MerkleCommitment").entered();
        if columns.is_empty() {
            // An empty tree is not lifted: it is committed as a single hash of no data, so its
            // height is 0 and the caller has to name that height. `MerkleVerifierLifted::new`
            // asserts the same, so a nonzero size here would be a tree the verifier rejects.
            assert!(
                lifting_log_size == 0,
                "lifting_log_size must be 0 when no columns are committed"
            );
            return Self { layers: vec![B::build_leaves(&[], lifting_log_size)] };
        }

        // We enter the first branch only during FRI commit phase, in which we commit 4 columns of
        // the same size. In particular, we don't need to sort the columns by size.
        let mut layers: Vec<Col<B, H::Hash>> = if log_rows_per_leaf > 0 {
            // TODO(Leo): add support for higher log_rows_per_leaf sizes.
            assert_eq!(
                log_rows_per_leaf, 2,
                "Leaf packing is only supported for log_rows_per_leaf = 2."
            );
            let columns: [&Col<B, BaseField>; SECURE_EXTENSION_DEGREE] =
                columns.try_into().unwrap();
            let packed_columns = B::pack_leaves_input(&columns);
            let max_log_size = packed_columns[0].len().ilog2();
            assert!(lifting_log_size >= max_log_size);
            B::build_layers(&packed_columns.iter().collect_vec(), lifting_log_size)
        } else {
            let sorted_columns = columns.into_iter().sorted_by_key(|c| c.len()).collect_vec();
            let max_log_size = sorted_columns.last().unwrap().len().ilog2();
            assert!(lifting_log_size >= max_log_size, "{lifting_log_size} < {max_log_size}");
            // The lowest layers may be left empty; `decommit` recomputes the hashes it needs
            // from the columns.
            B::build_layers_sparse(&sorted_columns, lifting_log_size)
        };
        layers.reverse();

        Self { layers }
    }

    /// Decommits to columns on the given queries.
    /// Queries are given as indices to the largest column.
    ///
    /// # Arguments
    ///
    /// * `query_positions` - Vector containing the positions of the queries. Note that the
    ///   positions are not necessarily sorted and may contain duplicates.
    /// * `columns` - A vector of references to columns.
    ///
    /// # Returns
    ///
    /// A tuple containing:
    /// * A vector `v` of queried values, where `v[col_idx][pos_idx]` contains the evaluation of the
    ///   `col_idx`-th column at the `query_position[pos_idx]` position.
    /// * A `MerkleDecommitment` containing the hash witness.
    pub fn decommit(
        &self,
        query_positions: &[usize],
        columns: Vec<&Col<B, BaseField>>,
    ) -> (ColumnVec<Vec<BaseField>>, ExtendedMerkleDecommitmentLifted<H>) {
        // Prepare output buffers.
        let mut decommitment = MerkleDecommitmentLifted::<H>::default();
        let mut all_node_values: Vec<HashMap<usize, <H as Hasher>::Hash>> = vec![];

        // Compute the queried values: a few random reads per column, done column by column in
        // parallel.
        let max_log_size = self.layers.len() - 1;
        let queried_values: ColumnVec<Vec<BaseField>> = parallel_iter!(&columns)
            .map(|col| {
                let log_size = col.len().ilog2() as usize;
                let shift = max_log_size - log_size;
                query_positions
                    .iter()
                    .map(|pos| col.at((pos >> (shift + 1) << 1) + (pos & 1)))
                    .collect()
            })
            .collect();

        let mut prev_layer_queries: Vec<usize> =
            query_positions.iter().copied().sorted().dedup().collect();
        // The lowest layers may not have been materialized by the commitment (see
        // `MerkleOpsLifted::build_layers_sparse`): the hashes of those layers that the
        // decommitment reads are recomputed here, from the columns.
        let n_omitted_layers =
            self.layers.iter().rev().take_while(|layer| layer.is_empty()).count();
        let omitted_layers = self.recompute_omitted_layers(&prev_layer_queries, &columns);
        // The largest log size of a layer is equal to `self.layers.len() - 1`. We start iterating
        // from the layer of log size `self.layers.len() - 2` so that we always have a previous
        // layer available for the computation.
        for layer_log_size in (0..self.layers.len() - 1).rev() {
            let mut all_node_values_for_layer = HashMap::<usize, <H as Hasher>::Hash>::new();
            // Prepare write buffer for queries to the current layer. This will propagate to the
            // next layer.
            let mut curr_layer_queries: Vec<usize> = vec![];

            // Each layer node is a hash of column values as previous layer hashes.
            // Prepare the previous layer hashes to read from.
            let prev_layer_hashes = self.layers.get(layer_log_size + 1).unwrap();
            // The height of the previous layer above the leaves.
            let prev_layer_level = max_log_size - (layer_log_size + 1);
            let prev_layer_hash_at = |index: usize| {
                if prev_layer_level < n_omitted_layers {
                    omitted_layers[prev_layer_level][&index]
                } else {
                    prev_layer_hashes.at(index)
                }
            };
            // All chunks have either length 1 (only one child is present) or 2 (both children are
            // present).
            for queries_chunk in prev_layer_queries.as_slice().chunk_by(|a, b| a ^ 1 == *b) {
                let first = queries_chunk[0];
                // If the brother of `first` was not queried before, add its hash to the witness.
                if queries_chunk.len() == 1 {
                    decommitment.hash_witness.push(prev_layer_hash_at(first ^ 1))
                }
                let curr_index = first >> 1;
                curr_layer_queries.push(curr_index);

                // Add the previous layer hashes to all_node_values.
                all_node_values_for_layer
                    .insert(2 * curr_index, prev_layer_hash_at(2 * curr_index));
                all_node_values_for_layer
                    .insert(2 * curr_index + 1, prev_layer_hash_at(2 * curr_index + 1));
            }
            // Propagate queries to the next layer.
            prev_layer_queries = curr_layer_queries;

            all_node_values.push(all_node_values_for_layer);
        }
        (
            queried_values,
            ExtendedMerkleDecommitmentLifted {
                decommitment,
                aux: MerkleDecommitmentLiftedAux { all_node_values },
            },
        )
    }

    /// Recomputes, for every layer that the commitment left empty (see
    /// [`MerkleOpsLifted::build_layers_sparse`]), the hashes that the decommitment of the sorted
    /// and deduplicated `query_positions` reads: both children of every node on a query's path.
    /// Entry `level` of the result maps the positions of the layer `level` above the leaves.
    fn recompute_omitted_layers(
        &self,
        query_positions: &[usize],
        columns: &[&Col<B, BaseField>],
    ) -> Vec<HashMap<usize, H::Hash>> {
        let n_omitted = self.layers.iter().rev().take_while(|layer| layer.is_empty()).count();
        if n_omitted == 0 || query_positions.is_empty() {
            return vec![];
        }
        let lifting_log_size = self.layers.len() as u32 - 1;
        // The leaves that the recomputation starts from: for every query, the aligned block of
        // `1 << n_omitted` leaves around it, which holds the sibling of every node on the query's
        // path through the omitted layers.
        let leaf_positions: Vec<usize> = query_positions
            .iter()
            .map(|position| position >> n_omitted)
            .dedup()
            .flat_map(|block| (block << n_omitted)..((block + 1) << n_omitted))
            .collect();
        // The leaves are hashed from the columns in the order the commitment hashed them.
        let sorted_columns = columns.iter().copied().sorted_by_key(|c| c.len()).collect_vec();
        let leaves = B::leaf_hashes_at(&sorted_columns, lifting_log_size, &leaf_positions);
        let mut layers: Vec<HashMap<usize, H::Hash>> =
            vec![HashMap::from_iter(leaf_positions.iter().copied().zip(leaves))];
        for level in 1..n_omitted {
            let prev = &layers[level - 1];
            let nodes = prev
                .keys()
                .map(|position| position >> 1)
                .sorted()
                .dedup()
                .map(|position| {
                    let children = (prev[&(2 * position)], prev[&(2 * position + 1)]);
                    (position, H::hash_children(children))
                })
                .collect();
            layers.push(nodes);
        }
        layers
    }

    pub fn root(&self) -> H::Hash {
        self.layers.first().unwrap().at(0)
    }
}

#[cfg(test)]
mod test {
    use num_traits::Zero;

    use super::*;
    use crate::core::fields::m31::M31;
    use crate::core::poly::circle::CanonicCoset;
    use crate::core::vcs::blake2_hash::{Blake2sHash, Blake2sHasher};
    use crate::core::vcs::blake2_merkle::Blake2sMerkleHasher as Blake2sMerkleHasherCurrent;
    use crate::core::vcs_lifted::blake2_merkle::Blake2sMerkleHasher;
    use crate::core::vcs_lifted::test_utils::lift_poly;
    use crate::prover::backend::CpuBackend;
    use crate::prover::backend::cpu::CpuCirclePoly;
    use crate::prover::vcs::prover::MerkleProver;

    #[test]
    fn test_empty_cols() {
        // Check Merkle commitment on empty columns.
        let mixed_degree_merkle_prover =
            MerkleProver::<CpuBackend, Blake2sMerkleHasherCurrent>::commit(vec![]);
        let lifted_merkle_prover =
            MerkleProverLifted::<CpuBackend, Blake2sMerkleHasher>::commit(vec![], 0, 0);
        assert_eq!(mixed_degree_merkle_prover.layers, lifted_merkle_prover.layers);
    }

    fn prepare_merkle() -> (Vec<Vec<BaseField>>, MerkleProverLifted<CpuBackend, Blake2sHasher>) {
        let max_log_size = 4;
        let columns: Vec<Vec<BaseField>> = (2..=max_log_size)
            .map(|i| (0..1 << i).map(M31::from_u32_unchecked).collect())
            .collect();
        let merkle_prover = MerkleProverLifted::<CpuBackend, Blake2sHasher>::commit(
            columns.iter().collect(),
            max_log_size,
            0,
        );
        (columns, merkle_prover)
    }

    #[test]
    fn test_lifted_merkle_leaves() {
        let (_, merkle_prover) = prepare_merkle();
        let leaves = &merkle_prover.layers.last().unwrap();

        // Compute the expected first leaf.
        let mut hasher = Blake2sHasher::default();
        let data = [0u8; 12];
        hasher.update(&data);
        assert_eq!(hasher.finalize(), leaves[0]);

        // Compute the expected fifth leaf.
        let mut hasher = Blake2sHasher::default();
        let mut data = Vec::new();
        data.extend(0_u32.to_le_bytes());
        data.extend(2_u32.to_le_bytes());
        data.extend(4_u32.to_le_bytes());
        hasher.update(&data);
        assert_eq!(hasher.finalize(), leaves[4]);

        // Compute the expected last leaf.
        let mut hasher = Blake2sHasher::default();
        let mut data = Vec::new();
        data.extend(3_u32.to_le_bytes());
        data.extend(7_u32.to_le_bytes());
        data.extend(15_u32.to_le_bytes());
        hasher.update(&data);

        assert_eq!(hasher.finalize(), *leaves.last().unwrap());
    }

    #[test]
    fn test_lifted_decommitted_values() {
        let (cols, merkle_prover) = prepare_merkle();
        // Test decommits at position 0.
        let queried_values = merkle_prover.decommit(&[0], cols.iter().collect_vec()).0;

        let expected_values = vec![vec![BaseField::zero()]; 3];
        assert_eq!(expected_values, queried_values);

        // Test decommits at position 4.
        let queried_values = merkle_prover.decommit(&[4], cols.iter().collect_vec()).0;
        let expected_values = vec![
            vec![BaseField::from_u32_unchecked(0)],
            vec![BaseField::from_u32_unchecked(2)],
            vec![BaseField::from_u32_unchecked(4)],
        ];
        assert_eq!(expected_values, queried_values);

        // Test decommits at position 15.
        let queried_values = merkle_prover.decommit(&[15], cols.iter().collect_vec()).0;
        let expected_values = vec![
            vec![BaseField::from_u32_unchecked(3)],
            vec![BaseField::from_u32_unchecked(7)],
            vec![BaseField::from_u32_unchecked(15)],
        ];
        assert_eq!(expected_values, queried_values);
    }

    /// See the docs of `[crate::prover::backend::cpu::blake2s_lifted::build_leaves]`.
    #[test]
    fn test_bit_reverse_lifted_merkle_cpu() {
        const LOG_SIZE: u32 = 3;
        const LIFTED_LOG_SIZE: u32 = 9;
        let domain = CanonicCoset::new(LOG_SIZE).circle_domain();
        let poly = CpuCirclePoly::new((0..1 << LOG_SIZE).map(BaseField::from).collect());
        let lifted_evaluation = lift_poly(&poly, LIFTED_LOG_SIZE);

        let last_column: Col<CpuBackend, BaseField> =
            (0..1 << LIFTED_LOG_SIZE).map(|_| M31::zero()).collect_vec();

        let mixed_degree_merkle_prover =
            MerkleProver::<CpuBackend, Blake2sMerkleHasherCurrent>::commit(vec![
                &lifted_evaluation.values,
                &last_column,
            ]);
        let lifted_merkle_prover_1 = MerkleProverLifted::<CpuBackend, Blake2sMerkleHasher>::commit(
            vec![&lifted_evaluation.values, &last_column],
            LIFTED_LOG_SIZE,
            0,
        );
        let lifted_merkle_prover_2 = MerkleProverLifted::<CpuBackend, Blake2sMerkleHasher>::commit(
            vec![&poly.evaluate(domain), &last_column],
            LIFTED_LOG_SIZE,
            0,
        );

        assert_eq!(lifted_merkle_prover_1.root(), lifted_merkle_prover_2.root());
        assert_eq!(mixed_degree_merkle_prover.root(), lifted_merkle_prover_1.root());
    }

    #[test]
    fn test_decommitment_aux() {
        let (columns, merkle_prover) = prepare_merkle();
        let (_, ExtendedMerkleDecommitmentLifted { decommitment: _, aux }) =
            merkle_prover.decommit(&[1], columns.iter().collect_vec());

        let mut expected: Vec<HashMap<usize, Blake2sHash>> = vec![];
        merkle_prover
            .layers
            .iter()
            .skip(1)
            .rev()
            .for_each(|layer| expected.push(HashMap::from_iter([(0, layer[0]), (1, layer[1])])));
        assert_eq!(expected, aux.all_node_values);
    }
}
