#[path = "../src/faction_brain/mod.rs"]
mod faction_brain;

use faction_brain::{
    plan, Action, Candidate, FactionView, LivePlanner, Need, NeedCategory, Observation, Planner,
};
use serde_json::json;
use std::collections::BTreeMap;

fn view() -> FactionView {
    FactionView {
        schema_version: 1,
        faction_id: "helios".into(),
        revision: 7,
        available_metal: 100,
        spending_limit: 60,
        priorities: BTreeMap::from([("defender".into(), 3), ("hauler".into(), 1)]),
        observations: vec![Observation {
            id: "own-shortage-7".into(),
            allowed_recipients: vec!["helios".into()],
        }],
        needs: vec![
            Need {
                id: "defense".into(),
                role: "defender".into(),
                category: NeedCategory::Unit,
                urgency: 5,
                evidence_ids: vec!["own-shortage-7".into()],
            },
            Need {
                id: "transport".into(),
                role: "hauler".into(),
                category: NeedCategory::Unit,
                urgency: 5,
                evidence_ids: vec!["own-shortage-7".into()],
            },
        ],
        candidates: vec![
            Candidate {
                id: "build-defense".into(),
                need_id: "defense".into(),
                action: Action::BuildUnit {
                    design_id: "guard-v1".into(),
                    approval_sha256: "a".repeat(64),
                    metal_cost: 40,
                },
                blockers: vec![],
            },
            Candidate {
                id: "request-hauler".into(),
                need_id: "transport".into(),
                action: Action::RequestDesign,
                blockers: vec![],
            },
        ],
    }
}

#[test]
fn saved_priorities_swap_equal_need_choices_without_changing_facts() {
    let mut input = view();
    let before = serde_json::to_value(&input).unwrap();
    let first = plan("helios", &input, "report-1", 100).unwrap();
    assert_eq!(first.action.as_ref().unwrap().candidate_id, "build-defense");
    assert_eq!(first.action.as_ref().unwrap().score, 15);
    assert_eq!(first.action.as_ref().unwrap().faction_id, "helios");
    assert_eq!(first.action.as_ref().unwrap().observed_revision, 7);
    assert_eq!(
        serde_json::to_value(&input).unwrap(),
        before,
        "planning cannot spend or create a unit"
    );
    input.priorities.insert("defender".into(), 1);
    input.priorities.insert("hauler".into(), 3);
    let second = plan("helios", &input, "report-2", 101).unwrap();
    assert_eq!(second.action.unwrap().candidate_id, "request-hauler");
}

#[test]
fn rejects_another_factions_context_and_private_observations() {
    let input = view();
    assert!(plan("tide", &input, "report-1", 100).is_err());
    let mut private = input;
    private.observations[0].allowed_recipients = vec!["tide".into()];
    assert!(plan("helios", &private, "report-1", 100).is_err());
}

#[test]
fn refuses_invented_or_duplicated_evidence() {
    let mut input = view();
    input.needs[0].evidence_ids = vec!["unseen-army".into()];
    assert!(plan("helios", &input, "report-1", 100).is_err());
    input.needs[0].evidence_ids = vec!["own-shortage-7".into(); 2];
    assert!(plan("helios", &input, "report-1", 100).is_err());
}

#[test]
fn blocked_high_priority_action_does_not_starve_a_feasible_action() {
    let mut input = view();
    input.candidates[0].blockers.push("no_factory".into());
    assert_eq!(
        plan("helios", &input, "report-1", 100)
            .unwrap()
            .action
            .unwrap()
            .candidate_id,
        "request-hauler"
    );
}

#[test]
fn actual_metal_and_delegated_spending_ceiling_both_limit_build_proposals() {
    for (metal, ceiling) in [(39, 60), (100, 39)] {
        let mut input = view();
        input.available_metal = metal;
        input.spending_limit = ceiling;
        assert_eq!(
            plan("helios", &input, "report-1", 100)
                .unwrap()
                .action
                .unwrap()
                .candidate_id,
            "request-hauler"
        );
    }
    let mut exact = view();
    exact.available_metal = 40;
    exact.spending_limit = 40;
    assert_eq!(
        plan("helios", &exact, "report-1", 100)
            .unwrap()
            .action
            .unwrap()
            .candidate_id,
        "build-defense"
    );
}

