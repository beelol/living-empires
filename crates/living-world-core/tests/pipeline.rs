use living_world_core::{
    Architecture, AssetPipeline, Evidence, EvidenceStatus, Flora, Palette, PipelineStatus,
    Provenance, Recipe, RecipeScene, SceneComponent, ReferencePreset, ReviewDecision, ReviewVerdict, ThemeContract,
    pipeline::{ComparisonStatus, ReviewComparison, VisualDefect},
};
use serde_json;

fn theme() -> ThemeContract {
    ThemeContract {
        id: "rusted-orchard".into(), name: "Rusted Orchard".into(), genre: "Machine frontier".into(),
        description: "A custom orchard civilization".into(), biome: "Red grassland".into(),
        shape_language: "Tall arches and broad platforms".into(), material_finish: "Oxidized iron".into(),
        weathering: "Wind-scoured surfaces".into(), technology: "Solar irrigation".into(),
        motifs: vec!["Seed capsules".into()], lighting: "Warm side light".into(), scale: "Human scale".into(),
        gameplay_role: "Safe outpost".into(), exclusions: vec!["No military insignia".into()],
        visual_checks: vec!["Entry remains visible".into()], assumptions: vec![],
        rendering_notes: vec!["Use warm side light".into()], reference_preset: ReferencePreset::Argonaut,
        palette: Palette { shell: "#D9DED0".into(), frame: "#34464A".into(), accent: "#80EDC4".into(), ground: "#53685D".into(), rock: "#81958A".into() },
        sky: "#B6D2CF".into(), flora: Flora::Crystal, architecture: Architecture::Fortified,
    }
}

fn recipe(theme_id: &str) -> Recipe {
    let parts: Vec<_> = (0..4).map(|i| living_world_core::Part {
        id: format!("post-{i}"), shape: "box".into(), role: "support".into(),
        position: [if i % 2 == 0 { -2.0 } else { 2.0 }, 1.5, if i < 2 { -2.0 } else { 2.0 }],
        size: [0.5, 3.0, 0.5], material: "frame".into(),
    }).collect();
    let part_ids = parts.iter().map(|part: &living_world_core::Part| part.id.clone()).collect();
    Recipe { theme_id: theme_id.into(), name: "Test outpost".into(), description: "Sensor relay".into(), sensor_radius: 12.0,
        parts, scene: Some(RecipeScene { half_extent: 12.0, components: vec![SceneComponent {
            id: "main-outpost".into(), name: "Main outpost".into(), role: "coherent sensor structure".into(), part_ids,
        }] }) }
}

fn review() -> ReviewDecision {
    ReviewDecision { theme_status: EvidenceStatus::Pass, render_status: EvidenceStatus::Pass, findings: vec![], summary: Some("Review confirms the theme and render are clear.".into()), comparison: None }
}

fn evidence(candidate_sha256: String) -> Evidence {
    Evidence { candidate_sha256, image_sha256: ["a", "b", "c"].iter().map(|v| v.repeat(64)).collect(), image_urls: vec![] }
}

fn comparison(current: &Evidence, previous: Option<&Evidence>, status: ComparisonStatus) -> ReviewComparison {
    ReviewComparison {
        candidate_hash: current.candidate_sha256.clone(), image_hashes: current.image_sha256.clone(),
        previous_candidate_hash: previous.map(|item| item.candidate_sha256.clone()),
        previous_image_hashes: previous.map(|item| item.image_sha256.clone()).unwrap_or_default(),
        status, summary: "Matched views preserve the requested form and entry.".into(),
        strengths: vec!["Entry remains visible".into()], defects: vec![], improvements: vec![], regressions: vec![], simulated: false,
    }
}

