use std::array;
use std::collections::HashMap;
use std::simd::u32x16;

use bytemuck::{cast_slice, cast_slice_mut};
use itertools::Itertools;
use num_traits::Zero;
#[cfg(feature = "parallel")]
use rayon::prelude::*;

use super::SimdBackend;
use super::m31::LOG_N_LANES;
use super::utils::to_lifted_simd;
use crate::core::fields::m31::{BaseField, N_BYTES_FELT};
use crate::core::fields::qm31::SECURE_EXTENSION_DEGREE;
use crate::core::utils::uninit_vec;
use crate::core::vcs::blake2_hash::Blake2sHash;
use crate::core::vcs_lifted::blake2_merkle::Blake2sMerkleHasher;
use crate::core::vcs_lifted::merkle_hasher::MerkleHasherLifted;
use crate::core::vcs_lifted::verifier::PACKED_LEAF_SIZE;
use crate::parallel_iter;
use crate::prover::backend::simd::blake2s::{
    INITIAL_STATE, compress_finalize, compress_unfinalized, transpose_msgs, untranspose_states,
};
use crate::prover::backend::simd::column::BaseColumn;
use crate::prover::backend::simd::m31::{N_LANES, PackedBaseField};
use crate::prover::backend::simd::utils::transpose_packed_leaf;
use crate::prover::backend::{Col, Column, CpuBackend};
use crate::prover::vcs_lifted::ops::{MerkleOpsLifted, PackLeavesOps};

const N_FELTS_IN_BLAKE_MESSAGE: usize = 16;
const N_FELTS_IN_BLAKE_STATE: usize = 8;
const N_BYTES_IN_BLAKE_MESSAGE: u64 = N_FELTS_IN_BLAKE_MESSAGE as u64 * N_BYTES_FELT as u64;
const LOG_N_HASHES_PER_SIMD_STATE: u32 = 4;
/// Number of packed output rows that are handled by a task of [`SimdBackend::pack_leaves_input`].
const ROWS_PER_PACK_TASK: usize = 512;

impl MerkleOpsLifted<Blake2sMerkleHasher> for SimdBackend {
    /// See the docs of [`crate::prover::backend::cpu::blake2s_lifted`].
    ///
    /// This function assumes that `columns` is sorted increasingly by column length.
    ///
    /// # Note
    ///
    /// If the length of a smallest column (e.g. the first) is smaller than `N_LANES`, the
    /// implementation falls back to the CPU implementation.
    #[allow(clippy::uninit_vec)]
    fn build_leaves(
        columns: &[&Col<Self, BaseField>],
        lifting_log_size: u32,
    ) -> Col<Self, Blake2sHash> {
        if columns.is_empty() {
            let hasher = Blake2sMerkleHasher::default();
            return vec![hasher.finalize()];
        }
        if columns.first().unwrap().len() < N_LANES {
            let cpu_cols = columns.iter().map(|column| column.to_cpu()).collect_vec();
            return <CpuBackend as MerkleOpsLifted<Blake2sMerkleHasher>>::build_leaves(
                &cpu_cols.iter().collect_vec(),
                lifting_log_size,
            );
        }
        let plan = LeavesPlan::new(columns);

        // Every state of the largest domain stands for `1 << extra_log_ratio` states of the
        // lifted domain.
        let lifting_log_size_packed = lifting_log_size - LOG_N_LANES;
        let extra_log_ratio = lifting_log_size_packed - plan.max_log_size;
        let hashes_per_state = N_HASHES_PER_SIMD_STATE << extra_log_ratio;
        // Safety: we never read from `res`, only write to it.
        let mut res: Vec<Blake2sHash> =
            unsafe { uninit_vec(1 << (lifting_log_size_packed + LOG_N_HASHES_PER_SIMD_STATE)) };

        chunks_mut_iter(&mut res, TILE * hashes_per_state).enumerate().for_each(
            |(tile_index, dst_tile)| {
                let first_state = tile_index * TILE;
                let mut tile_buffer = [INITIAL_STATE; TILE];
                let tile = &mut tile_buffer[..dst_tile.len() / hashes_per_state];
                plan.compute_tile(first_state, tile);
                // Lift the states if needed and untranspose them.
                for (k, state) in tile.iter().enumerate() {
                    let dst = &mut dst_tile[k * hashes_per_state..(k + 1) * hashes_per_state];
                    for (lifted_offset, dst) in
                        dst.chunks_exact_mut(N_HASHES_PER_SIMD_STATE).enumerate()
                    {
                        let lifted_index = ((first_state + k) << extra_log_ratio) + lifted_offset;
                        let lifted: [u32x16; N_FELTS_IN_BLAKE_STATE] = array::from_fn(|j| {
                            to_lifted_simd(state[j], extra_log_ratio, lifted_index)
                        });
                        store_states(untranspose_states(lifted), dst);
                    }
                }
            },
        );

        res
    }

