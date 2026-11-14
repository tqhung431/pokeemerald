extends Node3D
## Reusable move-effect templates. A move's manifest entry picks a template
## by name (`vfx`) and tints it by element; `hit_time` sets when the hit lands.
## Each template returns once the hit has landed so damage can play next.

## Seconds per unit of animation; lower in smoke runs.
var speed := 1.0


func play(template: String, from: Vector3, to: Vector3, color: Color, hit_time: float) -> void:
	match template:
		"projectile":
			await projectile(from, to, color, hit_time)
		"contact_strike":
			await contact_strike(to, color)
		"aura":
			await aura(to, color)
		_:
			push_warning("Unknown vfx template '%s', using projectile" % template)
			await projectile(from, to, color, hit_time)


func projectile(from: Vector3, to: Vector3, color: Color, travel: float) -> void:
	var orb := Placeholder.part(Placeholder.sphere(0.18, 12), Toon.glow(color))
	add_child(orb)
	orb.global_position = from
	var trail := _particles(color, 18, 0.35, 0.4)
	trail.one_shot = false
	trail.explosiveness = 0.0
	orb.add_child(trail)
	trail.emitting = true
	var t := create_tween()
	t.tween_property(orb, "global_position", to, maxf(travel, 0.1) * speed).set_trans(Tween.TRANS_SINE)
	await t.finished
	orb.queue_free()
	burst(to, color)


func contact_strike(at: Vector3, color: Color) -> void:
	# The attacker's lunge is animated by the battle scene; this adds the impact.
	burst(at, color, 28, 4.0)
	var ring := Placeholder.part(_torus(), Toon.glow(color), Vector3.ZERO)
	add_child(ring)
	ring.global_position = at
	ring.rotation.x = PI / 2
	ring.scale = Vector3.ONE * 0.2
	var t := create_tween().set_parallel()
	t.tween_property(ring, "scale", Vector3.ONE * 1.3, 0.25 * speed)
	t.tween_property(ring.material_override, "albedo_color:a", 0.0, 0.25 * speed)
	await t.finished
	ring.queue_free()


func aura(at: Vector3, color: Color) -> void:
	var ground := at
	ground.y = 0.05
	var ring := Placeholder.part(_torus(), Toon.glow(color), Vector3.ZERO)
	add_child(ring)
	ring.global_position = ground
	var rise := _particles(color, 30, 0.9, 1.5)
	rise.direction = Vector3.UP
	rise.spread = 15.0
	rise.gravity = Vector3(0, 1.5, 0)
	rise.emission_shape = CPUParticles3D.EMISSION_SHAPE_RING
	rise.emission_ring_axis = Vector3.UP
	rise.emission_ring_radius = 0.7
	rise.emission_ring_inner_radius = 0.5
	rise.emission_ring_height = 0.05
	add_child(rise)
	rise.global_position = ground
	rise.emitting = true
	var t := create_tween().set_parallel()
	t.tween_property(ring, "scale", Vector3.ONE * 1.8, 0.6 * speed)
	t.tween_property(ring.material_override, "albedo_color:a", 0.0, 0.6 * speed)
	await t.finished
	ring.queue_free()
	get_tree().create_timer(1.2).timeout.connect(rise.queue_free)


func burst(at: Vector3, color: Color, amount := 24, velocity := 3.0) -> void:
	var p := _particles(color, amount, 0.5, velocity)
	add_child(p)
	p.global_position = at
	p.emitting = true
	get_tree().create_timer(1.5).timeout.connect(p.queue_free)


func _particles(color: Color, amount: int, lifetime: float, velocity: float) -> CPUParticles3D:
	var p := CPUParticles3D.new()
	p.amount = amount
	p.lifetime = lifetime
	p.one_shot = true
	p.explosiveness = 1.0
	p.emitting = false
	p.direction = Vector3.UP
	p.spread = 180.0
	p.initial_velocity_min = velocity * 0.5
	p.initial_velocity_max = velocity
	p.gravity = Vector3(0, -5, 0)
	p.scale_amount_min = 0.05
	p.scale_amount_max = 0.12
	p.mesh = Placeholder.sphere(0.5, 6)
	p.material_override = Toon.glow(color, 1.5)
	return p


func _torus() -> TorusMesh:
	var m := TorusMesh.new()
	m.inner_radius = 0.55
	m.outer_radius = 0.7
	return m
