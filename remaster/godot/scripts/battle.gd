extends Node3D
## 3D battle scene. The Rust BattleSession decides everything; this script
## only turns its event stream into camera moves, animations, effects and HUD.

const ANCHORS := [Vector3(-1.9, 0, 1.4), Vector3(2.1, 0, -1.9)]
const CAM_WIDE := [Vector3(-4.6, 2.6, 6.2), Vector3(0.6, 0.7, -0.9)]
const CAM_SIDE := [
	[Vector3(-4.2, 1.9, 3.6), Vector3(0.6, 0.7, -0.9)], # player attacking
	[Vector3(4.6, 2.0, -0.2), Vector3(-1.0, 0.7, 0.2)], # foe attacking
]
const STAT_LABEL := {"attack": "Attack", "defense": "Defense", "sp_attack": "Sp. Atk", "sp_defense": "Sp. Def", "speed": "Speed"}

var session := BattleSession.new()
var mons: Array = [null, null]
var camera: Camera3D
var vfx: Node3D
var speed := 1.0
var auto_play := false
var return_scene := ""

# HUD
var message: Label
var move_grid: GridContainer
var party_row: HBoxContainer
var hud := [{}, {}]


func _ready() -> void:
	var pending: Dictionary = Game.pending_battle
	auto_play = pending.get("auto_play", false) or Game.smoke
	return_scene = pending.get("return_scene", "")
	if Game.smoke:
		speed = 0.35
	_build_stage()
	_build_ui()
	var foe: Array = pending.get("foe", [{"creature": "sproutail", "level": 7}, {"creature": "zapmoth", "level": 6}])
	if not session.load(Game.data_dir) or not session.start(Game.player_party, foe, pending.get("seed", 7)):
		_show("Battle failed to start: " + session.get_error())
		return
	await _send_in(1)
	await _say("A wild %s appeared!" % _info(1).name if return_scene.ends_with("town.tscn") else "%s is up against you!" % _info(1).name)
	await _send_in(0)
	await _say("Go, %s!" % _info(0).name)
	Game.capture("battle_start")
	_next()


# ---------------------------------------------------------------------------
# Stage


func _build_stage() -> void:
	var env := Environment.new()
	var sky_mat := ProceduralSkyMaterial.new()
	sky_mat.sky_top_color = Color("6fa8e8")
	sky_mat.sky_horizon_color = Color("cfe6f5")
	sky_mat.ground_horizon_color = Color("cfe6f5")
	sky_mat.ground_bottom_color = Color("7aa36a")
	var sky := Sky.new()
	sky.sky_material = sky_mat
	env.background_mode = Environment.BG_SKY
	env.sky = sky
	env.ambient_light_source = Environment.AMBIENT_SOURCE_SKY
	env.ambient_light_energy = 0.9
	env.tonemap_mode = Environment.TONE_MAPPER_FILMIC
	var we := WorldEnvironment.new()
	we.environment = env
	add_child(we)

	var sun := DirectionalLight3D.new()
	sun.rotation_degrees = Vector3(-50, -35, 0)
	sun.light_energy = 1.2
	sun.shadow_enabled = true
	add_child(sun)

	var ground := PlaneMesh.new()
	ground.size = Vector2(60, 60)
	add_child(Placeholder.part(ground, Toon.material(Color("86c06a"), false)))
	var arena := Placeholder.cylinder(5.2, 5.2, 0.06, 48)
	add_child(Placeholder.part(arena, Toon.material(Color("cdbb8a"), false), Vector3(0, 0.03, -0.2)))
	for a in ANCHORS:
		var pad := Placeholder.cylinder(1.0, 1.05, 0.1, 32)
		add_child(Placeholder.part(pad, Toon.material(Color("a7c98a")), a + Vector3(0, 0.05, 0)))
	# Background trees ring the arena.
	for i in 14:
		var ang := float(i) / 14.0 * TAU
		var tree := Placeholder.world_object({"object": "tree_large", "model": "", "width": 2, "depth": 2, "doors": []})
		tree.position = Vector3(cos(ang) * 11.0 - 1.0, 0, sin(ang) * 9.0 - 1.0)
		add_child(tree)

	camera = Camera3D.new()
	camera.fov = 45
	add_child(camera)
	_camera_to(CAM_WIDE[0], CAM_WIDE[1], 0.0)

	vfx = preload("res://scripts/vfx.gd").new()
	vfx.speed = speed
	add_child(vfx)


