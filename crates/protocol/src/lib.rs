//! Wire protocol types for Nova Horizon.
//!
//! Source of truth for message *shapes* also lives in `protocol/json/` and
//! `protocol/fixtures/`. This crate is the server (and golden-test) implementation.
//!
//! Envelope form (MVP): **flat** JSON object with discriminant `t` + version `v`.
//! WebSocket text frames; no transport-level seq/ack (TCP already reliable).

mod messages;

pub use messages::*;

use uuid::Uuid;

/// Protocol version negotiated at connect.
pub const PROTOCOL_VERSION: u16 = 1;

/// Content pack version must match client and server at join (MVP).
pub const DEFAULT_CONTENT_VERSION: &str = "0.1.0-dev";

/// Fixed namespace for station UUID v5 (content string id → wire id).
/// Key Decision #22 / design Appendix C.
pub const STATION_NAMESPACE: Uuid = Uuid::from_bytes([
    0x6b, 0xa7, 0xb8, 0x10, 0x9d, 0xad, 0x11, 0xd1, 0x80, 0xb4, 0x00, 0xc0, 0x4f, 0xd4, 0x30, 0xc8,
]);

/// Map a station content id (e.g. `st.earth_orbit`) to its wire UUID.
pub fn station_wire_id(content_id: &str) -> Uuid {
    Uuid::new_v5(&STATION_NAMESPACE, content_id.as_bytes())
}

/// Parse a single JSON text frame into a [`WireMessage`].
pub fn decode_frame(text: &str) -> Result<WireMessage, ProtocolError> {
    serde_json::from_str(text).map_err(|e| ProtocolError::Json(e.to_string()))
}

/// Encode a [`WireMessage`] as a JSON text frame.
pub fn encode_frame(msg: &WireMessage) -> Result<String, ProtocolError> {
    serde_json::to_string(msg).map_err(|e| ProtocolError::Json(e.to_string()))
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ProtocolError {
    #[error("json error: {0}")]
    Json(String),
    #[error("unknown message type: {0}")]
    UnknownType(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn fixtures_dir() -> PathBuf {
        // crates/protocol -> repo root protocol/fixtures
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../protocol/fixtures")
            .canonicalize()
            .expect("protocol/fixtures must exist")
    }

    #[test]
    fn station_wire_id_is_stable() {
        let a = station_wire_id("st.earth_orbit");
        let b = station_wire_id("st.earth_orbit");
        let c = station_wire_id("st.mars_depot");
        assert_eq!(a, b);
        assert_ne!(a, c);
        // Documented bridge: deterministic v5
        assert_eq!(a.get_version(), Some(uuid::Version::Sha1));
    }

    #[test]
    fn auth_hello_roundtrip() {
        let msg = WireMessage::AuthHello(AuthHello {
            v: PROTOCOL_VERSION,
            session_id: Uuid::nil(),
            connect_ticket: "test-ticket".into(),
            client_content_version: DEFAULT_CONTENT_VERSION.into(),
            client_protocol_v: PROTOCOL_VERSION,
        });
        let json = encode_frame(&msg).unwrap();
        let back = decode_frame(&json).unwrap();
        assert_eq!(msg, back);
        assert!(json.contains("\"t\":\"AuthHello\"") || json.contains("\"t\": \"AuthHello\""));
    }

    #[test]
    fn input_frame_has_no_pose_fields_in_json() {
        let msg = WireMessage::InputFrame(InputFrame {
            v: PROTOCOL_VERSION,
            input_seq: 1,
            thrust: 1.0,
            turn: -0.5,
            fire_mask: 1,
            target_id: None,
        });
        let json = encode_frame(&msg).unwrap();
        assert!(!json.contains("\"x\""));
        assert!(!json.contains("\"pos\""));
        assert!(!json.contains("velocity"));
    }

    /// Golden: every fixture in protocol/fixtures must decode and re-encode stably
    /// (parse → serialize → parse equality).
    #[test]
    fn golden_fixtures_roundtrip() {
        let dir = fixtures_dir();
        let mut count = 0usize;
        for entry in fs::read_dir(&dir).expect("read fixtures") {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            // Skip schema-only or non-message files if any
            let name = path.file_name().unwrap().to_string_lossy();
            if name.starts_with('_') {
                continue;
            }
            let text = fs::read_to_string(&path).unwrap_or_else(|e| {
                panic!("read {}: {e}", path.display());
            });
            let msg = decode_frame(&text).unwrap_or_else(|e| {
                panic!("decode {}: {e}\n{text}", path.display());
            });
            let encoded = encode_frame(&msg).unwrap();
            let again = decode_frame(&encoded).unwrap();
            assert_eq!(msg, again, "fixture {}", path.display());
            count += 1;
        }
        assert!(
            count >= 10,
            "expected many golden fixtures, found {count} in {}",
            dir.display()
        );
    }

    #[test]
    fn known_fixture_auth_hello_fields() {
        let path = fixtures_dir().join("auth_hello.json");
        let text = fs::read_to_string(path).unwrap();
        let msg = decode_frame(&text).unwrap();
        match msg {
            WireMessage::AuthHello(h) => {
                assert_eq!(h.client_content_version, "0.1.0-dev");
                assert_eq!(h.client_protocol_v, 1);
            }
            other => panic!("expected AuthHello, got {other:?}"),
        }
    }
}
