use std::io::{self, Read};
use living_world_core::{Recipe,ThemeContract,evaluate,evaluate_with_theme,access_scenario};
use living_world_core::pipeline::{AssetPipeline,JobLimits,Provenance,ReviewDecision,Evidence,RefinementContract,ReferenceContract,RoleModels};
use living_world_core::provider::{fixture_recipe,repair_fixture};
use serde::de::DeserializeOwned;
#[cfg(feature="live-provider")]
use serde::Serialize;
use serde_json::{Value,json};

#[cfg(feature="live-provider")]
fn provider_output<T:Serialize>(result:Result<T,living_world_core::provider::ProviderError>)->Result<Value,String>{
    match result{
        Ok(value)=>Ok(json!(value)),
        Err(error)=>{
            let message=error.to_string();
            match error.response{
            Some(response)=>Ok(provider_response_output(message,response)),
            None=>Err(message),
            }
        },
    }
}

#[cfg(feature="live-provider")]
fn provider_response_output(message:String,response:Value)->Value{
    json!({"providerError":{"message":message,"response":response}})
}

fn field<T:DeserializeOwned>(input:&Value,key:&str)->Result<T,String>{serde_json::from_value(input.get(key).cloned().ok_or_else(||format!("Missing {key}"))?).map_err(|e|format!("Invalid {key}: {e}"))}
fn run(input:Value)->Result<Value,String>{
    let now=input.get("now").and_then(Value::as_u64).unwrap_or(0);
    let op=input.get("op").and_then(Value::as_str).unwrap_or("validate");
    if op=="capabilities"{return Ok(json!({"engine":"living-world-core/0.1.0","nativeProvider":cfg!(feature="live-provider"),"pipelineVersion":1}));}
    if op=="create"{
        let mut pipeline=AssetPipeline::create(field(&input,"theme")?,field(&input,"prompt")?,now)?;
        if input.get("limits").is_some(){pipeline.set_limits(field::<JobLimits>(&input,"limits")?)?;}
        if input.get("roleModels").is_some(){pipeline.set_role_models(field::<RoleModels>(&input,"roleModels")?)?;}
        if input.get("sceneHalfExtent").is_some(){pipeline.set_scene_half_extent(field(&input,"sceneHalfExtent")?)?;}
        if input.get("refinement").is_some(){pipeline.set_refinement(field::<RefinementContract>(&input,"refinement")?)?;}
        if input.get("reference").is_some(){pipeline.set_reference(field::<ReferenceContract>(&input,"reference")?)?;}
        if input.get("provenance").is_some(){pipeline.set_provenance(field::<Provenance>(&input,"provenance")?)?;}
        return Ok(json!({"pipeline":pipeline}));
    }
    if op=="prepare-generation"{
        let request=field::<living_world_core::provider::GenerationRequest>(&input,"request")?;
        let (endpoint,mut body)=living_world_core::provider::build_generation_request(&request).map_err(|e|e.to_string())?;
        let mut metadata=Vec::with_capacity(request.reference_images.len());
        use base64::Engine as _;
        for image in &request.reference_images {
            let bytes=base64::engine::general_purpose::STANDARD.decode(&image.data).map_err(|_|"Invalid reference image base64".to_string())?;
            metadata.push(json!({"sha256":request.reference_sha256,"mimeType":image.mime_type,"bytes":bytes.len(),"destination":"Google Gemini","consent":request.reference_consent}));
        }
        if request.managed {
            if let Some(items)=body["input"].as_array_mut() {
                for item in items.iter_mut().filter(|item|item["type"]=="image") { item["data"]=json!("[redacted]"); }
            }
        } else if let Some(parts)=body["contents"][0]["parts"].as_array_mut() {
            for part in parts.iter_mut().filter(|part|part["inlineData"].is_object()) { part["inlineData"]["data"]=json!("[redacted]"); }
        }
        return Ok(json!({"endpoint":endpoint,"body":body,"referenceImages":metadata}));
    }
    if op=="fixture"{
        let theme=field::<ThemeContract>(&input,"theme")?;
        let result=if input.get("previous").is_some_and(|v|!v.is_null()){repair_fixture(&field::<Recipe>(&input,"previous")?,&theme)}else{fixture_recipe(&theme)};
        return result.map(|recipe|json!({"recipe":recipe})).map_err(|e|e.to_string());
    }
    if op=="generate"||op=="review"{
        #[cfg(feature="live-provider")]{
            use living_world_core::provider::{GenerationRequest,ReviewRequest,UreqTransport,generate_with_transport,review_with_transport};
            let key=std::env::var("GEMINI_API_KEY").or_else(|_|std::env::var("GOOGLE_API_KEY")).map_err(|_|"GEMINI_API_KEY is not configured")?;
            if op=="generate"{return provider_output(generate_with_transport(&field::<GenerationRequest>(&input,"request")?,&key,&UreqTransport));}
            return provider_output(review_with_transport(&field::<ReviewRequest>(&input,"request")?,&key,&UreqTransport));
        }
        #[cfg(not(feature="live-provider"))]{return Err("Native Gemini transport is disabled in this build; cargo build -p living-world-core --features live-provider".into());}
    }
    if ["continue","active","charge","submit","decide","cancel","package"].contains(&op){
        let mut pipeline=field::<AssetPipeline>(&input,"pipeline")?;
        let value=match op{
            "continue"=>json!(pipeline.can_continue(now)?),
            "active"=>json!(pipeline.can_work(now)?),
            "charge"=>json!(pipeline.charge_call(now)?),
            "submit"=>json!(pipeline.submit_candidate(field::<Recipe>(&input,"recipe")?,now)?),
            "decide"=>json!(pipeline.decide(field::<ReviewDecision>(&input,"review")?,field::<Evidence>(&input,"evidence")?,now)?),
            "cancel"=>{pipeline.cancel()?;Value::Null},
            "package"=>json!(pipeline.package(now)?),
            _=>unreachable!()
        };
        return Ok(json!({"pipeline":pipeline,"value":value}));
    }
    if op!="validate"{return Err("Unknown pipeline operation".into());}
    let recipe=field::<Recipe>(&input,"recipe")?;let theme_id=field::<String>(&input,"themeId")?;
    let theme:Option<ThemeContract>=input.get("theme").filter(|v|!v.is_null()).cloned().map(serde_json::from_value).transpose().map_err(|e|format!("Invalid theme: {e}"))?;
    let checks=if let Some(t)=&theme{if t.id!=theme_id{return Err("Request themeId does not match contract id".into());}evaluate_with_theme(&recipe,t)?}else{evaluate(&recipe,&theme_id)?};
    let(pass,trace)=access_scenario(&recipe);
    Ok(json!({"checks":checks,"movement":{"pass":pass,"trace":trace},"engine":"living-world-core/0.1.0","theme":theme}))
}
fn main(){
    let mut input=String::new();
    let result=io::stdin().take(12_000_000).read_to_string(&mut input).map_err(|_|"Unable to read request".to_string()).and_then(|_|serde_json::from_str(&input).map_err(|e|format!("Invalid JSON: {e}"))).and_then(run);
    match result{Ok(value)=>println!("{value}"),Err(error)=>{println!("{}",json!({"error":error}));std::process::exit(1);}}
}