    #[allow(clippy::uninit_vec)]
    fn build_next_layer(prev_layer: &Vec<Blake2sHash>) -> Vec<Blake2sHash> {
        // The log size of the current layer that needs to be built.
        let log_size: u32 = prev_layer.len().ilog2() - 1;
        if log_size < LOG_N_LANES {
            return parallel_iter!(0..1 << log_size)
                .map(|i| {
                    Blake2sMerkleHasher::hash_children((prev_layer[2 * i], prev_layer[2 * i + 1]))
                })
                .collect();
        }
        // Safety: no index in `res` is ever read without having been written to
        // before.
        let mut res: Vec<Blake2sHash> = unsafe { uninit_vec(1 << log_size) };

        chunks_mut_iter(&mut res, 1 << LOG_N_LANES).enumerate().for_each(|(i, dst)| {
            let state = INITIAL_STATE;
            let prev_chunk_u32s = cast_slice::<_, u32>(&prev_layer[(i << 5)..((i + 1) << 5)]);
            let msgs: [u32x16; N_FELTS_IN_BLAKE_MESSAGE] = array::from_fn(|j| {
                u32x16::from_array(std::array::from_fn(|k| prev_chunk_u32s[16 * j + k]))
            });
            let state = compress_finalize(state, transpose_msgs(msgs), N_BYTES_IN_BLAKE_MESSAGE);
            store_states(untranspose_states(state), dst);
        });
        res
    }

    /// Builds the leaves and the first [`FUSED_LEVELS`] inner layers together: every tile of
    /// [`TILE`] states is hashed, and then reduced to a single state, while it is still resident in
    /// the L1 cache, instead of being written, read back, and transposed once per layer.
    #[allow(clippy::uninit_vec)]
    fn build_layers(
        columns: &[&Col<Self, BaseField>],
        lifting_log_size: u32,
    ) -> Vec<Col<Self, Blake2sHash>> {
        if !fusable(columns, lifting_log_size) {
            let mut layers = vec![<Self as MerkleOpsLifted<Blake2sMerkleHasher>>::build_leaves(
                columns,
                lifting_log_size,
            )];
            for _ in 0..lifting_log_size {
                layers.push(<Self as MerkleOpsLifted<Blake2sMerkleHasher>>::build_next_layer(
                    layers.last().unwrap(),
                ));
            }
            return layers;
        }
        let plan = LeavesPlan::new(columns);

        // Safety: no index in the layers is ever read without having been written to before.
        let [mut l0, mut l1, mut l2, mut l3, mut l4, mut l5]: [Vec<Blake2sHash>; 6] =
            array::from_fn(|level| unsafe { uninit_vec(1 << (lifting_log_size - level as u32)) });
        let chunk = N_HASHES_PER_SIMD_STATE;
        chunks_mut_iter(&mut l0, chunk * TILE)
            .zip(chunks_mut_iter(&mut l1, chunk * TILE / 2))
            .zip(chunks_mut_iter(&mut l2, chunk * TILE / 4))
            .zip(chunks_mut_iter(&mut l3, chunk * TILE / 8))
            .zip(chunks_mut_iter(&mut l4, chunk * TILE / 16))
            .zip(chunks_mut_iter(&mut l5, chunk * TILE / 32))
            .enumerate()
            .for_each(|(tile_index, (((((d0, d1), d2), d3), d4), d5))| {
                let mut states = [INITIAL_STATE; TILE];
                plan.compute_tile(tile_index * TILE, &mut states);
                let mut n_states = TILE;
                for (level, dst) in [d0, d1, d2, d3, d4, d5].into_iter().enumerate() {
                    if level > 0 {
                        // Hash pairs of states into the states of the next layer.
                        n_states /= 2;
                        for k in 0..n_states {
                            states[k] = hash_state_pair(&states[2 * k], &states[2 * k + 1]);
                        }
                    }
                    for (state, dst) in states[..n_states].iter().zip(dst.chunks_exact_mut(chunk)) {
                        store_states(untranspose_states(*state), dst);
                    }
                }
            });

        let mut layers: Vec<Vec<Blake2sHash>> = vec![l0, l1, l2, l3, l4, l5];
        for _ in FUSED_LEVELS..lifting_log_size {
            layers.push(<Self as MerkleOpsLifted<Blake2sMerkleHasher>>::build_next_layer(
                layers.last().unwrap(),
            ));
        }
        layers
    }

