use argon_dev::{diplomacy::Diplomacy, game_master::Event};
use serde_json::{json, Value};
use std::fs;

fn report(id: &str, recipients: &[&str]) -> Event {
    Event {
        id: id.into(),
        source: "authority".into(),
        at: 10,
        kind: "building_need".into(),
        payload: json!({"need":"sensor coverage","site":"known-site"}),
        allowed_recipients: recipients.iter().map(|s| s.to_string()).collect(),
    }
}
fn wait() -> Value {
    json!({"action":"wait","target":"","message":"","summary":"Review delivered facts",
        "offer_id":"","offer_version":0,"stance":"neutral"})
}

#[test]
fn eligibility_is_not_delivery_and_restart_preserves_exactly_once_knowledge() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("world.json");
    let mut core = Diplomacy::load(&path).unwrap();
    let event = report("need-1", &["helios", "tide"]);
    core.ingest_report("helios", &event, 20).unwrap();
    assert_eq!(core.revision(), 1);
    let own = core.context("helios", 20).unwrap();
    assert_eq!(own["reports"][0]["payload"], event.payload);
    assert_eq!(own["reports"][0]["received_at"], 20);
    assert!(own["reports"][0].get("allowed_recipients").is_none());
    assert!(own["reports"][0].get("deliveries").is_none());
    assert_eq!(core.context("tide", 20).unwrap()["reports"], json!([]));
    assert_eq!(core.context("cinder", 20).unwrap()["reports"], json!([]));
    core.ingest_report("helios", &event, 21).unwrap();
    assert_eq!(core.revision(), 1);
    drop(core);
    let mut core = Diplomacy::load(&path).unwrap();
    core.ingest_report("helios", &event, 22).unwrap();
    core.ingest_report("tide", &event, 23).unwrap();
    assert_eq!(core.revision(), 2);
    assert_eq!(
        core.context("helios", 23).unwrap()["reports"][0]["received_at"],
        20
    );
    assert_eq!(
        core.context("tide", 23).unwrap()["reports"][0]["received_at"],
        23
    );
    assert_eq!(core.snapshot(23)["report_count"], 1);
    assert_eq!(core.snapshot(23)["delivery_count"], 2);
    assert_eq!(core.snapshot(23)["history"], json!([]));
    assert_eq!(core.snapshot(23)["active_pacts"], json!([]));
}

#[test]
fn source_identity_cannot_be_reused_with_changed_facts_or_permissions() {
    let dir = tempfile::tempdir().unwrap();
    let mut core = Diplomacy::load(dir.path().join("world.json")).unwrap();
    let event = report("need-1", &["helios", "tide"]);
    core.ingest_report("helios", &event, 20).unwrap();
    for variant in 0..4 {
        let mut changed = event.clone();
        match variant {
            0 => changed.payload = json!({"need":"invented supplies"}),
            1 => changed.allowed_recipients = vec!["tide".into()],
            2 => changed.source = "different-authority".into(),
            _ => changed.at += 1,
        }
        assert!(core.ingest_report("tide", &changed, 21).is_err());
    }
    assert_eq!(core.revision(), 1);
    assert_eq!(core.context("tide", 21).unwrap()["reports"], json!([]));
}

#[test]
fn unauthorized_unknown_future_and_operator_only_reports_do_not_change_state() {
    let dir = tempfile::tempdir().unwrap();
    let mut core = Diplomacy::load(dir.path().join("world.json")).unwrap();
    for (actor, event, received) in [
        ("tide", report("private", &["helios"]), 20),
        ("helios", report("unknown", &["helios", "invented"]), 20),
        ("helios", report("operator", &[]), 20),
        ("invented", report("actor", &["invented"]), 20),
        ("helios", report("future", &["helios"]), 9),
        ("helios", report("duplicate", &["helios", "helios"]), 20),
    ] {
        assert!(core.ingest_report(actor, &event, received).is_err());
    }
    assert_eq!(core.revision(), 0);
    assert_eq!(core.snapshot(20)["report_count"], 0);
}

