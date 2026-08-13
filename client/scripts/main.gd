extends Node2D
## Flight client with polished HUD, space visuals, and combat juice.

const CONTENT_VERSION := "0.1.0-dev"
const FIXED_DT := 1.0 / 20.0
const GameClientScript = preload("res://scripts/net/game_client.gd")
const ThemeFactory = preload("res://scripts/ui/theme_factory.gd")
const EntityFactory = preload("res://scripts/visuals/entity_factory.gd")
const JuiceScript = preload("res://scripts/visuals/juice.gd")
const StationConsoleScript = preload("res://scripts/ui/station_console.gd")
const GalaxyMapScript = preload("res://scripts/ui/galaxy_map.gd")

@onready var status_label: Label = $HUD/TopBar/Margin/HBox/TitleBlock/StatusLabel
@onready var system_label: Label = $HUD/TopBar/Margin/HBox/SystemLabel
@onready var credits_label: Label = $HUD/TopBar/Margin/HBox/CreditsLabel
@onready var shield_bar: ProgressBar = $HUD/FlightHUD/Margin/VBox/ShieldRow/ShieldBar
@onready var armor_bar: ProgressBar = $HUD/FlightHUD/Margin/VBox/ArmorRow/ArmorBar
@onready var energy_bar: ProgressBar = $HUD/FlightHUD/Margin/VBox/EnergyRow/EnergyBar
@onready var fuel_bar: ProgressBar = $HUD/FlightHUD/Margin/VBox/FuelRow/FuelBar
@onready var camera: Camera2D = $Camera2D
@onready var player_anchor: Node2D = $PlayerAnchor
@onready var email_edit: LineEdit = $HUD/LoginPanel/Margin/VBox/Email
@onready var password_edit: LineEdit = $HUD/LoginPanel/Margin/VBox/Password
@onready var connect_btn: Button = $HUD/LoginPanel/Margin/VBox/ConnectBtn
@onready var login_panel: Control = $HUD/LoginPanel
@onready var remotes_root: Node2D = $Remotes
@onready var starfield: Node2D = $Starfield
@onready var hud_root: CanvasLayer = $HUD
@onready var flight_hud: Control = $HUD/FlightHUD
@onready var map_panel: Control = $HUD/MapPanel
@onready var map_legend: Label = $HUD/MapLegend
@onready var minimap: Control = $HUD/MapPanel/Margin/Minimap
@onready var top_bar: Control = $HUD/TopBar

var client: Node
var player_ship: Node2D
var juice: Node
var station_console: Control
var galaxy_map_ui: Control
var _remote_nodes: Dictionary = {}
var _fixed_accum: float = 0.0
var _docked_station_id: String = ""
var _station_menu: Dictionary = {}
var _last_thrust: float = 0.0
var _in_world: bool = false
var _docked: bool = false
var _muzzle_cd: float = 0.0
var _prev_shield: float = -1.0
var _prev_armor: float = -1.0
var _fx_layer: Node2D
var _map_range: float = 12000.0
var _combat_fx_suppress: float = 0.0
var _credits_cache: int = 0
var _fuel_cache: int = 0


