#[path = "../src/events.rs"]
mod events;

use events::*;

#[test]
fn meteor_warns_impacts_blocks_and_recovers_without_regranting() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("events.json");
    let mut world = EventWorld::load(&path, 77).unwrap();
    world.advance(20).unwrap();
    let warning = world.snapshot();
    assert_eq!(warning.events.len(), 1);
    let event = &warning.events[0];
    assert_eq!(event.phase, Phase::Warning);
    let from = [event.x - event.radius - 2.0, event.y];
    let to = [event.x + event.radius + 2.0, event.y];
    assert!(!world.segment_blocked(from, to));
    world.advance(20).unwrap();
    assert!(world.segment_blocked(from, to));
    assert!(world
        .snapshot()
        .responses
        .iter()
        .all(|r| r.event_id == event.id));
    let before = world.snapshot();
    drop(world);
    let mut world = EventWorld::load(&path, 999).unwrap();
    assert_eq!(world.snapshot(), before);
    world.advance(0).unwrap();
    assert_eq!(world.snapshot(), before);
    world.advance(120).unwrap();
    assert!(!world.segment_blocked(from, to));
    assert_eq!(world.snapshot().events[0].phase, Phase::Cooled);
    assert!(world.snapshot().responses.iter().all(|r| r.completed));
}

#[test]
fn hidden_hazards_require_observation_before_faction_response() {
    let mut world = EventWorld::new(4);
    let id = world
        .schedule_meteor(100.0, 100.0, 12.0, 10, false)
        .unwrap();
    world.advance(10).unwrap();
    assert!(world.observations("helios").is_empty());
    assert!(world.snapshot().events.is_empty());
    assert!(world.snapshot().responses.is_empty());
    world.observe("helios", id).unwrap();
    assert_eq!(world.observations("helios").len(), 1);
    world.advance(1).unwrap();
    assert_eq!(world.responses("helios").len(), 1);
    assert!(world.snapshot().responses.is_empty());
    assert!(
        !world.snapshot().units.iter().any(|u| u.faction == "helios"),
        "private response movement must not leak the impact location"
    );
    assert!(world.observations("tide").is_empty());
    assert!(world.observe("unknown", id).is_err());
}

#[test]
fn terrain_event_center_is_persisted_and_cannot_move_established_events() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("events.json");
    let mut world = EventWorld::load(&path, 5).unwrap();
    world.set_center([1536.0, 1500.0]).unwrap();
    world.advance(20).unwrap();
    let event = world.snapshot().events[0].clone();
    assert!((event.x - 1536.0).abs() <= 80.0);
    assert!((event.y - 1500.0).abs() <= 80.0);
    assert!(world.set_center([600.0, 500.0]).is_err());
    drop(world);
    let world = EventWorld::load(path, 99).unwrap();
    assert_eq!(world.snapshot().events[0], event);
}

#[test]
fn event_replay_and_survey_motion_are_deterministic() {
    let mut one = EventWorld::new(81);
    let mut two = EventWorld::new(81);
    one.advance(80).unwrap();
    for _ in 0..80 {
        two.advance(1).unwrap();
    }
    assert_eq!(one.snapshot(), two.snapshot());
    let snapshot = one.snapshot();
    assert_eq!(snapshot.responses.len(), 3);
    for response in &snapshot.responses {
        let unit = snapshot
            .units
            .iter()
            .find(|u| u.faction == response.faction)
            .unwrap();
        assert_ne!([unit.x, unit.y], unit.home);
        assert!(response.completed);
    }
}

#[test]
fn corrupt_save_and_second_writer_never_reset_history() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("events.json");
    let world = EventWorld::load(&path, 5).unwrap();
    assert!(EventWorld::load(&path, 5).is_err());
    drop(world);
    std::fs::write(&path, "broken").unwrap();
    assert!(EventWorld::load(&path, 5).is_err());
    assert_eq!(std::fs::read_to_string(path).unwrap(), "broken");
}

#[test]
fn segment_collision_cannot_tunnel_and_allows_escape_from_impact() {
    let mut world = EventWorld::new(11);
    world.schedule_meteor(100.0, 100.0, 10.0, 1, true).unwrap();
    world.advance(1).unwrap();
    assert!(world.segment_blocked([0.0, 100.0], [200.0, 100.0]));
    assert!(!world.segment_blocked([100.0, 100.0], [112.0, 100.0]));
    assert!(world.segment_blocked([105.0, 100.0], [100.0, 100.0]));
    assert!(
        world.segment_blocked([105.0, 100.0], [80.0, 100.0]),
        "escaping cannot cut through the impact center"
    );
    assert!(world.schedule_meteor(f64::NAN, 0.0, 4.0, 1, true).is_err());
}

