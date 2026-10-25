use living_world_core::{
    access_scenario,
    provider::{build_generation_request, build_review_request, fixture_recipe, generate_with_transport, repair_fixture, review_with_transport, GenerationRequest, ProviderError, ProviderTransport, ReviewRequest},
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
        theme: theme(), prompt: "Build a forest relay".into(), previous: None,
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

#[test]
fn direct_payload_contains_the_full_frozen_theme_and_structured_schema() {
    let request=request(false);
    let (url,body)=build_generation_request(&request).unwrap();
    assert!(url.ends_with("/models/gemini-test:generateContent"));
    assert_eq!(body["generationConfig"]["responseMimeType"],"application/json");
    let prompt=body["contents"][0]["parts"][0]["text"].as_str().unwrap();
    for field in ["custom-forest","Temperate moss forest","Central path is open","Build a forest relay"] { assert!(prompt.contains(field),"missing {field}"); }
    assert_eq!(body["generationConfig"]["responseJsonSchema"]["properties"]["parts"]["maxItems"],48);
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
}

#[test]
fn malformed_or_invalid_provider_recipes_are_rejected_without_retry() {
    let request=request(false);
    let transport=MockTransport::default();
    *transport.response.lock().unwrap()=Some(text_response("not json".into()));
    assert!(generate_with_transport(&request,"key",&transport).unwrap_err().to_string().contains("malformed JSON"));
    let invalid=fixture_recipe(&request.theme).unwrap();
    let mut invalid=serde_json::to_value(invalid).unwrap();
    invalid["parts"][0]["material"]=json!("#fff");
    *transport.response.lock().unwrap()=Some(text_response(invalid.to_string()));
    assert!(generate_with_transport(&request,"key",&transport).unwrap_err().to_string().contains("deterministic validation"));
    assert_eq!(transport.calls.lock().unwrap().len(),2);
}

#[test]
fn reviewer_request_uses_three_actual_inline_pngs_and_checks_verdicts() {
    let request=ReviewRequest {theme:theme(),prompt:"Forest relay".into(),images:vec!["cG5nLWE=".into(),"cG5nLWI=".into(),"cG5nLWM=".into()],model:"gemini-review".into()};
    let (url,body)=build_review_request(&request).unwrap();
    assert!(url.ends_with(":generateContent"));
    assert_eq!(body["contents"][0]["parts"].as_array().unwrap().len(),4);
    assert_eq!(body["contents"][0]["parts"][1]["inlineData"]["mimeType"],"image/png");
    assert_eq!(body["contents"][0]["parts"][3]["inlineData"]["data"],"cG5nLWM=");
    let transport=MockTransport::default();
    *transport.response.lock().unwrap()=Some(text_response(r#"{"themeStatus":"pass","renderStatus":"blocked","findings":"View 3 is unclear."}"#.into()));
    let result=review_with_transport(&request,"key",&transport).unwrap();
    assert_eq!(result.theme_status,"pass");
    assert_eq!(result.render_status,"blocked");
    assert!(result.findings.contains("View 3"));
    *transport.response.lock().unwrap()=Some(text_response(r#"{"themeStatus":"maybe","renderStatus":"pass","findings":""}"#.into()));
    assert!(review_with_transport(&request,"key",&transport).unwrap_err().to_string().contains("invalid verdict"));
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