    /// Like [`Self::build_layers`], but the leaves and the first [`FUSED_LEVELS`] inner layers,
    /// which the tile pass only ever writes, are left empty: a decommitment reads a few dozen
    /// hashes of them, which [`Self::leaf_hashes_at`] recomputes from the columns. Every stored
    /// layer is identical to the one [`Self::build_layers`] builds.
    fn build_layers_sparse(
        columns: &[&Col<Self, BaseField>],
        lifting_log_size: u32,
    ) -> Vec<Col<Self, Blake2sHash>> {
        if !fusable(columns, lifting_log_size) {
            return <Self as MerkleOpsLifted<Blake2sMerkleHasher>>::build_layers(
                columns,
                lifting_log_size,
            );
        }
        let plan = LeavesPlan::new(columns);

        // Every tile of `TILE` states is reduced to one state of the layer `FUSED_LEVELS`.
        let mut top: Vec<Blake2sHash> =
            vec![Blake2sHash::default(); 1 << (lifting_log_size - FUSED_LEVELS)];
        chunks_mut_iter(&mut top, N_HASHES_PER_SIMD_STATE).enumerate().for_each(
            |(tile_index, dst)| {
                let mut states = [INITIAL_STATE; TILE];
                plan.compute_tile(tile_index * TILE, &mut states);
                let mut n_states = TILE;
                while n_states > 1 {
                    // Hash pairs of states into the states of the next layer.
                    n_states /= 2;
                    for k in 0..n_states {
                        states[k] = hash_state_pair(&states[2 * k], &states[2 * k + 1]);
                    }
                }
                store_states(untranspose_states(states[0]), dst);
            },
        );

        let mut layers: Vec<Vec<Blake2sHash>> = (0..FUSED_LEVELS).map(|_| Vec::new()).collect();
        layers.push(top);
        for _ in FUSED_LEVELS..lifting_log_size {
            layers.push(<Self as MerkleOpsLifted<Blake2sMerkleHasher>>::build_next_layer(
                layers.last().unwrap(),
            ));
        }
        layers
    }

    /// See [`MerkleOpsLifted::leaf_hashes_at`]: the leaves are recomputed one packed row at a
    /// time, through every pass, without the full-size state buffers of [`Self::build_leaves`].
    fn leaf_hashes_at(
        columns: &[&Col<Self, BaseField>],
        lifting_log_size: u32,
        positions: &[usize],
    ) -> Vec<Blake2sHash> {
        if columns.is_empty() || columns.first().unwrap().len() < N_LANES {
            // The shapes that `build_leaves` hands to the CPU backend.
            let leaves = <Self as MerkleOpsLifted<Blake2sMerkleHasher>>::build_leaves(
                columns,
                lifting_log_size,
            );
            return positions.iter().map(|&position| leaves[position]).collect();
        }
        let plan = LeavesPlan::new_lazy(columns);
        // Every state of the largest domain stands for `1 << extra_log_ratio` states of the
        // lifted domain, as in `build_leaves`.
        let extra_log_ratio = lifting_log_size - LOG_N_LANES - plan.max_log_size;
        let state_indices: Vec<usize> = positions
            .iter()
            .map(|&position| position >> (LOG_N_LANES + extra_log_ratio))
            .sorted()
            .dedup()
            .collect();
        let states: HashMap<usize, [u32x16; N_FELTS_IN_BLAKE_STATE]> =
            parallel_iter!(&state_indices)
                .map(|&state_index| (state_index, plan.compute_state(state_index)))
                .collect();
        positions
            .iter()
            .map(|&position| {
                let lifted_index = position >> LOG_N_LANES;
                let state_index = lifted_index >> extra_log_ratio;
                let state = states[&state_index];
                let lifted: [u32x16; N_FELTS_IN_BLAKE_STATE] =
                    array::from_fn(|j| to_lifted_simd(state[j], extra_log_ratio, lifted_index));
                // `store_states` writes the hashes `2k` and `2k + 1` from the `k`-th untransposed
                // vector.
                let lane = position % N_HASHES_PER_SIMD_STATE;
                let words = untranspose_states(lifted)[lane / 2].to_array();
                let offset = (lane % 2) * N_FELTS_IN_BLAKE_STATE;
                let mut hash = Blake2sHash::default();
                for (dst, word) in
                    hash.0.chunks_exact_mut(N_BYTES_FELT).zip(&words[offset..offset + 8])
                {
                    dst.copy_from_slice(&word.to_le_bytes());
                }
                hash
            })
            .collect()
    }
}

/// Whether [`MerkleOpsLifted::build_layers`] can hash `columns` tile by tile: the columns are
/// large enough for the SIMD leaves, the tree is tall enough for the fused levels, and no lifting
/// is needed beyond the largest column.
fn fusable(columns: &[&Col<SimdBackend, BaseField>], lifting_log_size: u32) -> bool {
    !columns.is_empty()
        && columns.first().unwrap().len() >= N_LANES
        && lifting_log_size >= LOG_N_LANES + FUSED_LEVELS
        && columns.last().unwrap().data.len().ilog2() + LOG_N_LANES == lifting_log_size
}