func _ready() -> void:
	await get_tree().process_frame
	for child in hud_root.get_children():
		if child is Control:
			ThemeFactory.apply_to_tree(child as Control)
	ThemeFactory.style_bar(shield_bar, Color("3dd6ff"))
	ThemeFactory.style_bar(armor_bar, Color("ff8a6a"))
	ThemeFactory.style_bar(energy_bar, Color("7cf5c8"))
	ThemeFactory.style_bar(fuel_bar, Color("ffc857"))

	if camera:
		camera.make_current()

	_fx_layer = Node2D.new()
	_fx_layer.name = "FxLayer"
	_fx_layer.z_index = 50
	add_child(_fx_layer)

	juice = JuiceScript.new()
	add_child(juice)

	player_ship = EntityFactory.make_player_ship()
	player_anchor.add_child(player_ship)

	client = GameClientScript.new()
	add_child(client)
	client.status_changed.connect(_on_status)
	client.self_state.connect(_on_self_state)
	client.entity_spawn.connect(_on_spawn)
	client.entity_despawn.connect(_on_despawn)
	client.entity_snapshot.connect(func(_s): pass)
	client.auth_ok.connect(_on_auth_ok)
	client.station_menu.connect(_on_station_menu)
	client.dock_result.connect(_on_dock_result)
	client.combat_event.connect(_on_combat)
	client.local_fire.connect(_on_local_fire)
	client.galaxy_map.connect(_on_galaxy_map_msg)
	client.jump_arrive.connect(_on_jump_arrive)
	client.connection_lost.connect(_on_connection_lost)
	connect_btn.pressed.connect(_on_connect)
	if connect_btn:
		connect_btn.text = "Login & Play"
	if has_node("HUD/LoginPanel/Margin/VBox/Hint"):
		$HUD/LoginPanel/Margin/VBox/Hint.text = "Server: 127.0.0.1:8080  ·  Arrows/WASD fly  ·  Space fire"

	station_console = StationConsoleScript.new()
	hud_root.add_child(station_console)
	ThemeFactory.apply_to_tree(station_console)
	station_console.undock_pressed.connect(_undock)
	station_console.buy_pressed.connect(func(): _trade(true))
	station_console.sell_pressed.connect(func(): _trade(false))
	station_console.refuel_pressed.connect(_refuel)
	station_console.jump_pressed.connect(_jump_quick)
	station_console.galaxy_map_pressed.connect(_open_galaxy_map)
	if station_console.has_signal("jump_to_system"):
		station_console.jump_to_system.connect(_jump_to)
	station_console.visible = false

	galaxy_map_ui = GalaxyMapScript.new()
	hud_root.add_child(galaxy_map_ui)
	ThemeFactory.apply_to_tree(galaxy_map_ui)
	galaxy_map_ui.jump_requested.connect(_jump_to)
	galaxy_map_ui.closed.connect(_on_galaxy_map_closed)
	galaxy_map_ui.visible = false

	# Hide legacy side panel if present in scene
	if has_node("HUD/StationPanel"):
		$HUD/StationPanel.visible = false

	# Radar sits lower-right (see main.tscn) so station header/content stay clear
	if map_panel:
		map_panel.z_index = 20
		_place_radar_bottom_right()
	if map_legend:
		map_legend.z_index = 20
	if flight_hud:
		flight_hud.z_index = 20

	flight_hud.visible = false
	map_panel.visible = false
	map_legend.visible = false
	email_edit.text = "pilot@example.com"
	password_edit.text = "password123"
	status_label.text = "Ready  ·  content %s" % CONTENT_VERSION
	if minimap:
		minimap.map_range_wu = _map_range


func _on_status(t: String) -> void:
	status_label.text = t
	# If join failed, allow another attempt
	var fail := (
		t.contains("failed")
		or t.contains("timeout")
		or t.contains("HTTP")
		or t.contains("AuthFail")
		or t.contains("server")
		or t.contains("error")
		or t.contains("closed")
		or t.contains("Connection")
	)
	if fail:
		connect_btn.disabled = false
		if not _in_world:
			login_panel.visible = true


func _on_connection_lost(reason: String) -> void:
	_in_world = false
	_docked = false
	_set_docked_mode(false)
	if station_console:
		station_console.close_console()
	if galaxy_map_ui:
		galaxy_map_ui.close_map()
	flight_hud.visible = false
	map_panel.visible = false
	map_legend.visible = false
	login_panel.visible = true
	connect_btn.disabled = false
	connect_btn.text = "Login & Play"
	# Bring login to front
	hud_root.move_child(login_panel, -1)
	status_label.text = reason
	# Clear remotes so a rejoin is clean
	for id in _remote_nodes.keys():
		var node: Node = _remote_nodes[id]
		if is_instance_valid(node):
			node.queue_free()
	_remote_nodes.clear()


func _on_auth_ok(info: Dictionary) -> void:
	login_panel.visible = false
	connect_btn.disabled = false
	_in_world = true
	_docked = false
	_set_docked_mode(false)
	var sys_id := str(info.get("system_id", ""))
	var sys := sys_id.replace("sys.", "").to_upper()
	system_label.text = "SYS %s" % sys
	status_label.text = "Flying · arrows/WASD · Space fire · D dock · G map"
	_prev_shield = -1.0
	_prev_armor = -1.0
	if minimap:
		minimap.set_system(sys)
	_show_flight_radar(true)
	# Focus leaves LineEdits so keys go to flight, not the login fields
	if connect_btn:
		connect_btn.release_focus()
	if email_edit:
		email_edit.release_focus()
	if password_edit:
		password_edit.release_focus()


