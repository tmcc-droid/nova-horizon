extends Control
## Full-screen galaxy map — pick a linked system to jump.

signal closed
signal jump_requested(dest_system_id: String)

var systems: Dictionary = {} # id -> {id,name,map_x,map_y,kind,links:[]}
var current_system_id: String = ""
var selected_id: String = ""
var _fuel: int = 0

var _title: Label
var _subtitle: Label
var _detail: Label
var _btn_jump: Button
var _canvas: Control
var _hint: Label

# View transform
var _zoom: float = 1.0
var _pan: Vector2 = Vector2.ZERO
var _dragging: bool = false
var _drag_last: Vector2 = Vector2.ZERO
var _hover_id: String = ""


func _ready() -> void:
	visible = false
	mouse_filter = Control.MOUSE_FILTER_IGNORE
	set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	anchor_right = 1.0
	anchor_bottom = 1.0
	offset_left = 0.0
	offset_top = 0.0
	offset_right = 0.0
	offset_bottom = 0.0
	z_index = 200
	process_mode = Node.PROCESS_MODE_ALWAYS
	_build()


func _unhandled_input(event: InputEvent) -> void:
	if not visible:
		return
	if event is InputEventKey and event.pressed and not event.echo:
		if event.keycode == KEY_ESCAPE or event.keycode == KEY_G:
			close_map()
			get_viewport().set_input_as_handled()
		elif event.keycode == KEY_ENTER or event.keycode == KEY_J:
			_try_jump()
			get_viewport().set_input_as_handled()


func open_map(galaxy: Dictionary, current: String, fuel: int) -> bool:
	systems.clear()
	var raw: Variant = galaxy.get("systems", [])
	if typeof(raw) == TYPE_ARRAY:
		for s in raw:
			# Godot JSON objects are Dictionaries; be permissive
			if not (s is Dictionary) and typeof(s) != TYPE_DICTIONARY:
				continue
			var d: Dictionary = s
			var id := str(d.get("id", ""))
			if id.is_empty():
				continue
			systems[id] = d
	current_system_id = current if not current.is_empty() else str(galaxy.get("current_system_id", ""))
	selected_id = ""
	_fuel = fuel
	_fit_view()
	_refresh_labels()
	mouse_filter = Control.MOUSE_FILTER_STOP
	visible = true
	show()
	move_to_front()
	set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	queue_redraw()
	if _canvas:
		_canvas.queue_redraw()
	return systems.size() > 0


func system_count() -> int:
	return systems.size()


func set_current(system_id: String) -> void:
	current_system_id = system_id
	_refresh_labels()
	if _canvas:
		_canvas.queue_redraw()


func set_fuel(fuel: int) -> void:
	_fuel = fuel
	_refresh_labels()


func close_map() -> void:
	visible = false
	mouse_filter = Control.MOUSE_FILTER_IGNORE
	closed.emit()


func _build() -> void:
	var dim := ColorRect.new()
	dim.set_anchors_preset(Control.PRESET_FULL_RECT)
	dim.color = Color(0.01, 0.02, 0.05, 0.96)
	dim.mouse_filter = Control.MOUSE_FILTER_STOP
	add_child(dim)

	var root := MarginContainer.new()
	root.set_anchors_preset(Control.PRESET_FULL_RECT)
	root.add_theme_constant_override("margin_left", 20)
	root.add_theme_constant_override("margin_right", 20)
	root.add_theme_constant_override("margin_top", 16)
	root.add_theme_constant_override("margin_bottom", 16)
	add_child(root)

	var v := VBoxContainer.new()
	v.add_theme_constant_override("separation", 10)
	root.add_child(v)

	var head := HBoxContainer.new()
	head.add_theme_constant_override("separation", 12)
	v.add_child(head)

	var head_text := VBoxContainer.new()
	head_text.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	head.add_child(head_text)

	_title = Label.new()
	_title.text = "GALAXY MAP"
	_title.add_theme_font_size_override("font_size", 26)
	head_text.add_child(_title)

	_subtitle = Label.new()
	_subtitle.add_theme_font_size_override("font_size", 13)
	_subtitle.add_theme_color_override("font_color", Color(0.55, 0.75, 0.9))
	_subtitle.text = "Linked systems only · drag to pan · wheel to zoom"
	head_text.add_child(_subtitle)

	var btn_close := Button.new()
	btn_close.text = "Close  (G / Esc)"
	btn_close.custom_minimum_size = Vector2(150, 40)
	btn_close.pressed.connect(close_map)
	head.add_child(btn_close)

	var body := HBoxContainer.new()
	body.size_flags_vertical = Control.SIZE_EXPAND_FILL
	body.add_theme_constant_override("separation", 14)
	v.add_child(body)

	_canvas = Control.new()
	_canvas.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	_canvas.size_flags_vertical = Control.SIZE_EXPAND_FILL
	_canvas.mouse_filter = Control.MOUSE_FILTER_STOP
	_canvas.draw.connect(_draw_galaxy)
	_canvas.gui_input.connect(_on_canvas_input)
	body.add_child(_canvas)

	var side := VBoxContainer.new()
	side.custom_minimum_size = Vector2(260, 0)
	side.add_theme_constant_override("separation", 10)
	body.add_child(side)

	var side_h := Label.new()
	side_h.text = "SYSTEM"
	side_h.add_theme_font_size_override("font_size", 16)
	side.add_child(side_h)

	_detail = Label.new()
	_detail.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	_detail.size_flags_vertical = Control.SIZE_EXPAND_FILL
	side.add_child(_detail)

	_btn_jump = Button.new()
	_btn_jump.text = "Jump"
	_btn_jump.custom_minimum_size = Vector2(0, 48)
	_btn_jump.disabled = true
	_btn_jump.pressed.connect(_try_jump)
	side.add_child(_btn_jump)

	_hint = Label.new()
	_hint.add_theme_font_size_override("font_size", 12)
	_hint.add_theme_color_override("font_color", Color(0.5, 0.65, 0.8))
	_hint.text = "Green = current · Cyan = reachable (linked) · Grey = out of range this hop"
	v.add_child(_hint)