#[test]
fn report_is_private_revision_bound_and_explicitly_a_proposal() {
    let selected = plan("helios", &view(), "report-1", 100).unwrap();
    let event = serde_json::to_value(selected.report.unwrap()).unwrap();
    assert_eq!(event["id"], "report-1");
    assert_eq!(event["source"], "faction-brain");
    assert_eq!(event["kind"], "unit_need");
    assert_eq!(event["at"], 100);
    assert_eq!(event["allowed_recipients"], json!(["helios"]));
    assert_eq!(event["payload"]["observed_revision"], 7);
    assert_eq!(event["payload"]["evidence_ids"], json!(["own-shortage-7"]));
    assert_eq!(event["payload"]["status"], "proposed");
    assert_eq!(event["payload"]["proposal"]["action"]["kind"], "build_unit");
    assert_eq!(event["payload"]["proposal"]["action"]["metal_cost"], 40);
    assert!(
        !event.to_string().contains("transport"),
        "unselected need must stay out of this report"
    );
}

#[test]
fn missing_capability_reports_a_block_instead_of_fabricating_an_order() {
    let mut input = view();
    input.candidates.clear();
    let result = plan("helios", &input, "report-1", 100).unwrap();
    assert!(result.action.is_none());
    let event = result.report.unwrap();
    assert_eq!(event.kind, "blocked_need");
    assert_eq!(event.payload["need_id"], "defense");
    assert_eq!(event.payload["blockers"], json!(["no_supported_action"]));
    assert_eq!(event.payload["status"], "blocked");
}

#[test]
fn blocked_report_preserves_actual_cost_and_authoritative_block_reasons() {
    let mut input = view();
    input.candidates.truncate(1);
    input.available_metal = 0;
    input.spending_limit = 0;
    input.candidates[0].blockers = vec!["no_factory".into()];
    let event = plan("helios", &input, "report-1", 100)
        .unwrap()
        .report
        .unwrap();
    assert_eq!(
        event.payload["blockers"],
        json!(["insufficient_metal", "no_factory", "spending_limit"])
    );
}

#[test]
fn zero_priority_or_resolved_needs_are_idle_without_a_content_job() {
    let mut input = view();
    input.needs.iter_mut().for_each(|n| n.urgency = 0);
    let idle = plan("helios", &input, "report-1", 100).unwrap();
    assert!(idle.action.is_none() && idle.report.is_none());
    input.needs.iter_mut().for_each(|n| n.urgency = 5);
    input.priorities.values_mut().for_each(|w| *w = 0);
    let idle = plan("helios", &input, "report-1", 100).unwrap();
    assert!(idle.action.is_none() && idle.report.is_none());
}

#[test]
fn score_and_ties_are_stable_at_large_weights_and_reordered_inputs() {
    let mut input = view();
    input.priorities.values_mut().for_each(|w| *w = u32::MAX);
    input.needs.iter_mut().for_each(|n| n.urgency = u32::MAX);
    let first = plan("helios", &input, "report-1", 100).unwrap();
    assert_eq!(first.action.as_ref().unwrap().candidate_id, "build-defense");
    assert_eq!(
        first.action.as_ref().unwrap().score,
        18_446_744_065_119_617_025
    );
    input.needs.reverse();
    input.candidates.reverse();
    assert_eq!(plan("helios", &input, "report-1", 100).unwrap(), first);
}

#[test]
fn rejects_bad_identity_schema_duplicate_candidates_and_unusable_approval_references() {
    let input = view();
    assert!(plan("helios", &input, "../report", 100).is_err());
    let mut invalid = input.clone();
    invalid.schema_version = 2;
    assert!(plan("helios", &invalid, "report-1", 100).is_err());
    invalid = input.clone();
    invalid.candidates.push(invalid.candidates[0].clone());
    assert!(plan("helios", &invalid, "report-1", 100).is_err());
    invalid = input.clone();
    invalid.candidates[0].need_id = "nonexistent-need".into();
    assert!(plan("helios", &invalid, "report-1", 100).is_err());
    for (hash, cost) in [("not-a-reviewed-hash".into(), 40), ("a".repeat(64), 0)] {
        invalid = input.clone();
        invalid.candidates[0].action = Action::BuildUnit {
            design_id: "guard-v1".into(),
            approval_sha256: hash,
            metal_cost: cost,
        };
        assert!(plan("helios", &invalid, "report-1", 100).is_err());
    }
}

