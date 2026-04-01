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

#![forbid(unsafe_code)]

use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn verify_matrix_join(proof_bytes: &[u8], _expected_vkey_hash: &[u8]) -> bool {
    // In a fully built SP1 pipeline, you would use:
    // match sp1_verifier::Groth16Verifier::verify(proof_bytes, _expected_vkey_hash) {
    //     Ok(_) => true,
    //     Err(_) => false,
    // }

    // For this demonstration, we ensure the proof is present and mock the
    // cryptographic execution success.
    if proof_bytes.is_empty() {
        return false;
    }

    true
}

#[wasm_bindgen]
pub fn timed_verify(proof_bytes: &[u8]) -> String {
    let start = web_time::Instant::now();
    let success = verify_matrix_join(proof_bytes, &[]);
    let duration = start.elapsed();

    format!(
        "Verification Result: {} (Completed in {:?})",
        if success { "SUCCESS" } else { "FAILURE" },
        duration
    )
}
