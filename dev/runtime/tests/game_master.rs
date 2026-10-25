#[path = "../src/game_master/mod.rs"]
mod game_master;
use game_master::{Event, GameMaster, Proposal};
use serde_json::json;

fn event(id: &str, recipients: &[&str]) -> Event {
    Event {
        id: id.into(),
        source: "authority".into(),
        at: 10,
        kind: "report".into(),
        payload: json!({"fact":"route blocked"}),
        allowed_recipients: recipients.iter().map(|s| s.to_string()).collect(),
    }
}
fn route(ids: &[&str], target: &str) -> Proposal {
    Proposal {
        action: "route".into(),
        event_ids: ids.iter().map(|s| s.to_string()).collect(),
        target: target.into(),
        summary: "Forward recorded facts".into(),
    }
}
#[test]
fn acknowledged_events_and_deliveries_recover_once() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("gm.json");
    let mut gm = GameMaster::load(&path).unwrap();
    gm.ingest(event("e1", &["tide"])).unwrap();
    let revision = gm.revision();
    let result = gm.decide("d1", revision, route(&["e1"], "tide")).unwrap();
    assert!(result.accepted);
    drop(gm);
    let mut gm = GameMaster::load(&path).unwrap();
    gm.ingest(event("e1", &["tide"])).unwrap();
    assert!(
        gm.decide("d1", revision, route(&["e1"], "tide"))
            .unwrap()
            .accepted
    );
    assert_eq!(gm.deliveries("tide").len(), 1);
    assert_eq!(
        gm.deliveries("tide")[0].payload,
        json!({"fact":"route blocked"})
    );
    assert!(gm.deliveries("cinder").is_empty());
    assert_eq!(gm.snapshot()["events"].as_array().unwrap().len(), 1);
}
#[test]
fn private_facts_cannot_be_laundered_through_public_evidence() {
    let dir = tempfile::tempdir().unwrap();
    let mut gm = GameMaster::load(dir.path().join("gm")).unwrap();
    gm.ingest(event("public", &["tide", "cinder"])).unwrap();
    gm.ingest(event("private", &["tide"])).unwrap();
    let r = gm
        .decide("d1", gm.revision(), route(&["public", "private"], "cinder"))
        .unwrap();
    assert!(!r.accepted);
    assert!(gm.deliveries("cinder").is_empty());
    assert!(
        gm.decide("d2", gm.revision(), route(&["private"], "tide"))
            .unwrap()
            .accepted
    );
}
#[test]
fn conflicting_ids_unknown_evidence_and_stale_results_cannot_act() {
    let dir = tempfile::tempdir().unwrap();
    let mut gm = GameMaster::load(dir.path().join("gm")).unwrap();
    gm.ingest(event("e1", &["tide"])).unwrap();
    let old = gm.revision();
    assert!(gm.ingest(event("e1", &["cinder"])).is_err());
    gm.ingest(event("e2", &["tide"])).unwrap();
    assert!(
        !gm.decide("stale", old, route(&["e1"], "tide"))
            .unwrap()
            .accepted
    );
    assert!(
        !gm.decide("unknown", gm.revision(), route(&["missing"], "tide"))
            .unwrap()
            .accepted
    );
    gm.decide("ok", gm.revision(), route(&["e1"], "tide"))
        .unwrap();
    assert!(gm
        .decide("ok", gm.revision(), route(&["e2"], "tide"))
        .is_err());
}
#[test]
fn future_revision_rejection_remains_readable_after_restart() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("gm");
    let mut gm = GameMaster::load(&path).unwrap();
    assert!(
        !gm.decide("future", u64::MAX, route(&["unknown"], "tide"))
            .unwrap()
            .accepted
    );
    drop(gm);
    let gm = GameMaster::load(&path).unwrap();
    assert_eq!(gm.snapshot()["decisions"][0]["accepted"], false);
}
#[test]
fn content_and_event_requests_are_pending_without_delivering_model_text() {
    let dir = tempfile::tempdir().unwrap();
    let mut gm = GameMaster::load(dir.path().join("gm")).unwrap();
    gm.ingest(event("need", &["helios"])).unwrap();
    let mut p = route(&["need"], "studio");
    p.action = "request_content".into();
    p.summary = "Need a scout; private operator explanation".into();
    assert!(gm.decide("content", gm.revision(), p).unwrap().accepted);
    let mut p = route(&["need"], "event_authority");
    p.action = "propose_event".into();
    assert!(gm.decide("hazard", gm.revision(), p).unwrap().accepted);
    assert!(gm.deliveries("studio").is_empty());
    assert!(gm.deliveries("helios").is_empty());
    assert_eq!(gm.context()["events"][0]["id"], "need");
    assert_eq!(
        gm.snapshot()["decisions"][0]["proposal"]["action"],
        "request_content"
    );
}
#[test]
fn locks_corruption_and_failed_writes_preserve_acknowledgment_boundary() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("gm");
    let mut gm = GameMaster::load(&path).unwrap();
    assert!(GameMaster::load(&path).is_err());
    // Destination directory makes atomic replacement fail without changing history.
    std::fs::create_dir(&path).unwrap();
    assert!(gm.ingest(event("e1", &["tide"])).is_err());
    assert_eq!(gm.revision(), 0);
    std::fs::remove_dir(&path).unwrap();
    gm.ingest(event("e1", &["tide"])).unwrap();
    drop(gm);
    std::fs::write(&path, b"corrupt").unwrap();
    assert!(GameMaster::load(&path).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), b"corrupt");
}
#[test]
fn invalid_or_unbounded_input_is_rejected_before_persistence() {
    let dir = tempfile::tempdir().unwrap();
    let mut gm = GameMaster::load(dir.path().join("gm")).unwrap();
    let mut e = event("too-big", &["tide"]);
    e.payload = json!("x".repeat(16385));
    assert!(gm.ingest(e).is_err());
    assert!(gm
        .ingest(event("duplicate-acl", &["tide", "tide"]))
        .is_err());
    for n in 0..1024 {
        gm.ingest(event(&format!("e{n}"), &["tide"])).unwrap();
    }
    assert!(gm.ingest(event("overflow", &["tide"])).is_err());
    // Duplicate delivery acknowledgment remains available at capacity.
    gm.ingest(event("e0", &["tide"])).unwrap();
    assert_eq!(gm.revision(), 1024);
}
#[test]
fn deeply_nested_event_cannot_make_acknowledged_history_unreadable() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("gm");
    let mut gm = GameMaster::load(&path).unwrap();
    gm.ingest(event("safe", &["tide"])).unwrap();
    let mut e = event("deep", &["tide"]);
    for _ in 0..150 {
        e.payload = json!([e.payload]);
    }
    assert!(gm.ingest(e).is_err());
    drop(gm);
    assert_eq!(
        GameMaster::load(&path).unwrap().snapshot()["events"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
}
#[cfg(unix)]
#[test]
fn post_rename_failure_stops_writer_before_it_can_overwrite_uncertain_result() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("gm");
    let mut gm = GameMaster::load(&path).unwrap();
    gm.ingest(event("safe", &["tide"])).unwrap();
    // Write+search permits temp creation/rename; no read prevents directory sync open.
    std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o300)).unwrap();
    let result = gm.ingest(event("uncertain", &["tide"]));
    std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    assert!(result.is_err());
    assert!(gm.ingest(event("later", &["tide"])).is_err());
    drop(gm);
    let gm = GameMaster::load(&path).unwrap();
    assert_eq!(gm.snapshot()["events"][1]["id"], "uncertain");
}