#[test]
fn refuses_unknown_input_fields_and_bounds_work_per_turn() {
    let mut serialized = serde_json::to_value(view()).unwrap();
    serialized["hidden_rivals"] = json!(["tide"]);
    assert!(serde_json::from_value::<FactionView>(serialized).is_err());
    let mut input = view();
    input.needs = (0..129)
        .map(|i| Need {
            id: format!("need-{i}"),
            ..input.needs[0].clone()
        })
        .collect();
    assert!(plan("helios", &input, "report-1", 100).is_err());
}

#[test]
fn lua_policy_hot_swap_occurs_at_decision_boundary_without_world_effects() {
    let mut planner = Planner::new("helios").unwrap();
    let input = view();
    let old = planner.plan("helios", &input, "report-1", 100).unwrap();
    planner
        .stage_policy(
            r#"return {schema_version=1, choose=function(options)
        for _, option in ipairs(options) do
            if option.role == 'hauler' then return option.id end
        end
        return options[1].id
    end}"#,
        )
        .unwrap();
    let changed = planner.plan("helios", &input, "report-2", 101).unwrap();
    assert_eq!(changed.action.unwrap().candidate_id, "request-hauler");
    assert_ne!(changed.policy_version, old.policy_version);
    assert!(!changed.policy_rejected);
    assert_eq!(input.available_metal, 100);
}

#[test]
fn broken_or_invented_lua_choice_keeps_last_good_policy_and_funds() {
    let mut planner = Planner::new("helios").unwrap();
    let input = view();
    let old = planner.plan("helios", &input, "report-1", 100).unwrap();
    assert!(planner.stage_policy("not valid Lua!").is_err());
    assert_eq!(
        planner
            .plan("helios", &input, "report-2", 101)
            .unwrap()
            .policy_version,
        old.policy_version
    );
    for source in [
        "return {schema_version=1, choose=function(_) return 'free-army' end}",
        "return {schema_version=1, choose=function(_) while true do end end}",
        "return {schema_version=1, choose=function(_) return io.open('world.json') end}",
    ] {
        planner.stage_policy(source).unwrap();
        let kept = planner.plan("helios", &input, "report-3", 102).unwrap();
        assert!(kept.policy_rejected);
        assert_eq!(kept.policy_version, old.policy_version);
        assert_eq!(kept.action.unwrap().candidate_id, "build-defense");
    }
    assert_eq!(input.available_metal, 100);
}

#[test]
fn lua_cannot_choose_a_blocked_or_unaffordable_candidate() {
    let mut planner = Planner::new("helios").unwrap();
    planner
        .stage_policy("return {schema_version=1, choose=function(_) return 'build-defense' end}")
        .unwrap();
    let mut input = view();
    input.available_metal = 0;
    let result = planner.plan("helios", &input, "report-1", 100).unwrap();
    assert!(result.policy_rejected);
    assert_eq!(result.action.unwrap().candidate_id, "request-hauler");
}

#[test]
fn policy_promotion_state_cannot_cross_faction_boundaries() {
    let mut planner = Planner::new("helios").unwrap();
    let input = view();
    planner
        .plan("helios", &input, "helios-report", 100)
        .unwrap();
    planner
        .stage_policy(
            "return {schema_version=1, choose=function(options) return options[2].id end}",
        )
        .unwrap();
    let mut tide = input.clone();
    tide.faction_id = "tide".into();
    tide.observations[0].allowed_recipients = vec!["tide".into()];
    assert!(
        planner.plan("tide", &tide, "tide-report", 101).is_err(),
        "another faction cannot promote, reject or observe the staged policy"
    );
    let own = planner
        .plan("helios", &input, "helios-report-2", 102)
        .unwrap();
    assert_eq!(own.action.unwrap().candidate_id, "request-hauler");
}

fn city_view() -> FactionView {
    let mut input = view();
    input.priorities.insert("generator".into(), 3);
    input.needs.truncate(1);
    input.needs[0].category = NeedCategory::Building;
    input.needs[0].role = "generator".into();
    input.candidates.truncate(1);
    input.candidates[0].action = Action::BuildStructure {
        design_id: "generator-v1".into(),
        approval_sha256: "b".repeat(64),
        metal_cost: 40,
        site_id: "authority-city-slot-4".into(),
    };
    input
}

