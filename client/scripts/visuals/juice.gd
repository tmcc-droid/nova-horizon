extends Node
## Combat & flight juice: shake, flashes, muzzle, hits, exhaust.

signal shake_done

var _shake_time: float = 0.0
var _shake_mag: float = 0.0
var _shake_offset: Vector2 = Vector2.ZERO
var _flash_nodes: Array = [] # {node, t, dur, base}


func _process(delta: float) -> void:
	if _shake_time > 0.0:
		_shake_time -= delta
		var t := clampf(_shake_time, 0.0, 1.0)
		_shake_offset = Vector2(
			randf_range(-1, 1),
			randf_range(-1, 1)
		) * _shake_mag * t
		if _shake_time <= 0.0:
			_shake_offset = Vector2.ZERO
			shake_done.emit()

	var i := 0
	while i < _flash_nodes.size():
		var e: Dictionary = _flash_nodes[i]
		e.t = float(e.t) - delta
		var node: CanvasItem = e.node
		if not is_instance_valid(node):
			_flash_nodes.remove_at(i)
			continue
		var dur: float = maxf(float(e.dur), 0.001)
		var k: float = clampf(float(e.t) / dur, 0.0, 1.0)
		var base: Color = e.base
		var flash: Color = e.flash
		node.modulate = base.lerp(flash, k)
		if float(e.t) <= 0.0:
			node.modulate = base
			_flash_nodes.remove_at(i)
		else:
			_flash_nodes[i] = e
			i += 1


func get_shake_offset() -> Vector2:
	return _shake_offset


func kick_camera(magnitude: float = 10.0, duration: float = 0.18) -> void:
	_shake_mag = magnitude
	_shake_time = maxf(_shake_time, duration)


func flash_node(node: CanvasItem, flash_color: Color = Color(1.5, 1.5, 1.5, 1.0), duration: float = 0.12) -> void:
	if node == null or not is_instance_valid(node):
		return
	_flash_nodes.append({
		"node": node,
		"t": duration,
		"dur": duration,
		"base": Color.WHITE,
		"flash": flash_color,
	})


func spawn_tracer(world: Node2D, from_global: Vector2, to_global: Vector2, color: Color = Color(0.55, 0.9, 1.0, 0.9)) -> void:
	var line := Line2D.new()
	line.width = 2.5
	line.default_color = color
	line.z_index = 40
	line.points = PackedVector2Array([from_global, to_global])
	world.add_child(line)
	var tw := line.create_tween()
	tw.tween_property(line, "modulate:a", 0.0, 0.12)
	tw.tween_callback(line.queue_free)


func spawn_muzzle_flash(parent: Node2D, local_pos: Vector2 = Vector2(0, -22)) -> void:
	var flash := Node2D.new()
	flash.position = local_pos
	flash.z_index = 5
	parent.add_child(flash)

	var core := Polygon2D.new()
	core.color = Color(1.0, 0.95, 0.7, 1.0)
	core.polygon = PackedVector2Array([
		Vector2(0, -10), Vector2(4, 2), Vector2(0, 0), Vector2(-4, 2)
	])
	flash.add_child(core)

	var glow := Polygon2D.new()
	glow.color = Color(0.4, 0.85, 1.0, 0.55)
	glow.polygon = PackedVector2Array([
		Vector2(0, -16), Vector2(8, 4), Vector2(0, 2), Vector2(-8, 4)
	])
	glow.z_index = -1
	flash.add_child(glow)

	var beam := Line2D.new()
	beam.width = 2.0
	beam.default_color = Color(0.6, 0.9, 1.0, 0.85)
	beam.points = PackedVector2Array([Vector2(0, -8), Vector2(0, -90)])
	beam.z_index = -2
	flash.add_child(beam)

	var tw := flash.create_tween()
	tw.set_parallel(true)
	tw.tween_property(flash, "scale", Vector2(1.4, 1.6), 0.06)
	tw.tween_property(flash, "modulate:a", 0.0, 0.12)
	tw.tween_property(beam, "modulate:a", 0.0, 0.1)
	tw.chain().tween_callback(flash.queue_free)


func spawn_hit_burst(world: Node2D, global_pos: Vector2, color: Color = Color(1.0, 0.55, 0.3, 1.0)) -> void:
	var burst := Node2D.new()
	burst.global_position = global_pos
	burst.z_index = 20
	world.add_child(burst)

	var ring := Polygon2D.new()
	ring.color = Color(color.r, color.g, color.b, 0.7)
	var pts := PackedVector2Array()
	for i in 10:
		var a := TAU * float(i) / 10.0
		pts.append(Vector2(cos(a), sin(a)) * 8.0)
	ring.polygon = pts
	burst.add_child(ring)

	for i in 6:
		var spark := Polygon2D.new()
		spark.color = color
		spark.polygon = PackedVector2Array([
			Vector2(-1.5, -1.5), Vector2(1.5, -1.5), Vector2(1.5, 1.5), Vector2(-1.5, 1.5)
		])
		var ang := randf() * TAU
		spark.position = Vector2(cos(ang), sin(ang)) * randf_range(2, 6)
		burst.add_child(spark)
		var dest := spark.position + Vector2(cos(ang), sin(ang)) * randf_range(18, 36)
		var tws := spark.create_tween()
		tws.set_parallel(true)
		tws.tween_property(spark, "position", dest, 0.2)
		tws.tween_property(spark, "modulate:a", 0.0, 0.2)

	var tw := burst.create_tween()
	tw.set_parallel(true)
	tw.tween_property(ring, "scale", Vector2(3.5, 3.5), 0.22)
	tw.tween_property(ring, "modulate:a", 0.0, 0.22)
	tw.chain().tween_callback(burst.queue_free)


func spawn_shield_ripple(ship: Node2D) -> void:
	var ripple := Polygon2D.new()
	ripple.name = "ShieldRipple"
	ripple.z_index = 8
	ripple.color = Color(0.4, 0.85, 1.0, 0.55)
	var pts := PackedVector2Array()
	for i in 16:
		var a := TAU * float(i) / 16.0
		pts.append(Vector2(cos(a), sin(a)) * 22.0)
	ripple.polygon = pts
	ship.add_child(ripple)
	var tw := ripple.create_tween()
	tw.set_parallel(true)
	tw.tween_property(ripple, "scale", Vector2(1.8, 1.8), 0.25)
	tw.tween_property(ripple, "modulate:a", 0.0, 0.25)
	tw.chain().tween_callback(ripple.queue_free)


static func make_exhaust_particles() -> GPUParticles2D:
	var p := GPUParticles2D.new()
	p.name = "Exhaust"
	p.position = Vector2(0, 14)
	p.amount = 24
	p.lifetime = 0.35
	p.explosiveness = 0.05
	p.randomness = 0.4
	p.local_coords = true
	p.z_index = -3
	p.emitting = false

	var mat := ParticleProcessMaterial.new()
	mat.direction = Vector3(0, 1, 0)
	mat.spread = 18.0
	mat.initial_velocity_min = 40.0
	mat.initial_velocity_max = 90.0
	mat.gravity = Vector3(0, 0, 0)
	mat.scale_min = 1.5
	mat.scale_max = 3.5
	mat.color = Color(1.0, 0.55, 0.2, 0.85)
	# Godot 4 gradient optional
	p.process_material = mat

	# Point texture
	var img := Image.create(4, 4, false, Image.FORMAT_RGBA8)
	img.fill(Color(1, 1, 1, 1))
	var tex := ImageTexture.create_from_image(img)
	p.texture = tex
	return p
