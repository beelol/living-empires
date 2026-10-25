use living_world_core::{
    Architecture, AssetPipeline, Evidence, EvidenceStatus, Flora, Palette, PipelineStatus,
    Provenance, Recipe, ReferencePreset, ReviewDecision, ReviewVerdict, ThemeContract,
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
    let parts = (0..4).map(|i| living_world_core::Part {
        id: format!("post-{i}"), shape: "box".into(), role: "support".into(),
        position: [if i % 2 == 0 { -2.0 } else { 2.0 }, 1.5, if i < 2 { -2.0 } else { 2.0 }],
        size: [0.5, 3.0, 0.5], material: "frame".into(),
    }).collect();
    Recipe { theme_id: theme_id.into(), name: "Test outpost".into(), description: "Sensor relay".into(), sensor_radius: 12.0, parts }
}

fn review() -> ReviewDecision {
    ReviewDecision { theme_status: EvidenceStatus::Pass, render_status: EvidenceStatus::Pass, findings: vec![], summary: Some("Review confirms the theme and render are clear.".into()) }
}

fn evidence(candidate_sha256: String) -> Evidence {
    Evidence { candidate_sha256, image_sha256: ["a", "b", "c"].iter().map(|v| v.repeat(64)).collect(), image_urls: vec![] }
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
    let first = pipeline.submit_candidate(obstructed, 101).unwrap();
    assert_eq!(first.checks.iter().find(|c| c.id == "GEN-05.access").unwrap().status, "fail");
    assert_eq!(pipeline.decide(review(), evidence(first.candidate_sha256), 102).unwrap(), ReviewVerdict::Repair);

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
    let reject = ReviewDecision { theme_status: EvidenceStatus::Fail, render_status: EvidenceStatus::Pass, findings: vec!["Repair needed".into()], summary: None };
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
    let missing_render = ReviewDecision { theme_status: EvidenceStatus::Pass, render_status: EvidenceStatus::Blocked, findings: vec![], summary: None };
    assert_eq!(pipeline.decide(missing_render, evidence(candidate.candidate_sha256.clone()), 2).unwrap(), ReviewVerdict::Blocked);
    assert_eq!(pipeline.status, PipelineStatus::Blocked);

    let mut failed = AssetPipeline::create(theme.clone(), "Build".into(), 0).unwrap();
    let candidate = failed.submit_candidate(recipe(&theme.id), 1).unwrap();
    let review_fail = ReviewDecision { theme_status: EvidenceStatus::Fail, render_status: EvidenceStatus::Pass, findings: vec!["Palette mismatch".into()], summary: None };
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
        ReviewDecision { theme_status: EvidenceStatus::Fail, render_status: EvidenceStatus::Pass, findings: vec![], summary: None },
        ReviewDecision { theme_status: EvidenceStatus::Pass, render_status: EvidenceStatus::Fail, findings: vec![], summary: None },
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
    pipeline.decide(review(), evidence(candidate.candidate_sha256), 3).unwrap();
    let package = pipeline.package(4).unwrap();
    assert!(!package.manifest.rehearsal);
    assert_eq!(package.manifest.provider, "gemini");
    assert_eq!(package.manifest.model, "gemini-2.5-pro");
    assert_eq!(package.manifest.agent.as_deref(), Some("builder"));
    assert_eq!(package.manifest.source_revision.as_deref(), Some("rev-abc"));
    assert_eq!(package.manifest.source_fingerprint.as_deref(), Some("fp-123"));
}
