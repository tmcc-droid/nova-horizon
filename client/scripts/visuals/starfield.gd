extends Node2D
## Parallax starfield + subtle nebula tint for deep-space feel.


var _layers: Array = [] # {stars: PackedVector2Array, sizes: PackedFloat32Array, color: Color, parallax: float}
var _rng := RandomNumberGenerator.new()


func _ready() -> void:
	_rng.seed = 42
	z_index = -100
	_build_layer(220, Color(0.55, 0.65, 0.85, 0.55), 0.08, 0.6, 1.2)
	_build_layer(120, Color(0.75, 0.85, 1.0, 0.75), 0.18, 1.0, 2.0)
	_build_layer(40, Color(0.55, 0.9, 1.0, 0.95), 0.32, 1.6, 3.0)
	queue_redraw()


func _build_layer(count: int, color: Color, parallax: float, size_min: float, size_max: float) -> void:
	var stars := PackedVector2Array()
	var sizes := PackedFloat32Array()
	stars.resize(count)
	sizes.resize(count)
	for i in count:
		stars[i] = Vector2(_rng.randf_range(-8000, 8000), _rng.randf_range(-8000, 8000))
		sizes[i] = _rng.randf_range(size_min, size_max)
	_layers.append({"stars": stars, "sizes": sizes, "color": color, "parallax": parallax})


func _process(_delta: float) -> void:
	queue_redraw()


func _draw() -> void:
	# Deep void
	var cam := get_viewport().get_camera_2d()
	var center := Vector2.ZERO
	if cam:
		center = cam.get_screen_center_position()
	# Large soft vignette rectangle in world space around camera
	var half := Vector2(2200, 1400)
	draw_rect(Rect2(center - half, half * 2.0), Color(0.04, 0.06, 0.1, 1.0), true)

	# Nebula blobs
	draw_circle(center + Vector2(400, -200), 700, Color(0.12, 0.08, 0.28, 0.12))
	draw_circle(center + Vector2(-600, 300), 900, Color(0.05, 0.15, 0.22, 0.1))
	draw_circle(center + Vector2(200, 500), 500, Color(0.2, 0.05, 0.15, 0.08))

	for layer in _layers:
		var parallax: float = layer.parallax
		var offset: Vector2 = center * parallax
		var stars: PackedVector2Array = layer.stars
		var sizes: PackedFloat32Array = layer.sizes
		var col: Color = layer.color
		for i in stars.size():
			var p: Vector2 = stars[i] + offset
			# wrap-ish: keep stars near camera for density
			var rel := p - center
			if absf(rel.x) > 3500.0:
				p.x -= signf(rel.x) * 7000.0
			if absf(rel.y) > 3500.0:
				p.y -= signf(rel.y) * 7000.0
			draw_circle(p, sizes[i], col)