func _on_connect() -> void:
	if client == null:
		return
	connect_btn.disabled = true
	status_label.text = "Linking to server…"
	# Reset local world flags for a clean join
	_in_world = false
	_docked = false
	client.configure("http://127.0.0.1:8080", "ws://127.0.0.1:8080/ws")
	await client.register_or_login(email_edit.text.strip_edges(), password_edit.text)
	# Re-enable if still on login screen (failed)
	if login_panel.visible:
		connect_btn.disabled = false


func _flight_axes() -> Vector2:
	# Returns (turn, thrust). Prefer physical keys so UI focus cannot eat arrows.
	var turn := 0.0
	var thrust := 0.0
	if Input.is_key_pressed(KEY_LEFT) or Input.is_key_pressed(KEY_A):
		turn -= 1.0
	if Input.is_key_pressed(KEY_RIGHT) or Input.is_key_pressed(KEY_D):
		turn += 1.0
	if Input.is_key_pressed(KEY_UP) or Input.is_key_pressed(KEY_W):
		thrust += 1.0
	if Input.is_key_pressed(KEY_DOWN) or Input.is_key_pressed(KEY_S):
		thrust -= 0.5
	# Fallback for default InputMap ui_* if present
	if turn == 0.0:
		if Input.is_action_pressed("ui_left"):
			turn -= 1.0
		if Input.is_action_pressed("ui_right"):
			turn += 1.0
	if thrust == 0.0:
		if Input.is_action_pressed("ui_up"):
			thrust += 1.0
		if Input.is_action_pressed("ui_down"):
			thrust -= 0.5
	return Vector2(turn, thrust)


func _physics_process(delta: float) -> void:
	if _muzzle_cd > 0.0:
		_muzzle_cd -= delta
	if _combat_fx_suppress > 0.0:
		_combat_fx_suppress -= delta

	if client == null or not _in_world:
		return
	if str(client.ship_id) == "":
		return
	# Dead socket — stop pretending we can fly
	if client.has_method("is_live") and not client.is_live():
		return

	var galaxy_open := galaxy_map_ui != null and galaxy_map_ui.visible

	# While docked or galaxy map open: hold still (no flight input)
	if _docked or galaxy_open:
		_last_thrust = 0.0
		client.send_input(0.0, 0.0, 0)
		EntityFactory.set_thrust_visual(player_ship, 0.0)
		# Still follow server pose so undock/jump snaps cleanly
		player_anchor.position = Vector2(float(client.pred_x), float(client.pred_y))
		player_anchor.rotation = float(client.pred_rot) + PI / 2.0
		camera.position = player_anchor.position
		if _docked and not galaxy_open:
			_update_minimap()
		return

	_fixed_accum += delta
	while _fixed_accum >= FIXED_DT:
		_fixed_accum -= FIXED_DT
		var axes := _flight_axes()
		var turn: float = axes.x
		var thrust: float = axes.y
		var fire := 1 if Input.is_key_pressed(KEY_SPACE) else 0
		_last_thrust = maxf(thrust, 0.0)
		client.send_input(thrust, turn, fire)

	player_anchor.position = Vector2(float(client.pred_x), float(client.pred_y))
	player_anchor.rotation = float(client.pred_rot) + PI / 2.0
	EntityFactory.set_thrust_visual(player_ship, _last_thrust)

	# Camera follows with shake offset from juice
	var shake := Vector2.ZERO
	if juice:
		shake = juice.get_shake_offset()
	camera.position = player_anchor.position + shake

	for id in _remote_nodes.keys():
		var node: Node2D = _remote_nodes[id]
		if client.remote_ships.has(id):
			var d: Dictionary = client.remote_ships[id]
			var target := Vector2(float(d.get("x", 0)), float(d.get("y", 0)))
			node.position = node.position.lerp(target, clampf(delta * 12.0, 0.0, 1.0))
			node.rotation = float(d.get("rot", 0)) + PI / 2.0

	_update_minimap()
	# Keep radar visible every frame while flying (recovers if something hid it)
	if map_panel and not map_panel.visible:
		_show_flight_radar(true)


func _on_local_fire(_mask: int) -> void:
	if _muzzle_cd > 0.0 or player_ship == null:
		return
	# Local predictive muzzle (server may still deny in safe zone)
	_muzzle_cd = 0.12
	if juice:
		juice.spawn_muzzle_flash(player_ship, Vector2(0, -22))
		juice.kick_camera(3.5, 0.08)