/// Number of inner layers that are built together with the leaves by
/// [`MerkleOpsLifted::build_layers`]: a tile of `1 << FUSED_LEVELS` states is reduced to one state.
const FUSED_LEVELS: u32 = 5;
/// Number of consecutive packed states that go through all the message blocks of a pass before the
/// next tile is processed.
const TILE: usize = 1 << FUSED_LEVELS;
/// Tile size of the passes that work on a smaller domain than the largest one.
const LOWER_TILE: usize = 64;
const N_HASHES_PER_SIMD_STATE: usize = 1 << LOG_N_HASHES_PER_SIMD_STATE;

#[cfg(feature = "parallel")]
fn chunks_mut_iter<T: Send>(
    slice: &mut [T],
    chunk_size: usize,
) -> impl IndexedParallelIterator<Item = &mut [T]> {
    slice.par_chunks_mut(chunk_size)
}

#[cfg(not(feature = "parallel"))]
fn chunks_mut_iter<T>(
    slice: &mut [T],
    chunk_size: usize,
) -> impl ExactSizeIterator<Item = &mut [T]> {
    slice.chunks_mut(chunk_size)
}

/// A group of columns `columns[start..end]` that is hashed on a domain of log size `log_size`: the
/// first 16 columns may be smaller than the others (they are lifted), while the columns
/// `columns[start + 16..end]` have all size `1 << log_size`.
struct Pass {
    start: usize,
    end: usize,
    log_size: u32,
    /// The number of bytes that are hashed before this pass.
    byte_count: u64,
    /// Log-ratios of the columns `columns[start..start + 16]` w.r.t. `log_size`.
    msg_log_ratios: [u32; N_FELTS_IN_BLAKE_MESSAGE],
}

/// The hashing of all the columns of a Merkle layer into the leaves, organized by tiles of states.
///
/// The passes over a domain that is smaller than the largest one are computed at construction,
/// one after the other, and their last states are kept. The remaining passes, including the last
/// chunk of columns that finalizes the hashes, are computed by [`Self::compute_tile`], for a tile
/// of states of the largest domain at a time.
struct LeavesPlan<'a> {
    columns: &'a [&'a Col<SimdBackend, BaseField>],
    /// The passes over the largest domain, except for the last chunk.
    top_passes: Vec<Pass>,
    last_chunk_index: usize,
    /// Log-ratios of the columns of the last chunk w.r.t. the largest domain.
    tail_log_ratios: Vec<u32>,
    /// The total number of hashed bytes.
    byte_count: u64,
    /// Log size of the largest domain (in terms of PackedM31).
    max_log_size: u32,
    /// States of the last pass over a smaller domain, and the log size of this domain.
    lower_states: Vec<[u32x16; N_FELTS_IN_BLAKE_STATE]>,
    lower_log_size: Option<u32>,
    /// The passes over a smaller domain than the largest one, in increasing order of log size.
    lower_passes: Vec<Pass>,
}