#[test]
fn freezes_full_theme_and_candidate_and_exports_compatible_package() {
    let frozen = theme();
    let mut pipeline = AssetPipeline::create(frozen.clone(), "Build a traversable orchard outpost".into(), 1_000).unwrap();
    assert_eq!(pipeline.contract.theme_sha256, living_world_core::pipeline::sha256_json(&frozen).unwrap());
    pipeline.charge_call(1_001).unwrap();
    let candidate = pipeline.submit_candidate(recipe(&frozen.id), 1_002).unwrap();
    let mut evidence = evidence(candidate.candidate_sha256.clone());
    evidence.image_urls = (0..3).map(|i| format!("/evidence/job/1-{i}.png")).collect();
    assert_eq!(pipeline.decide(review(), evidence, 1_003).unwrap(), ReviewVerdict::Accept);

    let package = pipeline.package(pipeline.contract.deadline_ms + 1).unwrap();
    assert_eq!(package.format, "living-empires.asset");
    assert_eq!(package.schema_version, 1);
    assert_eq!(package.asset_type, "building");
    assert_eq!(package.theme, frozen);
    assert_eq!(package.manifest.theme_sha256, pipeline.contract.theme_sha256);
    assert_eq!(package.manifest.content_sha256, candidate.candidate_sha256);
    assert_eq!(package.manifest.evidence.len(), 3);
    assert_eq!(package.manifest.evidence[0].url.as_deref(), Some("/evidence/job/1-0.png"));
    assert_eq!(package.manifest.created_at, "1970-01-01T00:00:01.003Z");
    assert_eq!(package.manifest.review_summary.as_deref(), Some("Review confirms the theme and render are clear."));
    assert!(package.manifest.rehearsal);
    assert_eq!(package.manifest.provider, "fixture");

    let restored: AssetPipeline = serde_json::from_slice(&serde_json::to_vec(&pipeline).unwrap()).unwrap();
    assert_eq!(restored.package(pipeline.contract.deadline_ms + 5).unwrap().recipe, package.recipe);
}

#[test]
fn deterministic_obstruction_requires_repair_before_acceptance() {
    let theme = theme();
    let mut pipeline = AssetPipeline::create(theme.clone(), "Build a traversable relay".into(), 100).unwrap();
    let mut obstructed = recipe(&theme.id);
    obstructed.parts.push(living_world_core::Part {
        id: "obstruction".into(), shape: "box".into(), role: "wall".into(),
        position: [0.0, 1.0, 2.5], size: [2.1, 2.0, 0.6], material: "frame".into(),
    });
    obstructed.scene.as_mut().unwrap().components[0].part_ids.push("obstruction".into());
    let first = pipeline.submit_candidate(obstructed, 101).unwrap();
    assert_eq!(first.checks.iter().find(|c| c.id == "GEN-05.access").unwrap().status, "fail");
    assert_eq!(pipeline.decide(review(), evidence(first.candidate_sha256.clone()), 102).unwrap(), ReviewVerdict::Repair);
    assert!(pipeline.decide(review(), evidence(first.candidate_sha256.clone()), 102).unwrap_err().contains("new candidate"));

    let repaired = pipeline.submit_candidate(recipe(&theme.id), 103).unwrap();
    assert_eq!(repaired.checks.iter().all(|check| check.status == "pass"), true);
    assert_eq!(pipeline.decide(review(), evidence(repaired.candidate_sha256), 104).unwrap(), ReviewVerdict::Accept);
}

#[test]
fn cancellation_deadline_and_call_attempt_budgets_reject_late_work() {
    let theme = theme();
    let mut cancelled = AssetPipeline::create(theme.clone(), "Build".into(), 0).unwrap();
    cancelled.cancel().unwrap();
    assert!(cancelled.submit_candidate(recipe(&theme.id), 1).is_err());
    assert_eq!(cancelled.status, PipelineStatus::Cancelled);

    let mut expired = AssetPipeline::create(theme.clone(), "Build".into(), 0).unwrap();
    assert!(expired.charge_call(expired.contract.deadline_ms).is_err());
    assert_eq!(expired.status, PipelineStatus::Exhausted);
    assert!(expired.submit_candidate(recipe(&theme.id), 1).is_err());

    let mut calls = AssetPipeline::create(theme.clone(), "Build".into(), 0).unwrap();
    for i in 0..calls.contract.limits.max_provider_calls { calls.charge_call(i as u64).unwrap(); }
    assert!(calls.charge_call(10).is_err());
    assert_eq!(calls.status, PipelineStatus::Exhausted);

    let mut attempts = AssetPipeline::create(theme.clone(), "Build".into(), 0).unwrap();
    let reject = ReviewDecision { theme_status: EvidenceStatus::Fail, render_status: EvidenceStatus::Pass, findings: vec!["Repair needed".into()], summary: None, comparison: None };
    for i in 0..attempts.contract.limits.max_attempts {
        let candidate = attempts.submit_candidate(recipe(&theme.id), i as u64).unwrap();
        let expected = if i + 1 == attempts.contract.limits.max_attempts { ReviewVerdict::Exhausted } else { ReviewVerdict::Repair };
        assert_eq!(attempts.decide(reject.clone(), evidence(candidate.candidate_sha256), i as u64 + 1).unwrap(), expected);
    }
    assert_eq!(attempts.status, PipelineStatus::Exhausted);
    assert!(attempts.submit_candidate(recipe(&theme.id), 10).is_err());
    assert_eq!(attempts.attempt_count, 3);
}

