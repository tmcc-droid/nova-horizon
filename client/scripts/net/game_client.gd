extends Node
## Network client — robust login + WebSocket join.

signal status_changed(text: String)
signal self_state(state: Dictionary)
signal entity_spawn(ent: Dictionary)
signal entity_despawn(id: String)
signal entity_snapshot(snap: Dictionary)
signal auth_ok(info: Dictionary)
signal station_menu(menu: Dictionary)
signal dock_result(msg: Dictionary)
signal combat_event(msg: Dictionary)
signal local_fire(fire_mask: int)
signal galaxy_map(msg: Dictionary)
signal jump_arrive(msg: Dictionary)
## Fired when the live WebSocket drops after a successful join (or mid-session).
signal connection_lost(reason: String)

const PROTOCOL_VERSION := 1
const CONTENT_VERSION := "0.1.0-dev"
const RECONCILE_EPSILON := 2.0
const INPUT_RING_MAX := 40

var base_http: String = "http://127.0.0.1:8080"
var ws_url: String = "ws://127.0.0.1:8080/ws"
var _http: HTTPRequest
var _ws := WebSocketPeer.new()
var _ws_active := false
var _busy := false

var access_token: String = ""
var session_id: String = ""
var refresh_token: String = ""
var character_id: String = ""
var ship_id: String = ""
var input_seq: int = 0
var _input_ring: Array = []
var pred_x: float = 0.0
var pred_y: float = 0.0
var pred_rot: float = 0.0
var pred_vx: float = 0.0
var pred_vy: float = 0.0
var thrust: float = 220.0
var max_speed: float = 500.0
var turn_rate: float = 2.094
var last_processed_seq: int = 0
var remote_ships: Dictionary = {}
var galaxy: Dictionary = {}
var current_system_id: String = ""
## True only after AuthOk; mid-handshake closes won't flash "connection lost".
var _session_ready: bool = false


func _ready() -> void:
	_http = HTTPRequest.new()
	_http.timeout = 12.0
	# Threaded requests avoid stalls if the frame loop is busy.
	_http.use_threads = true
	add_child(_http)


func _process(_delta: float) -> void:
	# Only the connect handshake polls while _busy; avoid dual-poll races.
	if not _ws_active or _busy:
		return
	_ws.poll()
	while _ws.get_available_packet_count() > 0:
		var pkt := _ws.get_packet()
		_on_ws_text(pkt.get_string_from_utf8())
	var st := _ws.get_ready_state()
	if st == WebSocketPeer.STATE_CLOSED or st == WebSocketPeer.STATE_CLOSING:
		_handle_ws_closed()


func _handle_ws_closed() -> void:
	var was_ready := _session_ready
	_ws_active = false
	_busy = false
	_session_ready = false
	if was_ready:
		ship_id = ""
		remote_ships.clear()
		connection_lost.emit("Connection lost — press Login & Play to rejoin")
	else:
		status_changed.emit("Connection closed — press Login & Play")


func is_live() -> bool:
	return (
		_session_ready
		and _ws_active
		and _ws.get_ready_state() == WebSocketPeer.STATE_OPEN
		and not ship_id.is_empty()
	)


## Call after hyperspace / respawn so prediction matches server transform.
## Does NOT reset input_seq — server tracks seq across the channel; zeroing it
## would make all new frames look "stale" relative to last_processed.
func reset_prediction(x: float, y: float, rot: float) -> void:
	pred_x = x
	pred_y = y
	pred_rot = rot
	pred_vx = 0.0
	pred_vy = 0.0
	_input_ring.clear()
	# Keep last_processed_seq until next SelfState; ring is empty so no bad replay
func configure(http_base: String, websocket_url: String) -> void:
	base_http = http_base.trim_suffix("/")
	ws_url = websocket_url


func register_or_login(email: String, password: String) -> void:
	if _busy:
		status_changed.emit("Already connecting… please wait")
		return
	_busy = true
	status_changed.emit("1/5 Contacting server…")

	var body := JSON.stringify({"email": email, "password": password})
	var text := await _http_json("POST", base_http + "/auth/login", body, [])
	if text.is_empty():
		# try register
		status_changed.emit("1/5 Registering new account…")
		text = await _http_json("POST", base_http + "/auth/register", body, [])
		if text.is_empty():
			_busy = false
			return

	var data = JSON.parse_string(text)
	if typeof(data) != TYPE_DICTIONARY:
		status_changed.emit("Bad login response")
		_busy = false
		return

	access_token = str(data.get("access_token", ""))
	session_id = str(data.get("session_id", ""))
	refresh_token = str(data.get("refresh_token", ""))
	if access_token.is_empty() or session_id.is_empty():
		status_changed.emit("Login missing tokens — is server running?")
		_busy = false
		return

	status_changed.emit("2/5 Loading pilot…")
	await _ensure_character()
	# _busy cleared in _connect_ws success/fail