impl<'a> LeavesPlan<'a> {
    /// Plans the passes without computing any state: [`Self::compute_state`] computes the final
    /// state of a single packed row of the largest domain from scratch.
    fn new_lazy(columns: &'a [&'a Col<SimdBackend, BaseField>]) -> Self {
        Self::plan(columns, false)
    }

    /// Plans the passes and computes the states of the passes over the smaller domains.
    fn new(columns: &'a [&'a Col<SimdBackend, BaseField>]) -> Self {
        Self::plan(columns, true)
    }

    #[allow(clippy::uninit_vec)]
    fn plan(columns: &'a [&'a Col<SimdBackend, BaseField>], compute_lower_states: bool) -> Self {
        // Note that, in this function, all variables that track log sizes
        // refer to the "size" in terms of PackedM31 (e.g. the log size of a column
        // of 4 PackedM31 elements is 2).
        let max_log_size: u32 = columns.last().unwrap().data.len().ilog2();

        // The last column chunk, which requires the `compress_finalize` permutation, is
        // `columns[last_chunk_index..]`.
        let last_chunk_index =
            (columns.len() - 1) / N_FELTS_IN_BLAKE_MESSAGE * N_FELTS_IN_BLAKE_MESSAGE;
        let lifting_indices =
            get_lifting_indices(columns.iter().map(|c| c.data.len()), last_chunk_index);
        let mut byte_count = 0_u64;
        let mut passes: Vec<Pass> = lifting_indices
            .into_iter()
            .tuple_windows()
            .map(|(start, end)| {
                let log_size: u32 = columns[end - 1].data.len().ilog2();
                let pass = Pass {
                    start,
                    end,
                    log_size,
                    byte_count,
                    msg_log_ratios: array::from_fn(|j| {
                        log_size - columns[start + j].data.len().ilog2()
                    }),
                };
                // We hash `((end - start) / N_FELTS_IN_BLAKE_MESSAGE) * N_BYTES_IN_BLAKE_MESSAGE =
                // 4 * (end - start)` bytes.
                byte_count += 4 * (end - start) as u64;
                pass
            })
            .collect();
        let tail_log_ratios: Vec<u32> = columns[last_chunk_index..]
            .iter()
            .map(|column| max_log_size - column.data.len().ilog2())
            .collect();
        byte_count += ((columns.len() - last_chunk_index) * N_BYTES_FELT) as u64;

        // The passes are sorted by increasing log size.
        let n_lower = passes.iter().take_while(|pass| pass.log_size < max_log_size).count();
        let top_passes = passes.split_off(n_lower);
        let lower_passes = passes;
        let lower_len = if compute_lower_states {
            lower_passes.last().map_or(0, |pass| 1usize << pass.log_size)
        } else {
            0
        };
        // We use two buffers to hold the states of the previous and of the current pass. In every
        // iteration, a possibly larger chunk of the buffers is used. This saves memory allocations.
        // Safety: no index in `next_layer_states` and `prev_layer_states` is ever read without
        // having been written to before.
        let mut prev_layer_states: Vec<[u32x16; N_FELTS_IN_BLAKE_STATE]> =
            unsafe { uninit_vec(lower_len) };
        let mut next_layer_states: Vec<[u32x16; N_FELTS_IN_BLAKE_STATE]> =
            unsafe { uninit_vec(lower_len) };

        // `None` stands for the initial state of the hash.
        let mut prev_log_size: Option<u32> = None;
        for pass in lower_passes.iter().take(if compute_lower_states { usize::MAX } else { 0 }) {
            let log_ratio = pass.log_size - prev_log_size.unwrap_or(0);
            let prev_states: &[[u32x16; N_FELTS_IN_BLAKE_STATE]] = &prev_layer_states;
            let next_states = &mut next_layer_states[0..1 << pass.log_size];
            chunks_mut_iter(next_states, LOWER_TILE).enumerate().for_each(|(tile_index, tile)| {
                let first_state = tile_index * LOWER_TILE;
                init_tile(tile, first_state, prev_log_size.map(|_| (prev_states, log_ratio)));
                hash_pass_tile(columns, pass, first_state, tile);
            });
            std::mem::swap(&mut prev_layer_states, &mut next_layer_states);
            prev_log_size = Some(pass.log_size);
        }

        Self {
            columns,
            top_passes,
            last_chunk_index,
            tail_log_ratios,
            byte_count,
            max_log_size,
            lower_states: prev_layer_states,
            lower_log_size: prev_log_size,
            lower_passes,
        }
    }

    /// Computes the final state of the packed row `state_index` of the largest domain from
    /// scratch, through every pass; the lazily planned counterpart of [`Self::compute_tile`].
    fn compute_state(&self, state_index: usize) -> [u32x16; N_FELTS_IN_BLAKE_STATE] {
        let mut state = INITIAL_STATE;
        let mut prev_log_size: Option<u32> = None;
        for pass in self.lower_passes.iter().chain(&self.top_passes) {
            let index = state_index >> (self.max_log_size - pass.log_size);
            if let Some(prev_log_size) = prev_log_size {
                let log_ratio = pass.log_size - prev_log_size;
                state = array::from_fn(|j| to_lifted_simd(state[j], log_ratio, index));
            }
            hash_pass_tile(self.columns, pass, index, std::slice::from_mut(&mut state));
            prev_log_size = Some(pass.log_size);
        }
        if let Some(prev_log_size) = prev_log_size {
            let log_ratio = self.max_log_size - prev_log_size;
            state = array::from_fn(|j| to_lifted_simd(state[j], log_ratio, state_index));
        }
        finalize_tile(
            self.columns,
            self.last_chunk_index,
            &self.tail_log_ratios,
            self.byte_count,
            state_index,
            std::slice::from_mut(&mut state),
        );
        state
    }

    /// Computes the final states `first_state..first_state + tile.len()` of the largest domain.
    #[inline(always)]
    fn compute_tile(&self, first_state: usize, tile: &mut [[u32x16; N_FELTS_IN_BLAKE_STATE]]) {
        let lower = self
            .lower_log_size
            .map(|log_size| (self.lower_states.as_slice(), self.max_log_size - log_size));
        init_tile(tile, first_state, lower);
        for pass in &self.top_passes {
            hash_pass_tile(self.columns, pass, first_state, tile);
        }
        finalize_tile(
            self.columns,
            self.last_chunk_index,
            &self.tail_log_ratios,
            self.byte_count,
            first_state,
            tile,
        );
    }
}