func _on_combat(msg: Dictionary) -> void:
	if juice == null or client == null:
		return
	var kind := str(msg.get("kind", ""))
	var src := str(msg.get("source_id", ""))
	var tgt := str(msg.get("target_id", ""))
	var my_id := str(client.ship_id)
	var impact := Vector2(float(msg.get("x", 0)), float(msg.get("y", 0)))
	# Prefer live node positions when available
	if tgt == my_id:
		impact = player_anchor.global_position
	elif _remote_nodes.has(tgt):
		impact = (_remote_nodes[tgt] as Node2D).global_position

	var from_pos := impact
	if src == my_id:
		from_pos = player_anchor.global_position
	elif _remote_nodes.has(src):
		from_pos = (_remote_nodes[src] as Node2D).global_position

	var i_am_target := tgt == my_id
	var i_am_source := src == my_id

	match kind:
		"Hit":
			_combat_fx_suppress = 0.35
			# Tracer from shooter → impact (shows who is shooting whom)
			if i_am_target:
				# Incoming fire (pirate or player)
				juice.spawn_tracer(_fx_layer, from_pos, impact, Color(1.0, 0.45, 0.35, 0.95))
				if _remote_nodes.has(src):
					juice.spawn_muzzle_flash(_remote_nodes[src], Vector2(0, -18))
				juice.spawn_hit_burst(_fx_layer, impact, Color(1.0, 0.55, 0.35, 1.0))
				juice.spawn_shield_ripple(player_ship)
				juice.flash_node(player_ship, Color(1.5, 0.45, 0.4, 1.0), 0.16)
				juice.kick_camera(14.0, 0.22)
				status_label.text = "INCOMING HIT  -%s" % str(msg.get("damage", "?"))
			elif i_am_source:
				# Our shot landed on someone else — never flash ourselves as damaged
				juice.spawn_tracer(_fx_layer, from_pos, impact, Color(0.45, 0.9, 1.0, 0.9))
				juice.spawn_hit_burst(_fx_layer, impact, Color(1.0, 0.75, 0.3, 1.0))
				if _remote_nodes.has(tgt):
					juice.flash_node(_remote_nodes[tgt], Color(1.6, 0.7, 0.45, 1.0), 0.12)
				juice.kick_camera(3.0, 0.08)
				status_label.text = "Hit confirmed  -%s" % str(msg.get("damage", "?"))
			else:
				# Third-party combat
				juice.spawn_tracer(_fx_layer, from_pos, impact, Color(0.8, 0.8, 0.9, 0.5))
				juice.spawn_hit_burst(_fx_layer, impact, Color(0.9, 0.7, 0.4, 0.8))
		"Death":
			_combat_fx_suppress = 0.5
			if i_am_target:
				juice.spawn_tracer(_fx_layer, from_pos, impact, Color(1.0, 0.3, 0.25, 0.95))
				juice.spawn_hit_burst(_fx_layer, impact, Color(1.0, 0.3, 0.2, 1.0))
				juice.spawn_hit_burst(_fx_layer, impact + Vector2(8, -6), Color(1.0, 0.7, 0.2, 1.0))
				juice.kick_camera(22.0, 0.35)
				juice.flash_node(player_ship, Color(2.0, 0.4, 0.3, 1.0), 0.25)
				status_label.text = "Hull lost — respawned near station (brief invuln)"
				_prev_shield = -1.0
				_prev_armor = -1.0
			elif i_am_source:
				juice.spawn_tracer(_fx_layer, from_pos, impact, Color(0.5, 1.0, 0.6, 0.9))
				juice.spawn_hit_burst(_fx_layer, impact, Color(1.0, 0.35, 0.2, 1.0))
				if _remote_nodes.has(tgt):
					juice.flash_node(_remote_nodes[tgt], Color(2.0, 0.5, 0.3, 1.0), 0.2)
				juice.kick_camera(8.0, 0.15)
				status_label.text = "Target destroyed"
			else:
				juice.spawn_hit_burst(_fx_layer, impact, Color(1.0, 0.4, 0.25, 0.9))
		"Miss":
			if i_am_source:
				juice.spawn_tracer(
					_fx_layer,
					player_anchor.global_position,
					impact,
					Color(0.5, 0.75, 1.0, 0.35)
				)
		"FireDenied":
			if i_am_source or src.is_empty() or src == "<null>":
				juice.flash_node(player_ship, Color(0.6, 0.7, 1.0, 1.0), 0.1)
				status_label.text = "Weapons locked"