#[test]
fn building_choice_preserves_authority_site_and_private_building_report() {
    let input = city_view();
    let decision = plan("helios", &input, "city-need-1", 100).unwrap();
    let event = decision.report.unwrap();
    assert_eq!(event.kind, "building_need");
    assert_eq!(event.allowed_recipients, vec!["helios"]);
    assert_eq!(event.payload["category"], "building");
    assert_eq!(event.payload["observed_revision"], 7);
    assert_eq!(
        event.payload["proposal"]["action"]["site_id"],
        "authority-city-slot-4"
    );
    assert_eq!(
        event.payload["proposal"]["action"]["approval_sha256"],
        "b".repeat(64)
    );
    assert_eq!(
        input.available_metal, 100,
        "a city proposal is not a paid build"
    );
}

#[test]
fn city_proposals_respect_both_budget_and_authoritative_route_blocks() {
    for (metal, ceiling, reason) in [(39, 60, "insufficient_metal"), (100, 39, "spending_limit")] {
        let mut input = city_view();
        input.available_metal = metal;
        input.spending_limit = ceiling;
        let result = plan("helios", &input, "city-need-1", 100).unwrap();
        assert!(result.action.is_none());
        let event = result.report.unwrap();
        assert_eq!(event.kind, "blocked_building_need");
        assert_eq!(event.payload["blockers"], json!([reason]));
    }
    let mut input = city_view();
    input.candidates[0].blockers = vec!["no_connected_route".into()];
    let result = plan("helios", &input, "city-need-1", 100).unwrap();
    assert!(result.action.is_none());
    assert_eq!(
        result.report.unwrap().payload["blockers"],
        json!(["no_connected_route"])
    );
}

#[test]
fn unknown_building_design_reports_need_without_creating_a_free_structure() {
    let mut input = city_view();
    input.candidates[0].action = Action::RequestDesign;
    let requested = plan("helios", &input, "city-need-1", 100).unwrap();
    assert_eq!(requested.report.unwrap().kind, "building_need");
    assert_eq!(requested.action.unwrap().action, Action::RequestDesign);
    input.candidates.clear();
    let blocked = plan("helios", &input, "city-need-2", 101).unwrap();
    assert!(blocked.action.is_none());
    assert_eq!(blocked.report.unwrap().kind, "blocked_building_need");
}

#[test]
fn rejects_structure_without_site_or_wrong_need_category() {
    let mut input = city_view();
    input.needs[0].category = NeedCategory::Unit;
    assert!(plan("helios", &input, "city-need-1", 100).is_err());
    input = city_view();
    input.candidates[0].action = Action::BuildUnit {
        design_id: "guard-v1".into(),
        approval_sha256: "a".repeat(64),
        metal_cost: 40,
    };
    assert!(plan("helios", &input, "city-need-1", 100).is_err());
    input = city_view();
    if let Action::BuildStructure { site_id, .. } = &mut input.candidates[0].action {
        site_id.clear();
    }
    assert!(plan("helios", &input, "city-need-1", 100).is_err());
    for (hash, cost) in [("unreviewed".into(), 40), ("b".repeat(64), 0)] {
        input = city_view();
        input.candidates[0].action = Action::BuildStructure {
            design_id: "generator-v1".into(),
            approval_sha256: hash,
            metal_cost: cost,
            site_id: "authority-city-slot-4".into(),
        };
        assert!(plan("helios", &input, "city-need-1", 100).is_err());
    }
}

#[test]
fn legacy_unit_view_without_category_keeps_v1_behavior() {
    let mut encoded = serde_json::to_value(view()).unwrap();
    for need in encoded["needs"].as_array_mut().unwrap() {
        need.as_object_mut().unwrap().remove("category");
    }
    let old: FactionView = serde_json::from_value(encoded).unwrap();
    let decision = plan("helios", &old, "report-1", 100).unwrap();
    assert_eq!(decision.action.unwrap().candidate_id, "build-defense");
    assert_eq!(decision.report.unwrap().kind, "unit_need");
}

#[test]
fn default_lua_policy_prioritizes_feasible_city_need_before_other_empire_work() {
    let mut input = view();
    input.priorities.insert("generator".into(), 1);
    input.needs.push(Need {
        id: "city-power".into(),
        role: "generator".into(),
        category: NeedCategory::Building,
        urgency: 1,
        evidence_ids: vec!["own-shortage-7".into()],
    });
    input.candidates.push(Candidate {
        id: "request-city-generator".into(),
        need_id: "city-power".into(),
        action: Action::RequestDesign,
        blockers: vec![],
    });
    let result = plan("helios", &input, "city-need-1", 100).unwrap();
    assert_eq!(
        result.action.unwrap().candidate_id,
        "request-city-generator"
    );
    assert_eq!(result.report.unwrap().kind, "building_need");
}

