//! Core implementation of the Star Graph (Sn) topology for Combinatorial Holography.
//! Optimized for cache locality via zero-allocation factoradic indexing and finite field arithmetic.

use p3_baby_bear::BabyBear;
use p3_field::PrimeField32;
use p3_matrix::dense::RowMajorMatrix;
use rand::Rng;
use rayon::prelude::*;

pub const MAX_N: usize = 12;

/// The number of field elements stored in each node (columns).
/// [0] -> is_active (1 or 0)
/// [1] -> parent_1_edge_idx (1 to n-1, 0 for genesis)
/// [2] -> parent_2_edge_idx (1 to n-1, 0 if single parent)
/// [3] -> current_val
/// [4] -> aux_val (e.g., previous state or resolution metadata)
pub const STATE_WIDTH: usize = 5;

pub struct StarGraph {
    /// The number of symbols in the permutation (n).
    pub n: usize,
    /// Multi-column flat buffer for node states.
    pub nodes: Vec<[BabyBear; STATE_WIDTH]>,
    /// Precomputed factorials for indexing performance.
    factorials: [usize; MAX_N + 1],
}

impl StarGraph {
    pub fn new(n: usize) -> Self {
        assert!(n <= MAX_N, "n exceeds MAX_N ({})", MAX_N);
        let mut factorials = [1; MAX_N + 1];
        for i in 1..=n {
            factorials[i] = factorials[i - 1] * i;
        }
        let size = factorials[n];

        Self {
            n,
            nodes: vec![[BabyBear::new(0); STATE_WIDTH]; size],
            factorials,
        }
    }