func _camera_to(pos: Vector3, target: Vector3, duration: float) -> void:
	if duration <= 0.0:
		camera.position = pos
		camera.look_at(target)
		return
	var from := camera.global_transform
	var to := Transform3D(Basis.IDENTITY, pos).looking_at(target)
	var t := create_tween()
	t.tween_method(func(w: float): camera.global_transform = from.interpolate_with(to, w), 0.0, 1.0, duration * speed).set_trans(Tween.TRANS_SINE)
	await t.finished


# ---------------------------------------------------------------------------
# HUD


func _build_ui() -> void:
	var layer := CanvasLayer.new()
	add_child(layer)
	var root := Control.new()
	root.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	root.mouse_filter = Control.MOUSE_FILTER_IGNORE
	layer.add_child(root)

	hud[1] = _status_panel(root, Vector2(24, 24))
	hud[0] = _status_panel(root, Vector2(1280 - 24 - 340, 720 - 210 - 120))

	var bottom := PanelContainer.new()
	bottom.add_theme_stylebox_override("panel", _box(Color(0.1, 0.11, 0.16, 0.92), 10))
	bottom.position = Vector2(16, 720 - 196)
	bottom.size = Vector2(1280 - 32, 180)
	root.add_child(bottom)
	var row := HBoxContainer.new()
	row.add_theme_constant_override("separation", 18)
	bottom.add_child(row)

	message = Label.new()
	message.custom_minimum_size = Vector2(520, 0)
	message.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	message.add_theme_font_size_override("font_size", 24)
	message.vertical_alignment = VERTICAL_ALIGNMENT_CENTER
	row.add_child(message)

	var right := VBoxContainer.new()
	right.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	row.add_child(right)
	move_grid = GridContainer.new()
	move_grid.columns = 2
	move_grid.size_flags_vertical = Control.SIZE_EXPAND_FILL
	right.add_child(move_grid)
	party_row = HBoxContainer.new()
	right.add_child(party_row)


func _status_panel(parent: Control, pos: Vector2) -> Dictionary:
	var panel := PanelContainer.new()
	panel.add_theme_stylebox_override("panel", _box(Color(0.97, 0.96, 0.92, 0.95), 12))
	panel.position = pos
	panel.custom_minimum_size = Vector2(340, 0)
	parent.add_child(panel)
	var v := VBoxContainer.new()
	panel.add_child(v)
	var top := HBoxContainer.new()
	v.add_child(top)
	var name_label := Label.new()
	name_label.add_theme_font_size_override("font_size", 22)
	name_label.add_theme_color_override("font_color", Color(0.12, 0.12, 0.16))
	name_label.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	top.add_child(name_label)
	var status := Label.new()
	status.add_theme_font_size_override("font_size", 16)
	top.add_child(status)
	var bar := ProgressBar.new()
	bar.show_percentage = false
	bar.custom_minimum_size = Vector2(0, 14)
	bar.add_theme_stylebox_override("background", _box(Color(0.25, 0.25, 0.3), 6))
	bar.add_theme_stylebox_override("fill", _box(Color("4cc35a"), 6))
	v.add_child(bar)
	var hp := Label.new()
	hp.add_theme_color_override("font_color", Color(0.2, 0.2, 0.25))
	hp.horizontal_alignment = HORIZONTAL_ALIGNMENT_RIGHT
	v.add_child(hp)
	return {"panel": panel, "name": name_label, "status": status, "bar": bar, "hp": hp}


func _box(color: Color, radius: int) -> StyleBoxFlat:
	var sb := StyleBoxFlat.new()
	sb.bg_color = color
	sb.set_corner_radius_all(radius)
	sb.set_content_margin_all(10)
	return sb