#[test]
fn final_attempt_and_call_responses_remain_processable() {
    let theme = theme();
    let mut final_attempt = AssetPipeline::create(theme.clone(), "Build".into(), 0).unwrap();
    let repair = ReviewDecision { theme_status: EvidenceStatus::Fail, render_status: EvidenceStatus::Pass, findings: vec!["Fix theme".into()], summary: None, comparison: None };
    for i in 0..2 {
        let candidate = final_attempt.submit_candidate(recipe(&theme.id), i * 2 + 1).unwrap();
        assert_eq!(final_attempt.decide(repair.clone(), evidence(candidate.candidate_sha256), i * 2 + 2).unwrap(), ReviewVerdict::Repair);
    }
    let last = final_attempt.submit_candidate(recipe(&theme.id), 5).unwrap();
    assert!(!final_attempt.can_continue(5).unwrap());
    assert!(final_attempt.can_work(5).unwrap());
    assert_eq!(final_attempt.decide(review(), evidence(last.candidate_sha256), 6).unwrap(), ReviewVerdict::Accept);

    let mut final_call = AssetPipeline::create(theme.clone(), "Build".into(), 0).unwrap();
    for i in 0..6 { final_call.charge_call(i).unwrap(); }
    assert!(!final_call.can_continue(7).unwrap());
    assert!(final_call.can_work(7).unwrap());
    let candidate = final_call.submit_candidate(recipe(&theme.id), 7).unwrap();
    assert!(final_call.can_work(7).unwrap());
    assert_eq!(final_call.decide(review(), evidence(candidate.candidate_sha256), 8).unwrap(), ReviewVerdict::Accept);
}

#[test]
fn review_requires_independent_statuses_and_exact_image_evidence() {
    let theme = theme();
    let mut pipeline = AssetPipeline::create(theme.clone(), "Build".into(), 0).unwrap();
    let candidate = pipeline.submit_candidate(recipe(&theme.id), 1).unwrap();
    let incomplete = Evidence { candidate_sha256: candidate.candidate_sha256.clone(), image_sha256: vec!["a".repeat(64); 2], image_urls: vec![] };
    assert!(pipeline.decide(review(), incomplete, 2).is_err());
    let duplicate = Evidence { candidate_sha256: candidate.candidate_sha256.clone(), image_sha256: vec!["a".repeat(64); 3], image_urls: vec![] };
    assert!(pipeline.decide(review(), duplicate, 2).is_err());
    let excess = Evidence { candidate_sha256: candidate.candidate_sha256.clone(), image_sha256: ["d", "e", "f", "g"].iter().map(|v| v.repeat(64)).collect(), image_urls: vec![] };
    assert!(pipeline.decide(review(), excess, 2).is_err());
    let missing_render = ReviewDecision { theme_status: EvidenceStatus::Pass, render_status: EvidenceStatus::Blocked, findings: vec![], summary: None, comparison: None };
    assert_eq!(pipeline.decide(missing_render, evidence(candidate.candidate_sha256.clone()), 2).unwrap(), ReviewVerdict::Blocked);
    assert_eq!(pipeline.status, PipelineStatus::Blocked);

    let mut failed = AssetPipeline::create(theme.clone(), "Build".into(), 0).unwrap();
    let candidate = failed.submit_candidate(recipe(&theme.id), 1).unwrap();
    let review_fail = ReviewDecision { theme_status: EvidenceStatus::Fail, render_status: EvidenceStatus::Pass, findings: vec!["Palette mismatch".into()], summary: None, comparison: None };
    assert_eq!(failed.decide(review_fail, evidence(candidate.candidate_sha256), 2).unwrap(), ReviewVerdict::Repair);
}

