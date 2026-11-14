extends Node3D
## Overworld scene. WorldSession (Rust) owns the grid rules; this script
## builds the 3D scene from placements and animates what the rules allow.

const DIRS := {"north": Vector2i(0, -1), "south": Vector2i(0, 1), "east": Vector2i(1, 0), "west": Vector2i(-1, 0)}
const STEP_TIME := 0.16
const CAM_OFFSET := Vector3(0, 10.0, 9.5)
const ENCOUNTER_CHANCE := 0.15
const SKILL_VERB := {"cut": "cut down", "smash": "smashed"}

var world: WorldSession
var objects := {} # placement index -> Node3D
var flower_tiles := {} # Vector2i -> true
var player: Node3D
var camera: Camera3D
var vfx: Node3D
var busy := false
var dialog: PanelContainer
var dialog_label: Label


func _ready() -> void:
	if not Game.ensure_world():
		return
	world = Game.world
	_build_environment()
	_build_map()
	_build_ui()
	player = Placeholder.character(Color("d9473f"), Color("2f3e66"))
	add_child(player)
	_place_player(Game.player_tile, Game.player_facing)
	camera.position = player.position + CAM_OFFSET
	camera.look_at(player.position + Vector3(0, 0.5, 0.8))
	if Game.smoke:
		_run_smoke.call_deferred()


func _build_environment() -> void:
	var env := Environment.new()
	var sky_mat := ProceduralSkyMaterial.new()
	sky_mat.sky_top_color = Color("6fa8e8")
	sky_mat.sky_horizon_color = Color("d6ebf7")
	var sky := Sky.new()
	sky.sky_material = sky_mat
	env.background_mode = Environment.BG_SKY
	env.sky = sky
	env.ambient_light_source = Environment.AMBIENT_SOURCE_COLOR
	env.ambient_light_color = Color(0.62, 0.68, 0.8)
	env.ambient_light_energy = 0.45
	env.tonemap_mode = Environment.TONE_MAPPER_LINEAR
	var we := WorldEnvironment.new()
	we.environment = env
	add_child(we)
	var sun := DirectionalLight3D.new()
	sun.rotation_degrees = Vector3(-55, -30, 0)
	sun.light_energy = 0.85
	sun.shadow_enabled = true
	add_child(sun)
	camera = Camera3D.new()
	camera.fov = 40
	add_child(camera)
	vfx = preload("res://scripts/vfx.gd").new()
	add_child(vfx)


func _build_map() -> void:
	var size: Vector2i = world.size()
	var ground := PlaneMesh.new()
	ground.size = Vector2(size.x + 20, size.y + 20)
	add_child(Placeholder.part(ground, Toon.material(Color("8cc56e"), false), Vector3(size.x * 0.5, 0, size.y * 0.5)))
	# A dirt path from the spawn to the east exit, purely decorative.
	var path := PlaneMesh.new()
	path.size = Vector2(size.x - 4, 1.6)
	add_child(Placeholder.part(path, Toon.material(Color("d9c48f"), false), Vector3(size.x * 0.5 + 1, 0.01, 8.5)))

	for p in world.placements():
		if p.removed:
			continue
		if p.object == "flower_patch":
			flower_tiles[Vector2i(p.x, p.y)] = true
		var node := Toon.try_load_model(p.model)
		if node == null:
			node = Placeholder.world_object(p)
		node.position = Vector3(p.x, 0, p.y)
		add_child(node)
		objects[int(p.index)] = node


func _build_ui() -> void:
	var layer := CanvasLayer.new()
	add_child(layer)
	var hint := Label.new()
	hint.text = "%s   |   Arrows: move   Enter: interact   Esc: menu" % world.map_name()
	hint.position = Vector2(16, 12)
	hint.add_theme_font_size_override("font_size", 18)
	hint.add_theme_color_override("font_outline_color", Color.BLACK)
	hint.add_theme_constant_override("outline_size", 6)
	layer.add_child(hint)

	dialog = PanelContainer.new()
	var sb := StyleBoxFlat.new()
	sb.bg_color = Color(0.98, 0.97, 0.93, 0.96)
	sb.border_color = Color(0.25, 0.3, 0.45)
	sb.set_border_width_all(4)
	sb.set_corner_radius_all(12)
	sb.set_content_margin_all(18)
	dialog.add_theme_stylebox_override("panel", sb)
	dialog.position = Vector2(140, 560)
	dialog.custom_minimum_size = Vector2(1000, 120)
	dialog.visible = false
	layer.add_child(dialog)
	dialog_label = Label.new()
	dialog_label.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	dialog_label.add_theme_font_size_override("font_size", 24)
	dialog_label.add_theme_color_override("font_color", Color(0.12, 0.12, 0.16))
	dialog.add_child(dialog_label)


# ---------------------------------------------------------------------------
# Player


func _tile_pos(tile: Vector2i) -> Vector3:
	return Vector3(tile.x + 0.5, 0, tile.y + 0.5)


func _place_player(tile: Vector2i, facing: String) -> void:
	Game.player_tile = tile
	Game.player_facing = facing
	player.position = _tile_pos(tile)
	_face(facing)


func _face(facing: String) -> void:
	Game.player_facing = facing
	var d: Vector2i = DIRS[facing]
	player.look_at(player.position + Vector3(d.x, 0, d.y), Vector3.UP)


func _process(delta: float) -> void:
	if player == null:
		return
	var target := player.position + CAM_OFFSET
	camera.position = camera.position.lerp(target, clampf(delta * 6.0, 0.0, 1.0))
	if busy or Game.smoke:
		return
	if dialog.visible:
		return
	for dir in DIRS:
		var action: String = {"north": "ui_up", "south": "ui_down", "east": "ui_right", "west": "ui_left"}[dir]
		if Input.is_action_pressed(action):
			_try_step(dir)
			return


