use living_world_core::{
    access_scenario,
    evaluate_with_theme,
    provider::{build_generation_request, build_review_request, fixture_recipe, generate_with_transport, repair_fixture, review_with_transport, GenerationRequest, PreviousReviewEvidence, ProviderError, ProviderTransport, ReviewRequest},
    Part,
    Architecture, Flora, Palette, ReferencePreset, ThemeContract,
};
use serde_json::{json, Value};
use std::sync::Mutex;

fn theme() -> ThemeContract {
    ThemeContract {
        id: "custom-forest".into(), name: "Custom forest".into(), genre: "Forest frontier".into(),
        description: "A quiet woodland relay station.".into(), biome: "Temperate moss forest".into(),
        shape_language: "Conical pavilion with slender posts".into(), material_finish: "Weathered timber and stone".into(),
        weathering: "Moss at joints".into(), technology: "Small sensor relay".into(), motifs: vec!["grove roof".into()],
        lighting: "Soft green light".into(), scale: "Human scale".into(), gameplay_role: "Route relay".into(),
        exclusions: vec!["chrome machinery".into()], visual_checks: vec!["Central path is open".into()],
        assumptions: vec!["Local test contract".into()], rendering_notes: vec!["Texture metadata is not separately rendered".into()],
        reference_preset: ReferencePreset::Sylvan,
        palette: Palette { shell:"#b4bd92".into(),frame:"#615444".into(),accent:"#ffc577".into(),ground:"#3f614e".into(),rock:"#788475".into() },
        sky:"#b9cdb1".into(),flora:Flora::Tree,architecture:Architecture::Organic,
    }
}

fn request(managed: bool) -> GenerationRequest {
    GenerationRequest {
        theme: theme(), prompt: "Build a forest relay".into(), user_feedback: None, scene_half_extent: Some(12.0), previous: None,
        findings: Some(vec!["clear front path".into()]), model: "gemini-test".into(),
        agent: "antigravity-preview-09-2026".into(), managed,
        interaction_id: None, environment_id: None,
    }
}

#[derive(Default)]
struct MockTransport { calls: Mutex<Vec<(String, String, Value)>>, response: Mutex<Option<Value>> }
impl ProviderTransport for MockTransport {
    fn post_json(&self, endpoint:&str, api_key:&str, body:&Value) -> Result<Value,ProviderError> {
        self.calls.lock().unwrap().push((endpoint.into(), api_key.into(), body.clone()));
        Ok(self.response.lock().unwrap().clone().unwrap_or_else(|| json!({})))
    }
}

fn text_response(text: String) -> Value {
    json!({"candidates":[{"content":{"parts":[{"text":text}]}}],"usageMetadata":{"totalTokenCount":21}})
}

fn png_view(label: &[u8]) -> (String, String) {
    use base64::Engine as _;
    let mut bytes = b"\x89PNG\r\n\x1a\n".to_vec();
    bytes.extend_from_slice(label);
    (base64::engine::general_purpose::STANDARD.encode(&bytes), living_world_core::pipeline::sha256(&bytes))
}

#[test]
fn direct_payload_contains_the_full_frozen_theme_and_structured_schema() {
    let mut request=request(false);
    request.user_feedback=Some("Give the roof a distinct arch while preserving the clear entrance".into());
    let (url,body)=build_generation_request(&request).unwrap();
    assert!(url.ends_with("/models/gemini-test:generateContent"));
    assert_eq!(body["generationConfig"]["responseMimeType"],"application/json");
    let prompt=body["contents"][0]["parts"][0]["text"].as_str().unwrap();
    for field in ["custom-forest","Temperate moss forest","Central path is open","Build a forest relay"] { assert!(prompt.contains(field),"missing {field}"); }
    assert!(prompt.contains("Give the roof a distinct arch"));
    assert!(prompt.contains("halfExtent of 12 metres"));
    assert_eq!(body["generationConfig"]["responseJsonSchema"]["properties"]["parts"]["maxItems"],128);
    assert!(prompt.contains("complete, cohesive 3D sensor-outpost scene"));
    assert_eq!(body["generationConfig"]["responseJsonSchema"]["required"].as_array().unwrap().iter().any(|v|v=="scene"),true);
}

