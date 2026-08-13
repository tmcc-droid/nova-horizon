extends Control
## Full-screen station interface — replaces space view while docked.
## Sector radar stays off while docked (flight-only). Galaxy map is a station action.

signal undock_pressed
signal buy_pressed
signal sell_pressed
signal refuel_pressed
signal jump_pressed
signal galaxy_map_pressed
signal jump_to_system(dest_id: String)

enum Tab { MARKET, SERVICES, NAV, INFO }

var _tab: int = Tab.MARKET
var _station_name: String = "STATION"
var _credits: int = 0
var _fuel: int = 0
var _fuel_price: int = 50
var _market: Array = []
var _services: Array = []
var _current_system_id: String = ""
var _link_rows: Array = [] # {id, name, fuel}

var title: Label
var subtitle: Label
var credits_lbl: Label
var market_list: ItemList
var market_panel: Control
var services_panel: Control
var nav_panel: Control
var info_panel: Control
var btn_market: Button
var btn_services: Button
var btn_nav: Button
var btn_info: Button
var services_info: Label
var nav_info: Label
var info_body: Label
var link_list: ItemList
var btn_galaxy: Button
var btn_jump_selected: Button


func _ready() -> void:
	visible = false
	set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	mouse_filter = Control.MOUSE_FILTER_IGNORE
	z_index = 100
	_build_ui()
	_set_tab(Tab.MARKET)


func _unhandled_input(event: InputEvent) -> void:
	if not visible:
		return
	if event is InputEventKey and event.pressed and not event.echo:
		if event.keycode == KEY_ESCAPE or event.keycode == KEY_U:
			undock_pressed.emit()
			get_viewport().set_input_as_handled()
		elif event.keycode == KEY_G:
			galaxy_map_pressed.emit()
			get_viewport().set_input_as_handled()


func open_console(menu: Dictionary, credits: int, fuel: int) -> void:
	_station_name = str(menu.get("def_id", "station")).replace("st.", "").replace("_", " ").to_upper()
	_credits = credits
	_fuel = fuel
	_fuel_price = int(menu.get("fuel_price_per_unit", 50))
	_market = menu.get("market", [])
	_services = menu.get("services", [])
	title.text = _station_name
	subtitle.text = "DOCKED  ·  SECURE BERTH"
	_refresh_lists()
	_set_tab(Tab.MARKET)
	mouse_filter = Control.MOUSE_FILTER_STOP
	visible = true
	show()
	move_to_front()


func set_nav_context(system_id: String, links: Array) -> void:
	_current_system_id = system_id
	_link_rows = links
	_refresh_nav_links()


func update_wallet(credits: int, fuel: int) -> void:
	_credits = credits
	_fuel = fuel
	if credits_lbl:
		credits_lbl.text = "Credits  ₵%d    Fuel  %d" % [_credits, _fuel]


func refresh_market(menu: Dictionary) -> void:
	_market = menu.get("market", [])
	_fuel_price = int(menu.get("fuel_price_per_unit", _fuel_price))
	_refresh_lists()


func close_console() -> void:
	visible = false
	mouse_filter = Control.MOUSE_FILTER_IGNORE


func selected_commodity() -> String:
	var sel := market_list.get_selected_items()
	if sel.is_empty() or _market.is_empty():
		return ""
	var idx: int = sel[0]
	if idx < 0 or idx >= _market.size():
		return ""
	return str(_market[idx].get("commodity_id", ""))


func selected_jump_dest() -> String:
	if link_list == null:
		return ""
	var sel := link_list.get_selected_items()
	if sel.is_empty() or _link_rows.is_empty():
		return ""
	var idx: int = sel[0]
	if idx < 0 or idx >= _link_rows.size():
		return ""
	return str(_link_rows[idx].get("id", ""))