/// Writes the states `first_state..first_state + tile.len()` of a pass to `tile`: either the
/// initial state of the hash, or the states of the previous pass (`prev`, together with the log
/// ratio between the two passes) lifted to the current domain.
#[inline(always)]
fn init_tile(
    tile: &mut [[u32x16; N_FELTS_IN_BLAKE_STATE]],
    first_state: usize,
    prev: Option<(&[[u32x16; N_FELTS_IN_BLAKE_STATE]], u32)>,
) {
    match prev {
        None => tile.fill(INITIAL_STATE),
        Some((prev_states, log_ratio)) => {
            for (k, state) in tile.iter_mut().enumerate() {
                let i = first_state + k;
                *state = array::from_fn(|j| {
                    to_lifted_simd(prev_states[i >> log_ratio][j], log_ratio, i)
                });
            }
        }
    }
}

/// Updates the states of a tile (states `first_state..first_state + tile.len()` of the pass) with
/// the columns of `pass`.
#[inline(always)]
fn hash_pass_tile(
    columns: &[&Col<SimdBackend, BaseField>],
    pass: &Pass,
    first_state: usize,
    tile: &mut [[u32x16; N_FELTS_IN_BLAKE_STATE]],
) {
    let mut local_byte_count = pass.byte_count + N_BYTES_IN_BLAKE_MESSAGE;
    // Lift the first chunk `columns[start..start + 16]`.
    for (k, state) in tile.iter_mut().enumerate() {
        let i = first_state + k;
        let msgs: [u32x16; N_FELTS_IN_BLAKE_MESSAGE] = array::from_fn(|j| {
            let log_ratio = pass.msg_log_ratios[j];
            to_lifted_simd(columns[pass.start + j].data[i >> log_ratio].into_simd(), log_ratio, i)
        });
        *state = compress_unfinalized(*state, msgs, local_byte_count);
    }
    // Deal with the subsequent chunks in `columns[start + 16..end]`. Note that since `start < end`
    // and both are multiples of 16, we have `start + 16 <= end`. All columns in
    // `columns[start + 16..end]` are guaranteed to be of the same size (hence no lifting is
    // required). The tile reads every column of a chunk in one short sequential run.
    for chunk_columns in columns[pass.start + 16..pass.end].chunks(N_FELTS_IN_BLAKE_MESSAGE) {
        local_byte_count += N_BYTES_IN_BLAKE_MESSAGE;
        let rows: [&[PackedBaseField]; N_FELTS_IN_BLAKE_MESSAGE] =
            array::from_fn(|j| &chunk_columns[j].data[first_state..first_state + tile.len()]);
        for (k, state) in tile.iter_mut().enumerate() {
            let msgs: [u32x16; N_FELTS_IN_BLAKE_MESSAGE] =
                array::from_fn(|j| rows[j][k].into_simd());
            *state = compress_unfinalized(*state, msgs, local_byte_count);
        }
    }
}

/// Hashes the last chunk `columns[last_chunk_index..]` into the states of a tile and finalizes
/// the hashes.
#[inline(always)]
fn finalize_tile(
    columns: &[&Col<SimdBackend, BaseField>],
    last_chunk_index: usize,
    tail_log_ratios: &[u32],
    byte_count: u64,
    first_state: usize,
    tile: &mut [[u32x16; N_FELTS_IN_BLAKE_STATE]],
) {
    for (k, state) in tile.iter_mut().enumerate() {
        let i = first_state + k;
        let mut msgs = [u32x16::splat(0); N_FELTS_IN_BLAKE_MESSAGE];
        for (j, column) in columns[last_chunk_index..].iter().enumerate() {
            let log_ratio = tail_log_ratios[j];
            msgs[j] = to_lifted_simd(column.data[i >> log_ratio].into_simd(), log_ratio, i);
        }
        *state = compress_finalize(*state, msgs, byte_count);
    }
}

/// Hashes the pairs of children of 16 nodes, given as two transposed states (16 hashes each): the
/// hash with index `2 * l` (resp. `2 * l + 1`) of the 32 hashes of both states is the first (resp.
/// second) child of node `l`.
#[inline(always)]
fn hash_state_pair(
    first: &[u32x16; N_FELTS_IN_BLAKE_STATE],
    second: &[u32x16; N_FELTS_IN_BLAKE_STATE],
) -> [u32x16; N_FELTS_IN_BLAKE_STATE] {
    let mut msgs = [u32x16::splat(0); N_FELTS_IN_BLAKE_MESSAGE];
    for w in 0..N_FELTS_IN_BLAKE_STATE {
        (msgs[w], msgs[N_FELTS_IN_BLAKE_STATE + w]) = first[w].deinterleave(second[w]);
    }
    compress_finalize(INITIAL_STATE, msgs, N_BYTES_IN_BLAKE_MESSAGE)
}

