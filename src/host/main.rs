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

use jolt_sdk;
use serde::{Deserialize, Serialize};

// Represents the binary, packed data we send to the guest as a Hint.
use ruma_common::{CanonicalJsonObject, OwnedEventId, OwnedRoomId, OwnedUserId, RoomVersionId};
use ruma_events::TimelineEventType;
use std::collections::{BTreeMap, HashMap, HashSet};

pub type StateMap<K> = BTreeMap<(ruma_events::StateEventType, String), K>;

use ruma_lean::LeanEvent;

pub mod raw_value_as_string {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use serde_json::value::RawValue;

    #[allow(clippy::borrowed_box)]
    pub fn serialize<S>(value: &Box<RawValue>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        value.get().serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Box<RawValue>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        RawValue::from_string(s).map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct GuestEvent {
    pub event: CanonicalJsonObject,
    #[serde(with = "raw_value_as_string")]
    pub content: Box<serde_json::value::RawValue>,
    pub event_id: OwnedEventId,
    pub room_id: OwnedRoomId,
    pub sender: OwnedUserId,
    pub event_type: TimelineEventType,
    pub prev_events: Vec<OwnedEventId>,
    pub auth_events: Vec<OwnedEventId>,
    pub public_key: Option<Vec<u8>>,
    pub signature: Option<Vec<u8>>,
    pub verified_on_host: bool,
}

impl GuestEvent {
    fn origin_server_ts(&self) -> ruma_common::MilliSecondsSinceUnixEpoch {
        let val = self
            .event
            .get("origin_server_ts")
            .expect("missing origin_server_ts");
        serde_json::from_value(val.clone().into()).expect("invalid origin_server_ts")
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct DAGMergeInput {
    pub room_version: RoomVersionId,
    pub event_map: BTreeMap<OwnedEventId, GuestEvent>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct DAGMergeOutput {
    pub resolved_state_hash: [u8; 32],
    pub event_count: u32,
}

mod fixtures;

fn main() {
    println!("* Starting ZK-Matrix-Join Jolt Demo...");
    println!("--------------------------------------------------");

    let room_id = OwnedRoomId::try_from("!demo:example.com").unwrap();
    let mut fixture_path_str = "res/custom".to_string();
    let total_raw_len;

    // The Host can load from JSON or from a Concise Fixture DSL
    let events: Vec<GuestEvent> = if let Ok(_fixture_str) = std::env::var("BATCH_FIXTURE") {
        println!("> Loading concise Matrix Fixtures from environment...");
        let rows: &[fixtures::FixtureRow] = &[
            ("Alice", 100, 10, 1, &[], "alice"),
            ("Bob", 50, 20, 2, &["0"], "bob"),
            ("Charlie", 100, 15, 2, &["0"], "charlie"),
        ];
        let evs = fixtures::parse_fixture_rows(&room_id, rows);
        total_raw_len = evs.len();
        evs
    } else {
        let state_file_path = "res/real_10k.json";
        let fallback_path = "res/massive_matrix_state.json";
        let ruma_path = "res/ruma_bootstrap_events.json";

        let path: String = std::env::var("MATRIX_FIXTURE_PATH").unwrap_or_else(|_| {
            if std::path::Path::new(state_file_path).exists() {
                state_file_path.to_string()
            } else if std::path::Path::new(fallback_path).exists() {
                fallback_path.to_string()
            } else {
                ruma_path.to_string()
            }
        });
        fixture_path_str = path.clone();

        println!("> Loading raw Matrix State DAG from {}...", path);
        let file_content = std::fs::read_to_string(&path)
            .expect("Failed to read JSON state file (try running the python fetcher!)");
        let raw_events: Vec<serde_json::Value> = serde_json::from_str(&file_content).unwrap();

        let raw_len = raw_events.len();
        total_raw_len = raw_len;
        let mut i = 0;
        raw_events
            .into_iter()
            .filter_map(|ev| {
                i += 1;
                let event_type_val = ev.get("type")?.as_str()?;
                if i % 2500 == 0 || i == raw_len {
                    println!(
                        "  ... [Parsing Event {}/{}] Type: {}",
                        i, raw_len, event_type_val
                    );
                }

                let event = match serde_json::from_value::<CanonicalJsonObject>(ev.clone()) {
                    Ok(x) => x,
                    Err(e) => {
                        if i == 1 {
                            println!("Event 1 Failed at event: {}", e);
                        }
                        return None;
                    }
                };
                let content_val = match ev.get("content") {
                    Some(v) => v.clone(),
                    None => {
                        if i == 1 {
                            println!("Event 1 Failed at content missing");
                        }
                        return None;
                    }
                };
                let content =
                    match serde_json::from_value::<Box<serde_json::value::RawValue>>(content_val) {
                        Ok(x) => x,
                        Err(e) => {
                            if i == 1 {
                                println!("Event 1 Failed at content: {}", e);
                            }
                            return None;
                        }
                    };
                let event_id = match serde_json::from_value::<OwnedEventId>(
                    ev.get("event_id")
                        .unwrap_or(&serde_json::Value::Null)
                        .clone(),
                ) {
                    Ok(x) => x,
                    Err(e) => {
                        if i == 1 {
                            println!("Event 1 Failed at event_id: {}", e);
                        }
                        return None;
                    }
                };
                let room_id_obj = match serde_json::from_value::<OwnedRoomId>(
                    ev.get("room_id")
                        .unwrap_or(&serde_json::Value::Null)
                        .clone(),
                ) {
                    Ok(x) => x,
                    Err(e) => {
                        if i == 1 {
                            println!("Event 1 Failed at room_id: {}", e);
                        }
                        return None;
                    }
                };
                let sender = match serde_json::from_value::<OwnedUserId>(
                    ev.get("sender").unwrap_or(&serde_json::Value::Null).clone(),
                ) {
                    Ok(x) => x,
                    Err(e) => {
                        if i <= 3 {
                            println!("Event {} Failed at sender: {}", i, e);
                        }
                        return None;
                    }
                };
                let event_type = match serde_json::from_value::<TimelineEventType>(
                    ev.get("type").unwrap_or(&serde_json::Value::Null).clone(),
                ) {
                    Ok(x) => x,
                    Err(e) => {
                        if i == 1 {
                            println!("Event 1 Failed at type: {}", e);
                        }
                        return None;
                    }
                };
                let prev_events: Vec<OwnedEventId> = serde_json::from_value(
                    ev.get("prev_events")
                        .unwrap_or(&serde_json::Value::Array(vec![]))
                        .clone(),
                )
                .unwrap_or_default();
                let auth_events: Vec<OwnedEventId> = serde_json::from_value(
                    ev.get("auth_events")
                        .unwrap_or(&serde_json::Value::Array(vec![]))
                        .clone(),
                )
                .unwrap_or_default();

                Some(GuestEvent {
                    event,
                    content,
                    event_id,
                    room_id: room_id_obj,
                    sender,
                    event_type,
                    prev_events,
                    auth_events,
                    public_key: None,
                    signature: None,
                    verified_on_host: false,
                })
            })
            .collect()
    };

    if total_raw_len == 0 {
        panic!("No events loaded! Check your fixture paths.");
    }

    // Parallel Public Key Fetching & Signature Verification
    println!(
        "> [Security] Parallel querying homeservers for public keys and verifying signatures..."
    );

    let key_cache_path = format!("{}.keys.json", fixture_path_str);
    let mut key_cache: HashMap<String, String> = if std::path::Path::new(&key_cache_path).exists() {
        let content = std::fs::read_to_string(&key_cache_path).unwrap();
        serde_json::from_str(&content).unwrap_or_default()
    } else {
        HashMap::new()
    };

    // Identify unique servers we need keys for
    let mut servers_to_query = HashSet::new();
    for ev in &events {
        if let Some(signatures) = ev.event.get("signatures").and_then(|s| s.as_object()) {
            for server in signatures.keys() {
                if !key_cache.contains_key(server) {
                    servers_to_query.insert(server.to_string());
                }
            }
        }
    }

    if !servers_to_query.is_empty() {
        println!(
            "  ... Querying {} homeservers for missing public keys...",
            servers_to_query.len()
        );
        use rayon::prelude::*;
        let _new_keys: Vec<(String, String)> = servers_to_query
            .into_par_iter()
            .filter_map(|server| {
                let url = format!("https://{}/_matrix/key/v2/server", server);
                let client = reqwest::blocking::Client::builder()
                    .timeout(std::time::Duration::from_secs(5))
                    .build()
                    .ok()?;

                let res = client.get(&url).send().ok()?;
                let json: serde_json::Value = res.json().ok()?;

                // Extract the first Ed25519 key found
                if let Some(keys) = json.get("verify_keys").and_then(|k| k.as_object()) {
                    for (key_id, key_info) in keys {
                        if key_id.starts_with("ed25519:") {
                            if let Some(key_base64) = key_info.get("key").and_then(|k| k.as_str()) {
                                // Convert base64 to hex for our simple cache
                                use base64::Engine;
                                if let Ok(bytes) =
                                    base64::engine::general_purpose::STANDARD.decode(key_base64)
                                {
                                    return Some((server, hex::encode(bytes)));
                                }
                            }
                        }
                    }
                }
                None
            })
            .collect();
    }

    use rayon::prelude::*;
    let events: Vec<GuestEvent> = events
        .into_par_iter()
        .map(|mut ev| {
            // Try to extract signature from the event
            if let Some(signatures) = ev.event.get("signatures").and_then(|s| s.as_object()) {
                for (server, sigs) in signatures {
                    if let Some(sig_map) = sigs.as_object() {
                        for (key_id, sig_val) in sig_map {
                            if key_id.starts_with("ed25519:") {
                                if let Some(sig_str) = sig_val.as_str() {
                                    if let Ok(sig_bytes) = hex::decode(sig_str) {
                                        if sig_bytes.len() == 64 {
                                            ev.signature = Some(sig_bytes);

                                            // Check if we have the public key in cache
                                            let server_name = server.to_string();
                                            if let Some(pk_hex) = key_cache.get(&server_name) {
                                                if let Ok(pk_bytes) = hex::decode(pk_hex) {
                                                    if pk_bytes.len() == 32 {
                                                        ev.public_key = Some(pk_bytes);
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // Verify signature if we have both
            if let (Some(pk), Some(sig)) = (&ev.public_key, &ev.signature) {
                // Host-side verification
                use ed25519_consensus::{Signature, VerificationKey};
                if let (Ok(vk), Ok(s)) = (
                    VerificationKey::try_from(pk.as_slice()),
                    Signature::try_from(sig.as_slice()),
                ) {
                    if vk.verify(&s, ev.event_id.as_bytes()).is_ok() {
                        ev.verified_on_host = true;
                    }
                }
            }
            ev
        })
        .collect();

    let skipped = total_raw_len - events.len();
    if skipped > 0 {
        println!("> Notice: Skipped {} ill-formed or legacy events that violate Ruma specs (e.g. >255 byte constraints)", skipped);
    }
    println!(
        "> Successfully mapped exactly {} Matrix Events into Ruma ZK hints!",
        events.len()
    );

    let mut event_map = BTreeMap::new();
    for guest_ev in &events {
        event_map.insert(guest_ev.event_id.clone(), guest_ev.clone());
    }

    println!("> Resolving state natively on host (Path A)...");

    let mut conflicted_events = HashMap::new();
    for guest_ev in &events {
        let lean_ev = LeanEvent {
            event_id: guest_ev.event_id.to_string(),
            power_level: 0, // Simplified for demo
            origin_server_ts: guest_ev.origin_server_ts().0.into(),
            prev_events: guest_ev
                .prev_events
                .iter()
                .map(|id| id.to_string())
                .collect(),
            depth: 0, // Simplified for demo
        };
        conflicted_events.insert(lean_ev.event_id.clone(), lean_ev);
    }

    let sorted_ids = ruma_lean::lean_kahn_sort(&conflicted_events, ruma_lean::StateResVersion::V2);

    // Build the resolved state map based on the sorted order (Last-Writer-Wins for conflicts)
    let mut resolved_state = BTreeMap::new();
    for id in sorted_ids {
        if let Some(ev) = event_map.get(&OwnedEventId::try_from(id).unwrap()) {
            let key = (
                ev.event_type.to_string(),
                ev.event
                    .get("state_key")
                    .unwrap()
                    .as_str()
                    .unwrap()
                    .to_string(),
            );
            resolved_state.insert(key, ev.event_id.clone());
        }
    }

    // Journal Commitment: Fingerprint the resolved state
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    for ((event_type, state_key), id) in &resolved_state {
        hasher.update(event_type.as_bytes());
        hasher.update(state_key.as_bytes());
        hasher.update(id.as_str().as_bytes());
    }
    let expected_hash: [u8; 32] = hasher.finalize().into();

    println!(
        "> Flattening the DAG to pass linear array of topological constraints... ({} total items)",
        events.len()
    );

    let mut edges: Vec<(u32, u32)> = Vec::new();
    const DIMS: usize = 10;

    fn event_to_coordinate(s: &str) -> u32 {
        let mut h = Sha256::new();
        h.update(s.as_bytes());
        let hash_bytes = h.finalize();
        let val = u32::from_be_bytes([hash_bytes[0], hash_bytes[1], hash_bytes[2], hash_bytes[3]]);
        val & ((1 << DIMS) - 1)
    }

    let mut last_coord = 0;
    for event in &events {
        let target_coord = event_to_coordinate(event.event_id.as_str());

        let mut parents = Vec::new();
        for prev in &event.prev_events {
            parents.push(prev.as_str().to_string());
        }
        if parents.is_empty() {
            parents.push(last_coord.to_string());
        }

        for prev_str in parents {
            let mut curr = if prev_str == last_coord.to_string() {
                last_coord
            } else {
                event_to_coordinate(&prev_str)
            };

            while curr != target_coord {
                let diff = curr ^ target_coord;
                let bit_to_flip = diff.trailing_zeros() as usize;
                let next = curr ^ (1 << bit_to_flip);

                edges.push((curr, next));
                curr = next;
            }
        }
        last_coord = target_coord;
    }

    let is_unoptimized = std::env::var("EXECUTE_UNOPTIMIZED").is_ok();
    let is_proving = std::env::var("JOLT_PROVE").is_ok();

    if is_proving {
        println!("Generating Jolt Proof for Matrix State Resolution...");
        if is_unoptimized {
            println!("> Mode: UNOPTIMIZED (Full Spec State Resolution)");
            let (cp, pp, _vp) = zk_matrix_join_guest_unoptimized::preprocess();
            let input = zk_matrix_join_guest_unoptimized::DAGMergeInput {
                room_version: RoomVersionId::V10,
                event_map: event_map
                    .into_iter()
                    .map(|(id, ev)| {
                        (
                            id,
                            zk_matrix_join_guest_unoptimized::GuestEvent {
                                event: ev.event,
                                content: ev.content,
                                event_id: ev.event_id,
                                room_id: ev.room_id,
                                sender: ev.sender,
                                event_type: ev.event_type,
                                prev_events: ev.prev_events,
                                auth_events: ev.auth_events,
                                public_key: ev.public_key,
                                signature: ev.signature,
                                verified_on_host: ev.verified_on_host,
                            },
                        )
                    })
                    .collect(),
            };
            let (output, _proof, _commitments) = zk_matrix_join_guest_unoptimized::prove_resolve_full_spec(&cp, pp, input);
            println!("✓ Jolt Proof Generated Successfully!");
            println!(
                "Matrix Resolved State Hash (Journal): {:?}",
                hex::encode(output.resolved_state_hash)
            );
            println!("Events Verified in Proof: {}", output.event_count);
        } else {
            println!("> Mode: OPTIMIZED (Topological Reducer)");
            let (cp, pp, _vp) = zk_matrix_join_guest::preprocess();
            let (output, _proof, _commitments) = zk_matrix_join_guest::prove_verify_topology(
                &cp,
                pp,
                edges,
                expected_hash,
                events.len() as u32,
            );
            println!("✓ Jolt Proof Generated Successfully!");
            println!(
                "Matrix Resolved State Hash (Journal): {:?}",
                hex::encode(output.resolved_state_hash)
            );
            println!("Events Verified in Proof: {}", output.event_count);
        }
    } else {
        println!("Simulating Jolt Execution for Matrix State Resolution...");
        if is_unoptimized {
            let input = zk_matrix_join_guest_unoptimized::DAGMergeInput {
                room_version: RoomVersionId::V10,
                event_map: event_map
                    .into_iter()
                    .map(|(id, ev)| {
                        (
                            id,
                            zk_matrix_join_guest_unoptimized::GuestEvent {
                                event: ev.event,
                                content: ev.content,
                                event_id: ev.event_id,
                                room_id: ev.room_id,
                                sender: ev.sender,
                                event_type: ev.event_type,
                                prev_events: ev.prev_events,
                                auth_events: ev.auth_events,
                                public_key: ev.public_key,
                                signature: ev.signature,
                                verified_on_host: ev.verified_on_host,
                            },
                        )
                    })
                    .collect(),
            };
            let output = zk_matrix_join_guest_unoptimized::resolve_full_spec(input);
            println!("--------------------------------------------------");
            println!("✓ Verifiable Simulation Complete!");
            println!(
                "Matrix Resolved State Hash: {:?}",
                hex::encode(output.resolved_state_hash)
            );
            println!("Events Verified: {}", output.event_count);
        } else {
            let output =
                zk_matrix_join_guest::verify_topology(edges, expected_hash, events.len() as u32);
            println!("--------------------------------------------------");
            println!("✓ Verifiable Simulation Complete!");
            println!(
                "Matrix Resolved State Hash: {:?}",
                hex::encode(output.resolved_state_hash)
            );
            println!("Events Verified: {}", output.event_count);
        }
    }
}