func _unhandled_input(event: InputEvent) -> void:
	if Game.smoke or busy:
		return
	if event.is_action_pressed("ui_accept"):
		if dialog.visible:
			dialog.visible = false
		else:
			_interact()
	elif event.is_action_pressed("ui_cancel"):
		get_tree().change_scene_to_file("res://scenes/main.tscn")


func _try_step(dir: String) -> void:
	busy = true
	_face(dir)
	var tile := Game.player_tile
	var r: Dictionary = world.step(tile.x, tile.y, dir)
	match r.result:
		"moved":
			var to := Vector2i(r.x, r.y)
			await _walk_to(to)
			if flower_tiles.has(to) and randf() < ENCOUNTER_CHANCE:
				await _encounter()
				return
		"door":
			var dest: String = r.warp
			await _say("The door leads to '%s'. Interiors are not built yet." % dest)
		"blocked":
			var hit: Dictionary = world.facing(tile.x, tile.y, dir)
			if hit.get("kind", "") == "push":
				await _push(hit, dir)
	busy = false


func _walk_to(to: Vector2i) -> void:
	Game.player_tile = to
	var t := create_tween()
	t.tween_property(player, "position", _tile_pos(to), STEP_TIME)
	# Small bob so the stand-in reads as walking.
	var bob := create_tween()
	bob.tween_property(player, "scale", Vector3(1, 0.93, 1), STEP_TIME * 0.5)
	bob.tween_property(player, "scale", Vector3.ONE, STEP_TIME * 0.5)
	await t.finished


func _interact() -> void:
	busy = true
	var tile := Game.player_tile
	var hit: Dictionary = world.facing(tile.x, tile.y, Game.player_facing)
	match hit.get("kind", ""):
		"read":
			var text: String = hit.text
			await _say(text if not text.is_empty() else "Nothing written here.")
		"field_skill":
			var skill: String = hit.skill
			await _remove_object(hit.placement)
			await _say("You %s the %s!" % [SKILL_VERB.get(skill, "used " + skill + " on"), String(hit.object).replace("_", " ")])
		"pickup":
			await _remove_object(hit.placement)
			Game.items_found += 1
			await _say("You found an item! (%d so far)" % Game.items_found)
		"harvest":
			vfx.burst(objects[int(hit.placement)].position + Vector3(0.5, 0.5, 0.5), Color("d9384a"))
			await _say("You picked some berries.")
		"script":
			await _say("(Script '%s' is not implemented yet.)" % hit.script)
		"push":
			await _say("A heavy boulder. Walk into it to push.")
	busy = false


func _remove_object(placement: int) -> void:
	if not world.remove(placement):
		return
	var node: Node3D = objects[placement]
	vfx.burst(node.position + Vector3(0.5, 0.4, 0.5), Color("8fd16a"), 30, 3.0)
	var t := create_tween()
	t.tween_property(node, "scale", Vector3(1.2, 0.01, 1.2), 0.25)
	await t.finished
	node.queue_free()
	objects.erase(placement)


func _push(hit: Dictionary, dir: String) -> void:
	var r: Dictionary = world.push(hit.placement, dir)
	if not r.ok:
		return
	var node: Node3D = objects[int(hit.placement)]
	var t := create_tween()
	t.tween_property(node, "position", Vector3(r.x, 0, r.y), 0.3)
	await t.finished


func _say(text: String) -> void:
	dialog_label.text = text
	dialog.visible = true
	if Game.smoke:
		await get_tree().create_timer(0.4).timeout
		dialog.visible = false
		return
	while dialog.visible:
		await get_tree().process_frame


func _encounter() -> void:
	var t := create_tween()
	t.tween_property(camera, "fov", 20.0, 0.35)
	await t.finished
	Game.start_battle(Game.random_wild_party(), "res://scenes/town.tscn")


# ---------------------------------------------------------------------------
# Smoke run: exercises movement and every interaction, then quits.


func _run_smoke() -> void:
	busy = true
	Game.capture("town_spawn")
	var errors := []
	# Spawn is in front of the player's house door; the welcome sign is south-east.
	var spawn := Game.player_tile
	if world.step(spawn.x, spawn.y, "north").result != "door":
		errors.append("expected the house door north of spawn")
	await _walk_to(spawn + Vector2i(0, 1))
	var hit: Dictionary = world.facing(Game.player_tile.x, Game.player_tile.y, "east")
	if hit.get("kind", "") != "read":
		errors.append("expected a sign east of %s, got %s" % [Game.player_tile, hit])
	await _walk_to(spawn + Vector2i(0, 2))
	# Walk east along the path to the cuttable bush at the exit.
	for i in 15:
		var tile := Game.player_tile
		var r: Dictionary = world.step(tile.x, tile.y, "east")
		if r.result != "moved":
			break
		_face("east")
		await _walk_to(Vector2i(r.x, r.y))
	Game.capture("town_walk")
	hit = world.facing(Game.player_tile.x, Game.player_tile.y, "east")
	if hit.get("kind", "") == "field_skill":
		await _remove_object(hit.placement)
	else:
		errors.append("expected the cuttable bush east of %s, got %s" % [Game.player_tile, hit])
	var after: Dictionary = world.step(Game.player_tile.x, Game.player_tile.y, "east")
	if after.result != "moved":
		errors.append("bush tile still blocked after cutting")
	if errors.is_empty():
		print("SMOKE OK: town")
	else:
		for e in errors:
			push_error("SMOKE FAIL: " + e)
	get_tree().quit(0 if errors.is_empty() else 1)
