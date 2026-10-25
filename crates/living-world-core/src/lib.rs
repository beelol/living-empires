//! Deterministic game-world contracts. Models propose recipes; this crate evaluates them.
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Recipe {
    pub theme_id: String,
    pub name: String,
    pub description: String,
    pub sensor_radius: f64,
    pub parts: Vec<Part>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scene: Option<RecipeScene>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecipeScene {
    pub half_extent: f64,
    pub components: Vec<SceneComponent>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SceneComponent {
    pub id: String,
    pub name: String,
    pub role: String,
    pub part_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct Part {
    pub id: String,
    pub shape: String,
    pub role: String,
    pub position: [f64; 3],
    pub size: [f64; 3],
    pub material: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ThemeContract {
    pub id: String,
    pub name: String,
    pub genre: String,
    pub description: String,
    pub biome: String,
    pub shape_language: String,
    pub material_finish: String,
    pub weathering: String,
    pub technology: String,
    pub motifs: Vec<String>,
    pub lighting: String,
    pub scale: String,
    pub gameplay_role: String,
    pub exclusions: Vec<String>,
    pub visual_checks: Vec<String>,
    pub assumptions: Vec<String>,
    pub rendering_notes: Vec<String>,
    pub reference_preset: ReferencePreset,
    pub palette: Palette,
    pub sky: String,
    pub flora: Flora,
    pub architecture: Architecture,
}

#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ReferencePreset { Argonaut, Sylvan, Ember }

#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Flora { Crystal, Tree, Basalt }

#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Architecture { Industrial, Organic, Fortified }

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Palette {
    pub shell: String,
    pub frame: String,
    pub accent: String,
    pub ground: String,
    pub rock: String,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
pub struct Position { pub x: f64, pub z: f64 }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Check { pub id: String, pub status: String, pub label: String, pub detail: String }

pub mod pipeline;
pub use pipeline::{AssetPipeline, ApprovedAssetPackage, Evidence, EvidenceStatus, FrozenJobContract, JobLimits, PipelineStatus, Provenance, ReviewDecision, ReviewVerdict};
pub mod provider;

pub fn validate_theme(theme: &ThemeContract) -> Result<(), String> {
    let slug = theme.id.as_bytes();
    if slug.is_empty() || slug.len() > 64 || !slug[0].is_ascii_lowercase()
        || !slug.iter().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || *b == b'-')
        || theme.id.ends_with('-') || theme.id.contains("--") {
        return Err("Theme id must be a lowercase hyphenated slug (1–64 characters)".into());
    }
    for (value, limit, label) in [
        (&theme.name, 100, "name"), (&theme.genre, 120, "genre"),
        (&theme.description, 2000, "description"), (&theme.biome, 200, "biome"),
        (&theme.shape_language, 500, "shapeLanguage"), (&theme.material_finish, 500, "materialFinish"),
        (&theme.weathering, 500, "weathering"), (&theme.technology, 500, "technology"),
        (&theme.lighting, 500, "lighting"), (&theme.scale, 300, "scale"),
        (&theme.gameplay_role, 500, "gameplayRole"),
    ] {
        if value.trim().is_empty() || value.len() > limit {
            return Err(format!("Theme {label} must be non-empty and at most {limit} characters"));
        }
    }
    for (values, min, label) in [
        (&theme.motifs, 1, "motifs"), (&theme.exclusions, 1, "exclusions"),
        (&theme.visual_checks, 1, "visualChecks"), (&theme.rendering_notes, 1, "renderingNotes"),
        (&theme.assumptions, 0, "assumptions"),
    ] {
        if values.len() < min || values.len() > 32 || values.iter().any(|v| v.trim().is_empty() || v.len() > 500) {
            return Err(format!("Theme {label} must contain {min}–32 non-empty entries of at most 500 characters"));
        }
    }
    for color in [&theme.palette.shell, &theme.palette.frame, &theme.palette.accent,
        &theme.palette.ground, &theme.palette.rock, &theme.sky] {
        if color.len() != 7 || !color.starts_with('#') || !color.as_bytes()[1..].iter().all(u8::is_ascii_hexdigit) {
            return Err("Theme palette colors and sky must use #RRGGBB hex".into());
        }
    }
    Ok(())
}

pub fn validate(recipe: &Recipe, frozen_theme: &str) -> Result<(), String> {
    if recipe.theme_id != frozen_theme || !["argonaut", "sylvan", "ember"].contains(&frozen_theme) {
        return Err("Candidate changed the frozen theme".into());
    }
    validate_recipe(recipe)
}

fn validate_recipe(recipe: &Recipe) -> Result<(), String> {
    if recipe.name.trim().is_empty() || recipe.name.len() > 100 || recipe.description.len() > 2000 {
        return Err("Invalid recipe identity".into());
    }
    if !recipe.sensor_radius.is_finite() || !(1.0..=20.0).contains(&recipe.sensor_radius) {
        return Err("Sensor radius must be 1–20 metres".into());
    }
    let max_parts = if recipe.scene.is_some() { 128 } else { 48 };
    if !(4..=max_parts).contains(&recipe.parts.len()) { return Err(format!("Provide 4–{max_parts} parts")); }
    let extent = if let Some(scene) = &recipe.scene {
        if !scene.half_extent.is_finite() || !(4.0..=16.0).contains(&scene.half_extent) {
            return Err("Scene halfExtent must be 4–16 metres".into());
        }
        if scene.components.is_empty() || scene.components.len() > 32 {
            return Err("Scene must contain 1–32 components".into());
        }
        let mut component_ids = HashSet::new();
        let mut covered = HashSet::new();
        for component in &scene.components {
            let slug = component.id.as_bytes();
            if slug.is_empty() || slug.len() > 64 || !slug[0].is_ascii_lowercase()
                || !slug.iter().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || *b == b'-')
                || component.id.ends_with('-') || component.id.contains("--")
                || !component_ids.insert(&component.id) {
                return Err("Scene component IDs must be unique lowercase hyphenated slugs".into());
            }
            if component.name.trim().is_empty() || component.name.len() > 100
                || component.role.trim().is_empty() || component.role.len() > 80
                || component.part_ids.is_empty() {
                return Err("Scene component names, roles, and part membership must be bounded and non-empty".into());
            }
            for id in &component.part_ids {
                if !covered.insert(id) { return Err("Each part must belong to exactly one scene component".into()); }
            }
        }
        if covered.len() != recipe.parts.len() || recipe.parts.iter().any(|p| !covered.contains(&p.id)) {
            return Err("Scene components must cover every part exactly once".into());
        }
        Some(scene.half_extent)
    } else { None };
    let mut ids = HashSet::new();
    for p in &recipe.parts {
        if p.id.len() > 80 || !ids.insert(&p.id) { return Err("Part IDs must be unique".into()); }
        if !["box", "cylinder", "cone"].contains(&p.shape.as_str())
            || !["wall", "support", "roof", "detail"].contains(&p.role.as_str())
            || !["shell", "frame", "accent"].contains(&p.material.as_str()) {
            return Err("Unsupported geometry or theme material".into());
        }
        if p.position.iter().any(|v| !v.is_finite()) || p.position[1].abs() > 10.0
            || p.position[0].abs() > 28.0 || p.position[2].abs() > 28.0
            || p.size.iter().any(|v| !v.is_finite() || *v <= 0.0 || *v > 12.0) {
            return Err("Invalid build envelope or dimensions".into());
        }
        let bound = extent.unwrap_or(4.0) + 0.05;
        if p.position[0].abs() + p.size[0]/2.0 > bound || p.position[2].abs() + p.size[2]/2.0 > bound {
            return Err(if extent.is_some() { "Part exceeds scene halfExtent".into() } else { "Part exceeds legacy ±4m outpost bounds".into() });
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
    Ok(evaluate_validated(recipe))
}

pub fn evaluate_with_theme(recipe: &Recipe, theme: &ThemeContract) -> Result<Vec<Check>, String> {
    validate_theme(theme)?;
    if recipe.theme_id != theme.id {
        return Err("Recipe theme id does not match its validated theme contract".into());
    }
    validate_recipe(recipe)?;
    Ok(evaluate_validated(recipe))
}

fn evaluate_validated(recipe: &Recipe) -> Vec<Check> {
    let (access, _)=access_scenario(recipe);
    let supports: Vec<_>=recipe.parts.iter().filter(|p| ["wall","support"].contains(&p.role.as_str())).collect();
    let grounded=!supports.is_empty() && supports.iter().all(|p|(p.position[1]-p.size[1]/2.0).abs()<0.08);
    let check=|id:&str,label:&str,pass:bool,detail:&str|Check{id:id.into(),label:label.into(),status:if pass{"pass".into()}else{"fail".into()},detail:detail.into()};
    vec![
        check("THEME-01","Frozen theme & material palette",true,"Theme identifier and palette tokens match the frozen contract."),
        check("GEN-05.access","Commander can enter",access,if access{"Collision-aware movement reaches the interior."}else{"Front-to-centre path is blocked. Clear a corridor for a 0.56m-wide commander."}),
        check("GEN-06.support","Structure meets the ground",grounded,"Wall/support bases must meet ground within 8cm."),
        check("GEN-05.sensor","Sensor detects the test target",recipe.sensor_radius>=8.0,"Test target is 8 metres east of the outpost."),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    fn recipe() -> Recipe {
        let parts=(0..4).map(|i|Part{id:format!("post-{i}"),shape:"box".into(),role:"support".into(),position:[if i%2==0{-2.0}else{2.0},1.5,if i<2{-2.0}else{2.0}],size:[0.5,3.0,0.5],material:"frame".into()}).collect();
        Recipe{theme_id:"argonaut".into(),name:"Test outpost".into(),description:"A sensor outpost".into(),sensor_radius:12.0,parts,scene:None}
    }
    #[test] fn open_entrance_is_reachable(){assert!(access_scenario(&recipe()).0);}
    #[test] fn blocked_entrance_prevents_movement(){let mut r=recipe();r.parts.push(Part{id:"blocker".into(),shape:"box".into(),role:"wall".into(),position:[0.0,1.0,2.5],size:[2.0,2.0,0.5],material:"frame".into()});assert!(!access_scenario(&r).0);}
    #[test] fn theme_drift_is_rejected(){assert!(validate(&recipe(),"sylvan").is_err());}
    #[test] fn arbitrary_material_is_rejected(){let mut r=recipe();r.parts[0].material="#ff00ff".into();assert!(validate(&r,"argonaut").is_err());}
    #[test] fn floating_support_fails_gate(){let mut r=recipe();r.parts[0].position[1]+=0.5;assert_eq!(evaluate(&r,"argonaut").unwrap()[2].status,"fail");}
    #[test] fn short_sensor_fails_gate(){let mut r=recipe();r.sensor_radius=2.0;assert_eq!(evaluate(&r,"argonaut").unwrap()[3].status,"fail");}
    #[test] fn legacy_recipe_keeps_its_bounded_extent_and_omits_scene_on_serialize(){
        let mut r=recipe();r.parts[0].position[0]=4.0;
        assert!(validate(&r,"argonaut").is_err());
        r.parts[0].position[0]=2.0;
        let value=serde_json::to_value(r).unwrap();
        assert!(value.get("scene").is_none());
    }
    #[test] fn bounded_scene_accepts_more_than_legacy_part_limit(){
        let mut r=recipe();
        for i in 4..50 { r.parts.push(Part{id:format!("detail-{i}"),shape:"box".into(),role:"detail".into(),position:[(i as f64%7.0)-3.0,0.5,(i as f64%5.0)-2.0],size:[0.2,0.2,0.2],material:"accent".into()}); }
        r.scene=Some(RecipeScene{half_extent:8.0,components:vec![SceneComponent{id:"main-outpost".into(),name:"Main outpost".into(),role:"sensor station".into(),part_ids:r.parts.iter().map(|p|p.id.clone()).collect()}]});
        assert!(validate(&r,"argonaut").is_ok());
    }
    #[test] fn scene_membership_must_cover_each_part_once(){
        let mut r=recipe();
        r.scene=Some(RecipeScene{half_extent:8.0,components:vec![SceneComponent{id:"main-outpost".into(),name:"Main outpost".into(),role:"sensor station".into(),part_ids:r.parts.iter().take(3).map(|p|p.id.clone()).collect()}]});
        assert!(validate(&r,"argonaut").unwrap_err().contains("cover every part"));
    }
}