func _unhandled_input(event: InputEvent) -> void:
	if client == null or not _in_world:
		return
	if galaxy_map_ui and galaxy_map_ui.visible:
		return # galaxy map owns input
	if event is InputEventKey and event.pressed and not event.echo:
		# Galaxy map available docked or in flight
		if event.keycode == KEY_G or event.keycode == KEY_J:
			_open_galaxy_map()
			get_viewport().set_input_as_handled()
			return
		if _docked:
			return # station console handles U/Esc
		if event.keycode == KEY_D:
			_try_dock_nearest()
		elif event.keycode == KEY_U:
			_undock()
		elif event.keycode == KEY_M:
			_cycle_map_range()


func _cycle_map_range() -> void:
	# Toggle tactical / sector range
	if _map_range < 10000.0:
		_map_range = 12000.0
	elif _map_range < 20000.0:
		_map_range = 25000.0
	else:
		_map_range = 6000.0
	if minimap:
		minimap.map_range_wu = _map_range
	status_label.text = "Radar range %.0f wu" % _map_range


func _update_minimap() -> void:
	if minimap == null or client == null:
		return
	minimap.set_player(
		Vector2(float(client.pred_x), float(client.pred_y)),
		float(client.pred_rot)
	)
	var items: Array = []
	for id in client.remote_ships.keys():
		var d: Dictionary = client.remote_ships[id]
		var kind := str(d.get("kind", "ship"))
		var pos := Vector2(float(d.get("x", 0)), float(d.get("y", 0)))
		# Prefer live node position when available
		if _remote_nodes.has(id):
			pos = (_remote_nodes[id] as Node2D).position
		if kind == "station":
			var label := str(d.get("def_id", id)).replace("st.", "").replace("_", " ")
			items.append({"pos": pos, "kind": "station", "label": label, "id": id})
		else:
			var pirate := str(d.get("pilot_name", "")).begins_with("Pirate")
			items.append({
				"pos": pos,
				"kind": "pirate" if pirate else "player_other",
				"label": str(d.get("pilot_name", "")),
				"id": id,
			})
	minimap.set_blips(items)


func _try_dock_nearest() -> void:
	if client == null:
		return
	for id in client.remote_ships.keys():
		var d: Dictionary = client.remote_ships[id]
		if str(d.get("kind", "")) == "station":
			var dx: float = float(d.get("x", 0)) - float(client.pred_x)
			var dy: float = float(d.get("y", 0)) - float(client.pred_y)
			if dx * dx + dy * dy < 500.0 * 500.0:
				client.send_dock(id)
				status_label.text = "Docking…"
				return
	status_label.text = "No station in range"


func _on_self_state(msg: Dictionary) -> void:
	var sh := float(msg.get("shield", 0))
	var ar := float(msg.get("armor", 0))
	# Only use SelfState as damage FX backup if no combat packet just handled it
	# (prevents "I shot and hurt myself" double-feedback)
	if (
		_combat_fx_suppress <= 0.0
		and _prev_shield >= 0.0
		and (sh < _prev_shield - 0.5 or ar < _prev_armor - 0.5)
	):
		if juice and player_ship and not _docked:
			juice.spawn_shield_ripple(player_ship)
			juice.flash_node(player_ship, Color(1.3, 0.55, 0.5, 1.0), 0.12)
			juice.kick_camera(10.0, 0.16)
			status_label.text = "Hull damage!"
	_prev_shield = sh
	_prev_armor = ar

	shield_bar.max_value = maxf(maxf(shield_bar.max_value, sh), 140.0)
	armor_bar.max_value = maxf(maxf(armor_bar.max_value, ar), 110.0)
	shield_bar.value = sh
	armor_bar.value = ar
	var en: float = float(msg.get("energy", 0))
	energy_bar.max_value = maxf(maxf(energy_bar.max_value, en), 120.0)
	energy_bar.value = en
	fuel_bar.max_value = 10.0
	_fuel_cache = int(msg.get("fuel", _fuel_cache))
	fuel_bar.value = float(_fuel_cache)
	if bool(msg.get("invuln", false)) and not _docked:
		status_label.text = "Invulnerable — get clear of pirates"
	if msg.has("credits") and msg.get("credits") != null:
		_credits_cache = int(msg.get("credits"))
		credits_label.text = "₵ %s" % str(_credits_cache)
	var docked = msg.get("docked_station_id", null)
	var now_docked := docked != null and str(docked) != "" and str(docked) != "<null>"
	if now_docked:
		_docked_station_id = str(docked)
		if not _docked:
			# SelfState can arrive before StationMenu; enter docked mode early
			_set_docked_mode(true)
	else:
		_docked_station_id = ""
		if _docked:
			_set_docked_mode(false)
			if station_console:
				station_console.close_console()
	if station_console and station_console.visible:
		station_console.update_wallet(_credits_cache, _fuel_cache)


