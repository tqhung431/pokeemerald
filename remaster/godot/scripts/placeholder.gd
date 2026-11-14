class_name Placeholder
## Primitive-mesh stand-ins for models that do not exist yet. Every builder
## returns a Node3D with its origin where the asset spec puts the pivot, so a
## real .glb dropped in later lines up without code changes.


static func part(mesh: Mesh, mat: Material, pos := Vector3.ZERO, scl := Vector3.ONE) -> MeshInstance3D:
	var mi := MeshInstance3D.new()
	mi.mesh = mesh
	mi.material_override = mat
	mi.position = pos
	mi.scale = scl
	return mi


static func sphere(radius := 0.5, segments := 16) -> SphereMesh:
	var m := SphereMesh.new()
	m.radius = radius
	m.height = radius * 2.0
	m.radial_segments = segments
	m.rings = maxi(segments / 2, 3)
	return m


static func box(size: Vector3) -> BoxMesh:
	var m := BoxMesh.new()
	m.size = size
	return m


static func cylinder(top: float, bottom: float, height: float, segments := 12) -> CylinderMesh:
	var m := CylinderMesh.new()
	m.top_radius = top
	m.bottom_radius = bottom
	m.height = height
	m.radial_segments = segments
	return m


## A creature: rounded body in its main type colour, accents in the second,
## eyes on the -Z (forward) side. Shape varies with the creature key.
## The body mesh is stored as meta "body" for hit flashes.
static func creature(key: String, main: Color, accent: Color) -> Node3D:
	var root := Node3D.new()
	root.name = key
	var h := absi(key.hash())
	var tall := 0.75 + float(h % 5) * 0.12
	var wide := 0.75 + float((h / 5) % 4) * 0.1
	var body := part(sphere(0.5, 20), Toon.material(main), Vector3(0, tall * 0.5, 0), Vector3(wide, tall, wide))
	root.add_child(body)
	root.set_meta("body", body)

	var accent_mat := Toon.material(accent)
	match h % 3:
		0: # ears
			for s in [-1.0, 1.0]:
				var ear := part(cylinder(0.0, 0.14, 0.4, 8), accent_mat, Vector3(s * wide * 0.28, tall * 0.98, 0))
				ear.rotation.z = -s * 0.35
				root.add_child(ear)
		1: # crest
			root.add_child(part(cylinder(0.0, 0.18, 0.5, 4), accent_mat, Vector3(0, tall * 1.05, 0.05)))
		2: # wings
			for s in [-1.0, 1.0]:
				var wing := part(box(Vector3(0.6, 0.06, 0.35)), accent_mat, Vector3(s * wide * 0.6, tall * 0.65, 0.05))
				wing.rotation.z = s * 0.4
				root.add_child(wing)
	var tail := part(sphere(0.18, 10), accent_mat, Vector3(0, tall * 0.3, wide * 0.5))
	root.add_child(tail)

	var white := Toon.material(Color.WHITE, false)
	var black := Toon.material(Color(0.05, 0.05, 0.08), false)
	for s in [-1.0, 1.0]:
		var eye_pos := Vector3(s * wide * 0.17, tall * 0.62, -wide * 0.44)
		root.add_child(part(sphere(0.09, 10), white, eye_pos))
		root.add_child(part(sphere(0.05, 8), black, eye_pos + Vector3(0, 0, -0.06)))
	return root


## A humanoid stand-in (player and NPCs), about 1.6 m tall, facing -Z.
static func character(shirt: Color, hat: Color) -> Node3D:
	var root := Node3D.new()
	var skin := Toon.material(Color("f1c8a0"))
	root.add_child(part(cylinder(0.12, 0.14, 0.7, 10), Toon.material(Color("3b4a6b")), Vector3(0, 0.35, 0)))
	root.add_child(part(cylinder(0.22, 0.26, 0.55, 12), Toon.material(shirt), Vector3(0, 0.97, 0)))
	root.add_child(part(sphere(0.21, 16), skin, Vector3(0, 1.42, 0)))
	root.add_child(part(cylinder(0.2, 0.23, 0.12, 14), Toon.material(hat), Vector3(0, 1.58, 0)))
	root.add_child(part(box(Vector3(0.3, 0.04, 0.18)), Toon.material(hat), Vector3(0, 1.53, -0.22)))
	var black := Toon.material(Color(0.05, 0.05, 0.08), false)
	for s in [-1.0, 1.0]:
		root.add_child(part(sphere(0.035, 8), black, Vector3(s * 0.08, 1.44, -0.19)))
	return root


