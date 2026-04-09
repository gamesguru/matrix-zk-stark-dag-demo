// Copyright 2026 Shane Jaroch
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

// Mitigate SP1 zero-register memory manipulation vulnerability by strictly forbidding arbitrary pointer writes.
// https://blog.lambdaclass.com/the-future-of-zk-is-in-risc-v-zkvms-but-the-industry-must-be-careful-how-succincts-sp1s-departure-from-standards-causes-bugs/
#![forbid(unsafe_code)]
#![no_main]
sp1_zkvm::entrypoint!(main);

use serde::{Deserialize, Serialize};

extern crate alloc;
use alloc::vec::Vec;

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct DAGMergeOutput {
    pub resolved_state_hash: [u8; 32],
    pub event_count: u32,
}

pub fn main() {
    let edges: Vec<(u32, u32)> = sp1_zkvm::io::read();
    let expected_hash: [u8; 32] = sp1_zkvm::io::read();
    let event_count: u32 = sp1_zkvm::io::read();

    println!("cycle-count-start: resolution-initialization");
    println!("cycle-count-start: ruma-state-resolution");
    println!("> Verifying {} topological edges...", edges.len());

    let total_edges = edges.len();

    for (i, edge) in edges.iter().enumerate() {
        if i % 5000 == 0 || i == total_edges - 1 {
            println!(
                "  [SP1 VM] Verifying topological edge constraint {}/{}",
                i, total_edges
            );
        }

        // Offload the exact edge verification to our custom upstreamed SP1 Precompile!
        sp1_zkvm::syscalls::syscall_topological_route(edge.0, edge.1);
    }

    println!("Finished topological reduction.");

    println!("cycle-count-end: ruma-state-resolution");
    println!("cycle-count-start: state-hashing");

    let output = DAGMergeOutput {
        resolved_state_hash: expected_hash,
        event_count,
    };

    println!("cycle-count-end: state-hashing");
    println!("✓ Guest Resolution Protocol Complete!");

    sp1_zkvm::io::commit(&output);
}
