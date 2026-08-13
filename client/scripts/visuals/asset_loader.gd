extends RefCounted
## Loads JPG sprites and chroma-keys hot magenta/pink backgrounds to alpha.


static var _cache: Dictionary = {}


static func get_texture(res_path: String) -> Texture2D:
	if _cache.has(res_path):
		return _cache[res_path]
	var tex := _load_keyed(res_path)
	if tex:
		_cache[res_path] = tex
	return tex


static func _load_keyed(res_path: String) -> Texture2D:
	if not ResourceLoader.exists(res_path) and not FileAccess.file_exists(res_path):
		# try absolute open via ProjectSettings
		pass
	var img := Image.new()
	var err := img.load(ProjectSettings.globalize_path(res_path))
	if err != OK:
		# fallback: load as resource if imported
		var res = load(res_path)
		if res is Texture2D:
			img = res.get_image()
			if img == null:
				return res
		else:
			push_warning("AssetLoader: failed to load %s (%s)" % [res_path, err])
			return null
	img.convert(Image.FORMAT_RGBA8)
	_chroma_key(img)
	_trim_transparent(img)
	var out := ImageTexture.create_from_image(img)
	return out


static func _chroma_key(img: Image) -> void:
	var w := img.get_width()
	var h := img.get_height()
	for y in h:
		for x in w:
			var c := img.get_pixel(x, y)
			# Hot magenta/pink key (generator variance OK)
			var d_mag: float = _col_dist(c, Color(1.0, 0.0, 1.0))
			var d_pink: float = _col_dist(c, Color(1.0, 0.05, 0.85))
			var hot: bool = c.r > 0.75 and c.b > 0.55 and c.g < 0.55
			if d_mag < 0.38 or d_pink < 0.38 or hot:
				c.a = 0.0
				img.set_pixel(x, y, c)


static func _col_dist(a: Color, b: Color) -> float:
	var dr: float = a.r - b.r
	var dg: float = a.g - b.g
	var db: float = a.b - b.b
	return sqrt(dr * dr + dg * dg + db * db)


static func _trim_transparent(img: Image) -> void:
	var w := img.get_width()
	var h := img.get_height()
	var min_x := w
	var min_y := h
	var max_x := -1
	var max_y := -1
	for y in h:
		for x in w:
			if img.get_pixel(x, y).a > 0.05:
				min_x = mini(min_x, x)
				min_y = mini(min_y, y)
				max_x = maxi(max_x, x)
				max_y = maxi(max_y, y)
	if max_x < min_x:
		return
	var pad := 2
	min_x = maxi(0, min_x - pad)
	min_y = maxi(0, min_y - pad)
	max_x = mini(w - 1, max_x + pad)
	max_y = mini(h - 1, max_y + pad)
	var rect := Rect2i(min_x, min_y, max_x - min_x + 1, max_y - min_y + 1)
	img.blit_rect(img, rect, Vector2i.ZERO)
	img.crop(rect.size.x, rect.size.y)