func _ensure_character() -> void:
	var headers := [
		"Content-Type: application/json",
		"Authorization: Bearer %s" % access_token,
	]
	var text := await _http_json("GET", base_http + "/characters", "", headers)
	if text.is_empty():
		_busy = false
		return
	var list = JSON.parse_string(text)
	if list is Array and list.size() > 0:
		character_id = str(list[0].get("id", ""))
	else:
		status_changed.emit("2/5 Creating pilot…")
		var name := "Pilot%d" % randi_range(1000, 9999)
		text = await _http_json(
			"POST",
			base_http + "/characters",
			JSON.stringify({"name": name}),
			headers
		)
		if text.is_empty():
			_busy = false
			return
		var ch = JSON.parse_string(text)
		character_id = str(ch.get("id", ""))

	if character_id.is_empty():
		status_changed.emit("No character id")
		_busy = false
		return

	status_changed.emit("3/5 Entering sector…")
	await _play_and_connect()


func _play_and_connect() -> void:
	var body := JSON.stringify({
		"session_id": session_id,
		"refresh_token": refresh_token,
		"character_id": character_id,
	})
	var text := await _http_json("POST", base_http + "/auth/play", body, ["Content-Type: application/json"])
	if text.is_empty():
		_busy = false
		return
	var play = JSON.parse_string(text)
	if typeof(play) != TYPE_DICTIONARY:
		status_changed.emit("Bad play response")
		_busy = false
		return
	ship_id = str(play.get("ship_id", ""))
	session_id = str(play.get("session_id", session_id))
	var ticket: String = str(play.get("connect_ticket", refresh_token))
	if ship_id.is_empty():
		status_changed.emit("Play failed: no ship")
		_busy = false
		return
	status_changed.emit("4/5 Opening WebSocket…")
	await _connect_ws(ticket)


func _connect_ws(connect_ticket: String) -> void:
	# Reset peer cleanly
	_session_ready = false
	if _ws.get_ready_state() != WebSocketPeer.STATE_CLOSED:
		_ws.close()
	_ws = WebSocketPeer.new()
	# Keep _ws_active false during handshake so _process does not dual-poll
	_ws_active = false

	var err := _ws.connect_to_url(ws_url)
	if err != OK:
		status_changed.emit("WS connect failed (err %s). Is server on :8080?" % err)
		_busy = false
		return

	var deadline := Time.get_ticks_msec() + 8000
	while _ws.get_ready_state() != WebSocketPeer.STATE_OPEN:
		_ws.poll()
		await get_tree().process_frame
		if Time.get_ticks_msec() > deadline:
			var st := _ws.get_ready_state()
			status_changed.emit("WS timeout (state=%s). Restart game-server." % st)
			_busy = false
			return

	status_changed.emit("5/5 Authenticating session…")
	var hello := {
		"t": "AuthHello",
		"v": PROTOCOL_VERSION,
		"session_id": session_id,
		"connect_ticket": connect_ticket,
		"client_content_version": CONTENT_VERSION,
		"client_protocol_v": PROTOCOL_VERSION,
	}
	_ws.send_text(JSON.stringify(hello))

	# Wait for AuthOk / AuthFail (handshake still owns the socket)
	deadline = Time.get_ticks_msec() + 8000
	while Time.get_ticks_msec() < deadline:
		_ws.poll()
		while _ws.get_available_packet_count() > 0:
			var msg_text := _ws.get_packet().get_string_from_utf8()
			_on_ws_text(msg_text)
			if _session_ready:
				# Hand off to _process for ongoing traffic
				_ws_active = true
				_busy = false
				status_changed.emit("Loading galaxy chart…")
				await _fetch_galaxy_chart()
				return
			if not _busy:
				# AuthFail path
				return
		await get_tree().process_frame

	if _busy:
		status_changed.emit("No AuthOk from server — check server logs")
		_busy = false


func _fetch_galaxy_chart() -> void:
	var ok := await ensure_galaxy_chart()
	if ok:
		var n := 0
		if galaxy.has("systems"):
			n = (galaxy.get("systems") as Array).size()
		status_changed.emit("In sector · %d systems charted · G for map" % n)
	else:
		status_changed.emit("Joined, but galaxy chart failed to load (map limited)")


