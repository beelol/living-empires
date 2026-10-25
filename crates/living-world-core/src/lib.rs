//! Deterministic game-world contracts. Models propose recipes; this crate evaluates them.
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Recipe {
    pub theme_id: String,
    pub name: String,
    pub description: String,
    pub sensor_radius: f64,
    pub parts: Vec<Part>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Part {
    pub id: String,
    pub shape: String,
    pub role: String,
    pub position: [f64; 3],
    pub size: [f64; 3],
    pub material: String,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
pub struct Position { pub x: f64, pub z: f64 }

#[derive(Debug, Serialize)]
pub struct Check { pub id: String, pub status: String, pub label: String, pub detail: String }

pub fn validate(recipe: &Recipe, frozen_theme: &str) -> Result<(), String> {
    if recipe.theme_id != frozen_theme || !["argonaut", "sylvan", "ember"].contains(&frozen_theme) {
        return Err("Candidate changed the frozen theme".into());
    }
    if recipe.name.trim().is_empty() || recipe.name.len() > 100 || recipe.description.len() > 2000 {
        return Err("Invalid recipe identity".into());
    }
    if !recipe.sensor_radius.is_finite() || !(1.0..=20.0).contains(&recipe.sensor_radius) {
        return Err("Sensor radius must be 1–20 metres".into());
    }
    if !(4..=48).contains(&recipe.parts.len()) { return Err("Provide 4–48 parts".into()); }
    let mut ids = HashSet::new();
    for p in &recipe.parts {
        if p.id.len() > 80 || !ids.insert(&p.id) { return Err("Part IDs must be unique".into()); }
        if !["box", "cylinder", "cone"].contains(&p.shape.as_str())
            || !["wall", "support", "roof", "detail"].contains(&p.role.as_str())
            || !["shell", "frame", "accent"].contains(&p.material.as_str()) {
            return Err("Unsupported geometry or theme material".into());
        }
        if p.position.iter().any(|v| !v.is_finite() || v.abs() > 10.0)
            || p.size.iter().any(|v| !v.is_finite() || *v <= 0.0 || *v > 12.0) {
            return Err("Invalid build envelope or dimensions".into());
        }
        if p.position[1] - p.size[1]/2.0 < -0.05 { return Err("Geometry intersects ground".into()); }
    }
    Ok(())
}

pub fn collides(recipe: &Recipe, point: Position) -> bool {
    recipe.parts.iter().any(|p| {
        p.position[1]-p.size[1]/2.0 < 1.5 && p.position[1]+p.size[1]/2.0 > 0.08
            && (point.x-p.position[0]).abs() < p.size[0]/2.0+0.28
            && (point.z-p.position[2]).abs() < p.size[2]/2.0+0.28
    })
}

pub fn move_commander(recipe: &Recipe, current: Position, target: Position, dt: f64) -> Position {
    let dx=target.x-current.x; let dz=target.z-current.z; let distance=dx.hypot(dz);
    if distance < 0.06 { return current; }
    let step=distance.min(4.0*dt.clamp(0.0, 0.05));
    let next=Position{x:current.x+dx/distance*step,z:current.z+dz/distance*step};
    if collides(recipe,next) { current } else { next }
}

pub fn access_scenario(recipe: &Recipe) -> (bool, Vec<Position>) {
    let mut point=Position{x:0.0,z:9.0}; let mut trace=vec![point];
    for i in 0..300 { point=move_commander(recipe,point,Position{x:0.0,z:0.0},1.0/60.0); if i%15==0 { trace.push(point); } }
    (point.x.hypot(point.z)<0.3, trace)
}

pub fn evaluate(recipe: &Recipe, theme: &str) -> Result<Vec<Check>, String> {
    validate(recipe,theme)?;
    let (access, _)=access_scenario(recipe);
    let supports: Vec<_>=recipe.parts.iter().filter(|p| ["wall","support"].contains(&p.role.as_str())).collect();
    let grounded=!supports.is_empty() && supports.iter().all(|p|(p.position[1]-p.size[1]/2.0).abs()<0.08);
    let check=|id:&str,label:&str,pass:bool,detail:&str|Check{id:id.into(),label:label.into(),status:if pass{"pass".into()}else{"fail".into()},detail:detail.into()};
    Ok(vec![
        check("THEME-01","Frozen theme & material palette",true,"Theme identifier and palette tokens match the frozen contract."),
        check("GEN-05.access","Commander can enter",access,if access{"Collision-aware movement reaches the interior."}else{"Front-to-centre path is blocked. Clear a corridor for a 0.56m-wide commander."}),
        check("GEN-06.support","Structure meets the ground",grounded,"Wall/support bases must meet ground within 8cm."),
        check("GEN-05.sensor","Sensor detects the test target",recipe.sensor_radius>=8.0,"Test target is 8 metres east of the outpost."),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    fn recipe() -> Recipe {
        let parts=(0..4).map(|i|Part{id:format!("post-{i}"),shape:"box".into(),role:"support".into(),position:[if i%2==0{-2.0}else{2.0},1.5,if i<2{-2.0}else{2.0}],size:[0.5,3.0,0.5],material:"frame".into()}).collect();
        Recipe{theme_id:"argonaut".into(),name:"Test outpost".into(),description:"A sensor outpost".into(),sensor_radius:12.0,parts}
    }
    #[test] fn open_entrance_is_reachable(){assert!(access_scenario(&recipe()).0);}
    #[test] fn blocked_entrance_prevents_movement(){let mut r=recipe();r.parts.push(Part{id:"blocker".into(),shape:"box".into(),role:"wall".into(),position:[0.0,1.0,2.5],size:[2.0,2.0,0.5],material:"frame".into()});assert!(!access_scenario(&r).0);}
    #[test] fn theme_drift_is_rejected(){assert!(validate(&recipe(),"sylvan").is_err());}
    #[test] fn arbitrary_material_is_rejected(){let mut r=recipe();r.parts[0].material="#ff00ff".into();assert!(validate(&r,"argonaut").is_err());}
    #[test] fn floating_support_fails_gate(){let mut r=recipe();r.parts[0].position[1]+=0.5;assert_eq!(evaluate(&r,"argonaut").unwrap()[2].status,"fail");}
    #[test] fn short_sensor_fails_gate(){let mut r=recipe();r.sensor_radius=2.0;assert_eq!(evaluate(&r,"argonaut").unwrap()[3].status,"fail");}
}
