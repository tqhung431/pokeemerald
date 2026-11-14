use core_data::{AssetManifest, Effectiveness, GameData};

const DATA: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data");

fn data() -> GameData {
    GameData::load_dir(DATA).expect("sample data loads")
}

#[test]
fn sample_data_resolves() {
    let d = data();
    assert_eq!(d.type_count(), 8);
    let zapmoth = d.creature(d.creature_id("zapmoth").unwrap());
    assert_eq!(zapmoth.types.len(), 2);
    assert_eq!(zapmoth.moves_at_level(5, 4).len(), 2);
}

#[test]
fn chart_and_dual_type_matchups() {
    let d = data();
    let t = |k| d.type_id(k).unwrap();
    assert_eq!(
        d.effectiveness(t("ember"), t("bloom")),
        Effectiveness::SUPER
    );
    assert_eq!(
        d.effectiveness(t("ember"), t("tide")),
        Effectiveness::RESISTED
    );
    assert_eq!(
        d.effectiveness(t("volt"), t("stone")),
        Effectiveness::IMMUNE
    );
    assert_eq!(d.effectiveness(t("gale"), t("tide")), Effectiveness::NORMAL);

    // stone vs volt/gale: 2 * 2 = 4x
    let m = d.matchup(t("stone"), &[t("volt"), t("gale")]);
    assert_eq!(m.apply(10), 40);
    // gale vs volt/gale: 0.5 * 1
    assert!(d.matchup(t("gale"), &[t("volt"), t("gale")]).is_resisted());
    assert!(d.matchup(t("neutral"), &[t("shade")]).is_immune());
}

#[test]
fn moves_at_level_keeps_newest() {
    let d = data();
    let c = d.creature(d.creature_id("emberkit").unwrap());
    let keys: Vec<_> = c
        .moves_at_level(100, 2)
        .into_iter()
        .map(|m| d.move_def(m).key.as_str())
        .collect();
    assert_eq!(keys, ["cinder", "quick_dash"]);
}

#[test]
fn manifest_covers_sample_data() {
    let d = data();
    let manifest = AssetManifest::load(format!("{DATA}/assets.json")).unwrap();
    assert_eq!(manifest.validate(&d), Vec::<String>::new());
}

#[test]
fn manifest_reports_gaps() {
    let d = data();
    let mut manifest = AssetManifest::load(format!("{DATA}/assets.json")).unwrap();
    manifest.moves.remove("bump");
    manifest
        .creatures
        .get_mut("cobblet")
        .unwrap()
        .animations
        .remove("faint");
    let problems = manifest.validate(&d);
    assert_eq!(problems.len(), 2, "{problems:?}");
}

#[test]
fn rejects_bad_references() {
    let types = r#"{"types":["a"]}"#;
    let moves = r#"[{"id":"m","name":"M","type":"zzz","category":"physical","power":10,"pp":5}]"#;
    let err = GameData::from_json(types, moves, "[]").unwrap_err();
    assert!(err.to_string().contains("unknown type 'zzz'"), "{err}");

    let moves = r#"[{"id":"m","name":"M","type":"a","category":"status","power":10,"pp":5}]"#;
    assert!(GameData::from_json(types, moves, "[]").is_err());
}

#[test]
fn manifest_object_rules() {
    use core_data::{Door, Interaction};
    let d = data();
    let mut manifest = AssetManifest::load(format!("{DATA}/assets.json")).unwrap();
    assert!(manifest.objects.len() >= 10);

    let house = manifest.objects.get_mut("house_small").unwrap();
    house.doors.push(Door { x: 9, y: 0 });
    let bush = manifest.objects.get_mut("bush_cuttable").unwrap();
    bush.animations.clear();
    let boulder = manifest.objects.get_mut("boulder").unwrap();
    boulder.footprint = [2, 2];
    let sign = manifest.objects.get_mut("signpost").unwrap();
    sign.footprint = [0, 1];
    sign.interaction = Some(Interaction::Read);

    let problems = manifest.validate(&d);
    assert_eq!(problems.len(), 4, "{problems:#?}");
}