## Load (or reload) the fixed galaxy chart over HTTP. Returns true on success.
func ensure_galaxy_chart() -> bool:
	var text := await _http_json("GET", base_http + "/galaxy", "", [])
	if text.is_empty():
		return false
	var data = JSON.parse_string(text)
	if typeof(data) != TYPE_DICTIONARY:
		return false
	if not data.has("systems") or typeof(data.get("systems")) != TYPE_ARRAY:
		return false
	if (data.get("systems") as Array).is_empty():
		return false
	galaxy = data
	if current_system_id != "":
		galaxy["current_system_id"] = current_system_id
	galaxy_map.emit(galaxy)
	return true


## Returns response body text, or "" on failure (status already emitted).
func _http_json(method: String, url: String, body: String, headers: Array) -> String:
	var hdrs: PackedStringArray = PackedStringArray()
	if headers.is_empty() and method != "GET":
		hdrs.append("Content-Type: application/json")
	else:
		for h in headers:
			hdrs.append(str(h))

	var http_method := HTTPClient.METHOD_GET
	match method:
		"POST":
			http_method = HTTPClient.METHOD_POST
		"PUT":
			http_method = HTTPClient.METHOD_PUT
		_:
			http_method = HTTPClient.METHOD_GET

	# Cancel any stuck request
	_http.cancel_request()
	var err := _http.request(url, hdrs, http_method, body)
	if err != OK:
		status_changed.emit("HTTP request error %s (server down?)" % err)
		return ""

	var result: Array = await _http.request_completed
	# result: result, response_code, headers, body
	var code: int = int(result[1])
	var text: String = (result[3] as PackedByteArray).get_string_from_utf8()

	if code == 0:
		status_changed.emit("No response from %s — start game-server" % base_http)
		return ""
	if code >= 400:
		# login 401 is normal before register
		if code == 401 and url.ends_with("/auth/login"):
			return ""
		var brief := text
		if brief.length() > 120:
			brief = brief.substr(0, 120) + "…"
		status_changed.emit("HTTP %s: %s" % [code, brief])
		return ""
	return text


func send_input(thrust_a: float, turn: float, fire_mask: int = 0) -> void:
	if not _ws_active or _ws.get_ready_state() != WebSocketPeer.STATE_OPEN:
		return
	input_seq += 1
	_input_ring.append({"seq": input_seq, "thrust": thrust_a, "turn": turn})
	while _input_ring.size() > INPUT_RING_MAX:
		_input_ring.pop_front()
	var step := predict_step(pred_x, pred_y, pred_rot, pred_vx, pred_vy, thrust_a, turn, 1.0 / 20.0)
	pred_x = step.x
	pred_y = step.y
	pred_rot = step.rot
	pred_vx = step.vx
	pred_vy = step.vy
	_ws.send_text(JSON.stringify({
		"t": "InputFrame", "v": PROTOCOL_VERSION,
		"input_seq": input_seq, "thrust": thrust_a, "turn": turn, "fire_mask": fire_mask,
	}))
	if fire_mask != 0:
		local_fire.emit(fire_mask)


func send_dock(station_id: String) -> void:
	_rpc("DockRequest", {"station_id": station_id})


func send_undock() -> void:
	_rpc("UndockRequest", {})


func send_trade(station_id: String, commodity_id: String, buy: bool, qty: int) -> void:
	_rpc("TradeExecute", {
		"station_id": station_id,
		"commodity_id": commodity_id,
		"side": "buy" if buy else "sell",
		"quantity": qty,
	})


func send_refuel(station_id: String) -> void:
	_rpc("RefuelRequest", {"station_id": station_id, "mode": "fill"})


func send_jump(dest: String) -> void:
	_rpc("HyperspaceRequest", {"dest_system_id": dest})


func _rpc(t: String, fields: Dictionary) -> void:
	if not _ws_active or _ws.get_ready_state() != WebSocketPeer.STATE_OPEN:
		return
	fields["t"] = t
	fields["v"] = PROTOCOL_VERSION
	fields["request_id"] = _uuid()
	_ws.send_text(JSON.stringify(fields))


func _uuid() -> String:
	return "%08x-%04x-%04x-%04x-%012x" % [
		randi(), randi() % 0x10000, (randi() % 0x1000) | 0x4000,
		(randi() % 0x4000) | 0x8000, randi() | (randi() << 16)
	]


func predict_step(
	x: float, y: float, rot: float, vx: float, vy: float,
	thrust_axis: float, turn_axis: float, dt: float
) -> Dictionary:
	thrust_axis = clampf(thrust_axis, -1.0, 1.0)
	turn_axis = clampf(turn_axis, -1.0, 1.0)
	var omega: float = turn_rate * turn_axis
	rot += omega * dt
	var c: float = cos(rot)
	var s: float = sin(rot)
	vx += c * thrust * thrust_axis * dt
	vy += s * thrust * thrust_axis * dt
	var speed: float = sqrt(vx * vx + vy * vy)
	if speed > max_speed and speed > 0.0:
		var k: float = max_speed / speed
		vx *= k
		vy *= k
	return {"x": x + vx * dt, "y": y + vy * dt, "rot": rot, "vx": vx, "vy": vy}


