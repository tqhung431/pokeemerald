use core_data::{AssetManifest, Interaction};
use core_world::{Dir, MapLayout, Placement, Step, World};

const DATA: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data");

fn manifest() -> AssetManifest {
    AssetManifest::load(format!("{DATA}/assets.json")).unwrap()
}

fn town() -> World {
    let layout = MapLayout::load(format!("{DATA}/maps/mossvale_town.json")).unwrap();
    World::build(layout, manifest()).unwrap_or_else(|p| panic!("{p:#?}"))
}

fn tiny(placements: Vec<Placement>) -> MapLayout {
    MapLayout {
        id: "t".into(),
        name: "T".into(),
        width: 6,
        height: 6,
        spawn: [5, 0],
        placements,
    }
}

fn at(object: &str, x: i32, y: i32) -> Placement {
    Placement {
        object: object.into(),
        x,
        y,
        variant: None,
        text: None,
        warp: None,
    }
}

#[test]
fn sample_town_builds() {
    let w = town();
    let [x, y] = w.layout().spawn;
    assert!(w.walkable(x, y));
    // Border trees block; the east exit is open.
    assert!(!w.walkable(0, 5));
    assert!(w.walkable(23, 8) || w.step(22, 8, Dir::East) != Step::Blocked);
}

#[test]
fn doors_are_entered_not_walked_through() {
    // house_small is 5x4 with its door at (2, 3).
    let w = World::build(tiny(vec![at("house_small", 0, 0)]), manifest()).unwrap();
    assert_eq!(w.step(2, 4, Dir::North), Step::Door { placement: 0 });
    assert_eq!(w.step(1, 4, Dir::North), Step::Blocked);
    assert_eq!(w.step(5, 1, Dir::West), Step::Blocked);
}

#[test]
fn signs_read_and_bushes_cut() {
    let mut sign = at("signpost", 2, 2);
    sign.text = Some("Hello".into());
    let mut w = World::build(tiny(vec![sign, at("bush_cuttable", 4, 2)]), manifest()).unwrap();

    let hit = w.facing(2, 3, Dir::North).unwrap();
    assert_eq!(hit.interaction, &Interaction::Read);
    assert_eq!(hit.text, Some("Hello"));

    let bush = w.facing(3, 2, Dir::East).unwrap().placement;
    assert_eq!(w.step(3, 2, Dir::East), Step::Blocked);
    assert!(w.remove(bush));
    assert!(!w.remove(bush));
    assert_eq!(w.step(3, 2, Dir::East), Step::Moved { x: 4, y: 2 });
}

#[test]
fn boulders_push_until_blocked() {
    let mut w = World::build(
        tiny(vec![at("boulder", 2, 2), at("fence_segment", 4, 2)]),
        manifest(),
    )
    .unwrap();
    assert_eq!(w.push(0, Dir::East), Some((3, 2)));
    assert_eq!(w.push(0, Dir::East), None, "fence in the way");
    assert!(w.walkable(2, 2));
    assert!(!w.walkable(3, 2));
    assert_eq!(w.push(1, Dir::East), None, "fences are not pushable");
}

#[test]
fn layout_errors_are_all_reported() {
    let mut bad_variant = at("tree_round", 5, 5);
    bad_variant.variant = Some("winter".into());
    let layout = tiny(vec![
        at("house_small", 0, 0),
        at("tree_round", 1, 1), // overlaps the house
        at("nope", 3, 5),       // unknown object
        at("tree_large", 5, 5), // 2x2 leaves the map
        bad_variant,
        at("flower_patch", 2, 2), // non-blocking: may overlap
    ]);
    let problems = World::build(layout, manifest()).unwrap_err();
    assert_eq!(problems.len(), 4, "{problems:#?}");

    let mut layout = tiny(vec![at("house_small", 0, 0)]);
    layout.spawn = [1, 1];
    assert_eq!(
        World::build(layout, manifest()).unwrap_err(),
        ["spawn (1, 1) is not walkable"]
    );
}
