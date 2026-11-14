extends SceneTree
## Headless checks that the Rust bridge loads and behaves:
## godot --headless --path godot -s res://tests/bridge_test.gd

var failures := 0


func check(ok: bool, what: String) -> void:
	if ok:
		print("  ok  ", what)
	else:
		failures += 1
		printerr("  FAIL ", what)


func _initialize() -> void:
	var data_dir := ProjectSettings.globalize_path("res://").path_join("../data").simplify_path()

	var s := BattleSession.new()
	check(s.load(data_dir), "BattleSession loads data " + s.get_error())
	check(not s.load("/nonexistent"), "BattleSession rejects a missing data dir")
	check(s.load(data_dir), "BattleSession reloads")
	var manifest = JSON.parse_string(s.manifest_json())
	check(manifest is Dictionary and manifest.creatures.has("emberkit"), "manifest JSON round-trips")
	check(not s.start([{"creature": "nope"}], [{"creature": "duskit"}], 1), "unknown creature rejected")
	check(s.start([{"creature": "emberkit", "level": 12}, {"creature": "cobblet", "level": 10}],
		[{"creature": "sproutail", "level": 12}, {"creature": "zapmoth", "level": 11}], 99), "battle starts")
	check(s.run_turn({"move": 9}, {"move": 0}).is_empty(), "invalid move rejected")

	var turns := 0
	var seen := {}
	while s.phase().kind != "finished" and turns < 300:
		var phase: Dictionary = s.phase()
		var events: Array = []
		if phase.kind == "choose":
			events = s.run_turn(s.ai_action(0), s.ai_action(1))
			turns += 1
		else:
			for side in 2:
				if phase.need[side]:
					var info: Dictionary = s.side_info(side)
					for i in info.party.size():
						if info.party[i].hp > 0:
							events = s.replace(side, i)
							break
		for e in events:
			seen[e.event] = true
	check(s.phase().kind == "finished", "AI battle finishes (%d turns)" % turns)
	var needed := ["move_used", "damage", "fainted", "battle_ended"]
	check(needed.all(func(k): return seen.has(k)), "events include %s: %s" % [needed, seen.keys()])

	var w := WorldSession.new()
	check(w.load(data_dir, "mossvale_town"), "WorldSession loads the town " + w.get_error())
	var spawn: Vector2i = w.spawn()
	check(w.step(spawn.x, spawn.y, "north").result == "door", "door north of spawn")
	check(w.step(spawn.x, spawn.y, "south").result == "moved", "can walk south from spawn")
	check(w.placements().size() > 50, "placements listed")
	check(not w.load(data_dir, "missing_map"), "missing map rejected")

	print("bridge_test: %s" % ("PASSED" if failures == 0 else "%d FAILED" % failures))
	quit(failures)