/// Stores 16 untransposed hashes: the `k`-th vector holds the hashes `2 * k` and `2 * k + 1`.
#[inline(always)]
fn store_states(untransposed: [u32x16; N_FELTS_IN_BLAKE_STATE], dst: &mut [Blake2sHash]) {
    let dst_words = cast_slice_mut::<_, u32>(dst);
    for (vector, words) in
        untransposed.iter().zip(dst_words.chunks_exact_mut(N_FELTS_IN_BLAKE_MESSAGE))
    {
        words.copy_from_slice(&vector.to_array());
    }
}

impl PackLeavesOps for SimdBackend {
    fn pack_leaves_input(
        values: &[&Col<SimdBackend, BaseField>; SECURE_EXTENSION_DEGREE],
    ) -> [Col<SimdBackend, BaseField>; SECURE_EXTENSION_DEGREE * PACKED_LEAF_SIZE] {
        let input_len = values[0].len();
        assert!(values.iter().all(|c| c.len() == input_len));
        assert!(input_len.is_multiple_of(PACKED_LEAF_SIZE));
        let output_len = input_len / PACKED_LEAF_SIZE;
        let output_packed_len = output_len.div_ceil(N_LANES);

        let mut packed_simd: [Vec<PackedBaseField>; SECURE_EXTENSION_DEGREE * PACKED_LEAF_SIZE] =
            unsafe { core::array::from_fn(|_| uninit_vec(output_packed_len)) };

        let output_packed_len_floor = output_len / N_LANES;

        // The rows are packed in parallel: the output columns are split into disjoint runs of
        // `ROWS_PER_PACK_TASK` rows, one run of every column per task.
        let mut runs_per_column: Vec<_> = packed_simd
            .iter_mut()
            .map(|column| column[..output_packed_len_floor].chunks_mut(ROWS_PER_PACK_TASK))
            .collect();
        let n_tasks = output_packed_len_floor.div_ceil(ROWS_PER_PACK_TASK);
        let tasks: Vec<[&mut [PackedBaseField]; SECURE_EXTENSION_DEGREE * PACKED_LEAF_SIZE]> = (0
            ..n_tasks)
            .map(|_| core::array::from_fn(|i| runs_per_column[i].next().unwrap()))
            .collect();
        parallel_iter!(tasks).enumerate().for_each(|(task_index, outputs)| {
            for row_in_run in 0..outputs[0].len() {
                let row = task_index * ROWS_PER_PACK_TASK + row_in_run;
                let packed_start_idx = row * PACKED_LEAF_SIZE;
                let packed_values = core::array::from_fn(|j| {
                    core::array::from_fn(|i| values[i].data[packed_start_idx + j])
                });
                let packed_row = transpose_packed_leaf(packed_values);
                for (offset, packed_leaf_column) in packed_row.into_iter().enumerate() {
                    for coord in 0..SECURE_EXTENSION_DEGREE {
                        outputs[coord + offset * SECURE_EXTENSION_DEGREE][row_in_run] =
                            packed_leaf_column[coord];
                    }
                }
            }
        });

        // Transpose the tail. If `tail_rows > 0` then necessarily we haven't entered the previous
        // loop.
        let tail_rows = output_len % N_LANES;
        if tail_rows > 0 {
            // The last `N_LANES - tail_rows` rows are zeros. Note that this padding is effectively
            // ignored by the Merkle prover because we return an array of `BaseColumns` with length
            // = `output_len`.
            let mut tail_columns: [[BaseField; N_LANES];
                SECURE_EXTENSION_DEGREE * PACKED_LEAF_SIZE] =
                core::array::from_fn(|_| [BaseField::zero(); N_LANES]);
            #[allow(clippy::needless_range_loop)]
            for row in 0..tail_rows {
                // The index in the input vector corresponding to `row`.
                let source_row_start = (output_packed_len_floor * N_LANES + row) * PACKED_LEAF_SIZE;
                for offset in 0..PACKED_LEAF_SIZE {
                    let coords: [BaseField; 4] =
                        core::array::from_fn(|i| values[i].at(source_row_start + offset));
                    for coord in 0..SECURE_EXTENSION_DEGREE {
                        tail_columns[coord + offset * SECURE_EXTENSION_DEGREE][row] = coords[coord];
                    }
                }
            }
            for column_idx in 0..SECURE_EXTENSION_DEGREE * PACKED_LEAF_SIZE {
                *packed_simd[column_idx].last_mut().unwrap() =
                    PackedBaseField::from_array(tail_columns[column_idx]);
            }
        }

        packed_simd.map(|data| BaseColumn { data, length: output_len })
    }
}
/// Given a vector of columns sorted by size (in ascending order) and an index `last_chunk_index`
/// which is a multiple of N_FELTS_IN_BLAKE_MESSAGE, returns a vector of indices `0 = i₁ < i₂ < ...
/// < iₙ = last_chunk_index` (if `last_chunk_index = 0` then n = 1 and i₁ = 0) such that:
/// * All indices are multiples of N_FELTS_IN_BLAKE_MESSAGE.
/// * For all 1 <= k < n:
///     1. the sizes in `col_sizes[iₖ + N_FELTS_IN_BLAKE_MESSAGE..iₖ₊₁]` are all equal.
///     2. `col_sizes[iₖ] < col_sizes[iₖ₊₁]`.
fn get_lifting_indices(
    col_sizes: impl Iterator<Item = usize>,
    last_chunk_index: usize,
) -> Vec<usize> {
    let mut prev_size = 0;
    let mut res = vec![];
    for (idx, col_size) in col_sizes.enumerate().step_by(N_FELTS_IN_BLAKE_MESSAGE).skip(1) {
        if col_size > prev_size {
            res.push(idx - N_FELTS_IN_BLAKE_MESSAGE);
            prev_size = col_size;
        }
    }
    res.push(last_chunk_index);
    // Sanity check that there are no duplicates.
    debug_assert!(res.iter().duplicates().next().is_none());
    res
}

