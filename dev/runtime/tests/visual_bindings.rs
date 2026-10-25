use argon_dev::{settlement::{ApprovedVisualRef, Role}, world::World};

fn approved(id: &str, digit: char) -> ApprovedVisualRef {
    ApprovedVisualRef::from_verified_activation(id, 1, digit.to_string().repeat(64)).unwrap()
}

#[test]
fn existing_paid_building_visual_revision_survives_restart_once() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("world.json");
    let mut world = World::load_terrain(&path, Some(72913)).unwrap();
    world.initialize_settlements().unwrap();
    world.tick_settlements(120).unwrap();
    let id = world.plan_settlement("helios", "visual-bridge-generator", Role::Generator)
        .unwrap().unwrap();
    world.tick_settlements(120).unwrap();

    let before = world.buildings().into_iter().find(|b| b.id == id).unwrap();
    assert!(before.complete);
    let metal_before = world.faction_metal("helios").unwrap();
    let old_visual = approved("starter-generator", 'a');
    let next_visual = approved("accepted-generator", 'b');
    assert!(world.revise_building_visual(&id, "visual-activation-1", None, old_visual.clone()).unwrap());
    let path_after_first = std::fs::read(&path).unwrap();

    // A stale compare-and-swap and an unknown entity both reject without committing.
    assert!(world.revise_building_visual(&id, "visual-stale", None, next_visual.clone()).is_err());
    assert!(world.revise_building_visual("building-999999", "visual-free-spawn", None, next_visual.clone()).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), path_after_first);
    assert_eq!(world.faction_metal("helios"), Some(metal_before));

    assert!(world.revise_building_visual(&id, "visual-activation-2", Some(&old_visual), next_visual.clone()).unwrap());
    let after = world.buildings().into_iter().find(|b| b.id == id).unwrap();
    assert_eq!(after.visual_asset, Some(next_visual.clone()));
    assert_eq!(after.id, before.id);
    assert_eq!(after.faction_id, before.faction_id);
    assert_eq!(after.species, before.species);
    assert_eq!(after.role, before.role);
    assert_eq!(after.center, before.center);
    assert_eq!(after.ground_height, before.ground_height);
    assert_eq!(after.half_extents, before.half_extents);
    assert_eq!(after.entrance, before.entrance);
    assert_eq!(after.approach_bounds, before.approach_bounds);
    assert_eq!(after.content_hash, before.content_hash, "gameplay recipe remains unchanged");
    assert_eq!(after.completed_ticks, before.completed_ticks);
    assert_eq!(after.work_ticks, before.work_ticks);
    assert_eq!(after.complete, before.complete);
    assert_eq!(world.faction_metal("helios"), Some(metal_before));

    drop(world);
    let mut restored = World::load(&path).unwrap();
    let durable = restored.buildings().into_iter().find(|b| b.id == id).unwrap();
    assert_eq!(durable, after);
    assert!(!restored.revise_building_visual(
        &id, "visual-activation-2", Some(&old_visual), next_visual.clone()
    ).unwrap(), "the acknowledged operation replays without another write");
    assert!(restored.revise_building_visual(
        &id, "visual-activation-2", Some(&old_visual), approved("different", 'c')
    ).is_err(), "operation IDs cannot be reused with different terms");
    assert_eq!(restored.buildings().into_iter().find(|b| b.id == id).unwrap(), durable);
    assert_eq!(restored.faction_metal("helios"), Some(metal_before));
}

#[test]
fn malformed_visual_reference_cannot_enter_world_state() {
    assert!(ApprovedVisualRef::from_verified_activation("bad id", 1, "a".repeat(64)).is_err());
    assert!(ApprovedVisualRef::from_verified_activation("asset", 0, "a".repeat(64)).is_err());
    assert!(ApprovedVisualRef::from_verified_activation("asset", 1, "xyz").is_err());
}