#[test]
fn building_report_enters_real_gm_once_and_cannot_route_to_another_faction() {
    use argon_dev::game_master::{Event, GameMaster, Proposal};
    let report = plan("helios", &city_view(), "city-need-1", 100)
        .unwrap()
        .report
        .unwrap();
    let event: Event = serde_json::from_value(serde_json::to_value(report).unwrap()).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("gm.json");
    let mut gm = GameMaster::load(&path).unwrap();
    gm.ingest(event.clone()).unwrap();
    gm.ingest(event.clone()).unwrap();
    assert_eq!(
        gm.revision(),
        1,
        "repeated planner delivery must not add facts twice"
    );
    let route = |target: &str| Proposal {
        action: "route".into(),
        event_ids: vec!["city-need-1".into()],
        target: target.into(),
        summary: "Forward the recorded building need".into(),
    };
    let denied = gm
        .decide("wrong-recipient", gm.revision(), route("tide"))
        .unwrap();
    assert!(!denied.accepted);
    assert!(gm.deliveries("tide").is_empty());
    let accepted = gm
        .decide("own-recipient", gm.revision(), route("helios"))
        .unwrap();
    assert!(accepted.accepted);
    drop(gm);
    let restored = GameMaster::load(&path).unwrap();
    assert_eq!(restored.deliveries("helios"), vec![event]);
    assert!(restored.deliveries("tide").is_empty());
}

#[test]
fn lua_finalizer_cannot_hold_policy_reload_or_shutdown() {
    const MARKER: &str = "ARGON_FACTION_FINALIZER_MARKER";
    if let Some(marker) = std::env::var_os(MARKER) {
        std::fs::write(marker, "started").unwrap();
        let mut planner = Planner::new("helios").unwrap();
        planner
            .stage_policy(
                r#"return {schema_version=1, choose=function(options)
            for _, option in ipairs(options) do
                if option.role == 'hauler' then return option.id end
            end
            return options[1].id
        end}"#,
            )
            .unwrap();
        let active = planner
            .plan("helios", &view(), "before-hostile", 99)
            .unwrap();
        assert_eq!(active.action.unwrap().candidate_id, "request-hauler");
        let hostile = r#"
            local object = setmetatable({}, {__gc=function() while true do end end})
            return {schema_version=1, choose=function(options) return options[1].id end}
        "#;
        assert!(planner.stage_policy(hostile).is_err());
        let retained = planner.plan("helios", &view(), "report-1", 100).unwrap();
        assert_eq!(retained.policy_version, active.policy_version);
        assert_eq!(retained.action.unwrap().candidate_id, "request-hauler");
        return;
    }
    use std::{
        process::{Command, Stdio},
        time::{Duration, Instant},
    };
    let directory = tempfile::tempdir().unwrap();
    let marker = directory.path().join("child-started");
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "lua_finalizer_cannot_hold_policy_reload_or_shutdown",
            "--nocapture",
        ])
        .env(MARKER, &marker)
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();
    let startup = Instant::now();
    let mut running_since = None;
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            assert!(status.success(), "sandbox child failed");
            break;
        }
        if marker.exists() && running_since.is_none() {
            running_since = Some(Instant::now());
        }
        let hung = running_since.is_some_and(|started| started.elapsed() > Duration::from_secs(3));
        if hung || startup.elapsed() > Duration::from_secs(15) {
            child.kill().unwrap();
            child.wait().unwrap();
            assert!(
                !hung,
                "Lua finalizer held policy reload/shutdown beyond its execution bound"
            );
            panic!("sandbox test child did not start within startup allowance");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn write_live_view(path: &std::path::Path, input: FactionView) {
    std::fs::write(
        path,
        serde_json::to_vec(&json!({
            "schema_version":1,"observed_at":100,"views":[input]
        }))
        .unwrap(),
    )
    .unwrap();
}

#[test]
fn live_reports_retain_original_facts_across_repoll_restart_and_gm_ingest() {
    use argon_dev::game_master::{Event, GameMaster};
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("views.json");
    let mut live = LivePlanner::new(None);
    assert!(live.poll(&path, 101).unwrap().is_empty());
    write_live_view(&path, city_view());
    let first = live.poll(&path, 101).unwrap();
    assert_eq!(first.len(), 1);
    assert_eq!(first[0].at, 100);
    assert_eq!(first[0].allowed_recipients, ["helios"]);
    assert_eq!(live.poll(&path, 200).unwrap(), first);
    assert_eq!(LivePlanner::new(None).poll(&path, 300).unwrap(), first);
    let mut gm = GameMaster::load(dir.path().join("gm.json")).unwrap();
    let event: Event = serde_json::from_value(serde_json::to_value(&first[0]).unwrap()).unwrap();
    gm.ingest(event.clone()).unwrap();
    gm.ingest(event).unwrap();
    assert_eq!(gm.revision(), 1);
    let mut next = city_view();
    next.revision += 1;
    write_live_view(&path, next);
    assert_ne!(live.poll(&path, 301).unwrap()[0].id, first[0].id);
}

#[test]
fn live_input_rejects_private_corrupt_future_or_oversized_contexts() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("views.json");
    let mut live = LivePlanner::new(None);
    let mut private = city_view();
    private.observations[0].allowed_recipients = vec!["tide".into()];
    write_live_view(&path, private);
    assert!(live.poll(&path, 101).is_err());
    write_live_view(&path, city_view());
    assert!(live.poll(&path, 99).is_err());
    std::fs::write(&path, b"{").unwrap();
    assert!(live.poll(&path, 101).is_err());
    std::fs::write(&path, vec![b' '; 2 * 1024 * 1024 + 1]).unwrap();
    assert!(live.poll(&path, 101).is_err());
    assert!(live.poll(dir.path(), 101).is_err());
    #[cfg(unix)]
    {
        let link = dir.path().join("link.json");
        std::os::unix::fs::symlink(&path, &link).unwrap();
        assert!(live.poll(&link, 101).is_err());
    }
}