func _build_ui() -> void:
	var dim := ColorRect.new()
	dim.set_anchors_preset(Control.PRESET_FULL_RECT)
	dim.color = Color(0.02, 0.04, 0.08, 0.97)
	dim.mouse_filter = Control.MOUSE_FILTER_STOP
	add_child(dim)

	var root := MarginContainer.new()
	root.set_anchors_preset(Control.PRESET_FULL_RECT)
	root.add_theme_constant_override("margin_left", 28)
	root.add_theme_constant_override("margin_right", 28)
	root.add_theme_constant_override("margin_top", 24)
	root.add_theme_constant_override("margin_bottom", 24)
	add_child(root)

	var v := VBoxContainer.new()
	v.add_theme_constant_override("separation", 12)
	root.add_child(v)

	var head := HBoxContainer.new()
	head.add_theme_constant_override("separation", 12)
	v.add_child(head)

	var head_text := VBoxContainer.new()
	head_text.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	head.add_child(head_text)

	title = Label.new()
	title.add_theme_font_size_override("font_size", 28)
	title.text = "STATION"
	head_text.add_child(title)

	subtitle = Label.new()
	subtitle.add_theme_font_size_override("font_size", 13)
	subtitle.add_theme_color_override("font_color", Color(0.55, 0.75, 0.9))
	subtitle.text = "Docked"
	head_text.add_child(subtitle)

	credits_lbl = Label.new()
	credits_lbl.add_theme_font_size_override("font_size", 16)
	credits_lbl.text = "Credits  ₵0"
	head.add_child(credits_lbl)

	# Always-visible actions (not buried in a tab)
	btn_galaxy = Button.new()
	btn_galaxy.text = "Galaxy Map  (G)"
	btn_galaxy.custom_minimum_size = Vector2(150, 40)
	btn_galaxy.pressed.connect(func(): galaxy_map_pressed.emit())
	head.add_child(btn_galaxy)

	var btn_undock := Button.new()
	btn_undock.text = "UNDOCK  (U)"
	btn_undock.custom_minimum_size = Vector2(140, 40)
	btn_undock.pressed.connect(func(): undock_pressed.emit())
	head.add_child(btn_undock)

	var body := HBoxContainer.new()
	body.size_flags_vertical = Control.SIZE_EXPAND_FILL
	body.add_theme_constant_override("separation", 16)
	v.add_child(body)

	var side := VBoxContainer.new()
	side.custom_minimum_size = Vector2(180, 0)
	side.add_theme_constant_override("separation", 8)
	body.add_child(side)

	btn_market = _side_btn("Market", side, Tab.MARKET)
	btn_services = _side_btn("Services", side, Tab.SERVICES)
	btn_nav = _side_btn("Navigation", side, Tab.NAV)
	btn_info = _side_btn("Station Info", side, Tab.INFO)

	var content := PanelContainer.new()
	content.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	content.size_flags_vertical = Control.SIZE_EXPAND_FILL
	body.add_child(content)

	var cmargin := MarginContainer.new()
	cmargin.add_theme_constant_override("margin_left", 16)
	cmargin.add_theme_constant_override("margin_right", 16)
	cmargin.add_theme_constant_override("margin_top", 14)
	cmargin.add_theme_constant_override("margin_bottom", 14)
	content.add_child(cmargin)

	var stack := Control.new()
	stack.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	stack.size_flags_vertical = Control.SIZE_EXPAND_FILL
	cmargin.add_child(stack)

	market_panel = _make_market_panel()
	services_panel = _make_services_panel()
	nav_panel = _make_nav_panel()
	info_panel = _make_info_panel()
	for p in [market_panel, services_panel, nav_panel, info_panel]:
		p.set_anchors_preset(Control.PRESET_FULL_RECT)
		stack.add_child(p)

	var footer := Label.new()
	footer.add_theme_font_size_override("font_size", 12)
	footer.add_theme_color_override("font_color", Color(0.5, 0.65, 0.8))
	footer.text = "Sector radar: lower-right.  G = galaxy map  ·  U / Esc = undock."
	v.add_child(footer)


func _side_btn(text: String, parent: Control, tab: int) -> Button:
	var b := Button.new()
	b.text = text
	b.custom_minimum_size = Vector2(0, 42)
	b.pressed.connect(func(): _set_tab(tab))
	parent.add_child(b)
	return b


func _make_market_panel() -> Control:
	var p := VBoxContainer.new()
	p.add_theme_constant_override("separation", 10)
	var h := Label.new()
	h.text = "COMMODITY EXCHANGE"
	h.add_theme_font_size_override("font_size", 18)
	p.add_child(h)
	market_list = ItemList.new()
	market_list.size_flags_vertical = Control.SIZE_EXPAND_FILL
	market_list.custom_minimum_size = Vector2(0, 280)
	p.add_child(market_list)
	var row := HBoxContainer.new()
	row.add_theme_constant_override("separation", 12)
	p.add_child(row)
	var buy := Button.new()
	buy.text = "Buy ×1"
	buy.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	buy.pressed.connect(func(): buy_pressed.emit())
	row.add_child(buy)
	var sell := Button.new()
	sell.text = "Sell ×1"
	sell.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	sell.pressed.connect(func(): sell_pressed.emit())
	row.add_child(sell)
	return p


func _make_services_panel() -> Control:
	var p := VBoxContainer.new()
	p.add_theme_constant_override("separation", 12)
	var h := Label.new()
	h.text = "STATION SERVICES"
	h.add_theme_font_size_override("font_size", 18)
	p.add_child(h)
	services_info = Label.new()
	services_info.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	services_info.size_flags_vertical = Control.SIZE_EXPAND_FILL
	p.add_child(services_info)
	var refuel := Button.new()
	refuel.text = "Refuel to full"
	refuel.custom_minimum_size = Vector2(0, 44)
	refuel.pressed.connect(func(): refuel_pressed.emit())
	p.add_child(refuel)
	var shipyard := Button.new()
	shipyard.text = "Shipyard (coming soon)"
	shipyard.disabled = true
	p.add_child(shipyard)
	return p