#[test]
fn mismatched_evidence_and_tampered_deserialized_state_are_rejected() {
    let theme = theme();
    let mut pipeline = AssetPipeline::create(theme.clone(), "Build".into(), 0).unwrap();
    let candidate = pipeline.submit_candidate(recipe(&theme.id), 1).unwrap();
    let wrong_evidence = evidence("0".repeat(64));
    assert!(pipeline.decide(review(), wrong_evidence, 2).is_err());

    let mut restored: AssetPipeline = serde_json::from_slice(&serde_json::to_vec(&pipeline).unwrap()).unwrap();
    restored.contract.theme.exclusions.push("tampered exclusion".into());
    assert!(restored.charge_call(2).unwrap_err().contains("tampered"));

    let mut candidate_tamper: AssetPipeline = serde_json::from_slice(&serde_json::to_vec(&pipeline).unwrap()).unwrap();
    candidate_tamper.candidates[0].recipe.name.push_str(" mutated");
    assert!(candidate_tamper.decide(review(), evidence(candidate.candidate_sha256), 2).unwrap_err().contains("tampered"));

    let mut limits_tamper: AssetPipeline = serde_json::from_slice(&serde_json::to_vec(&pipeline).unwrap()).unwrap();
    limits_tamper.contract.limits.max_provider_calls = 99;
    assert!(limits_tamper.can_continue(2).unwrap_err().contains("tampered"));
}

#[test]
fn theme_and_render_review_statuses_must_each_pass() {
    for decision in [
        ReviewDecision { theme_status: EvidenceStatus::Fail, render_status: EvidenceStatus::Pass, findings: vec![], summary: None, comparison: None },
        ReviewDecision { theme_status: EvidenceStatus::Pass, render_status: EvidenceStatus::Fail, findings: vec![], summary: None, comparison: None },
    ] {
        let theme = theme();
        let mut pipeline = AssetPipeline::create(theme.clone(), "Build".into(), 0).unwrap();
        let candidate = pipeline.submit_candidate(recipe(&theme.id), 1).unwrap();
        assert_eq!(pipeline.decide(decision, evidence(candidate.candidate_sha256), 2).unwrap(), ReviewVerdict::Repair);
    }
}

#[test]
fn provenance_is_frozen_and_exported_without_loss() {
    let theme = theme();
    let mut pipeline = AssetPipeline::create(theme.clone(), "Build".into(), 0).unwrap();
    pipeline.set_provenance(Provenance {
        rehearsal: false, provider: "gemini".into(), model: "gemini-2.5-pro".into(), agent: Some("builder".into()),
        source_revision: Some("rev-abc".into()), source_fingerprint: Some("fp-123".into()),
    }).unwrap();
    assert!(!pipeline.contract.provenance.rehearsal);
    pipeline.charge_call(1).unwrap();
    assert!(pipeline.set_provenance(Provenance::default()).is_err());
    let candidate = pipeline.submit_candidate(recipe(&theme.id), 2).unwrap();
    let current_evidence = evidence(candidate.candidate_sha256);
    let mut decision = review();
    decision.comparison = Some(comparison(&current_evidence, None, ComparisonStatus::Initial));
    pipeline.decide(decision, current_evidence, 3).unwrap();
    let package = pipeline.package(4).unwrap();
    assert!(!package.manifest.rehearsal);
    assert_eq!(package.manifest.provider, "gemini");
    assert_eq!(package.manifest.model, "gemini-2.5-pro");
    assert_eq!(package.manifest.agent.as_deref(), Some("builder"));
    assert_eq!(package.manifest.source_revision.as_deref(), Some("rev-abc"));
    assert_eq!(package.manifest.source_fingerprint.as_deref(), Some("fp-123"));
}