#[cfg(test)]
mod offline_tests {
    use super::*;

    #[test]
    fn create_accepts_and_freezes_role_models_and_remaining_limits() {
        let input = json!({
            "op":"create","now":1000,"prompt":"Build one forest relay",
            "theme":{
                "id":"custom-forest","name":"Forest relay","genre":"Frontier","description":"A forest sensor relay.",
                "biome":"Moss forest","shapeLanguage":"Arched timber","materialFinish":"Weathered wood","weathering":"Moss at joints",
                "technology":"Sensor relay","motifs":["seed pods"],"lighting":"Soft green","scale":"Human scale",
                "gameplayRole":"Route marker","exclusions":["military insignia"],"visualChecks":["Entry remains visible"],
                "assumptions":[],"renderingNotes":["Texture is contract metadata"],"referencePreset":"sylvan",
                "palette":{"shell":"#b4bd92","frame":"#615444","accent":"#ffc577","ground":"#3f614e","rock":"#788475"},
                "sky":"#b9cdb1","flora":"tree","architecture":"organic"
            },
            "limits":{"maxAttempts":2,"timeBudgetMs":400000,"maxProviderCalls":5,"requiredImageHashes":3},
            "roleModels":{"creator":"gemini-3.8-flash","repair":"gemini-4-argon","reviewer":"gemini-4-argon"}
        });
        let output = run(input).unwrap();
        let pipeline = &output["pipeline"];
        assert_eq!(pipeline["contract"]["deadlineMs"],401000);
        assert_eq!(pipeline["contract"]["limits"]["maxProviderCalls"],5);
        assert_eq!(pipeline["contract"]["roleModels"]["creator"],"gemini-3.8-flash");
        assert_eq!(pipeline["contract"]["roleModels"]["repair"],"gemini-4-argon");
        assert_eq!(pipeline["contract"]["roleModels"]["reviewer"],"gemini-4-argon");
    }