## A world object from a WorldSession placement dictionary. The returned node
## is positioned by the caller at the footprint's north-west corner; meshes are
## laid out inside the footprint in +X / +Z.
static func world_object(p: Dictionary) -> Node3D:
	var root := Node3D.new()
	var w := float(p.width)
	var d := float(p.depth)
	var center := Vector3(w * 0.5, 0, d * 0.5)
	var model: String = p.model
	match String(p.object):
		"house_small", "heal_center", "item_shop":
			var roof_color := Color("4f7fc4")
			var wall_color := Color("efe4cc")
			if p.object == "heal_center":
				roof_color = Color("2fa39a")
				wall_color = Color("f6f2ea")
			elif p.object == "item_shop":
				roof_color = Color("e0a03a")
			elif model.contains("_red"):
				roof_color = Color("c4544f")
			var wall_h := 1.8 if p.object == "house_small" else 2.3
			root.add_child(part(box(Vector3(w - 0.3, wall_h, d - 0.4)), Toon.material(wall_color), center + Vector3(0, wall_h * 0.5, 0)))
			var roof := PrismMesh.new()
			roof.size = Vector3(w, 1.1, d - 0.1)
			root.add_child(part(roof, Toon.material(roof_color), center + Vector3(0, wall_h + 0.55, 0)))
			for door in p.doors:
				var dp := Vector3(float(door.x) + 0.5, 0.6, float(door.y) + 0.85)
				root.add_child(part(box(Vector3(0.75, 1.2, 0.12)), Toon.material(Color("7a4b2a")), dp))
			# Windows on the front face.
			var glass := Toon.material(Color("a9d6f0"), false)
			for i in range(int(w)):
				var is_door := false
				for door in p.doors:
					if int(door.x) == i:
						is_door = true
				if not is_door and i > 0 and i < int(w) - 1:
					root.add_child(part(box(Vector3(0.5, 0.45, 0.06)), glass, Vector3(i + 0.5, wall_h * 0.6, d - 0.18)))
		"tree_round", "tree_large":
			var big := p.object == "tree_large"
			var leaf := Color("e08a3a") if model.contains("autumn") else Color("3f9a4a")
			var s := 1.6 if big else 1.0
			root.add_child(part(cylinder(0.12 * s, 0.16 * s, 0.9 * s, 8), Toon.material(Color("7a5434")), center + Vector3(0, 0.45 * s, 0)))
			root.add_child(part(sphere(0.55 * s, 10), Toon.material(leaf), center + Vector3(0, 1.25 * s, 0)))
			root.add_child(part(sphere(0.4 * s, 10), Toon.material(leaf.lightened(0.12)), center + Vector3(0.15 * s, 1.75 * s, 0.05)))
		"bush_cuttable":
			root.add_child(part(sphere(0.42, 6), Toon.material(Color("2f7d3a")), center + Vector3(0, 0.38, 0)))
		"rock_breakable":
			var rock := part(sphere(0.42, 6), Toon.material(Color("9a8f86")), center + Vector3(0, 0.3, 0), Vector3(1, 0.75, 1))
			rock.rotation.y = 0.6
			root.add_child(rock)
		"boulder":
			root.add_child(part(sphere(0.48, 8), Toon.material(Color("7d7770")), center + Vector3(0, 0.46, 0)))
		"berry_plant":
			root.add_child(part(sphere(0.32, 8), Toon.material(Color("4f9a3a")), center + Vector3(0, 0.32, 0)))
			for i in 4:
				var a := float(i) * TAU / 4.0
				root.add_child(part(sphere(0.08, 8), Toon.material(Color("d9384a"), false), center + Vector3(cos(a) * 0.26, 0.45, sin(a) * 0.26)))
		"item_capsule":
			var cap := CapsuleMesh.new()
			cap.radius = 0.16
			cap.height = 0.5
			var c := part(cap, Toon.material(Color("f2c63a")), center + Vector3(0, 0.17, 0))
			c.rotation.z = PI / 2
			root.add_child(c)
		"signpost":
			root.add_child(part(box(Vector3(0.1, 0.9, 0.1)), Toon.material(Color("7a5434")), center + Vector3(0, 0.45, 0)))
			root.add_child(part(box(Vector3(0.8, 0.45, 0.08)), Toon.material(Color("c79a5c")), center + Vector3(0, 0.95, 0)))
		"fence_segment":
			var wood := Toon.material(Color("d8c3a0"))
			for x in [0.1, 0.9]:
				root.add_child(part(box(Vector3(0.1, 0.7, 0.1)), wood, Vector3(x, 0.35, 0.5)))
			for y in [0.3, 0.55]:
				root.add_child(part(box(Vector3(1.0, 0.08, 0.06)), wood, Vector3(0.5, y, 0.5)))
		"flower_patch":
			var colors := [Color("f25c7a"), Color("f7e05a"), Color("ffffff"), Color("b07af0")]
			for i in 6:
				var a := float(i) * 1.7
				var pos := center + Vector3(cos(a) * 0.3, 0.08, sin(a * 1.3) * 0.3)
				root.add_child(part(sphere(0.07, 6), Toon.material(colors[i % colors.size()], false), pos))
		_:
			root.add_child(part(box(Vector3(w * 0.8, 0.8, d * 0.8)), Toon.material(Color.MAGENTA), center + Vector3(0, 0.4, 0)))
	return root