#[test]
fn live_repairs_require_a_hash_bound_improvement_and_export_the_review() {
    let theme = theme();
    let mut pipeline = AssetPipeline::create(theme.clone(), "Build a traversable orchard outpost".into(), 0).unwrap();
    pipeline.set_provenance(Provenance { rehearsal: false, provider: "gemini".into(), model: "gemini-review".into(), agent: None, source_revision: None, source_fingerprint: None }).unwrap();

    let first = pipeline.submit_candidate(recipe(&theme.id), 1).unwrap();
    let first_evidence = evidence(first.candidate_sha256.clone());
    let mut first_comparison = comparison(&first_evidence, None, ComparisonStatus::Initial);
    first_comparison.defects.push(VisualDefect { view: 2, location: "central doorway".into(), criterion: "Entry remains visible".into(), issue: "A post obscures the entry in the front view".into(), repair: "Move the post outside the doorway".into() });
    let first_review = ReviewDecision { theme_status: EvidenceStatus::Fail, render_status: EvidenceStatus::Pass, findings: vec!["Obstructed entrance".into()], summary: Some("Initial view found an entrance obstruction".into()), comparison: Some(first_comparison) };
    assert_eq!(pipeline.decide(first_review, first_evidence.clone(), 2).unwrap(), ReviewVerdict::Repair);

    let mut repaired_recipe = recipe(&theme.id);
    repaired_recipe.name = "Open Orchard Relay".into();
    repaired_recipe.parts[0].position[0] -= 0.2;
    let second = pipeline.submit_candidate(repaired_recipe, 3).unwrap();
    let mut second_evidence = evidence(second.candidate_sha256.clone());
    second_evidence.image_sha256 = ["d", "e", "f"].iter().map(|v| v.repeat(64)).collect();
    let mut second_review = review();
    let mut improved = comparison(&second_evidence, Some(&first_evidence), ComparisonStatus::Improved);
    improved.improvements = vec!["The central doorway is unobstructed in the matched front view".into()];
    improved.strengths.push("The broad roof silhouette is preserved".into());
    second_review.comparison = Some(improved.clone());
    assert_eq!(pipeline.decide(second_review, second_evidence.clone(), 4).unwrap(), ReviewVerdict::Accept);
    let package = pipeline.package(5).unwrap();
    assert_eq!(package.manifest.comparison, Some(improved));

    let mut tampered: AssetPipeline = serde_json::from_slice(&serde_json::to_vec(&pipeline).unwrap()).unwrap();
    tampered.candidates[1].comparison.as_mut().unwrap().improvements.clear();
    assert!(tampered.package(5).unwrap_err().contains("tampered"));
}