func _make_nav_panel() -> Control:
	var p := VBoxContainer.new()
	p.add_theme_constant_override("separation", 10)
	var h := Label.new()
	h.text = "HYPERSPACE LINKS"
	h.add_theme_font_size_override("font_size", 18)
	p.add_child(h)
	nav_info = Label.new()
	nav_info.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	nav_info.text = "Linked systems from this berth (max 5). Select one and jump, or open the full galaxy map."
	p.add_child(nav_info)
	link_list = ItemList.new()
	link_list.size_flags_vertical = Control.SIZE_EXPAND_FILL
	link_list.custom_minimum_size = Vector2(0, 220)
	p.add_child(link_list)
	var row := HBoxContainer.new()
	row.add_theme_constant_override("separation", 12)
	p.add_child(row)
	btn_jump_selected = Button.new()
	btn_jump_selected.text = "Jump to selected"
	btn_jump_selected.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	btn_jump_selected.custom_minimum_size = Vector2(0, 44)
	btn_jump_selected.pressed.connect(_on_jump_selected)
	row.add_child(btn_jump_selected)
	var map_btn := Button.new()
	map_btn.text = "Full galaxy map"
	map_btn.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	map_btn.custom_minimum_size = Vector2(0, 44)
	map_btn.pressed.connect(func(): galaxy_map_pressed.emit())
	row.add_child(map_btn)
	return p


func _on_jump_selected() -> void:
	var dest := selected_jump_dest()
	if dest.is_empty():
		return
	jump_to_system.emit(dest)


func _make_info_panel() -> Control:
	var p := VBoxContainer.new()
	p.add_theme_constant_override("separation", 12)
	var h := Label.new()
	h.text = "STATION DIRECTORY"
	h.add_theme_font_size_override("font_size", 18)
	p.add_child(h)
	info_body = Label.new()
	info_body.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	info_body.size_flags_vertical = Control.SIZE_EXPAND_FILL
	p.add_child(info_body)
	return p


func _refresh_nav_links() -> void:
	if link_list == null:
		return
	link_list.clear()
	for row in _link_rows:
		var name := str(row.get("name", row.get("id", "?")))
		var fuel := int(row.get("fuel", 1))
		var kind := str(row.get("kind", ""))
		var tag := ""
		if kind == "transit":
			tag = "  [pass-through]"
		elif kind == "hub":
			tag = "  [hub]"
		link_list.add_item("%s    fuel %d%s" % [name, fuel, tag])
	if nav_info:
		if _link_rows.is_empty():
			nav_info.text = "No hyperspace links charted for this system yet."
		else:
			nav_info.text = (
				"You are in %s.  %d direct link(s).  Select a destination, or open the full map."
				% [_current_system_id.replace("sys.", ""), _link_rows.size()]
			)


func _refresh_lists() -> void:
	if credits_lbl:
		credits_lbl.text = "Credits  ₵%d    Fuel  %d" % [_credits, _fuel]
	if market_list:
		market_list.clear()
		for row in _market:
			var cname := str(row.get("commodity_id", "")).replace("commodity.", "").capitalize()
			market_list.add_item(
				"%s    stock %s    buy ₵%s    sell ₵%s"
				% [cname, row.get("stock"), row.get("buy_price"), row.get("sell_price")]
			)
	if services_info:
		var svc := ", ".join(PackedStringArray(_services.map(func(s): return str(s)))) if _services.size() else "none"
		services_info.text = (
			"Available: %s\n\nRefuel: ₵%d / unit\nYour fuel: %d\n\nShipyard and outfits will appear here later."
			% [svc, _fuel_price, _fuel]
		)
	_refresh_nav_links()
	if info_body:
		info_body.text = (
			"%s\n\nWelcome, captain.\n\n• Market — buy and sell commodities\n"
			+ "• Services — refuel and hangar (later)\n"
			+ "• Navigation — linked jumps + galaxy map\n\n"
			+ "Press U or Esc to undock."
			% _station_name
		)


func _set_tab(t: int) -> void:
	_tab = t
	if market_panel:
		market_panel.visible = t == Tab.MARKET
	if services_panel:
		services_panel.visible = t == Tab.SERVICES
	if nav_panel:
		nav_panel.visible = t == Tab.NAV
	if info_panel:
		info_panel.visible = t == Tab.INFO
	if btn_market:
		btn_market.disabled = t == Tab.MARKET
	if btn_services:
		btn_services.disabled = t == Tab.SERVICES
	if btn_nav:
		btn_nav.disabled = t == Tab.NAV
	if btn_info:
		btn_info.disabled = t == Tab.INFO
