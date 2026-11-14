extends Control
## Title menu. With `-- --smoke` it runs an AI-vs-AI battle, then the town
## smoke run, and quits.


func _ready() -> void:
	var bg := ColorRect.new()
	bg.color = Color("1d2a44")
	bg.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	add_child(bg)
	var box := VBoxContainer.new()
	box.set_anchors_and_offsets_preset(Control.PRESET_CENTER)
	box.add_theme_constant_override("separation", 14)
	add_child(box)
	var title := Label.new()
	title.text = "Remaster Prototype"
	title.add_theme_font_size_override("font_size", 48)
	title.horizontal_alignment = HORIZONTAL_ALIGNMENT_CENTER
	box.add_child(title)

	if Game.manifest.is_empty():
		var err := Label.new()
		err.text = "Game data failed to load. Build the Rust bridge and check the output log."
		err.add_theme_color_override("font_color", Color("ff8070"))
		box.add_child(err)

	var first := _button(box, "Explore Mossvale Town", func(): get_tree().change_scene_to_file("res://scenes/town.tscn"))
	_button(box, "Quick battle", func(): Game.start_battle([{"creature": "sproutail", "level": 8}, {"creature": "zapmoth", "level": 7}], ""))
	_button(box, "Watch AI battle", func(): Game.start_battle([{"creature": "cobblet", "level": 8}, {"creature": "duskit", "level": 8}], "", true))
	_button(box, "Quit", func(): get_tree().quit())
	first.grab_focus()

	if Game.smoke:
		Game.start_battle([{"creature": "sproutail", "level": 7}, {"creature": "zapmoth", "level": 6}], "res://scenes/town.tscn", true)


func _button(parent: Control, text: String, action: Callable) -> Button:
	var b := Button.new()
	b.text = text
	b.custom_minimum_size = Vector2(360, 54)
	b.add_theme_font_size_override("font_size", 22)
	b.pressed.connect(action)
	parent.add_child(b)
	return b