#[test]
fn refinement_freezes_feedback_baseline_camera_and_requires_improvement_over_accepted_asset() {
    let frozen_theme=theme();
    let mut baseline_recipe=recipe(&frozen_theme.id);
    baseline_recipe.scene.as_mut().unwrap().half_extent=10.0;
    let baseline_hash=living_world_core::pipeline::sha256_json(&baseline_recipe).unwrap();
    let baseline_evidence=Evidence {
        candidate_sha256:baseline_hash.clone(), image_sha256:["1","2","3"].iter().map(|v|v.repeat(64)).collect(),
        image_urls:vec!["/baseline/1.png".into(),"/baseline/2.png".into(),"/baseline/3.png".into()],
    };
    let refinement=living_world_core::pipeline::RefinementContract {
        user_feedback:"Replace the repeated posts with a recognizable arch but keep the entrance open".into(),
        baseline_recipe:baseline_recipe.clone(), baseline_evidence:baseline_evidence.clone(),
    };
    let mut pipeline=AssetPipeline::create(frozen_theme.clone(),"Design the Orchard Relay".into(),100).unwrap();
    pipeline.set_scene_half_extent(10.0).unwrap();
    pipeline.set_refinement(refinement.clone()).unwrap();
    pipeline.set_provenance(Provenance { rehearsal:false, provider:"gemini".into(), model:"gemini-review".into(), agent:None, source_revision:None, source_fingerprint:None }).unwrap();

    let mut candidate_recipe=recipe(&frozen_theme.id);
    candidate_recipe.name="Orchard Arch Relay".into();
    candidate_recipe.parts[0].position[0]-=0.2;
    candidate_recipe.scene.as_mut().unwrap().half_extent=10.0;
    let candidate=pipeline.submit_candidate(candidate_recipe.clone(),101).unwrap();
    let mut current=evidence(candidate.candidate_sha256.clone());
    current.image_sha256=["4","5","6"].iter().map(|v|v.repeat(64)).collect();
    let mut decision=review();
    let mut improved=comparison(&current,Some(&baseline_evidence),ComparisonStatus::Improved);
    improved.improvements=vec!["The entry arch is distinct in matched front and side views".into()];
    decision.comparison=Some(improved.clone());
    assert_eq!(pipeline.decide(decision.clone(),current.clone(),102).unwrap(),ReviewVerdict::Accept);
    assert_eq!(pipeline.package(103).unwrap().manifest.comparison,Some(improved));

    let mut non_improved=AssetPipeline::create(frozen_theme.clone(),"Design the Orchard Relay".into(),200).unwrap();
    non_improved.set_scene_half_extent(10.0).unwrap();
    non_improved.set_refinement(refinement.clone()).unwrap();
    non_improved.set_provenance(Provenance { rehearsal:false, provider:"gemini".into(), model:"gemini-review".into(), agent:None, source_revision:None, source_fingerprint:None }).unwrap();
    let candidate=non_improved.submit_candidate(candidate_recipe,201).unwrap();
    let mut evidence=evidence(candidate.candidate_sha256.clone()); evidence.image_sha256=["7","8","9"].iter().map(|v|v.repeat(64)).collect();
    let mut unchanged=review(); unchanged.comparison=Some(comparison(&evidence,Some(&baseline_evidence),ComparisonStatus::Unchanged));
    assert_eq!(non_improved.decide(unchanged,evidence,202).unwrap(),ReviewVerdict::Repair);

    let mut mismatch=refinement; mismatch.user_feedback="different feedback after freeze".into();
    let mut tampered:AssetPipeline=serde_json::from_value(serde_json::to_value(&pipeline).unwrap()).unwrap();
    tampered.contract.refinement.as_mut().unwrap().user_feedback=mismatch.user_feedback;
    assert!(tampered.package(103).unwrap_err().contains("tampered"));
}

#[test]
fn live_candidate_must_have_frozen_coherent_scene_extent() {
    let theme=theme();
    let mut pipeline=AssetPipeline::create(theme.clone(),"Build an orchard relay".into(),0).unwrap();
    pipeline.set_scene_half_extent(10.0).unwrap();
    pipeline.set_provenance(Provenance { rehearsal:false, provider:"gemini".into(), model:"gemini-builder".into(), agent:None, source_revision:None, source_fingerprint:None }).unwrap();
    let mut invalid=recipe(&theme.id); invalid.scene=None;
    let missing=pipeline.submit_candidate(invalid,1).unwrap();
    assert!(missing.validation_error.as_deref().unwrap().contains("require the complete coherent scene"));
    let fail=ReviewDecision { theme_status:EvidenceStatus::Fail,render_status:EvidenceStatus::Pass,findings:vec!["Candidate omitted scene groups".into()],summary:None,comparison:None };
    assert_eq!(pipeline.decide(fail,evidence(missing.candidate_sha256),2).unwrap(),ReviewVerdict::Repair);

    let mut wrong_extent=recipe(&theme.id); wrong_extent.scene.as_mut().unwrap().half_extent=12.0;
    let candidate=pipeline.submit_candidate(wrong_extent,3).unwrap();
    assert!(candidate.validation_error.as_deref().unwrap().contains("frozen camera extent"));
}

