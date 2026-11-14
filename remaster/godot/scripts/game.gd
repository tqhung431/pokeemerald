extends Node
## Global state shared between scenes: where the rules data lives, the
## overworld session, the player's position and party, and battle hand-off.

const TYPE_COLORS := {
	"neutral": Color("c9c1ae"),
	"ember": Color("ee7a3a"),
	"tide": Color("4a8ff0"),
	"bloom": Color("5cb85a"),
	"volt": Color("f2cc2e"),
	"stone": Color("a68a58"),
	"gale": Color("9ccbe8"),
	"shade": Color("6a52a3"),
}

## Wild creatures that can appear in flower patches: [key, min level, max level].
const WILD_TABLE := [["sproutail", 3, 6], ["zapmoth", 3, 5], ["cobblet", 4, 6], ["duskit", 3, 5]]

var data_dir := ""
var manifest: Dictionary = {}
var world: WorldSession
var player_tile := Vector2i.ZERO
var player_facing := "south"
var player_party: Array = [{"creature": "emberkit", "level": 8}, {"creature": "pondling", "level": 7}]
var items_found := 0
## Set by start_battle and read by the battle scene.
var pending_battle: Dictionary = {}
## True when launched with `-- --smoke`: scenes drive themselves and quit.
var smoke := false
var screenshot_dir := ""


func _ready() -> void:
	data_dir = ProjectSettings.globalize_path("res://").path_join("../data").simplify_path()
	var args := OS.get_cmdline_user_args()
	smoke = args.has("--smoke")
	for a in args:
		if a.begins_with("--screenshots="):
			screenshot_dir = a.trim_prefix("--screenshots=")
	var probe := BattleSession.new()
	if probe.load(data_dir):
		manifest = JSON.parse_string(probe.manifest_json())
	else:
		push_error("Could not load game data from %s: %s" % [data_dir, probe.get_error()])


func type_color(type_key: String) -> Color:
	return TYPE_COLORS.get(type_key, Color.WHITE)


func ensure_world(map_id := "mossvale_town") -> bool:
	if world != null:
		return true
	world = WorldSession.new()
	if not world.load(data_dir, map_id):
		push_error("Could not load map %s: %s" % [map_id, world.get_error()])
		world = null
		return false
	player_tile = world.spawn()
	return true


func random_wild_party() -> Array:
	var pick: Array = WILD_TABLE.pick_random()
	return [{"creature": pick[0], "level": randi_range(pick[1], pick[2])}]


func start_battle(foe_party: Array, return_scene: String, auto_play := false) -> void:
	pending_battle = {"foe": foe_party, "seed": randi(), "return_scene": return_scene, "auto_play": auto_play}
	get_tree().change_scene_to_file.call_deferred("res://scenes/battle.tscn")


## Saves a screenshot when `--screenshots=<dir>` was given.
func capture(name: String) -> void:
	if screenshot_dir.is_empty():
		return
	await RenderingServer.frame_post_draw
	var img := get_viewport().get_texture().get_image()
	DirAccess.make_dir_recursive_absolute(screenshot_dir)
	img.save_png(screenshot_dir.path_join(name + ".png"))
	print("saved screenshot ", name)