#[test]
fn learning_invalidates_old_proposals_without_granting_world_effects() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("world.json");
    let mut core = Diplomacy::load(&path).unwrap();
    let prior = core.context("helios", 20).unwrap();
    core.record_attempt("helios", "before-report", prior, "test-model", 20)
        .unwrap();
    core.ingest_report("helios", &report("need", &["helios"]), 21)
        .unwrap();
    let current = core.revision();
    assert!(core
        .apply("helios", current, "clock-backwards", wait(), 20)
        .is_err());
    assert!(core
        .apply("helios", 0, "before-report", wait(), 22)
        .is_err());
    assert_eq!(core.revision(), 1);
    core.apply("helios", current, "fresh", wait(), 23).unwrap();
    assert_eq!(core.revision(), 2);
    drop(core);
    assert_eq!(Diplomacy::load(path).unwrap().revision(), 2);
}

#[test]
fn report_prompts_fit_but_full_evidence_survives_restart() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("world.json");
    let mut core = Diplomacy::load(&path).unwrap();
    for n in 0..4 {
        let mut event = report(&format!("report-{n}"), &["helios"]);
        event.payload = json!({"observed":"🌍".repeat(3500)});
        core.ingest_report("helios", &event, 20 + n).unwrap();
    }
    let context = core.context("helios", 24).unwrap();
    assert!(serde_json::to_vec(&context).unwrap().len() <= 32768);
    let recent = context["reports"].as_array().unwrap();
    assert!(!recent.is_empty() && recent.len() < 4);
    assert_eq!(recent[0]["id"], "report-3");
    core.record_attempt("helios", "after-reports", context, "test-model", 24)
        .unwrap();
    drop(core);
    let core = Diplomacy::load(path).unwrap();
    assert_eq!(core.snapshot(24)["report_count"], 4);
    assert_eq!(
        core.snapshot(24)["reports"][0]["event"]["payload"]["observed"],
        "🌍".repeat(3500)
    );
}

#[test]
fn invalid_payload_and_failed_save_never_acknowledge_new_knowledge() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("world.json");
    let mut core = Diplomacy::load(&path).unwrap();
    let mut oversized = report("large", &["helios"]);
    oversized.payload = json!("x".repeat(16385));
    assert!(core.ingest_report("helios", &oversized, 20).is_err());
    let mut deep = report("deep", &["helios"]);
    for _ in 0..140 {
        deep.payload = json!([deep.payload]);
    }
    assert!(core.ingest_report("helios", &deep, 20).is_err());
    let original = fs::read(&path).unwrap();
    fs::remove_file(&path).unwrap();
    fs::create_dir(&path).unwrap();
    assert!(core
        .ingest_report("helios", &report("disk", &["helios"]), 20)
        .is_err());
    assert_eq!(core.revision(), 0);
    assert_eq!(core.context("helios", 20).unwrap()["reports"], json!([]));
    fs::remove_dir(&path).unwrap();
    fs::write(&path, original).unwrap();
    drop(core);
    assert_eq!(Diplomacy::load(path).unwrap().revision(), 0);
}

#[test]
fn old_sidecar_loads_and_tampered_delivery_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("world.json");
    drop(Diplomacy::load(&path).unwrap());
    let mut old: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    old.as_object_mut().unwrap().remove("reports");
    fs::write(&path, serde_json::to_vec(&old).unwrap()).unwrap();
    let mut core = Diplomacy::load(&path).unwrap();
    core.ingest_report("helios", &report("need", &["helios"]), 20)
        .unwrap();
    drop(core);
    let mut saved: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    saved["reports"][0]["deliveries"][0]["actor"] = json!("tide");
    fs::write(&path, serde_json::to_vec(&saved).unwrap()).unwrap();
    assert!(Diplomacy::load(path).is_err());
}