    #[inline(always)]
    fn decode_stack(&self, mut index: usize) -> [usize; MAX_N] {
        let mut symbols = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11];
        let mut perm = [0; MAX_N];
        for i in (1..=self.n).rev() {
            let fact = self.factorials[i - 1];
            let digit = index / fact;
            index %= fact;
            perm[self.n - i] = symbols[digit];
            for j in digit..MAX_N - 1 {
                symbols[j] = symbols[j + 1];
            }
        }
        perm
    }

    #[inline(always)]
    fn encode_stack(&self, perm: [usize; MAX_N]) -> usize {
        let mut index = 0;
        let mut symbols = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11];
        for i in (1..=self.n).rev() {
            let s = perm[self.n - i];
            let pos = symbols
                .iter()
                .position(|&x| x == s)
                .expect("Symbol not found");
            for j in pos..MAX_N - 1 {
                symbols[j] = symbols[j + 1];
            }
            index += pos * self.factorials[i - 1];
        }
        index
    }

    pub fn get_neighbor_index(&self, current_index: usize, swap_pos: usize) -> usize {
        debug_assert!(swap_pos > 0 && swap_pos < self.n);
        let mut perm = self.decode_stack(current_index);
        perm.swap(0, swap_pos);
        self.encode_stack(perm)
    }

    /// Verifies local consistency against a branching DAG constraint.
    pub fn is_locally_consistent<C>(&self, index: usize, constraint: &C) -> bool
    where
        C: Fn([BabyBear; STATE_WIDTH], &[[BabyBear; STATE_WIDTH]]) -> BabyBear + Sync,
    {
        let state = self.nodes[index];
        if state[0] == BabyBear::new(0) {
            return true;
        }

        let mut neighbors = [[BabyBear::new(0); STATE_WIDTH]; MAX_N];
        for i in 1..self.n {
            let neighbor_idx = self.get_neighbor_index(index, i);
            neighbors[i - 1] = self.nodes[neighbor_idx];
        }

        constraint(state, &neighbors[..self.n - 1]) == BabyBear::new(0)
    }

    pub fn verify_entire_topology<C>(&self, constraint: C) -> bool
    where
        C: Fn([BabyBear; STATE_WIDTH], &[[BabyBear; STATE_WIDTH]]) -> BabyBear + Sync,
    {
        (0..self.nodes.len())
            .into_par_iter()
            .all(|i| self.is_locally_consistent(i, &constraint))
    }

    /// Binary Merkle Commitment: Hashes the trace into a tree structure.
    pub fn commit_to_merkle(&self) -> ([u8; 32], Vec<RowMajorMatrix<BabyBear>>) {
        let flat_nodes: Vec<BabyBear> = self.nodes.iter().flat_map(|s| s.iter()).copied().collect();
        let matrix = RowMajorMatrix::new(flat_nodes, STATE_WIDTH);
        let matrices = vec![matrix];
        let root = self.simplified_merkle_root();
        (root, matrices)
    }

    fn simplified_merkle_root(&self) -> [u8; 32] {
        use tiny_keccak::{Hasher, Keccak};

        let mut current_layer: Vec<[u8; 32]> = self
            .nodes
            .par_iter()
            .map(|state| {
                let mut h = [0u8; 32];
                let mut k = Keccak::v256();
                for element in state {
                    k.update(&element.as_canonical_u32().to_le_bytes());
                }
                k.finalize(&mut h);
                h
            })
            .collect();

        while current_layer.len() > 1 {
            current_layer = current_layer
                .par_chunks(2)
                .map(|chunk| {
                    let mut h = [0u8; 32];
                    let mut k = Keccak::v256();
                    k.update(&chunk[0]);
                    if chunk.len() > 1 {
                        k.update(&chunk[1]);
                    } else {
                        k.update(&chunk[0]);
                    }
                    k.finalize(&mut h);
                    h
                })
                .collect();
        }
        current_layer[0]
    }

    /// Generates a Raw Topological Proof.
    pub fn prove(&self, k: usize) -> RawProof {
        let tree = self.build_merkle_tree_full();
        let root = tree.last().unwrap()[0];
        let mut rng = rand::thread_rng();
        let mut openings = Vec::with_capacity(k);

        for _ in 0..k {
            let idx = rng.gen_range(0..self.nodes.len());
            let state = self.nodes[idx].map(|f| f.as_canonical_u32());
            let path = self.get_path(&tree, idx);
            openings.push(Opening {
                index: idx,
                state,
                path,
            });
        }

        RawProof { root, openings }
    }

    fn build_merkle_tree_full(&self) -> Vec<Vec<[u8; 32]>> {
        use tiny_keccak::{Hasher, Keccak};
        let mut tree = Vec::new();
        let mut current_layer: Vec<[u8; 32]> = self
            .nodes
            .par_iter()
            .map(|state| {
                let mut h = [0u8; 32];
                let mut k = Keccak::v256();
                for e in state {
                    k.update(&e.as_canonical_u32().to_le_bytes());
                }
                k.finalize(&mut h);
                h
            })
            .collect();

        tree.push(current_layer.clone());
        while current_layer.len() > 1 {
            current_layer = current_layer
                .par_chunks(2)
                .map(|chunk| {
                    let mut h = [0u8; 32];
                    let mut k = Keccak::v256();
                    k.update(&chunk[0]);
                    if chunk.len() > 1 {
                        k.update(&chunk[1]);
                    } else {
                        k.update(&chunk[0]);
                    }
                    k.finalize(&mut h);
                    h
                })
                .collect();
            tree.push(current_layer.clone());
        }
        tree
    }

    fn get_path(&self, tree: &[Vec<[u8; 32]>], mut idx: usize) -> Vec<[u8; 32]> {
        let mut path = Vec::new();
        for layer in tree.iter().take(tree.len() - 1) {
            let is_even = (idx % 2) == 0;
            let sibling_idx = if is_even {
                if idx + 1 < layer.len() {
                    idx + 1
                } else {
                    idx
                }
            } else {
                idx - 1
            };
            path.push(layer[sibling_idx]);
            idx /= 2;
        }
        path
    }

    pub fn verify_opening(root: [u8; 32], opening: &Opening) -> bool {
        use tiny_keccak::{Hasher, Keccak};
        let mut current_hash = [0u8; 32];
        let mut k = Keccak::v256();
        for e in &opening.state {
            k.update(&e.to_le_bytes());
        }
        k.finalize(&mut current_hash);

        let mut idx = opening.index;
        for sibling in &opening.path {
            let mut k = Keccak::v256();
            let is_even = (idx % 2) == 0;
            if is_even {
                k.update(&current_hash);
                k.update(sibling);
            } else {
                k.update(sibling);
                k.update(&current_hash);
            }
            k.finalize(&mut current_hash);
            idx /= 2;
        }
        current_hash == root
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct RawProof {
    pub root: [u8; 32],
    pub openings: Vec<Opening>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Opening {
    pub index: usize,
    pub state: [u32; STATE_WIDTH],
    pub path: Vec<[u8; 32]>,
}
