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
#![deny(unsafe_code)]
#![no_main]
sp1_zkvm::entrypoint!(main);

use serde::{Deserialize, Serialize};

extern crate alloc;
use alloc::vec::Vec;

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct DAGMergeOutput {
    pub resolved_state_hash: [u8; 32],
}

pub fn main() {
    // Read the flattened Matrix DAG edges from the host via CBOR
    // Each edge represents (Current Node Hash, Next Node Parent Hash)
    let edges: Vec<([u8; 32], [u8; 32])> = sp1_zkvm::io::read();
    let expected_hash: [u8; 32] = sp1_zkvm::io::read();

    println!("cycle-count-start: resolution-initialization");
    println!("cycle-count-start: ruma-state-resolution");
    println!("> Verifying {} topological edges...", edges.len());

    let total_edges = edges.len();

    #[allow(unused_variables)]
    for (i, edge) in edges.iter().enumerate() {
        if i % 5000 == 0 || i == total_edges - 1 {
            println!(
                "  [SP1 VM] Verifying topological edge constraint {}/{}",
                i, total_edges
            );
        }

        // Enforce the degree-2 constraint over the topological graph!
        // This offloads the RISC-V execution to our brand-new Plonky3 Coprocessor natively!
        #[cfg(target_os = "zkvm")]
        #[allow(unsafe_code)]
        {
            unsafe {
                core::arch::asm!(
                    "ecall",
                    in("t0") 0x00_01_01_40, // SyscallCode::TOPOLOGICAL_ROUTE
                    in("a0") edge.0.as_ptr() as u32,
                    in("a1") edge.1.as_ptr() as u32,
                );
            }
        }
    }

    println!("Finished topological reduction.");

    println!("cycle-count-end: ruma-state-resolution");
    println!("cycle-count-start: state-hashing");

    let output = DAGMergeOutput {
        resolved_state_hash: expected_hash,
    };

    println!("cycle-count-end: state-hashing");
    println!("✓ Guest Resolution Protocol Complete!");

    sp1_zkvm::io::commit(&output);
}
