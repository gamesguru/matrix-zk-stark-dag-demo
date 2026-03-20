#![no_main]
sp1_zkvm::entrypoint!(main);

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

// In a full production implementation, these would map directly to ruma_events::AnyStateEvent
// and ruma_state_res::StateMap. For the demonstration of the ZK-circuit, we use simplified
// structs to represent the Auth Chain and Conflicting States.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MinimalStateEvent {
    pub event_id: String,
    pub sender: String,
    pub state_key: String,
    pub power_level: i64,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct DAGMergeInput {
    pub room_version: String,
    pub conflicting_states: Vec<Vec<MinimalStateEvent>>,
    pub auth_chain: Vec<MinimalStateEvent>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct DAGMergeOutput {
    pub resolved_state_hash: [u8; 32],
}

pub fn main() {
    // Read the input from the Host (the conflicting states and auth chain)
    let input: DAGMergeInput = sp1_zkvm::io::read();

    // Mathematically verify the Matrix State Resolution v2 algorithm.
    // In a real implementation: `ruma_state_res::resolve(input.room_version, ...)`
    // Here we perform a deterministic mock of Kahn's sorting and Ed25519 signature validation
    // to simulate the heavy lifting that the SP1 zkVM will prove.

    let mut hasher = Sha256::new();

    // Sort conflicting states based on Matrix rules (simulated via ID comparison)
    let mut resolved_state = Vec::new();
    let mut all_conflicts = input.conflicting_states.concat();
    all_conflicts.sort_by(|a, b| a.event_id.cmp(&b.event_id)); // lexicographical tie-break

    for event in all_conflicts {
        // Enforce protocol rules (e.g., negative test: user must have power level > 0)
        if event.power_level < 0 {
            panic!("Invalid Power Level detected in Auth Chain! ZK Proof Generation Failed.");
        }

        hasher.update(event.event_id.as_bytes());
        resolved_state.push(event);
    }

    // Commit the final resolved state hash to the STARK public values.
    let expected_hash: [u8; 32] = hasher.finalize().into();

    let output = DAGMergeOutput {
        resolved_state_hash: expected_hash,
    };

    sp1_zkvm::io::commit(&output);
}