#[test]
fn direct_generation_parses_and_validates_recipe_without_exposing_key_in_result() {
    let request=request(false);
    let recipe=fixture_recipe(&request.theme).unwrap();
    let transport=MockTransport::default();
    *transport.response.lock().unwrap()=Some(text_response(serde_json::to_string(&recipe).unwrap()));
    let result=generate_with_transport(&request,"not-serialized-key",&transport).unwrap();
    assert_eq!(result.recipe.theme_id,"custom-forest");
    assert_eq!(result.usage.as_ref().unwrap()["totalTokenCount"],21);
    assert!(result.interaction_id.is_none());
    assert!(serde_json::to_string(&result).unwrap().find("not-serialized-key").is_none());
    let calls=transport.calls.lock().unwrap();
    assert_eq!(calls.len(),1);
    assert_eq!(calls[0].1,"not-serialized-key");
}

#[test]
fn managed_generation_resumes_context_and_environment_and_extracts_final_text() {
    let mut request=request(true);
    request.interaction_id=Some("previous-turn".into());
    request.environment_id=Some("sandbox-2".into());
    let recipe=fixture_recipe(&request.theme).unwrap();
    let transport=MockTransport::default();
    *transport.response.lock().unwrap()=Some(json!({"id":"next-turn","environment_id":"sandbox-2","outputs":[{"type":"reasoning","text":"ignore"},{"type":"text","text":format!("```json\n{}\n```",serde_json::to_string(&recipe).unwrap())}],"usage":{"total_tokens":7}}));
    let result=generate_with_transport(&request,"key",&transport).unwrap();
    assert_eq!(result.interaction_id.as_deref(),Some("next-turn"));
    assert_eq!(result.environment_id.as_deref(),Some("sandbox-2"));
    assert_eq!(result.usage.as_ref().unwrap()["total_tokens"],7);
    let call=&transport.calls.lock().unwrap()[0];
    assert!(call.0.ends_with("/interactions"));
    assert_eq!(call.2["previous_interaction_id"],"previous-turn");
    assert_eq!(call.2["environment"],"sandbox-2");
    assert_eq!(call.2["agent_config"]["model"],"gemini-test");
    assert_eq!(call.2["agent_config"]["max_total_tokens"],50_000);
}

#[test]
fn completed_managed_model_output_step_is_used_and_raw_response_metadata_is_preserved(){
    let request=request(true);
    let recipe=fixture_recipe(&request.theme).unwrap();
    let raw=json!({"id":"completed-interaction","status":"completed","usage":{"total_tokens":31},"environment_id":"remote-env","agent":"antigravity-preview-09-2026","steps":[
        {"type":"thought","summary":"ignore this reasoning"},
        {"type":"code_execution_call","arguments":{"command":"ignored tool call"}},
        {"type":"code_execution_result","result":"ignored tool result"},
        {"type":"model_output","content":[{"type":"text","text":format!("```json\n{}\n```",serde_json::to_string(&recipe).unwrap())}]}
    ]});
    let transport=MockTransport::default();*transport.response.lock().unwrap()=Some(raw.clone());
    let result=generate_with_transport(&request,"key",&transport).unwrap();
    assert_eq!(result.recipe.theme_id,"custom-forest");
    assert_eq!(result.interaction_id.as_deref(),Some("completed-interaction"));
    assert_eq!(result.environment_id.as_deref(),Some("remote-env"));
    assert_eq!(result.usage.as_ref().unwrap()["total_tokens"],31);
    assert_eq!(result.response,raw);

    let malformed=json!({"id":"unknown-outcome-id","status":"completed","usage":{"total_tokens":47},"environment_id":"remote-env","steps":[{"type":"model_output","content":[{"type":"text","text":"not recipe json"}]}],"agent":"antigravity-preview-09-2026"});
    *transport.response.lock().unwrap()=Some(malformed.clone());
    let error=generate_with_transport(&request,"key",&transport).unwrap_err();
    assert!(error.to_string().contains("malformed JSON"));
    assert_eq!(error.response,Some(malformed));
}