func _on_ws_text(text: String) -> void:
	var msg = JSON.parse_string(text)
	if typeof(msg) != TYPE_DICTIONARY:
		return
	match str(msg.get("t", "")):
		"AuthOk":
			current_system_id = str(msg.get("system_id", current_system_id))
			if msg.has("ship_id") and str(msg.get("ship_id", "")) != "":
				ship_id = str(msg.get("ship_id"))
			_session_ready = true
			# _busy cleared by handshake after galaxy fetch starts
			status_changed.emit("In world: %s" % current_system_id)
			auth_ok.emit(msg)
		"AuthFail":
			_busy = false
			_session_ready = false
			_ws_active = false
			status_changed.emit("AuthFail: %s" % msg.get("message", msg.get("code", "?")))
		"SelfState":
			_reconcile_self(msg)
			self_state.emit(msg)
		"EntitySpawn":
			var id := str(msg.get("id", ""))
			if id != ship_id:
				remote_ships[id] = msg
			entity_spawn.emit(msg)
		"EntityDespawn":
			var id2 := str(msg.get("id", ""))
			remote_ships.erase(id2)
			entity_despawn.emit(id2)
		"EntitySnapshot":
			for e in msg.get("entities", []):
				var eid := str(e.get("id", ""))
				if remote_ships.has(eid):
					remote_ships[eid]["x"] = e.get("x", 0)
					remote_ships[eid]["y"] = e.get("y", 0)
					remote_ships[eid]["rot"] = e.get("rot", 0)
			entity_snapshot.emit(msg)
		"StationMenu":
			station_menu.emit(msg)
		"DockResult":
			dock_result.emit(msg)
		"TradeResult", "RefuelResult":
			status_changed.emit("%s ok=%s" % [msg.get("t"), msg.get("ok")])
		"JumpCountdown":
			status_changed.emit("Jump channel… %s" % str(msg.get("dest_system_id", "")))
		"JumpArrive":
			# Hard snap prediction — leftover input ring causes broken flight after jump
			reset_prediction(
				float(msg.get("x", 0)),
				float(msg.get("y", 0)),
				float(msg.get("rot", 0))
			)
			current_system_id = str(msg.get("system_id", current_system_id))
			if galaxy is Dictionary:
				galaxy["current_system_id"] = current_system_id
			remote_ships.clear()
			status_changed.emit("Arrived %s" % current_system_id)
			jump_arrive.emit(msg)
		"JumpRejected":
			status_changed.emit("Jump rejected: %s" % msg.get("code"))
		"GalaxyMap":
			galaxy = msg
			current_system_id = str(msg.get("current_system_id", current_system_id))
			galaxy_map.emit(msg)
		"EventCombat":
			combat_event.emit(msg)
			var k := str(msg.get("kind", ""))
			if k == "Hit":
				status_changed.emit("Hit confirmed")
			elif k == "Death":
				status_changed.emit("Kill / destruction")
			elif k == "FireDenied":
				status_changed.emit("Weapons locked (%s)" % str(msg.get("reason", "denied")))
		"ServerNotice":
			status_changed.emit(str(msg.get("message", "")))


func _reconcile_self(msg: Dictionary) -> void:
	var sx: float = float(msg.get("x", 0))
	var sy: float = float(msg.get("y", 0))
	var srot: float = float(msg.get("rot", 0))
	last_processed_seq = int(msg.get("last_processed_input_seq", 0))
	var err := Vector2(pred_x - sx, pred_y - sy).length()
	if err > RECONCILE_EPSILON or absf(pred_rot - srot) > 0.2:
		pred_x = sx
		pred_y = sy
		pred_rot = srot
		pred_vx = float(msg.get("vx", 0))
		pred_vy = float(msg.get("vy", 0))
		for item in _input_ring:
			if int(item.seq) > last_processed_seq:
				var step := predict_step(
					pred_x, pred_y, pred_rot, pred_vx, pred_vy,
					float(item.thrust), float(item.turn), 1.0 / 20.0
				)
				pred_x = step.x
				pred_y = step.y
				pred_rot = step.rot
				pred_vx = step.vx
				pred_vy = step.vy
	var kept: Array = []
	for item in _input_ring:
		if int(item.seq) > last_processed_seq:
			kept.append(item)
	_input_ring = kept