func _on_station_menu(msg: Dictionary) -> void:
	_station_menu = msg
	_set_docked_mode(true)
	if station_console:
		station_console.open_console(msg, _credits_cache, _fuel_cache)
		_fill_station_nav_links()
	status_label.text = "Docked · Market / Nav / Galaxy Map (G) · U undock"
	if juice:
		juice.kick_camera(2.0, 0.08)


func _fill_station_nav_links() -> void:
	if station_console == null or client == null:
		return
	if not station_console.has_method("set_nav_context"):
		return
	var cur := str(client.current_system_id)
	var links: Array = []
	var systems: Array = client.galaxy.get("systems", [])
	var by_id: Dictionary = {}
	for s in systems:
		if typeof(s) == TYPE_DICTIONARY:
			by_id[str(s.get("id", ""))] = s
	if by_id.has(cur):
		var me: Dictionary = by_id[cur]
		for link in me.get("links", []):
			if typeof(link) != TYPE_DICTIONARY:
				continue
			var to_id := str(link.get("to", ""))
			var dest: Dictionary = by_id.get(to_id, {})
			links.append({
				"id": to_id,
				"name": str(dest.get("name", to_id.replace("sys.", ""))),
				"fuel": int(link.get("fuel_cost", 1)),
				"kind": str(dest.get("kind", "")),
			})
	station_console.set_nav_context(cur, links)


func _on_dock_result(msg: Dictionary) -> void:
	if msg.get("ok", false):
		status_label.text = "Hard dock secured"
		if juice:
			juice.kick_camera(5.0, 0.12)
	else:
		status_label.text = "Dock failed: %s" % msg.get("code", "?")
		_set_docked_mode(false)
		if station_console:
			station_console.close_console()
		if juice:
			juice.flash_node(player_ship, Color(1.0, 0.8, 0.4, 1.0), 0.1)


func _set_docked_mode(docked: bool) -> void:
	_docked = docked
	# Hide the live space view while docked — station UI owns the screen
	if starfield:
		starfield.visible = not docked
	if remotes_root:
		remotes_root.visible = not docked
	if player_anchor:
		player_anchor.visible = not docked
	if _fx_layer:
		_fx_layer.visible = not docked
	if top_bar:
		top_bar.modulate = Color(1, 1, 1, 0.7 if docked else 1.0)
	# Sector radar is flight-only — never over the station panels
	_refresh_hud_visibility()
	if docked and station_console and station_console.visible:
		station_console.z_index = 100
		hud_root.move_child(station_console, -1)


func _place_radar_bottom_right() -> void:
	# Lower-right: clears station header / market list (top & center)
	if map_panel:
		map_panel.set_anchors_preset(Control.PRESET_BOTTOM_RIGHT)
		map_panel.anchor_left = 1.0
		map_panel.anchor_top = 1.0
		map_panel.anchor_right = 1.0
		map_panel.anchor_bottom = 1.0
		map_panel.offset_left = -228.0
		map_panel.offset_top = -292.0
		map_panel.offset_right = -12.0
		map_panel.offset_bottom = -52.0
		map_panel.grow_horizontal = Control.GROW_DIRECTION_BEGIN
		map_panel.grow_vertical = Control.GROW_DIRECTION_BEGIN
	if map_legend:
		map_legend.set_anchors_preset(Control.PRESET_BOTTOM_RIGHT)
		map_legend.anchor_left = 1.0
		map_legend.anchor_top = 1.0
		map_legend.anchor_right = 1.0
		map_legend.anchor_bottom = 1.0
		map_legend.offset_left = -228.0
		map_legend.offset_top = -48.0
		map_legend.offset_right = -12.0
		map_legend.offset_bottom = -8.0