#[test]
fn malformed_or_invalid_provider_recipes_are_rejected_without_retry() {
    let request=request(false);
    let transport=MockTransport::default();
    let malformed=text_response("not json".into());
    *transport.response.lock().unwrap()=Some(malformed.clone());
    let error=generate_with_transport(&request,"key",&transport).unwrap_err();
    assert!(error.to_string().contains("malformed JSON"));
    assert_eq!(error.response,Some(malformed));
    let invalid=fixture_recipe(&request.theme).unwrap();
    let mut invalid=serde_json::to_value(invalid).unwrap();
    invalid["parts"][0]["material"]=json!("#fff");
    *transport.response.lock().unwrap()=Some(text_response(invalid.to_string()));
    let result=generate_with_transport(&request,"key",&transport).unwrap();
    assert!(evaluate_with_theme(&result.recipe,&request.theme).is_err(),"pipeline owns deterministic failure recording and repair");
    assert_eq!(transport.calls.lock().unwrap().len(),2);
}

#[test]
fn reviewer_request_uses_three_actual_inline_pngs_and_checks_verdicts() {
    let views: Vec<_> = [b"a".as_slice(),b"b".as_slice(),b"c".as_slice()].iter().map(|label| png_view(label)).collect();
    let request=ReviewRequest {theme:theme(),prompt:"Forest relay".into(),user_feedback:None,scene_half_extent:Some(12.0),images:views.iter().map(|view|view.0.clone()).collect(),candidate_hash:"d".repeat(64),image_hashes:views.iter().map(|view|view.1.clone()).collect(),previous:None,baseline:None,model:"gemini-review".into()};
    let (url,body)=build_review_request(&request).unwrap();
    assert!(url.ends_with(":generateContent"));
    assert_eq!(body["contents"][0]["parts"].as_array().unwrap().len(),7);
    assert_eq!(body["contents"][0]["parts"][2]["inlineData"]["mimeType"],"image/png");
    assert_eq!(body["contents"][0]["parts"][6]["inlineData"]["data"],request.images[2]);
    assert_eq!(body["contents"][0]["parts"][1]["text"],"CURRENT VIEW 1");
    assert!(body["contents"][0]["parts"][0]["text"].as_str().unwrap().contains("Set simulated=false"));
    let comparison=json!({"candidateHash":request.candidate_hash.clone(),"imageHashes":request.image_hashes.clone(),"previousCandidateHash":null,"previousImageHashes":[],"status":"initial","summary":"Three matched views show the expected forest relay silhouette.","strengths":["Entry remains visible"],"defects":[],"improvements":[],"regressions":[],"simulated":false});
    let transport=MockTransport::default();
    *transport.response.lock().unwrap()=Some(text_response(json!({"themeStatus":"pass","renderStatus":"blocked","findings":"View 3 is unclear.","comparison":comparison}).to_string()));
    let result=review_with_transport(&request,"key",&transport).unwrap();
    assert_eq!(result.theme_status,"pass");
    assert_eq!(result.render_status,"blocked");
    assert!(result.findings.contains("View 3"));
    assert_eq!(result.comparison.image_hashes,request.image_hashes);
    *transport.response.lock().unwrap()=Some(text_response(r#"{"themeStatus":"maybe","renderStatus":"pass","findings":"","comparison":{}}"#.into()));
    assert!(review_with_transport(&request,"key",&transport).unwrap_err().to_string().contains("invalid verdict"));
}

#[test]
fn reviewer_hashes_decoded_png_bytes_and_binds_matched_previous_views() {
    let current: Vec<_> = [b"new-a".as_slice(),b"new-b".as_slice(),b"new-c".as_slice()].iter().map(|label| png_view(label)).collect();
    let previous: Vec<_> = [b"old-a".as_slice(),b"old-b".as_slice(),b"old-c".as_slice()].iter().map(|label| png_view(label)).collect();
    let mut request = ReviewRequest { theme: theme(), prompt: "Forest relay".into(), user_feedback:Some("Keep the open arch; make the roof less repetitive".into()), scene_half_extent:Some(12.0), images: current.iter().map(|v|v.0.clone()).collect(),
        candidate_hash: "a".repeat(64), image_hashes: current.iter().map(|v|v.1.clone()).collect(),
        previous: Some(PreviousReviewEvidence { candidate_hash:"b".repeat(64), image_hashes:previous.iter().map(|v|v.1.clone()).collect(), images:previous.iter().map(|v|v.0.clone()).collect(), strengths:vec!["Open entry".into()] }), baseline:None, model:"gemini-review".into() };
    let (_, body) = build_review_request(&request).unwrap();
    let parts = body["contents"][0]["parts"].as_array().unwrap();
    assert!(parts.iter().any(|part|part["text"]=="PREVIOUS VIEW 1"));
    assert!(parts.iter().any(|part|part["text"]=="CURRENT VIEW 1"));
    assert_eq!(parts.iter().filter(|part|part["inlineData"].is_object()).count(),6);
    let prompt=parts[0]["text"].as_str().unwrap();
    assert!(prompt.contains("Frozen user feedback: Keep the open arch"));
    assert!(prompt.contains("Frozen review camera halfExtent: 12"));
    request.image_hashes[0] = "c".repeat(64);
    assert!(build_review_request(&request).unwrap_err().to_string().contains("hash does not match"));
}

#[test]
fn refinement_reviewer_receives_frozen_user_feedback_and_labeled_accepted_baseline_views() {
    let current: Vec<_> = [b"current-a".as_slice(),b"current-b".as_slice(),b"current-c".as_slice()].iter().map(|v|png_view(v)).collect();
    let baseline: Vec<_> = [b"base-a".as_slice(),b"base-b".as_slice(),b"base-c".as_slice()].iter().map(|v|png_view(v)).collect();
    let request=ReviewRequest {
        theme:theme(), prompt:"Relay at a forest edge".into(),
        user_feedback:Some("Replace repeated posts with a recognizable arch while keeping the clear entrance".into()),
        scene_half_extent:Some(10.0), images:current.iter().map(|v|v.0.clone()).collect(),
        candidate_hash:"a".repeat(64), image_hashes:current.iter().map(|v|v.1.clone()).collect(),
        previous:None, baseline:Some(PreviousReviewEvidence {
            candidate_hash:"b".repeat(64), image_hashes:baseline.iter().map(|v|v.1.clone()).collect(),
            images:baseline.iter().map(|v|v.0.clone()).collect(), strengths:vec!["Entrance is already readable".into()],
        }), model:"gemini-review".into(),
    };
    let (_,body)=build_review_request(&request).unwrap();
    let parts=body["contents"][0]["parts"].as_array().unwrap();
    let prompt=parts[0]["text"].as_str().unwrap();
    assert!(prompt.contains("Frozen user feedback: Replace repeated posts"));
    assert!(prompt.contains("Accepted baseline candidate hash"));
    assert!(prompt.contains("Entrance is already readable"));
    assert!(parts.iter().any(|part|part["text"]=="BASELINE VIEW 1"));
    assert!(parts.iter().any(|part|part["text"]=="CURRENT VIEW 1"));
    assert_eq!(parts.iter().filter(|part|part["inlineData"].is_object()).count(),6);
}

#[test]
fn fixture_recipe_uses_snapshot_geometry_and_contract_identity() {
    let mut theme=theme();theme.reference_preset=ReferencePreset::Ember;
    let recipe=fixture_recipe(&theme).unwrap();
    assert_eq!(recipe.theme_id,"custom-forest");
    assert_eq!(recipe.name,"Custom forest");
    assert_eq!(recipe.description,"A quiet woodland relay station.");
    assert!(recipe.parts.iter().any(|part| part.id=="vent-west"));
    assert_eq!(recipe.parts.len(),14);
}

#[test]
fn repair_fixture_removes_only_audit_obstruction_and_preserves_identity() {
    let theme = theme();
    let mut previous = fixture_recipe(&theme).unwrap();
    previous.name = "Custom Sensor Relay".into();
    previous.description = "Keep this explicit identity during fixture repair".into();
    previous.parts.push(Part {
        id: "audit-obstruction".into(), shape: "box".into(), role: "wall".into(),
        position: [0.0, 1.0, 2.5], size: [2.1, 2.0, 0.6], material: "frame".into(),
    });
    assert!(!access_scenario(&previous).0);
    let expected_parts: Vec<_> = previous.parts.iter().filter(|part| part.id != "audit-obstruction").cloned().collect();
    let repaired = repair_fixture(&previous, &theme).unwrap();
    assert_eq!(repaired.theme_id, previous.theme_id);
    assert_eq!(repaired.name, previous.name);
    assert_eq!(repaired.description, previous.description);
    assert_eq!(repaired.sensor_radius, previous.sensor_radius);
    assert_eq!(repaired.parts, expected_parts);
    assert!(access_scenario(&repaired).0);

    let mut mismatch = theme.clone();
    mismatch.id = "other-theme".into();
    assert!(repair_fixture(&previous, &mismatch).unwrap_err().to_string().contains("frozen theme"));
}
