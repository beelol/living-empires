use std::io::{self, Read};
use living_world_core::{Recipe,ThemeContract,evaluate,evaluate_with_theme,access_scenario};
use living_world_core::pipeline::{AssetPipeline,Provenance,ReviewDecision,Evidence};
use living_world_core::provider::{fixture_recipe,repair_fixture};
use serde::de::DeserializeOwned;
use serde_json::{Value,json};

fn field<T:DeserializeOwned>(input:&Value,key:&str)->Result<T,String>{serde_json::from_value(input.get(key).cloned().ok_or_else(||format!("Missing {key}"))?).map_err(|e|format!("Invalid {key}: {e}"))}
fn run(input:Value)->Result<Value,String>{
    let now=input.get("now").and_then(Value::as_u64).unwrap_or(0);
    let op=input.get("op").and_then(Value::as_str).unwrap_or("validate");
    if op=="capabilities"{return Ok(json!({"engine":"living-world-core/0.1.0","nativeProvider":cfg!(feature="live-provider"),"pipelineVersion":1}));}
    if op=="create"{
        let mut pipeline=AssetPipeline::create(field(&input,"theme")?,field(&input,"prompt")?,now)?;
        if input.get("provenance").is_some(){pipeline.set_provenance(field::<Provenance>(&input,"provenance")?)?;}
        return Ok(json!({"pipeline":pipeline}));
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
            if op=="generate"{return generate_with_transport(&field::<GenerationRequest>(&input,"request")?,&key,&UreqTransport).map(|r|json!(r)).map_err(|e|e.to_string());}
            return review_with_transport(&field::<ReviewRequest>(&input,"request")?,&key,&UreqTransport).map(|r|json!(r)).map_err(|e|e.to_string());
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