func _info(side: int) -> Dictionary:
	var s: Dictionary = session.side_info(side)
	return s.party[s.active]


func _refresh_panel(side: int, animate_hp := -1) -> void:
	var info := _info(side)
	var h: Dictionary = hud[side]
	h.name.text = "%s  Lv%d" % [info.name, info.level]
	h.status.text = String(info.status).to_upper()
	var bar: ProgressBar = h.bar
	bar.max_value = info.max_hp
	var hp_value: int = info.hp if animate_hp < 0 else animate_hp
	h.hp.text = "%d / %d" % [hp_value, info.max_hp]
	if animate_hp < 0:
		bar.value = info.hp
	else:
		var t := create_tween()
		t.tween_property(bar, "value", float(animate_hp), 0.45 * speed)
		await t.finished
	var frac := bar.value / maxf(bar.max_value, 1.0)
	var fill: StyleBoxFlat = bar.get_theme_stylebox("fill")
	fill.bg_color = Color("4cc35a") if frac > 0.5 else (Color("e8c13a") if frac > 0.2 else Color("e0503a"))


func _say(text: String) -> void:
	message.text = text
	await get_tree().create_timer(0.9 * speed).timeout


func _show(text: String) -> void:
	message.text = text


func _clear_choices() -> void:
	for c in move_grid.get_children():
		c.queue_free()
	for c in party_row.get_children():
		c.queue_free()


# ---------------------------------------------------------------------------
# Flow


func _next() -> void:
	var phase: Dictionary = session.phase()
	match phase.kind:
		"choose":
			if auto_play:
				_take_turn(session.ai_action(0))
			else:
				_offer_moves()
		"replace":
			for side in [1, 0]:
				if phase.need[side]:
					if side == 1 or auto_play:
						await _play(session.replace(side, _first_healthy(side)))
					else:
						_offer_party(true)
						return
			_next()
		"finished":
			_finish(int(phase.winner))


func _first_healthy(side: int) -> int:
	var s: Dictionary = session.side_info(side)
	for i in s.party.size():
		if s.party[i].hp > 0 and i != s.active:
			return i
	return s.active


func _offer_moves() -> void:
	_clear_choices()
	var info := _info(0)
	_show("What will %s do?" % info.name)
	var all_empty := true
	for m in info.moves:
		if m.pp > 0:
			all_empty = false
	for i in info.moves.size():
		var m: Dictionary = info.moves[i]
		var b := Button.new()
		b.text = "%s   %d/%d" % [m.name, m.pp, m.max_pp]
		b.custom_minimum_size = Vector2(300, 52)
		b.add_theme_font_size_override("font_size", 20)
		var col := Game.type_color(m.type)
		b.add_theme_stylebox_override("normal", _box(col.darkened(0.15), 8))
		b.add_theme_stylebox_override("hover", _box(col, 8))
		b.add_theme_stylebox_override("pressed", _box(col.lightened(0.2), 8))
		b.disabled = m.pp == 0 and not all_empty
		b.pressed.connect(_take_turn.bind({"move": i}))
		move_grid.add_child(b)
	_offer_party(false)
	if move_grid.get_child_count() > 0:
		(move_grid.get_child(0) as Button).grab_focus()


## Party buttons: a switch during the choose phase, or a forced replacement.
func _offer_party(forced: bool) -> void:
	if forced:
		_clear_choices()
		_show("Choose who to send out next.")
	var s: Dictionary = session.side_info(0)
	for i in s.party.size():
		var m: Dictionary = s.party[i]
		if i == s.active or m.hp <= 0:
			continue
		var b := Button.new()
		b.text = "Switch: %s (%d HP)" % [m.name, m.hp]
		if forced:
			b.pressed.connect(_replace_with.bind(i))
		else:
			b.pressed.connect(_take_turn.bind({"switch": i}))
		party_row.add_child(b)


func _replace_with(party_index: int) -> void:
	_clear_choices()
	await _play(session.replace(0, party_index))
	_next()


