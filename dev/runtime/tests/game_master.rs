use argon_dev::game_master::{Event, GameMaster, Proposal};
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
#[test]
fn model_spend_is_durable_and_interrupted_attempt_cannot_apply_later() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("gm");
    let mut gm = GameMaster::load(&path).unwrap();
    gm.ingest(event("need", &["helios"])).unwrap();
    let input = gm.begin_attempt("gm-1", "gemini-4-argon", 10).unwrap();
    assert_eq!(input["events"][0]["id"], "need");
    assert_eq!(gm.attempt_count(), 1);
    assert!(gm.begin_attempt("gm-1", "gemini-4-argon", 11).is_err());
    drop(gm);
    let mut gm = GameMaster::load(&path).unwrap();
    assert_eq!(gm.attempt_count(), 1);
    assert!(gm
        .complete_attempt("gm-1", Ok(route(&["need"], "helios")))
        .is_err());
    assert!(gm.deliveries("helios").is_empty());
    assert!(gm.snapshot()["attempts"][0]["error"]
        .as_str()
        .unwrap()
        .contains("interrupted"));
}
#[test]
fn provider_outcomes_and_inputs_are_saved_before_any_dispatch_acknowledgment() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("gm");
    let mut gm = GameMaster::load(&path).unwrap();
    gm.ingest(event("need", &["helios"])).unwrap();
    gm.begin_attempt("gm-1", "gemini-4-argon", 10).unwrap();
    gm.complete_attempt("gm-1", Err("provider unavailable"))
        .unwrap();
    gm.begin_attempt("gm-2", "gemini-4-argon", 11).unwrap();
    let mut p = route(&["need"], "studio");
    p.action = "request_content".into();
    gm.complete_attempt("gm-2", Ok(p)).unwrap();
    drop(gm);
    let gm = GameMaster::load(&path).unwrap();
    assert_eq!(gm.attempt_count(), 2);
    assert_eq!(
        gm.snapshot()["attempts"][0]["error"],
        "provider unavailable"
    );
    assert_eq!(gm.snapshot()["decisions"][0]["accepted"], true);
    assert_eq!(gm.snapshot()["attempts"][1]["finished"], true);
    assert!(gm.deliveries("studio").is_empty());
}
#[test]
fn studio_intake_preserves_permissions_and_rejects_conflicting_batch_atomically() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("gm");
    let mut gm = GameMaster::load(&path).unwrap();
    gm.ingest(event("known", &["tide"])).unwrap();
    let inbox = dir.path().join("inbox.json");
    std::fs::write(
        &inbox,
        serde_json::to_vec(&vec![
            event("new", &["helios"]),
            event("known", &["cinder"]),
        ])
        .unwrap(),
    )
    .unwrap();
    assert!(gm.import_file(&inbox).is_err());
    assert_eq!(gm.snapshot()["events"].as_array().unwrap().len(), 1);
    std::fs::write(
        &inbox,
        serde_json::to_vec(&vec![event("new", &["helios"]), event("known", &["tide"])]).unwrap(),
    )
    .unwrap();
    gm.import_file(&inbox).unwrap();
    gm.import_file(&inbox).unwrap();
    assert_eq!(gm.snapshot()["events"].as_array().unwrap().len(), 2);
    assert!(
        !gm.decide("leak", gm.revision(), route(&["new"], "tide"))
            .unwrap()
            .accepted
    );
    std::fs::write(&inbox, b"[{\"model\":\"bad input\"}]").unwrap();
    assert!(gm.import_file(&inbox).is_err());
}
#[test]
fn real_diplomacy_results_reach_gm_without_private_reasoning_or_broadened_acl() {
    let dir = tempfile::tempdir().unwrap();
    let mut diplomacy =
        argon_dev::diplomacy::Diplomacy::load(dir.path().join("diplomacy")).unwrap();
    diplomacy.apply("helios",0,"turn-1",json!({"action":"message","target":"tide","message":"Known route needs survey", "summary":"private strategist note", "offer_id":"", "offer_version":0,"stance":"neutral"}),10).unwrap();
    let mut gm = GameMaster::load(dir.path().join("gm")).unwrap();
    gm.observe_diplomacy(&diplomacy.snapshot(10)).unwrap();
    gm.observe_diplomacy(&diplomacy.snapshot(11)).unwrap();
    assert_eq!(gm.snapshot()["events"].as_array().unwrap().len(), 1);
    assert!(!gm.context().to_string().contains("private strategist note"));
    assert_eq!(
        gm.snapshot()["events"][0]["allowed_recipients"],
        json!(["helios", "tide"])
    );
    assert!(
        !gm.decide(
            "leak",
            gm.revision(),
            route(&["diplomacy-history-1"], "cinder")
        )
        .unwrap()
        .accepted
    );
}
#[test]
fn repeated_gm_turns_cannot_enqueue_duplicate_unresolved_studio_work() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("gm");
    let mut gm = GameMaster::load(&path).unwrap();
    gm.ingest(event("need-a", &["helios"])).unwrap();
    gm.ingest(event("need-b", &["helios"])).unwrap();
    let mut p = route(&["need-a", "need-b"], "studio");
    p.action = "request_content".into();
    gm.begin_attempt("gm-1", "gemini-4-argon", 10).unwrap();
    gm.complete_attempt("gm-1", Ok(p.clone())).unwrap();
    drop(gm);
    let mut gm = GameMaster::load(&path).unwrap();
    p.event_ids.reverse();
    gm.begin_attempt("gm-2", "gemini-4-argon", 11).unwrap();
    gm.complete_attempt("gm-2", Ok(p.clone())).unwrap();
    assert_eq!(gm.snapshot()["decisions"][1]["accepted"], false);
    assert!(
        !gm.decide("manual-duplicate", gm.revision(), p)
            .unwrap()
            .accepted
    );
    assert_eq!(
        gm.snapshot()["decisions"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|r| r["accepted"] == true)
            .count(),
        1
    );
}
