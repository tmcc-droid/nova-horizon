extends RefCounted
## Builds ship/station nodes — sprites when available, polygons as fallback.

const AssetLoader = preload("res://scripts/visuals/asset_loader.gd")

const TEX_PLAYER := "res://assets/ships/player_shuttle.jpg"
const TEX_PIRATE := "res://assets/ships/pirate_raider.jpg"
const TEX_STATION := "res://assets/stations/ring_station.jpg"


static func make_player_ship() -> Node2D:
	var root := Node2D.new()
	root.name = "PlayerShip"

	var tex := AssetLoader.get_texture(TEX_PLAYER)
	if tex:
		var spr := Sprite2D.new()
		spr.name = "Sprite"
		spr.texture = tex
		spr.centered = true
		# Scale so ship is ~40–48 px tall in world
		var th: float = float(tex.get_height())
		var scale: float = 48.0 / maxf(th, 1.0)
		spr.scale = Vector2(scale, scale)
		# Art faces up (nose 12 o'clock); game rot 0 is +X so main adds PI/2
		root.add_child(spr)
	else:
		_add_polygon_player(root)

	# Soft under-glow
	var glow := Polygon2D.new()
	glow.color = Color(0.2, 0.7, 1.0, 0.14)
	glow.polygon = _ellipse(30, 24, 14)
	glow.z_index = -2
	root.add_child(glow)

	# Engine flame (juice)
	var flame := Polygon2D.new()
	flame.name = "Flame"
	flame.color = Color(1.0, 0.55, 0.15, 0.9)
	flame.polygon = PackedVector2Array([Vector2(-5, 14), Vector2(5, 14), Vector2(0, 30)])
	flame.z_index = -1
	flame.visible = false
	root.add_child(flame)

	var flame2 := Polygon2D.new()
	flame2.name = "FlameCore"
	flame2.color = Color(1.0, 0.9, 0.5, 0.95)
	flame2.polygon = PackedVector2Array([Vector2(-2.5, 14), Vector2(2.5, 14), Vector2(0, 22)])
	flame2.z_index = 0
	flame2.visible = false
	root.add_child(flame2)

	var Juice = load("res://scripts/visuals/juice.gd")
	if Juice:
		var exhaust: GPUParticles2D = Juice.make_exhaust_particles()
		root.add_child(exhaust)

	var muzzle := Marker2D.new()
	muzzle.name = "Muzzle"
	muzzle.position = Vector2(0, -24)
	root.add_child(muzzle)
	return root


static func make_remote_ship(pirate: bool = false) -> Node2D:
	var root := Node2D.new()
	var path := TEX_PIRATE if pirate else TEX_PLAYER
	var tex := AssetLoader.get_texture(path)
	if tex:
		var spr := Sprite2D.new()
		spr.name = "Sprite"
		spr.texture = tex
		spr.centered = true
		var th: float = float(tex.get_height())
		var scale: float = (42.0 if pirate else 40.0) / maxf(th, 1.0)
		spr.scale = Vector2(scale, scale)
		if not pirate:
			spr.modulate = Color(1.0, 0.92, 0.75, 1.0) # ally tint
		root.add_child(spr)
	else:
		var hull := Polygon2D.new()
		if pirate:
			hull.color = Color(1.0, 0.4, 0.28, 1.0)
			hull.polygon = PackedVector2Array([
				Vector2(0, -16), Vector2(12, 4), Vector2(8, 14), Vector2(0, 8), Vector2(-8, 14), Vector2(-12, 4)
			])
		else:
			hull.color = Color(1.0, 0.78, 0.35, 1.0)
			hull.polygon = PackedVector2Array([
				Vector2(0, -16), Vector2(11, 12), Vector2(0, 6), Vector2(-11, 12)
			])
		root.add_child(hull)
	return root


static func make_station() -> Node2D:
	var root := Node2D.new()
	root.name = "Station"
	var tex := AssetLoader.get_texture(TEX_STATION)
	if tex:
		var spr := Sprite2D.new()
		spr.name = "Sprite"
		spr.texture = tex
		spr.centered = true
		var th: float = float(tex.get_height())
		var scale: float = 96.0 / maxf(th, 1.0)
		spr.scale = Vector2(scale, scale)
		root.add_child(spr)
		# soft pad under station
		var glow := Polygon2D.new()
		glow.color = Color(0.3, 0.9, 0.55, 0.1)
		glow.polygon = _ellipse(55, 55, 20)
		glow.z_index = -1
		root.add_child(glow)
	else:
		var outer := Polygon2D.new()
		outer.color = Color(0.25, 0.75, 0.5, 0.85)
		outer.polygon = _ellipse(48, 48, 28)
		root.add_child(outer)
	return root


static func set_thrust_visual(ship: Node2D, thrust_amount: float) -> void:
	var flame := ship.get_node_or_null("Flame") as Polygon2D
	var core := ship.get_node_or_null("FlameCore") as Polygon2D
	var exhaust := ship.get_node_or_null("Exhaust") as GPUParticles2D
	var t := clampf(thrust_amount, 0.0, 1.0)
	if flame:
		flame.scale = Vector2(1.0, 0.35 + t * 1.1)
		flame.modulate.a = 0.25 + t * 0.75
		flame.visible = t > 0.05
	if core:
		core.scale = Vector2(1.0, 0.4 + t * 0.9)
		core.visible = t > 0.05
	if exhaust:
		exhaust.emitting = t > 0.08
		if "amount_ratio" in exhaust:
			exhaust.amount_ratio = clampf(t, 0.15, 1.0)


static func _add_polygon_player(root: Node2D) -> void:
	var hull := Polygon2D.new()
	hull.name = "Hull"
	hull.color = Color(0.45, 0.85, 1.0, 1.0)
	hull.polygon = PackedVector2Array([
		Vector2(0, -20), Vector2(10, -4), Vector2(14, 14), Vector2(6, 8),
		Vector2(0, 12), Vector2(-6, 8), Vector2(-14, 14), Vector2(-10, -4),
	])
	root.add_child(hull)


static func _ellipse(rx: float, ry: float, segments: int) -> PackedVector2Array:
	var pts := PackedVector2Array()
	for i in segments:
		var a := TAU * float(i) / float(segments)
		pts.append(Vector2(cos(a) * rx, sin(a) * ry))
	return pts