func _show_flight_radar(show: bool) -> void:
	# Above station console (z=100) when docked so lower-right radar stays visible
	var z := 150 if _docked else 20
	if map_panel:
		map_panel.visible = show
		map_panel.z_index = z
		if show:
			_place_radar_bottom_right()
			map_panel.show()
			if _docked and hud_root:
				hud_root.move_child(map_panel, -1)
	if map_legend:
		map_legend.visible = show
		map_legend.z_index = z
		if show and _docked and hud_root:
			hud_root.move_child(map_legend, -1)
	if minimap:
		minimap.visible = show
		if show:
			minimap.queue_redraw()


func _refresh_hud_visibility() -> void:
	var galaxy_open := galaxy_map_ui != null and galaxy_map_ui.visible
	# Flight bars only while free-flying; radar stays at lower-right in flight and at berth
	var show_flight_bars := _in_world and not _docked and not galaxy_open
	var show_radar := _in_world and not galaxy_open
	if flight_hud:
		flight_hud.visible = show_flight_bars
	_show_flight_radar(show_radar)


func _trade(buy: bool) -> void:
	if client == null:
		return
	var commodity := ""
	if station_console:
		commodity = station_console.selected_commodity()
	if commodity.is_empty():
		status_label.text = "Select a commodity first"
		return
	var st: String = str(_station_menu.get("station_id", _docked_station_id))
	client.send_trade(st, commodity, buy, 1)
	status_label.text = ("Buying…" if buy else "Selling…")


func _refuel() -> void:
	if client == null:
		return
	client.send_refuel(str(_station_menu.get("station_id", _docked_station_id)))
	status_label.text = "Refueling…"


func _undock() -> void:
	if client == null:
		return
	client.send_undock()
	if station_console:
		station_console.close_console()
	if galaxy_map_ui and galaxy_map_ui.visible:
		galaxy_map_ui.close_map()
	_set_docked_mode(false)
	_show_flight_radar(true)
	status_label.text = "Clearing moorings… · radar online"
	if juice:
		juice.kick_camera(4.0, 0.1)


func _on_galaxy_map_msg(msg: Dictionary) -> void:
	var n := 0
	if msg.has("systems"):
		n = (msg.get("systems") as Array).size()
	status_label.text = "Galaxy chart received · %d systems" % n
	if _docked:
		_fill_station_nav_links()


func _open_galaxy_map() -> void:
	# Use deferred open so button signals / await don't race the UI tree
	call_deferred("_open_galaxy_map_impl")


func _open_galaxy_map_impl() -> void:
	if client == null or galaxy_map_ui == null:
		return
	status_label.text = "Opening galaxy map…"
	var has_systems := false
	if not client.galaxy.is_empty() and client.galaxy.has("systems"):
		var arr: Variant = client.galaxy.get("systems")
		has_systems = typeof(arr) == TYPE_ARRAY and (arr as Array).size() > 0
	if not has_systems and client.has_method("ensure_galaxy_chart"):
		status_label.text = "Downloading galaxy chart…"
		has_systems = await client.ensure_galaxy_chart()
	if not has_systems:
		status_label.text = "Galaxy chart unavailable — check server /galaxy"
		return

	# Hide station while map is open (restore if still docked on close)
	if station_console and station_console.visible:
		station_console.close_console()

	galaxy_map_ui.z_index = 300
	hud_root.move_child(galaxy_map_ui, -1)
	var cur := str(client.current_system_id)
	var ok: bool = galaxy_map_ui.open_map(client.galaxy, cur, _fuel_cache)
	_refresh_hud_visibility()
	if ok:
		status_label.text = "Galaxy map · cyan = jumpable · Esc / G closes"
	else:
		status_label.text = "Galaxy map failed (parse error)"
		if _docked and not _station_menu.is_empty() and station_console:
			station_console.open_console(_station_menu, _credits_cache, _fuel_cache)
			_fill_station_nav_links()


func _on_galaxy_map_closed() -> void:
	_refresh_hud_visibility()
	if _docked and not _station_menu.is_empty() and station_console:
		station_console.open_console(_station_menu, _credits_cache, _fuel_cache)
		_fill_station_nav_links()
	elif _in_world and not _docked:
		_show_flight_radar(true)