func _fit_view() -> void:
	if systems.is_empty():
		_zoom = 1.0
		_pan = Vector2.ZERO
		return
	var min_p := Vector2(1e9, 1e9)
	var max_p := Vector2(-1e9, -1e9)
	for id in systems.keys():
		var s: Dictionary = systems[id]
		var p := Vector2(float(s.get("map_x", 0)), float(s.get("map_y", 0)))
		min_p = min_p.min(p)
		max_p = max_p.max(p)
	var center := (min_p + max_p) * 0.5
	var span := (max_p - min_p).length()
	if span < 1.0:
		span = 100.0
	# rough fit; refined when canvas has size
	_zoom = 1.8
	_pan = -center


func _world_to_screen(p: Vector2) -> Vector2:
	var sz := _canvas.size if _canvas else size
	var mid := sz * 0.5
	return mid + (p + _pan) * _zoom


func _screen_to_world(sp: Vector2) -> Vector2:
	var sz := _canvas.size if _canvas else size
	var mid := sz * 0.5
	return (sp - mid) / maxf(_zoom, 0.01) - _pan


func _draw_galaxy() -> void:
	if _canvas == null:
		return
	var c := _canvas
	# soft grid
	c.draw_rect(Rect2(Vector2.ZERO, c.size), Color(0.04, 0.07, 0.12, 1.0), true)

	# links first
	for id in systems.keys():
		var s: Dictionary = systems[id]
		var from := _world_to_screen(Vector2(float(s.get("map_x", 0)), float(s.get("map_y", 0))))
		for link in s.get("links", []):
			var to_id := str(link.get("to", ""))
			if to_id <= id:
				continue # draw once
			if not systems.has(to_id):
				continue
			var t: Dictionary = systems[to_id]
			var to := _world_to_screen(Vector2(float(t.get("map_x", 0)), float(t.get("map_y", 0))))
			var linked_here := id == current_system_id or to_id == current_system_id
			var col := Color(0.25, 0.4, 0.55, 0.35) if not linked_here else Color(0.35, 0.75, 0.95, 0.55)
			c.draw_line(from, to, col, 1.5 if linked_here else 1.0)

	var neighbors := _neighbor_ids()

	for id in systems.keys():
		var s: Dictionary = systems[id]
		var pos := _world_to_screen(Vector2(float(s.get("map_x", 0)), float(s.get("map_y", 0))))
		var kind := str(s.get("kind", "transit"))
		var r := 3.5
		if kind == "hub":
			r = 6.0
		elif kind == "populated":
			r = 4.5

		var col := Color(0.35, 0.4, 0.48)
		if id == current_system_id:
			col = Color(0.35, 0.95, 0.55)
			r = maxf(r, 6.5)
		elif neighbors.has(id):
			col = Color(0.35, 0.85, 1.0)
		elif kind == "hub":
			col = Color(0.95, 0.75, 0.35)
		elif kind == "populated":
			col = Color(0.7, 0.75, 0.9)

		if id == selected_id:
			c.draw_arc(pos, r + 5.0, 0.0, TAU, 24, Color(1.0, 0.9, 0.4, 0.9), 2.0)
		if id == _hover_id:
			c.draw_circle(pos, r + 3.0, Color(1, 1, 1, 0.12))

		c.draw_circle(pos, r, col)

		# labels for current, selected, neighbors, hover
		if id == current_system_id or id == selected_id or neighbors.has(id) or id == _hover_id:
			var name := str(s.get("name", id))
			c.draw_string(
				ThemeDB.fallback_font,
				pos + Vector2(8, 4),
				name,
				HORIZONTAL_ALIGNMENT_LEFT,
				-1,
				12,
				Color(0.85, 0.92, 1.0, 0.95)
			)


