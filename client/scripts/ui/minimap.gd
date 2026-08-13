extends Control
## Local sector radar / HUD map — stations, ships, player heading.

@export var map_range_wu: float = 12000.0
@export var show_labels: bool = true

var player_pos: Vector2 = Vector2.ZERO
var player_rot: float = 0.0 # game radians (0 = +X)
var system_name: String = "—"
## Array of {pos: Vector2, kind: String, label: String, id: String}
var blips: Array = []


func _ready() -> void:
	custom_minimum_size = Vector2(200, 200)
	mouse_filter = Control.MOUSE_FILTER_IGNORE
	queue_redraw()


func set_player(pos: Vector2, rot: float) -> void:
	player_pos = pos
	player_rot = rot
	queue_redraw()


func set_system(name: String) -> void:
	system_name = name
	queue_redraw()


func set_blips(items: Array) -> void:
	blips = items
	queue_redraw()


func _process(_delta: float) -> void:
	queue_redraw()


func _draw() -> void:
	var r := get_rect().size
	var cx := r.x * 0.5
	var cy := r.y * 0.5
	var radius := minf(cx, cy) - 8.0

	# Background disc
	draw_circle(Vector2(cx, cy), radius + 4.0, Color(0.04, 0.07, 0.12, 0.92))
	draw_arc(Vector2(cx, cy), radius, 0.0, TAU, 64, Color(0.24, 0.75, 0.95, 0.55), 2.0, true)

	# Range rings
	for i in 3:
		var rr: float = radius * (float(i + 1) / 3.0)
		draw_arc(Vector2(cx, cy), rr, 0.0, TAU, 48, Color(0.3, 0.55, 0.7, 0.2), 1.0, true)

	# Crosshair
	draw_line(Vector2(cx - radius, cy), Vector2(cx + radius, cy), Color(0.3, 0.5, 0.65, 0.25), 1.0)
	draw_line(Vector2(cx, cy - radius), Vector2(cx, cy + radius), Color(0.3, 0.5, 0.65, 0.25), 1.0)

	# Scale: world units → map pixels
	var scale: float = radius / maxf(map_range_wu, 1.0)

	# Blips (stations / ships)
	for b in blips:
		var world: Vector2 = b.get("pos", Vector2.ZERO)
		var delta: Vector2 = world - player_pos
		var local: Vector2 = delta * scale
		# clamp to edge with arrow if outside
		var outside := local.length() > radius - 6.0
		if outside:
			local = local.normalized() * (radius - 6.0)
		var p: Vector2 = Vector2(cx, cy) + Vector2(local.x, local.y)
		var kind: String = str(b.get("kind", "ship"))
		match kind:
			"station":
				_draw_station_blip(p, outside)
				if show_labels and not outside:
					var label: String = str(b.get("label", "STN"))
					draw_string(
						ThemeDB.fallback_font,
						p + Vector2(8, -4),
						label,
						HORIZONTAL_ALIGNMENT_LEFT,
						-1,
						11,
						Color(0.55, 0.95, 0.7, 0.95)
					)
			"pirate":
				_draw_ship_blip(p, Color(1.0, 0.4, 0.28, 1.0), outside)
			"player_other":
				_draw_ship_blip(p, Color(1.0, 0.85, 0.35, 1.0), outside)
			_:
				_draw_ship_blip(p, Color(0.7, 0.85, 1.0, 0.9), outside)

	# Player (center) with heading — game rot 0 is +X; map +X right, +Y down
	# Screen heading: rot 0 → right; our ship art uses +PI/2 so visual nose is rot direction after that
	var heading: float = player_rot # point triangle along velocity/facing
	var nose := Vector2(cos(heading), sin(heading)) * 9.0
	var left := Vector2(cos(heading + 2.4), sin(heading + 2.4)) * 7.0
	var right := Vector2(cos(heading - 2.4), sin(heading - 2.4)) * 7.0
	var origin := Vector2(cx, cy)
	draw_colored_polygon(
		PackedVector2Array([origin + nose, origin + left, origin + right]),
		Color(0.35, 0.9, 1.0, 1.0)
	)
	draw_circle(origin, 2.5, Color(1, 1, 1, 0.9))

	# Title
	draw_string(
		ThemeDB.fallback_font,
		Vector2(10, 16),
		"RADAR  %s" % system_name.to_upper(),
		HORIZONTAL_ALIGNMENT_LEFT,
		-1,
		12,
		Color(0.55, 0.85, 1.0, 0.9)
	)
	draw_string(
		ThemeDB.fallback_font,
		Vector2(10, r.y - 8),
		"range %.0f wu" % map_range_wu,
		HORIZONTAL_ALIGNMENT_LEFT,
		-1,
		10,
		Color(0.45, 0.6, 0.75, 0.75)
	)


func _draw_station_blip(p: Vector2, edge: bool) -> void:
	var col := Color(0.4, 0.95, 0.65, 1.0 if not edge else 0.75)
	draw_arc(p, 5.0, 0.0, TAU, 16, col, 2.0, true)
	draw_circle(p, 2.0, col)


func _draw_ship_blip(p: Vector2, col: Color, edge: bool) -> void:
	if edge:
		col.a = 0.7
	draw_circle(p, 3.5, col)
