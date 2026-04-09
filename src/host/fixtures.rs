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

use crate::GuestEvent;
use ruma_common::{OwnedEventId, OwnedRoomId, OwnedUserId};
use ruma_events::TimelineEventType;
use serde_json::{json, value::RawValue};

/// Concise Fixture DSL Parser
/// Format: (Sender, PowerLevel, Timestamp, Depth, Parents, StateKey)
pub type FixtureRow<'a> = (&'a str, i64, u64, u64, &'a [&'a str], &'a str);

pub fn parse_fixture_rows(room_id: &OwnedRoomId, rows: &[FixtureRow]) -> Vec<GuestEvent> {
    let mut events = Vec::new();

    for (i, r) in rows.iter().enumerate() {
        let (sender_name, _power_level, ts, depth, parents, state_key) = *r;

        let event_id = OwnedEventId::try_from(format!("${}:example.com", i)).unwrap();
        let sender = OwnedUserId::try_from(format!("@{}:example.com", sender_name)).unwrap();

        let event_json = json!({
            "event_id": event_id,
            "room_id": room_id,
            "sender": sender,
            "type": "m.room.member",
            "state_key": state_key,
            "content": { "membership": "join" },
            "origin_server_ts": ts,
            "prev_events": parents.iter().map(|p| format!("${}:example.com", p)).collect::<Vec<_>>(),
            "auth_events": [],
            "depth": depth,
        });

        let event_obj = serde_json::from_value(event_json).unwrap();
        let content = RawValue::from_string("{\"membership\":\"join\"}".to_string()).unwrap();

        events.push(GuestEvent {
            event: event_obj,
            content,
            event_id,
            room_id: room_id.clone(),
            sender,
            event_type: TimelineEventType::RoomMember,
            prev_events: parents
                .iter()
                .map(|p| OwnedEventId::try_from(format!("${}:example.com", p)).unwrap())
                .collect(),
            auth_events: vec![],
            public_key: None,
            signature: None,
            verified_on_host: false,
        });
    }

    events
}

#[cfg(test)]
mod tests {
    use super::*;
    use ruma_common::OwnedRoomId;

    #[test]
    fn test_fixture_parsing() {
        let room_id = OwnedRoomId::try_from("!test:example.com").unwrap();
        let rows: &[FixtureRow] = &[
            ("Alice", 100, 10, 1, &[], "alice"),
            ("Bob", 50, 20, 2, &["0"], "bob"),
        ];

        let events = parse_fixture_rows(&room_id, rows);
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].sender.to_string(), "@Alice:example.com");
        assert_eq!(events[1].prev_events[0].to_string(), "$0:example.com");
    }
}