#[test]
fn nesting_is_rejected_before_delivery_can_block_the_next_attempt() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("world.json");
    let mut core = Diplomacy::load(&path).unwrap();
    let mut too_deep = report("deep", &["helios"]);
    too_deep.payload = Value::Null;
    for _ in 0..122 {
        too_deep.payload = json!([too_deep.payload]);
    }
    assert!(core.ingest_report("helios", &too_deep, 20).is_err());
    assert_eq!(core.revision(), 0);

    let mut supported = report("bounded", &["helios"]);
    supported.payload = Value::Null;
    for _ in 0..64 {
        supported.payload = json!([supported.payload]);
    }
    core.ingest_report("helios", &supported, 20).unwrap();
    let context = core.context("helios", 21).unwrap();
    core.record_attempt("helios", "next", context, "test-model", 21)
        .unwrap();
    drop(core);
    let core = Diplomacy::load(path).unwrap();
    assert_eq!(core.attempts(), 1);
    assert_eq!(
        core.context("helios", 21).unwrap()["reports"][0]["payload"],
        supported.payload
    );
}

#[test]
fn gm_route_delivers_exact_source_facts_without_operator_summary() {
    use argon_dev::game_master::{GameMaster, Proposal};
    let dir = tempfile::tempdir().unwrap();
    let gm_path = dir.path().join("gm.json");
    let core_path = dir.path().join("world.json");
    let event = report("known-need", &["helios", "tide"]);
    let mut gm = GameMaster::load(&gm_path).unwrap();
    gm.ingest(event.clone()).unwrap();
    let receipt = gm
        .decide(
            "route-1",
            gm.revision(),
            Proposal {
                action: "route".into(),
                event_ids: vec![event.id.clone()],
                target: "helios".into(),
                summary: "Private operator commentary".into(),
            },
        )
        .unwrap();
    assert!(receipt.accepted);
    let mut core = Diplomacy::load(&core_path).unwrap();
    for event in gm.deliveries("helios") {
        core.ingest_report("helios", &event, 20).unwrap();
    }
    assert!(gm.deliveries("tide").is_empty());
    assert_eq!(core.context("tide", 20).unwrap()["reports"], json!([]));
    assert_eq!(
        core.context("helios", 20).unwrap()["reports"][0]["payload"],
        event.payload
    );
    assert!(!core
        .context("helios", 20)
        .unwrap()
        .to_string()
        .contains("Private operator commentary"));
    drop(gm);
    drop(core);
    let gm = GameMaster::load(gm_path).unwrap();
    let mut core = Diplomacy::load(core_path).unwrap();
    for event in gm.deliveries("helios") {
        core.ingest_report("helios", &event, 21).unwrap();
    }
    assert_eq!(core.revision(), 1);
    assert_eq!(
        core.context("helios", 21).unwrap()["reports"][0]["received_at"],
        20
    );
}

#[test]
fn full_delivery_history_keeps_replays_valid_without_accepting_new_facts() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("world.json");
    drop(Diplomacy::load(&path).unwrap());
    let mut saved: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    saved["revision"] = json!(1024);
    saved["reports"] = json!((0..1024)
        .map(|n| json!({
            "event":report(&format!("need-{n}"), &["helios", "tide"]),
            "deliveries":[{"actor":"helios","received_at":20}]
        }))
        .collect::<Vec<_>>());
    fs::write(&path, serde_json::to_vec(&saved).unwrap()).unwrap();
    let mut core = Diplomacy::load(&path).unwrap();
    core.ingest_report("helios", &report("need-0", &["helios", "tide"]), 21)
        .unwrap();
    assert!(core
        .ingest_report("tide", &report("need-0", &["helios", "tide"]), 21)
        .is_err());
    assert!(core
        .ingest_report("helios", &report("new", &["helios"]), 21)
        .is_err());
    assert_eq!(core.revision(), 1024);
    assert_eq!(core.context("tide", 21).unwrap()["reports"], json!([]));
    assert_eq!(core.snapshot(21)["report_count"], 1024);
}