#[test]
fn live_batch_is_private_per_faction_and_rejects_duplicates_before_policy_changes() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("views.json");
    let helios = view();
    let mut tide = view();
    tide.faction_id = "tide".into();
    tide.observations[0].allowed_recipients = vec!["tide".into()];
    tide.priorities.insert("hauler".into(), 10);
    std::fs::write(
        &path,
        serde_json::to_vec(&json!({
            "schema_version":1,"observed_at":100,"views":[helios,tide]
        }))
        .unwrap(),
    )
    .unwrap();
    let mut live = LivePlanner::new(None);
    let reports = live.poll(&path, 101).unwrap();
    assert_eq!(reports.len(), 2);
    for report in reports {
        let actor = report.payload["faction_id"].as_str().unwrap();
        assert_eq!(report.allowed_recipients, [actor]);
        assert_eq!(report.payload["proposal"]["faction_id"], actor);
        let expected = if actor == "tide" {
            "request-hauler"
        } else {
            "build-defense"
        };
        assert_eq!(report.payload["proposal"]["candidate_id"], expected);
    }
    std::fs::write(
        &path,
        serde_json::to_vec(&json!({
            "schema_version":1,"observed_at":100,"views":[view(),view()]
        }))
        .unwrap(),
    )
    .unwrap();
    assert!(live.poll(&path, 101).is_err());
}

#[test]
fn live_policy_reload_uses_last_good_and_reports_actual_policy_version() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("views.json");
    let policy = dir.path().join("policy.lua");
    write_live_view(&path, view());
    let mut live = LivePlanner::new(Some(policy.clone()));
    let original = live.poll(&path, 101).unwrap();
    std::fs::write(
        &policy,
        r#"return {schema_version=1, choose=function(options)
        for _, option in ipairs(options) do
            if option.role == 'hauler' then return option.id end
        end
        return options[1].id
    end}"#,
    )
    .unwrap();
    let changed = live.poll(&path, 102).unwrap();
    assert_eq!(
        changed[0].payload["proposal"]["candidate_id"],
        "request-hauler"
    );
    assert_ne!(changed[0].id, original[0].id);
    std::fs::write(&policy, "malformed Lua!").unwrap();
    assert_eq!(live.poll(&path, 103).unwrap(), changed);
    assert!(live.policy_error().is_some());
}