#[test]
fn live_improvement_claim_is_rejected_for_identical_geometry_or_identical_rendered_views() {
    let theme=theme();
    let mut pipeline=AssetPipeline::create(theme.clone(),"Build an orchard relay".into(),0).unwrap();
    pipeline.set_provenance(Provenance { rehearsal:false, provider:"gemini".into(), model:"gemini-review".into(), agent:None, source_revision:None, source_fingerprint:None }).unwrap();
    let first=pipeline.submit_candidate(recipe(&theme.id),1).unwrap();
    let first_evidence=evidence(first.candidate_sha256.clone());
    let mut first_review=ReviewDecision { theme_status:EvidenceStatus::Fail,render_status:EvidenceStatus::Pass,findings:vec!["Visible arch needs definition".into()],summary:None,comparison:None };
    let mut initial=comparison(&first_evidence,None,ComparisonStatus::Initial);
    initial.defects.push(VisualDefect { view:1,location:"roofline".into(),criterion:"Entry remains visible".into(),issue:"The entry arch is not distinct".into(),repair:"Give the arch a separate silhouette".into() });
    first_review.comparison=Some(initial);
    assert_eq!(pipeline.decide(first_review,first_evidence.clone(),2).unwrap(),ReviewVerdict::Repair);

    // A metadata-only rename with different captures still cannot substantiate visible improvement.
    let mut renamed=recipe(&theme.id); renamed.name="Improved Orchard Relay".into();
    let second=pipeline.submit_candidate(renamed,3).unwrap();
    let mut second_evidence=evidence(second.candidate_sha256.clone());
    second_evidence.image_sha256=["d","e","f"].iter().map(|v|v.repeat(64)).collect();
    let mut second_review=review();
    let mut claimed=comparison(&second_evidence,Some(&first_evidence),ComparisonStatus::Improved);
    claimed.improvements=vec!["The arch is more distinct".into()];
    second_review.comparison=Some(claimed);
    assert_eq!(pipeline.decide(second_review,second_evidence.clone(),4).unwrap(),ReviewVerdict::Repair);

    // Changed geometry with an identical ordered PNG triple is also not a visible improvement.
    let mut moved=recipe(&theme.id); moved.parts[0].position[0]-=0.2;
    let third=pipeline.submit_candidate(moved,5).unwrap();
    let mut third_evidence=second_evidence.clone();
    third_evidence.candidate_sha256=third.candidate_sha256.clone();
    let mut third_review=review();
    let mut claimed=comparison(&third_evidence,Some(&second_evidence),ComparisonStatus::Improved);
    claimed.improvements=vec!["The arch is more distinct".into()];
    third_review.comparison=Some(claimed);
    assert_eq!(pipeline.decide(third_review,third_evidence,6).unwrap(),ReviewVerdict::Exhausted);
    assert_eq!(pipeline.status,PipelineStatus::Exhausted);
    assert_eq!(pipeline.candidates[2].candidate_sha256,third.candidate_sha256);
}

#[test]
fn deterministic_live_failure_keeps_capture_as_the_next_comparison_baseline() {
    let theme = theme();
    let mut pipeline = AssetPipeline::create(theme.clone(), "Build a traversable relay".into(), 0).unwrap();
    pipeline.set_provenance(Provenance { rehearsal: false, provider: "gemini".into(), model: "gemini-review".into(), agent: None, source_revision: None, source_fingerprint: None }).unwrap();
    let invalid = pipeline.submit_candidate(recipe("wrong-theme"), 1).unwrap();
    assert!(invalid.validation_error.is_some());
    let first_evidence = evidence(invalid.candidate_sha256);
    let fail = ReviewDecision { theme_status: EvidenceStatus::Fail, render_status: EvidenceStatus::Fail, findings: vec!["Recipe theme identity mismatch".into()], summary: None, comparison: None };
    assert_eq!(pipeline.decide(fail, first_evidence.clone(), 2).unwrap(), ReviewVerdict::Repair);

    let mut fixed_recipe=recipe(&theme.id);
    fixed_recipe.parts[0].position[0]-=0.2;
    let fixed = pipeline.submit_candidate(fixed_recipe, 3).unwrap();
    let mut current = evidence(fixed.candidate_sha256);
    current.image_sha256 = ["d", "e", "f"].iter().map(|value| value.repeat(64)).collect();
    let mut decision = review();
    let mut critique = comparison(&current, Some(&first_evidence), ComparisonStatus::Improved);
    critique.improvements.push("Recipe now carries the frozen theme identity".into());
    decision.comparison = Some(critique);
    assert_eq!(pipeline.decide(decision, current, 4).unwrap(), ReviewVerdict::Accept);
}