#[test]
fn failed_event_save_never_advances_the_authoritative_clock() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("events.json");
    let mut world = EventWorld::load(&path, 12).unwrap();
    let before = world.snapshot();
    std::fs::remove_file(&path).unwrap();
    std::fs::create_dir(&path).unwrap();
    assert!(world.advance(40).is_err());
    assert_eq!(world.snapshot(), before);
    std::fs::remove_dir(&path).unwrap();
    world.advance(40).unwrap();
    assert_eq!(world.snapshot().events.len(), 1);
    assert_eq!(world.snapshot().events[0].phase, Phase::Impact);
    drop(world);
    assert_eq!(EventWorld::load(path, 99).unwrap().snapshot().now, 40);
}

#[test]
fn public_phase_and_gm_broadcast_receipt_commit_together_and_recover_once() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("events.json");
    let mut world = EventWorld::load(&path, 77).unwrap();
    world.advance_recorded(20, 1_800_000_000).unwrap();
    let warning = world.broadcasts();
    assert_eq!(warning.len(), 1);
    assert_eq!(warning[0].at, 1_800_000_000);
    assert_eq!(warning[0].allowed_recipients, FACTIONS);
    assert_eq!(warning[0].payload["simulation_time"], 20);
    assert_eq!(warning[0].payload["event"]["phase"], "Warning");
    let id = warning[0].id.clone();
    drop(world);
    let mut world = EventWorld::load(&path, 123).unwrap();
    world.advance_recorded(0, 1_800_000_001).unwrap();
    assert_eq!(world.broadcasts()[0].id, id);
    std::fs::remove_file(&path).unwrap();
    std::fs::create_dir(&path).unwrap();
    assert!(world.advance_recorded(20, 1_800_000_020).is_err());
    assert_eq!(world.snapshot().events[0].phase, Phase::Warning);
    assert_eq!(world.broadcasts().len(), 1);
    std::fs::remove_dir(&path).unwrap();
    world.advance_recorded(20, 1_800_000_020).unwrap();
    assert_eq!(world.broadcasts().len(), 2);
    assert_eq!(world.broadcasts()[1].payload["event"]["phase"], "Impact");
}

#[test]
fn private_events_and_uncaptured_old_phases_do_not_get_gm_broadcasts() {
    let mut world = EventWorld::new(77);
    world.schedule_meteor(100.0, 100.0, 12.0, 1, false).unwrap();
    world.advance(20).unwrap(); // Fixture clock: no real receipt was captured.
    assert!(world.broadcasts().is_empty());
    world.advance_recorded(1, 1_800_000_000).unwrap();
    assert!(
        world.broadcasts().is_empty(),
        "do not backfill old warning/impact times"
    );
    world.advance_recorded(19, 1_800_000_019).unwrap();
    let messages = world.broadcasts();
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].payload["event"]["id"], 2);
    assert_eq!(messages[0].payload["event"]["phase"], "Impact");
    assert!(
        world.advance_recorded(1, 1_700_000_000).is_err(),
        "receipt clock cannot silently rewrite history"
    );
}

#[test]
fn broadcast_envelope_enters_existing_gm_once_and_routes_only_recorded_facts() {
    use argon_dev::game_master::{Event, GameMaster, Proposal};
    let dir = tempfile::tempdir().unwrap();
    let mut world = EventWorld::new(77);
    world.advance_recorded(20, 1_800_000_000).unwrap();
    let broadcast = world.broadcasts().remove(0);
    let event: Event = serde_json::from_value(serde_json::to_value(&broadcast).unwrap()).unwrap();
    let mut gm = GameMaster::load(dir.path().join("gm.json")).unwrap();
    gm.ingest(event.clone()).unwrap();
    let revision = gm.revision();
    gm.ingest(event).unwrap();
    assert_eq!(gm.revision(), revision);
    let result = gm
        .decide(
            "route-meteor",
            revision,
            Proposal {
                action: "route".into(),
                event_ids: vec![broadcast.id],
                target: "helios".into(),
                summary: "Pass on orbital warning".into(),
            },
        )
        .unwrap();
    assert!(result.accepted);
    assert_eq!(gm.deliveries("helios")[0].payload, broadcast.payload);
    assert!(gm.deliveries("tide").is_empty());
}

#[test]
fn old_sidecar_without_outbox_loads_without_fabricating_historical_receipts() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("events.json");
    let mut world = EventWorld::load(&path, 77).unwrap();
    world.advance(20).unwrap();
    drop(world);
    let mut old: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    old.as_object_mut().unwrap().remove("namespace");
    old.as_object_mut().unwrap().remove("broadcasts");
    std::fs::write(&path, serde_json::to_vec(&old).unwrap()).unwrap();
    let mut world = EventWorld::load(&path, 123).unwrap();
    world.advance_recorded(1, 1_800_000_000).unwrap();
    assert!(world.broadcasts().is_empty());
    world.advance_recorded(19, 1_800_000_019).unwrap();
    assert_eq!(world.broadcasts().len(), 1);
    assert_eq!(world.broadcasts()[0].payload["event"]["phase"], "Impact");
}