func _take_turn(action: Dictionary) -> void:
	_clear_choices()
	var events: Array = session.run_turn(action, session.ai_action(1))
	if events.is_empty():
		_show("Invalid action: " + session.get_error())
		await get_tree().create_timer(1.0).timeout
		_next()
		return
	await _play(events)
	_next()


func _finish(winner: int) -> void:
	_clear_choices()
	var text := "You won!" if winner == 0 else ("You lost..." if winner == 1 else "It's a draw.")
	await _say(text)
	Game.capture("battle_end")
	await get_tree().create_timer(1.0 * speed).timeout
	if not return_scene.is_empty():
		get_tree().change_scene_to_file(return_scene)
	elif Game.smoke:
		get_tree().quit(0)
	else:
		get_tree().change_scene_to_file("res://scenes/main.tscn")


# ---------------------------------------------------------------------------
# Event playback


func _name(side: int) -> String:
	var prefix := "" if side == 0 else "The foe's "
	return prefix + String(_info(side).name)


func _play(events: Array) -> void:
	for e in events:
		var side: int = e.get("side", 0)
		match e.event:
			"turn_started":
				pass
			"move_used":
				await _say("%s used %s!" % [_name(side), e.name])
				await _move_effect(e)
			"struggled":
				await _say("%s has no moves left and lashes out!" % _name(side))
				await _lunge(side)
			"missed":
				await _say("%s's attack missed!" % _name(side))
			"no_effect":
				await _say("It doesn't affect %s..." % _name(side).to_lower())
			"failed":
				await _say("But it failed!")
			"fully_paralyzed":
				await _shake(side, Color("f2cc2e"))
				await _say("%s is paralyzed and can't move!" % _name(side))
			"damage":
				_shake(side, Color.WHITE)
				await _refresh_panel(side, e.hp_after)
				if e.crit:
					await _say("A critical hit!")
				if e.effectiveness == "super":
					await _say("It's super effective!")
				elif e.effectiveness == "resisted":
					await _say("It's not very effective...")
			"recoil":
				await _refresh_panel(side, e.hp_after)
				await _say("%s is hurt by recoil!" % _name(side))
			"status_applied":
				vfx.aura(_mon_center(side), _status_color(e.status))
				_refresh_panel(side)
				await _say("%s is now %s!" % [_name(side), _status_text(e.status)])
			"status_damage":
				vfx.burst(_mon_center(side), _status_color(e.status))
				await _refresh_panel(side, e.hp_after)
				await _say("%s is hurt by its %s!" % [_name(side), e.status])
			"stat_changed":
				var up: bool = e.delta > 0
				await vfx.aura(_mon_center(side), Color("6fd3ff") if up else Color("ff7a6f"))
				var how := ("rose" if absi(e.delta) == 1 else "rose sharply") if up else ("fell" if absi(e.delta) == 1 else "harshly fell")
				await _say("%s's %s %s!" % [_name(side), STAT_LABEL.get(e.stat, e.stat), how])
			"fainted":
				await _faint(side)
				await _say("%s fainted!" % _name(side))
			"switched_out":
				await _withdraw(side)
			"switched_in":
				await _send_in(side)
				await _say(("Go, %s!" if side == 0 else "The foe sent out %s!") % _info(side).name)
			"battle_ended":
				pass


func _mon_center(side: int) -> Vector3:
	return ANCHORS[side] + Vector3(0, 0.7, 0)


func _status_color(status: String) -> Color:
	return {"burn": Color("ee7a3a"), "poison": Color("a052c8"), "paralysis": Color("f2cc2e")}.get(status, Color.WHITE)


func _status_text(status: String) -> String:
	return {"burn": "burned", "poison": "poisoned", "paralysis": "paralyzed"}.get(status, status)