    #[test]
    fn create_freezes_optional_component_edit_descriptor_with_its_baseline() {
        let theme = json!({
            "id":"custom-forest","name":"Forest relay","genre":"Frontier","description":"A forest sensor relay.",
            "biome":"Moss forest","shapeLanguage":"Arched timber","materialFinish":"Weathered wood","weathering":"Moss at joints",
            "technology":"Sensor relay","motifs":["seed pods"],"lighting":"Soft green","scale":"Human scale",
            "gameplayRole":"Route marker","exclusions":["military insignia"],"visualChecks":["Entry remains visible"],
            "assumptions":[],"renderingNotes":["Texture is contract metadata"],"referencePreset":"sylvan",
            "palette":{"shell":"#b4bd92","frame":"#615444","accent":"#ffc577","ground":"#3f614e","rock":"#788475"},
            "sky":"#b9cdb1","flora":"tree","architecture":"organic"
        });
        let recipe = living_world_core::Recipe {
            theme_id:"custom-forest".into(), name:"Forest relay".into(), description:"A forest sensor relay.".into(), sensor_radius:12.0,
            parts:(0..4).map(|i| living_world_core::Part {
                id:format!("post-{i}"), shape:"box".into(), role:"support".into(),
                position:[if i%2==0{-2.0}else{2.0},1.5,if i<2{-2.0}else{2.0}], size:[0.5,3.0,0.5], material:"frame".into(),
            }).collect(),
            scene:Some(living_world_core::RecipeScene { half_extent:12.0, components:vec![living_world_core::SceneComponent {
                id:"main-outpost".into(),name:"Main outpost".into(),role:"sensor structure".into(),part_ids:(0..4).map(|i|format!("post-{i}")).collect(),
            }] }),
        };
        let candidate_hash = living_world_core::pipeline::sha256_json(&recipe).unwrap();
        let input = json!({
            "op":"create","now":20,"theme":theme,"prompt":"Modify the relay component",
            "refinement":{
                "userFeedback":"Change the target geometry only",
                "baselineRecipe":recipe,
                "baselineEvidence":{"candidateSha256":candidate_hash,"imageSha256":["1".repeat(64),"2".repeat(64),"3".repeat(64)],"imageUrls":[]},
                "componentEdit":{"mode":"modify","componentId":"main-outpost"}
            }
        });
        let output = run(input).unwrap();
        assert_eq!(output["pipeline"]["contract"]["refinement"]["componentEdit"]["mode"],"modify");
        assert_eq!(output["pipeline"]["contract"]["refinement"]["componentEdit"]["componentId"],"main-outpost");
    }

    #[test]
    fn prepare_generation_routes_frozen_feedback_without_credentials_or_network() {
        let input=json!({
            "op":"prepare-generation",
            "request":{
                "theme":{
                    "id":"custom-forest","name":"Forest relay","genre":"Frontier","description":"A forest sensor relay.",
                    "biome":"Moss forest","shapeLanguage":"Arched timber","materialFinish":"Weathered wood","weathering":"Moss at joints",
                    "technology":"Sensor relay","motifs":["seed pods"],"lighting":"Soft green","scale":"Human scale",
                    "gameplayRole":"Route marker","exclusions":["military insignia"],"visualChecks":["Entry remains visible"],
                    "assumptions":[],"renderingNotes":["Texture is contract metadata"],"referencePreset":"sylvan",
                    "palette":{"shell":"#b4bd92","frame":"#615444","accent":"#ffc577","ground":"#3f614e","rock":"#788475"},
                    "sky":"#b9cdb1","flora":"tree","architecture":"organic"
                },
                "prompt":"Build the Forest Relay","userFeedback":"Make the arch unmistakable; keep its entrance open.",
                "sceneHalfExtent":10.0,"model":"gemini-test","agent":"test-agent","managed":false
            }
        });
        let mut input=input;
        use base64::Engine as _;
        let reference_bytes=b"\x89PNG\r\n\x1a\nreference";
        let reference=base64::engine::general_purpose::STANDARD.encode(reference_bytes);
        input["request"]["referenceImages"]=json!([{"mimeType":"image/png","data":reference}]);
        input["request"]["referenceConsent"]=json!(true);
        input["request"]["referenceSha256"]=json!(living_world_core::pipeline::sha256(reference_bytes));
        input["request"]["referenceMimeType"]=json!("image/png");
        let prepared=run(input).unwrap();
        assert!(prepared["endpoint"].as_str().unwrap().ends_with("/models/gemini-test:generateContent"));
        let prompt=prepared["body"]["contents"][0]["parts"][0]["text"].as_str().unwrap();
        assert!(prompt.contains("Make the arch unmistakable"));
        assert!(prompt.contains("halfExtent of 10 metres"));
        assert_eq!(prepared["body"]["contents"][0]["parts"][1]["inlineData"]["mimeType"],"image/png");
        assert_eq!(prepared["body"]["contents"][0]["parts"][1]["inlineData"]["data"],"[redacted]");
        assert_eq!(prepared["referenceImages"][0]["mimeType"],"image/png");
        assert_eq!(prepared["referenceImages"][0]["sha256"],living_world_core::pipeline::sha256(reference_bytes));
        assert_eq!(prepared["referenceImages"][0]["consent"],true);
        assert!(prepared["body"].get("key").is_none());
    }
}

#[cfg(all(test,feature="live-provider"))]
mod tests{
    use super::*;

    #[test]
    fn provider_output_preserves_raw_received_response_without_turning_it_into_transport_failure(){
        let raw=json!({"id":"interaction-7","status":"completed","usage":{"total_tokens":123},"environment_id":"env-2","steps":[{"type":"model_output","content":[{"type":"text","text":"unparseable recipe"}]}],"agent":"managed-agent"});
        let envelope=provider_response_output("Managed agent returned invalid recipe".into(),raw.clone());
        assert_eq!(envelope["providerError"]["message"],"Managed agent returned invalid recipe");
        assert_eq!(envelope["providerError"]["response"],raw);
    }
}
