#[path = "../src/faction_brain/mod.rs"]
mod faction_brain;

use faction_brain::{plan, Action, Candidate, FactionView, Need, Observation, Planner};
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
                urgency: 5,
                evidence_ids: vec!["own-shortage-7".into()],
            },
            Need {
                id: "transport".into(),
                role: "hauler".into(),
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