func _jump_to(dest: String) -> void:
	if client == null or dest.is_empty():
		return
	# Close overlays and leave berth into hyperspace
	if galaxy_map_ui and galaxy_map_ui.visible:
		galaxy_map_ui.close_map()
	if station_console and station_console.visible:
		station_console.close_console()
	if _docked:
		_set_docked_mode(false)
	# Clear prediction so channel/arrive don't fight leftover inputs
	if client.has_method("reset_prediction"):
		client.reset_prediction(float(client.pred_x), float(client.pred_y), float(client.pred_rot))
	client.send_jump(dest)
	var pretty := dest.replace("sys.", "").replace("_", " ")
	status_label.text = "Hyperspace channel… %s" % pretty
	if juice and player_ship:
		juice.flash_node(player_ship, Color(0.7, 0.5, 1.4, 1.0), 0.3)
		juice.kick_camera(8.0, 0.25)


func _jump_quick() -> void:
	if client == null:
		return
	var cur := str(client.current_system_id)
	var systems: Array = client.galaxy.get("systems", [])
	for s in systems:
		if typeof(s) != TYPE_DICTIONARY:
			continue
		if str(s.get("id", "")) != cur:
			continue
		var links: Array = s.get("links", [])
		if links.is_empty():
			status_label.text = "No jump links from this system"
			return
		_jump_to(str(links[0].get("to", "")))
		return
	status_label.text = "Open galaxy map (G) or Navigation tab for destinations"


func _on_jump_arrive(msg: Dictionary) -> void:
	# Fully exit docked / map UI and restore flight controls
	_docked = false
	if station_console:
		station_console.close_console()
	if galaxy_map_ui and galaxy_map_ui.visible:
		galaxy_map_ui.close_map()

	for id in _remote_nodes.keys():
		var node: Node = _remote_nodes[id]
		if is_instance_valid(node):
			node.queue_free()
	_remote_nodes.clear()

	var sys_id := str(msg.get("system_id", ""))
	var sys := sys_id.replace("sys.", "").to_upper()
	system_label.text = "SYS %s" % sys
	if minimap:
		minimap.set_system(sys)
	if galaxy_map_ui:
		galaxy_map_ui.set_current(sys_id)

	var px := float(msg.get("x", 0))
	var py := float(msg.get("y", 0))
	var prot := float(msg.get("rot", 0))
	if client and client.has_method("reset_prediction"):
		client.reset_prediction(px, py, prot)
	else:
		client.pred_x = px
		client.pred_y = py
		client.pred_rot = prot

	player_anchor.visible = true
	player_anchor.position = Vector2(px, py)
	player_anchor.rotation = prot + PI / 2.0
	camera.position = player_anchor.position
	if starfield:
		starfield.visible = true
	if remotes_root:
		remotes_root.visible = true
	if _fx_layer:
		_fx_layer.visible = true

	_set_docked_mode(false)
	_show_flight_radar(true)
	status_label.text = "Arrived · %s · arrows/WASD to fly" % sys_id
	if juice:
		juice.kick_camera(10.0, 0.2)


func _on_spawn(msg: Dictionary) -> void:
	if client == null:
		return
	var id := str(msg.get("id", ""))
	var kind := str(msg.get("kind", ""))
	if kind == "station":
		if _remote_nodes.has(id):
			return
		var st := EntityFactory.make_station()
		st.position = Vector2(float(msg.get("x", 0)), float(msg.get("y", 0)))
		remotes_root.add_child(st)
		_remote_nodes[id] = st
		client.remote_ships[id] = msg
		client.remote_ships[id]["kind"] = "station"
		client.remote_ships[id]["def_id"] = msg.get("def_id", "")
		return
	if id == str(client.ship_id):
		client.pred_x = float(msg.get("x", 0))
		client.pred_y = float(msg.get("y", 0))
		client.pred_rot = float(msg.get("rot", 0))
		return
	if _remote_nodes.has(id):
		return
	var pirate := str(msg.get("pilot_name", "")).begins_with("Pirate")
	var ship := EntityFactory.make_remote_ship(pirate)
	ship.position = Vector2(float(msg.get("x", 0)), float(msg.get("y", 0)))
	remotes_root.add_child(ship)
	_remote_nodes[id] = ship
	client.remote_ships[id] = msg
	# Keep def_id / pilot_name for radar labels
	client.remote_ships[id]["def_id"] = msg.get("def_id", "")
	client.remote_ships[id]["pilot_name"] = msg.get("pilot_name", "")
	client.remote_ships[id]["kind"] = msg.get("kind", "ship")


func _on_despawn(id: String) -> void:
	if _remote_nodes.has(id):
		(_remote_nodes[id] as Node).queue_free()
		_remote_nodes.erase(id)