func _move_effect(e: Dictionary) -> void:
	var side: int = e.side
	var target := 1 - side
	var assets: Dictionary = Game.manifest.get("moves", {}).get(e.move, {})
	var template: String = assets.get("vfx", "projectile")
	var hit_time: float = assets.get("hit_time", 0.4)
	var color := Game.type_color(e.type)
	await _camera_to(CAM_SIDE[side][0], CAM_SIDE[side][1], 0.35)
	if e.category == "status":
		await vfx.play(template, _mon_center(side), _mon_center(side), color, hit_time)
	elif template == "contact_strike":
		await _lunge(side)
		await vfx.play(template, _mon_center(side), _mon_center(target), color, hit_time)
	else:
		await _hop(side)
		await vfx.play(template, _mon_center(side), _mon_center(target), color, hit_time)
	_camera_to(CAM_WIDE[0], CAM_WIDE[1], 0.4)


func _send_in(side: int) -> void:
	var info := _info(side)
	if mons[side]:
		mons[side].queue_free()
	var types: Array = info.types
	var main := Game.type_color(types[0])
	var accent := Game.type_color(types[1]) if types.size() > 1 else main.lightened(0.4)
	var assets: Dictionary = Game.manifest.get("creatures", {}).get(info.creature, {})
	var node := Toon.try_load_model(assets.get("model", ""))
	if node == null:
		node = Placeholder.creature(info.creature, main, accent)
	node.scale = Vector3.ONE * float(assets.get("scale", 1.0)) * (1.15 if side == 1 else 1.0)
	add_child(node)
	node.position = ANCHORS[side]
	node.look_at(ANCHORS[1 - side] * Vector3(1, 0, 1), Vector3.UP)
	mons[side] = node
	_refresh_panel(side)
	var full := node.scale
	node.scale = Vector3.ONE * 0.01
	vfx.burst(_mon_center(side), Color.WHITE, 30, 2.5)
	var t := create_tween()
	t.tween_property(node, "scale", full, 0.35 * speed).set_trans(Tween.TRANS_BACK).set_ease(Tween.EASE_OUT)
	await t.finished


func _withdraw(side: int) -> void:
	var node: Node3D = mons[side]
	if node == null:
		return
	var t := create_tween()
	t.tween_property(node, "scale", Vector3.ONE * 0.01, 0.3 * speed)
	await t.finished
	node.queue_free()
	mons[side] = null


func _faint(side: int) -> void:
	var node: Node3D = mons[side]
	if node == null:
		return
	var t := create_tween().set_parallel()
	t.tween_property(node, "position:y", -1.2, 0.6 * speed).set_trans(Tween.TRANS_QUAD).set_ease(Tween.EASE_IN)
	t.tween_property(node, "scale", node.scale * Vector3(1.2, 0.4, 1.2), 0.6 * speed)
	await t.finished
	node.queue_free()
	mons[side] = null


func _lunge(side: int) -> void:
	var node: Node3D = mons[side]
	if node == null:
		return
	var start := node.position
	var toward := start.lerp(ANCHORS[1 - side], 0.6)
	var t := create_tween()
	t.tween_property(node, "position", toward, 0.18 * speed).set_trans(Tween.TRANS_QUAD).set_ease(Tween.EASE_IN)
	t.tween_property(node, "position", start, 0.25 * speed).set_trans(Tween.TRANS_QUAD)
	await get_tree().create_timer(0.18 * speed).timeout


func _hop(side: int) -> void:
	var node: Node3D = mons[side]
	if node == null:
		return
	var y := node.position.y
	var t := create_tween()
	t.tween_property(node, "position:y", y + 0.35, 0.12 * speed)
	t.tween_property(node, "position:y", y, 0.12 * speed)
	await t.finished


## Shakes the creature and flashes its body.
func _shake(side: int, flash: Color) -> void:
	var node: Node3D = mons[side]
	if node == null:
		return
	var body: MeshInstance3D = node.get_meta("body") if node.has_meta("body") else null
	var original: Material = body.material_override if body else null
	if body:
		body.material_override = Toon.glow(flash, 1.0)
	var x := node.position.x
	var t := create_tween()
	for i in 3:
		t.tween_property(node, "position:x", x + 0.12, 0.04 * speed)
		t.tween_property(node, "position:x", x - 0.12, 0.04 * speed)
	t.tween_property(node, "position:x", x, 0.04 * speed)
	await t.finished
	if body and is_instance_valid(body):
		body.material_override = original
