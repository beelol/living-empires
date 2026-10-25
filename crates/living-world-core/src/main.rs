use std::io::{self, Read};
use living_world_core::{Recipe,evaluate,access_scenario};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all="camelCase")]
struct Request {recipe:Recipe,theme_id:String}

fn main(){
    let mut input=String::new();
    if io::stdin().take(100_000).read_to_string(&mut input).is_err(){eprintln!("Unable to read recipe");std::process::exit(1);}
    let result=serde_json::from_str::<Request>(&input).map_err(|e|e.to_string()).and_then(|r|{
        let checks=evaluate(&r.recipe,&r.theme_id)?;
        let(pass,trace)=access_scenario(&r.recipe);
        Ok(serde_json::json!({"checks":checks,"movement":{"pass":pass,"trace":trace},"engine":"living-world-core/0.1.0"}))
    });
    match result{Ok(value)=>println!("{value}"),Err(error)=>{println!("{}",serde_json::json!({"error":error}));std::process::exit(1);}}
}
