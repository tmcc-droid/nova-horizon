extends RefCounted
class_name NetCodec
## JSON WebSocket codec — hand-synced with crates/protocol + protocol/fixtures.
## Golden parity: field names and enum string values must match Rust serde output.

const PROTOCOL_VERSION := 1
const CONTENT_VERSION := "0.1.0-dev"


static func encode_auth_hello(session_id: String, connect_ticket: String) -> String:
	return JSON.stringify({
		"t": "AuthHello",
		"v": PROTOCOL_VERSION,
		"session_id": session_id,
		"connect_ticket": connect_ticket,
		"client_content_version": CONTENT_VERSION,
		"client_protocol_v": PROTOCOL_VERSION,
	})


static func encode_input_frame(
	input_seq: int,
	thrust: float,
	turn: float,
	fire_mask: int,
	target_id: Variant = null
) -> String:
	var msg := {
		"t": "InputFrame",
		"v": PROTOCOL_VERSION,
		"input_seq": input_seq,
		"thrust": thrust,
		"turn": turn,
		"fire_mask": fire_mask,
	}
	if target_id != null:
		msg["target_id"] = target_id
	return JSON.stringify(msg)


static func encode_dock_request(request_id: String, station_id: String) -> String:
	return JSON.stringify({
		"t": "DockRequest",
		"v": PROTOCOL_VERSION,
		"request_id": request_id,
		"station_id": station_id,
	})


static func encode_undock_request(request_id: String) -> String:
	return JSON.stringify({
		"t": "UndockRequest",
		"v": PROTOCOL_VERSION,
		"request_id": request_id,
	})


static func encode_trade_execute(
	request_id: String,
	station_id: String,
	commodity_id: String,
	side: String,
	quantity: int
) -> String:
	return JSON.stringify({
		"t": "TradeExecute",
		"v": PROTOCOL_VERSION,
		"request_id": request_id,
		"station_id": station_id,
		"commodity_id": commodity_id,
		"side": side, # "buy" | "sell"
		"quantity": quantity,
	})


static func encode_refuel_request(
	request_id: String,
	station_id: String,
	mode: String,
	quantity: Variant = null
) -> String:
	var msg := {
		"t": "RefuelRequest",
		"v": PROTOCOL_VERSION,
		"request_id": request_id,
		"station_id": station_id,
		"mode": mode, # "fill" | "quantity"
	}
	if quantity != null:
		msg["quantity"] = quantity
	return JSON.stringify(msg)


static func encode_hyperspace_request(request_id: String, dest_system_id: String) -> String:
	return JSON.stringify({
		"t": "HyperspaceRequest",
		"v": PROTOCOL_VERSION,
		"request_id": request_id,
		"dest_system_id": dest_system_id,
	})


static func encode_clock_sync_request(client_send_ms: int) -> String:
	return JSON.stringify({
		"t": "ClockSyncRequest",
		"v": PROTOCOL_VERSION,
		"client_send_ms": client_send_ms,
	})


static func decode_object(text: String) -> Variant:
	var json := JSON.new()
	var err := json.parse(text)
	if err != OK:
		push_error("NetCodec: JSON parse failed: %s" % json.get_error_message())
		return null
	return json.data


static func message_type(obj: Variant) -> String:
	if typeof(obj) != TYPE_DICTIONARY:
		return ""
	return str(obj.get("t", ""))
