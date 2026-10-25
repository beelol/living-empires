use crate::{Recipe, SceneComponent};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, PartialEq)]
pub enum EditScope { Append { component_id: String }, Modify { component_id: String } }
#[derive(Debug, Clone, PartialEq)]
pub enum ComponentDelta { Added { component: SceneComponent }, Modified { before: SceneComponent, after: SceneComponent } }

fn component<'a>(r: &'a Recipe, id: &str) -> Option<&'a SceneComponent> {
    r.scene.as_ref()?.components.iter().find(|c| c.id == id)
}
fn owned<'a>(r: &'a Recipe, c: &SceneComponent) -> HashMap<&'a str, &'a crate::Part> {
    let ids: HashSet<&str> = c.part_ids.iter().map(String::as_str).collect();
    r.parts.iter().filter(|p| ids.contains(p.id.as_str())).map(|p| (p.id.as_str(), p)).collect()
}
fn parts_outside<'a>(r: &'a Recipe, c: Option<&SceneComponent>) -> HashMap<&'a str, &'a crate::Part> {
    let ids: HashSet<&str> = c.into_iter().flat_map(|c| c.part_ids.iter().map(String::as_str)).collect();
    r.parts.iter().filter(|p| !ids.contains(p.id.as_str())).map(|p| (p.id.as_str(), p)).collect()
}
pub fn validate_component_edit(previous: &Recipe, current: &Recipe, scope: &EditScope) -> Result<ComponentDelta, String> {
    if previous.theme_id != current.theme_id || previous.name != current.name || previous.description != current.description || previous.sensor_radius != current.sensor_radius { return Err("Edit changed theme or unrelated recipe metadata".into()); }
    let a=previous.scene.as_ref().ok_or("Previous recipe has no scene")?; let b=current.scene.as_ref().ok_or("Current recipe has no scene")?;
    if a.half_extent != b.half_extent { return Err("Edit changed scene extent".into()); }
    let id=match scope { EditScope::Append{component_id}|EditScope::Modify{component_id}=>component_id.as_str() };
    let old=component(previous,id); let new=component(current,id).ok_or("Target component is missing")?;
    let old_ids: HashSet<_>=a.components.iter().map(|c|c.id.as_str()).collect();
    let new_ids: HashSet<_>=b.components.iter().map(|c|c.id.as_str()).collect();
    if old_ids.len()!=a.components.len() || new_ids.len()!=b.components.len() { return Err("Duplicate component IDs".into()); }
    let before_parts: HashMap<_,_>=previous.parts.iter().map(|p|(p.id.as_str(),p)).collect();
    let after_parts: HashMap<_,_>=current.parts.iter().map(|p|(p.id.as_str(),p)).collect();
    if before_parts.len()!=previous.parts.len() || after_parts.len()!=current.parts.len() { return Err("Duplicate part IDs".into()); }
    match scope {
      EditScope::Append{..} => {
        if old.is_some() || b.components.len()!=a.components.len()+1 || !a.components.iter().all(|c|b.components.contains(c)) { return Err("Append must preserve old components and add one unique component".into()); }
        if parts_outside(current,Some(new)) != before_parts { return Err("Append changed or removed an old part".into()); }
        let own=owned(current,new);
        if own.len()!=new.part_ids.len() || own.is_empty() || own.keys().any(|pid|before_parts.contains_key(pid)) { return Err("Appended component must own new unique part IDs".into()); }
        if after_parts.len()!=before_parts.len()+own.len() { return Err("Appended parts are not exclusively owned by the new component".into()); }
        Ok(ComponentDelta::Added{component:new.clone()})
      },
      EditScope::Modify{..} => {
        let old=old.ok_or("Target component does not exist")?;
        if a.components.len()!=b.components.len() || !a.components.iter().filter(|c|c.id!=id).all(|c|b.components.contains(c)) { return Err("Modify changed unrelated components".into()); }
        let outside_old=parts_outside(previous,Some(old)); let outside_new=parts_outside(current,Some(new));
        if outside_old!=outside_new { return Err("Modify changed parts outside the target component".into()); }
        let old_own=owned(previous,old); let new_own=owned(current,new);
        if new_own.len()!=new.part_ids.len() || new_own.is_empty() { return Err("Target component has invalid part membership".into()); }
        if old==new && old_own==new_own { return Err("No component edit was made".into()); }
        Ok(ComponentDelta::Modified{before:old.clone(),after:new.clone()})
      }
    }
}

#[cfg(test)]
mod tests {
 use super::*;
 use crate::{Part, RecipeScene};
 fn base()->Recipe { Recipe { theme_id:"argonaut".into(), name:"base".into(), description:"d".into(), sensor_radius:5.0, parts:vec![Part{id:"p1".into(),shape:"box".into(),role:"wall".into(),position:[0.0,0.0,0.0],size:[1.0,1.0,1.0],material:"shell".into()}], scene:Some(RecipeScene{half_extent:8.0,components:vec![SceneComponent{id:"core".into(),name:"Core".into(),role:"station".into(),part_ids:vec!["p1".into()]}]}) } }
 #[test] fn append_component_with_new_part_passes(){let a=base();let mut b=a.clone();b.parts.push(Part{id:"p2".into(),shape:"box".into(),role:"support".into(),position:[1.0,0.0,0.0],size:[1.0,1.0,1.0],material:"frame".into()});b.scene.as_mut().unwrap().components.push(SceneComponent{id:"tower".into(),name:"Tower".into(),role:"watch".into(),part_ids:vec!["p2".into()]});assert!(matches!(validate_component_edit(&a,&b,&EditScope::Append{component_id:"tower".into()}),Ok(ComponentDelta::Added{..})));}
 #[test] fn targeted_geometry_change_passes(){let a=base();let mut b=a.clone();b.parts[0].size[0]=2.0;assert!(matches!(validate_component_edit(&a,&b,&EditScope::Modify{component_id:"core".into()}),Ok(ComponentDelta::Modified{..})));}
 #[test] fn unrelated_part_change_fails(){let mut a=base();a.parts.push(Part{id:"p2".into(),shape:"box".into(),role:"support".into(),position:[1.0,0.0,0.0],size:[1.0,1.0,1.0],material:"frame".into()});a.scene.as_mut().unwrap().components.push(SceneComponent{id:"other".into(),name:"Other".into(),role:"base".into(),part_ids:vec!["p2".into()]});let mut b=a.clone();b.parts[1].size[0]=2.0;assert!(validate_component_edit(&a,&b,&EditScope::Modify{component_id:"core".into()}).is_err());}
 #[test] fn no_op_fails(){let a=base();assert!(validate_component_edit(&a,&a,&EditScope::Modify{component_id:"core".into()}).is_err());}
}
