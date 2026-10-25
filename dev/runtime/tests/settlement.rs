#[allow(dead_code)]
#[path = "../src/construction.rs"]
mod construction;
#[allow(dead_code)]
#[path = "../src/placement.rs"]
mod placement;
#[path = "../src/settlement.rs"]
mod settlement;
use argon_dev::terrain;
use settlement::*;
use terrain::Terrain;

#[test]
fn paid_site_retries_and_restore_preserve_one_building_and_charge() {
    let terrain = Terrain::generate(20261025);
    let mut state = SettlementState::new();
    let center = terrain.spawn();
    let anchor = [center[0], center[1] - 24.0];
    let first = state
        .start(&terrain, "helios", "core-1", Role::Core, center, anchor, 0)
        .unwrap();
    assert_eq!(state.metal("helios").unwrap(), STARTER_METAL - 100);
    let bytes = serde_json::to_vec(&state).unwrap();
    let mut restored: SettlementState = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(
        restored
            .start(&terrain, "helios", "core-1", Role::Core, center, anchor, 0)
            .unwrap(),
        first
    );
    assert_eq!(restored.snapshot().len(), 1);
    assert!(restored
        .start(
            &terrain,
            "helios",
            "core-1",
            Role::Generator,
            center,
            anchor,
            0
        )
        .is_err());
    assert!(restored
        .start(
            &terrain,
            "tide",
            "core-2",
            Role::Core,
            center,
            anchor,
            restored.revision()
        )
        .is_err());
    assert_eq!(serde_json::to_vec(&restored).unwrap(), bytes);
    restored.advance(120, |_| true).unwrap();
    assert!(restored.snapshot()[0].complete);
    assert_eq!(restored.events().len(), 2);
    restored.advance(120, |_| true).unwrap();
    assert_eq!(restored.events().len(), 2);
}

#[test]
fn rejected_site_or_stale_revision_never_debits() {
    let terrain = Terrain::generate(20261025);
    let mut state = SettlementState::new();
    let c = terrain.spawn();
    let a = [c[0], c[1] - 24.0];
    let before = serde_json::to_vec(&state).unwrap();
    assert!(state
        .start(&terrain, "helios", "bad", Role::Generator, c, a, 0)
        .is_err());
    assert!(state
        .start(&terrain, "helios", "bad", Role::Core, c, a, 1)
        .is_err());
    assert!(state
        .start(&terrain, "unknown", "bad", Role::Core, c, a, 0)
        .is_err());
    assert_eq!(serde_json::to_vec(&state).unwrap(), before);
}

#[test]
fn corrupt_saved_geometry_or_progress_is_rejected() {
    let terrain = Terrain::generate(20261025);
    let mut state = SettlementState::new();
    let c = terrain.spawn();
    state
        .start(
            &terrain,
            "helios",
            "core",
            Role::Core,
            c,
            [c[0], c[1] - 24.0],
            0,
        )
        .unwrap();
    state.validate().unwrap();
    let mut raw = serde_json::to_value(&state).unwrap();
    raw["buildings"][0]["view"]["complete"] = serde_json::json!(true);
    let bad: SettlementState = serde_json::from_value(raw).unwrap();
    assert!(bad.validate().is_err());
    assert!(!state.advance(120, |_| false).unwrap());
    assert_eq!(state.snapshot()[0].completed_ticks, 0);
}
