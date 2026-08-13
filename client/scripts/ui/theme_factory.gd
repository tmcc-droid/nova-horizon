extends RefCounted
## Neon “deep space ops” UI theme — applied at runtime so no Theme resource import is required.


static func apply_to_tree(root: Control) -> void:
	var theme := build_theme()
	root.theme = theme
	_try_panel_texture(root)


static func _try_panel_texture(root: Control) -> void:
	# Optional: style PanelContainers with generated sci-fi frame if present.
	var AssetLoader = load("res://scripts/visuals/asset_loader.gd")
	if AssetLoader == null:
		return
	var tex: Texture2D = AssetLoader.get_texture("res://assets/ui/panel_frame.jpg")
	if tex == null:
		return
	var sb := StyleBoxTexture.new()
	sb.texture = tex
	# Approximate 9-slice margins for neon frame
	sb.texture_margin_left = 48
	sb.texture_margin_top = 48
	sb.texture_margin_right = 48
	sb.texture_margin_bottom = 48
	sb.content_margin_left = 18
	sb.content_margin_right = 18
	sb.content_margin_top = 16
	sb.content_margin_bottom = 16
	sb.modulate_color = Color(1, 1, 1, 0.96)
	_apply_panel_style_recursive(root, sb)


static func _apply_panel_style_recursive(n: Node, sb: StyleBox) -> void:
	if n is PanelContainer:
		(n as PanelContainer).add_theme_stylebox_override("panel", sb)
	for c in n.get_children():
		_apply_panel_style_recursive(c, sb)


static func build_theme() -> Theme:
	var t := Theme.new()
	var accent := Color("3dd6ff")
	var panel_bg := Color("0f1626ee")
	var border := Color("3dd6ff99")
	var text := Color("e8f4ff")
	var muted := Color("8aa0b8")
	var danger := Color("ff5a6a")

	# Panel
	var panel := StyleBoxFlat.new()
	panel.bg_color = panel_bg
	panel.set_border_width_all(2)
	panel.border_color = border
	panel.set_corner_radius_all(10)
	panel.content_margin_left = 14
	panel.content_margin_right = 14
	panel.content_margin_top = 12
	panel.content_margin_bottom = 12
	panel.shadow_color = Color(0, 0, 0, 0.45)
	panel.shadow_size = 8
	panel.shadow_offset = Vector2(0, 3)
	t.set_stylebox("panel", "PanelContainer", panel)

	# Buttons
	var btn_n := _btn(Color("1a2740"), accent, text)
	var btn_h := _btn(Color("243556"), Color("6ae0ff"), text)
	var btn_p := _btn(Color("122038"), Color("2aa8cc"), text)
	var btn_d := _btn(Color("151b28"), Color("445566"), muted)
	t.set_stylebox("normal", "Button", btn_n)
	t.set_stylebox("hover", "Button", btn_h)
	t.set_stylebox("pressed", "Button", btn_p)
	t.set_stylebox("disabled", "Button", btn_d)
	t.set_color("font_color", "Button", text)
	t.set_color("font_hover_color", "Button", Color.WHITE)
	t.set_color("font_pressed_color", "Button", accent)
	t.set_font_size("font_size", "Button", 15)
	t.set_constant("h_separation", "Button", 8)

	# Labels
	t.set_color("font_color", "Label", text)
	t.set_font_size("font_size", "Label", 15)

	# LineEdit
	var le := StyleBoxFlat.new()
	le.bg_color = Color("0a101c")
	le.set_border_width_all(1)
	le.border_color = Color("3dd6ff66")
	le.set_corner_radius_all(6)
	le.content_margin_left = 10
	le.content_margin_right = 10
	le.content_margin_top = 8
	le.content_margin_bottom = 8
	t.set_stylebox("normal", "LineEdit", le)
	t.set_stylebox("focus", "LineEdit", le)
	t.set_color("font_color", "LineEdit", text)
	t.set_color("font_placeholder_color", "LineEdit", muted)

	# ItemList
	var il := StyleBoxFlat.new()
	il.bg_color = Color("0a101c")
	il.set_border_width_all(1)
	il.border_color = Color("3dd6ff44")
	il.set_corner_radius_all(6)
	t.set_stylebox("panel", "ItemList", il)
	var il_sel := StyleBoxFlat.new()
	il_sel.bg_color = Color("1e3a55")
	il_sel.set_corner_radius_all(4)
	t.set_stylebox("selected", "ItemList", il_sel)
	t.set_stylebox("selected_focus", "ItemList", il_sel)
	t.set_color("font_color", "ItemList", text)
	t.set_color("font_selected_color", "ItemList", accent)
	t.set_font_size("font_size", "ItemList", 13)

	# Progress bars (HUD)
	var bar_bg := StyleBoxFlat.new()
	bar_bg.bg_color = Color("0a101c")
	bar_bg.set_border_width_all(1)
	bar_bg.border_color = Color("ffffff22")
	bar_bg.set_corner_radius_all(4)
	var bar_fill := StyleBoxFlat.new()
	bar_fill.bg_color = accent
	bar_fill.set_corner_radius_all(3)
	t.set_stylebox("background", "ProgressBar", bar_bg)
	t.set_stylebox("fill", "ProgressBar", bar_fill)
	t.set_color("font_color", "ProgressBar", text)
	t.set_font_size("font_size", "ProgressBar", 11)

	# Accent helper colors stored on meta via defaults
	t.set_color("font_outline_color", "Label", danger) # unused; keep theme valid
	return t


static func _btn(bg: Color, border_c: Color, _fg: Color) -> StyleBoxFlat:
	var b := StyleBoxFlat.new()
	b.bg_color = bg
	b.set_border_width_all(2)
	b.border_color = border_c
	b.set_corner_radius_all(8)
	b.content_margin_left = 12
	b.content_margin_right = 12
	b.content_margin_top = 8
	b.content_margin_bottom = 8
	return b


static func style_bar(bar: ProgressBar, fill: Color) -> void:
	var bg := StyleBoxFlat.new()
	bg.bg_color = Color("0a101c")
	bg.set_border_width_all(1)
	bg.border_color = Color("ffffff22")
	bg.set_corner_radius_all(4)
	var f := StyleBoxFlat.new()
	f.bg_color = fill
	f.set_corner_radius_all(3)
	bar.add_theme_stylebox_override("background", bg)
	bar.add_theme_stylebox_override("fill", f)