func _neighbor_ids() -> Dictionary:
	var out := {}
	if not systems.has(current_system_id):
		return out
	var cur: Dictionary = systems[current_system_id]
	for link in cur.get("links", []):
		out[str(link.get("to", ""))] = int(link.get("fuel_cost", 1))
	return out


func _on_canvas_input(event: InputEvent) -> void:
	if event is InputEventMouseButton:
		var mb := event as InputEventMouseButton
		if mb.button_index == MOUSE_BUTTON_WHEEL_UP and mb.pressed:
			_zoom = clampf(_zoom * 1.12, 0.25, 12.0)
			_canvas.queue_redraw()
		elif mb.button_index == MOUSE_BUTTON_WHEEL_DOWN and mb.pressed:
			_zoom = clampf(_zoom / 1.12, 0.25, 12.0)
			_canvas.queue_redraw()
		elif mb.button_index == MOUSE_BUTTON_LEFT and mb.pressed:
			var hit := _pick_system(mb.position)
			if not hit.is_empty():
				selected_id = hit
				_refresh_labels()
				_canvas.queue_redraw()
		elif mb.button_index == MOUSE_BUTTON_MIDDLE or mb.button_index == MOUSE_BUTTON_RIGHT:
			_dragging = mb.pressed
			_drag_last = mb.position
	elif event is InputEventMouseMotion:
		var mm := event as InputEventMouseMotion
		if _dragging:
			var delta := mm.position - _drag_last
			_drag_last = mm.position
			_pan += delta / maxf(_zoom, 0.01)
			_canvas.queue_redraw()
		else:
			var h := _pick_system(mm.position)
			if h != _hover_id:
				_hover_id = h
				_canvas.queue_redraw()


func _pick_system(screen_pos: Vector2) -> String:
	var best := ""
	var best_d := 14.0
	for id in systems.keys():
		var s: Dictionary = systems[id]
		var p := _world_to_screen(Vector2(float(s.get("map_x", 0)), float(s.get("map_y", 0))))
		var d := p.distance_to(screen_pos)
		if d < best_d:
			best_d = d
			best = id
	return best


func _refresh_labels() -> void:
	var cur_name := current_system_id
	if systems.has(current_system_id):
		cur_name = str(systems[current_system_id].get("name", current_system_id))
	_subtitle.text = "You are in %s  ·  Fuel %d  ·  drag pan · wheel zoom" % [cur_name, _fuel]

	if selected_id.is_empty() or not systems.has(selected_id):
		_detail.text = "Select a system.\n\nOnly cyan (linked) neighbors can be jumped to this hop.\nTransit systems have no station — pass-through only."
		_btn_jump.disabled = true
		_btn_jump.text = "Jump"
		return

	var s: Dictionary = systems[selected_id]
	var kind := str(s.get("kind", "transit"))
	var neighbors := _neighbor_ids()
	var fuel_need := int(neighbors.get(selected_id, -1))
	var reachable := neighbors.has(selected_id)
	var link_n := (s.get("links", []) as Array).size()
	var text := "%s\n%s\n\nKind: %s\nLinks: %d\n" % [
		str(s.get("name", selected_id)),
		selected_id,
		kind,
		link_n,
	]
	if selected_id == current_system_id:
		text += "\nYou are here."
		_btn_jump.disabled = true
		_btn_jump.text = "Current system"
	elif reachable:
		text += "\nFuel cost: %d\n" % fuel_need
		if _fuel < fuel_need:
			text += "Need more fuel."
			_btn_jump.disabled = true
			_btn_jump.text = "Not enough fuel"
		else:
			text += "Ready to channel."
			_btn_jump.disabled = false
			_btn_jump.text = "Jump  (fuel %d)" % fuel_need
	else:
		text += "\nNot linked from your current system.\nTravel through neighbors first."
		_btn_jump.disabled = true
		_btn_jump.text = "No direct link"
	_detail.text = text


func _try_jump() -> void:
	if selected_id.is_empty() or selected_id == current_system_id:
		return
	var neighbors := _neighbor_ids()
	if not neighbors.has(selected_id):
		return
	var fuel_need := int(neighbors[selected_id])
	if _fuel < fuel_need:
		return
	jump_requested.emit(selected_id)
	close_map()
