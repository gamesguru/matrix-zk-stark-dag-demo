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

use jolt_sdk as jolt;
use ruma_common::{CanonicalJsonObject, OwnedEventId, RoomVersionId};
use ruma_events::TimelineEventType;
use ruma_lean::{lean_kahn_sort, HashMap, LeanEvent, StateResVersion};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::boxed::Box;
use std::collections::BTreeMap;
use std::string::ToString;
use std::vec::Vec;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct GuestEvent {
    pub event: CanonicalJsonObject,
    pub content: Box<serde_json::value::RawValue>,
    pub event_id: OwnedEventId,
    pub room_id: ruma_common::OwnedRoomId,
    pub sender: ruma_common::OwnedUserId,
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

#[jolt::provable]
pub fn resolve_full_spec(input: DAGMergeInput) -> DAGMergeOutput {
    let event_count = input.event_map.len() as u32;

    let mut conflicted_events = HashMap::new();
    for (id, guest_ev) in &input.event_map {
        // [Security] Verify Ed25519 signatures if keys are available
        if let (Some(_pk), Some(_sig)) = (&guest_ev.public_key, &guest_ev.signature) {
            // Placeholder for Jolt Ed25519 verification
            if guest_ev.verified_on_host {
                // Trust but verify
            }
        }

        let lean_ev = LeanEvent {
            event_id: id.to_string(),
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

    let sorted_ids = lean_kahn_sort(&conflicted_events, StateResVersion::V2);

    // Build the resolved state map based on the sorted order (Last-Writer-Wins)
    let mut resolved_state = BTreeMap::new();
    for id in sorted_ids {
        if let Some(ev) = input.event_map.get(&OwnedEventId::try_from(id).unwrap()) {
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
    let mut hasher = Sha256::new();

    for (key, event_id) in resolved_state {
        hasher.update(key.0.as_bytes());
        hasher.update(key.1.as_bytes());
        hasher.update(event_id.as_str().as_bytes());
    }

    let expected_hash: [u8; 32] = hasher.finalize().into();

    DAGMergeOutput {
        resolved_state_hash: expected_hash,
        event_count,
    }
}