#[cfg(test)]
mod tests {

    use itertools::Itertools;

    use crate::core::fields::m31::{BaseField, M31};
    use crate::core::vcs::blake2_hash::{Blake2sHash, Blake2sHasher};
    use crate::core::vcs_lifted::blake2_merkle::Blake2sMerkleHasher;
    use crate::prover::backend::simd::SimdBackend;
    use crate::prover::backend::simd::column::BaseColumn;
    use crate::prover::backend::{Column, CpuBackend};
    use crate::prover::vcs_lifted::ops::MerkleOpsLifted;
    use crate::prover::vcs_lifted::prover::MerkleProverLifted;

    #[test]
    fn test_build_next_layer() {
        const LOG_SIZE: u32 = 6;
        let layer: Vec<Blake2sHash> =
            (0u32..1 << (LOG_SIZE + 1)).map(|i| Blake2sHasher::hash(&i.to_le_bytes())).collect();
        assert_eq!(
            <CpuBackend as MerkleOpsLifted<Blake2sMerkleHasher>>::build_next_layer(&layer),
            <SimdBackend as MerkleOpsLifted<Blake2sMerkleHasher>>::build_next_layer(&layer)
        );
    }

    fn prepare_blake_merkle_commit() -> (Blake2sHash, Blake2sHash) {
        const MAX_LOG_N_ROWS: u32 = 9;
        const N_COLS: u32 = 100;
        let mut cols: Vec<Vec<BaseField>> = (0..N_COLS)
            .map(|i| (0..1 << MAX_LOG_N_ROWS).map(|j| M31::from(100 * i + j)).collect_vec())
            .collect();

        // Make the first two columns smaller to test a non-uniform sized trace.
        cols[0] = (0..1 << (MAX_LOG_N_ROWS - 4)).map(M31::from_u32_unchecked).collect_vec();
        cols[1] = (0..1 << (MAX_LOG_N_ROWS - 3)).map(M31::from_u32_unchecked).collect_vec();
        let cols_simd: Vec<BaseColumn> = cols.iter().map(|c| BaseColumn::from_cpu(c)).collect();

        (
            MerkleProverLifted::<CpuBackend, Blake2sMerkleHasher>::commit(
                cols.iter().collect(),
                MAX_LOG_N_ROWS,
                0,
            )
            .root(),
            MerkleProverLifted::<SimdBackend, Blake2sMerkleHasher>::commit(
                cols_simd.iter().collect(),
                MAX_LOG_N_ROWS,
                0,
            )
            .root(),
        )
    }

    #[test]
    fn test_blake_merkle_commit() {
        let (cpu_root, simd_root) = prepare_blake_merkle_commit();
        assert_eq!(cpu_root, simd_root);
    }

    #[test]
    fn test_merkle_commit_small_column() {
        for log_size in 1..8 {
            let col = BaseColumn::from_cpu(&(0..1 << log_size).map(M31::from).collect_vec());

            assert_eq!(
                <CpuBackend as MerkleOpsLifted<Blake2sMerkleHasher>>::build_leaves(
                    &[&col.clone().to_cpu()],
                    log_size
                ),
                <SimdBackend as MerkleOpsLifted<Blake2sMerkleHasher>>::build_leaves(
                    &[&col],
                    log_size
                )
            );
        }
    }
}
