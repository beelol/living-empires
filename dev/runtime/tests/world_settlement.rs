use argon_dev::{settlement::Role, world::World};
#[test]
fn paid_cities_survive_world_restart_and_failed_save() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("world.json");
    let mut world = World::load_terrain(&path, Some(72913)).unwrap();
    world.initialize_settlements().unwrap();
    assert_eq!(world.buildings().len(), 3);
    assert_eq!(world.faction_metal("helios"), Some(900));
    let before = world.buildings();
    drop(world);
    let mut world = World::load_terrain(&path, Some(72913)).unwrap();
    assert_eq!(world.buildings(), before);
    world.initialize_settlements().unwrap();
    assert_eq!(world.faction_metal("helios"), Some(900));
    world.tick_settlements(120).unwrap();
    let id = world
        .plan_settlement("helios", "generator-1", Role::Generator)
        .unwrap()
        .unwrap();
    assert_eq!(world.faction_metal("helios"), Some(840));
    assert_eq!(
        world
            .plan_settlement("helios", "generator-1", Role::Generator)
            .unwrap(),
        Some(id)
    );
    let before = world.buildings();
    drop(world);
    let mut world = World::load(&path).unwrap();
    assert_eq!(world.buildings(), before);
    let save = std::fs::read(&path).unwrap();
    std::fs::remove_file(&path).unwrap();
    std::fs::create_dir(&path).unwrap();
    assert!(world.tick_settlements(120).is_err());
    assert_eq!(world.buildings(), before);
    std::fs::remove_dir(&path).unwrap();
    std::fs::write(&path, save).unwrap();
}

#[test]
fn reserved_building_blocks_crossing_but_not_clear_approach() {
    use argon_dev::{settlement::SettlementState, terrain::Terrain};
    let terrain = Terrain::generate(72913);
    let mut state = SettlementState::new();
    let c = [1440.0, 1536.0];
    state
        .start(
            &terrain,
            "helios",
            "core",
            Role::Core,
            c,
            [1440.0, 1512.0],
            0,
        )
        .unwrap();
    assert!(state.blocks_movement([1430.0, 1536.0], [1450.0, 1536.0]));
    assert!(!state.blocks_movement([1430.0, 1520.0], [1450.0, 1520.0]));
    assert!(state.blocks_movement(c, c));
}

#[test]
fn three_civilizations_build_paid_core_generator_and_storehouse() {
 let temp=tempfile::tempdir().unwrap();let path=temp.path().join("world.json");
 let mut world=World::load_terrain(&path,Some(72913)).unwrap();
 world.initialize_settlements().unwrap();world.tick_settlements(120).unwrap();
 for faction in ["helios","tide","cinder"] {
  assert!(world.plan_settlement(faction,"starter-generator",Role::Generator).unwrap().is_some(),"generator blocked for {faction}");
  assert!(world.plan_settlement(faction,"starter-storehouse",Role::Storehouse).unwrap().is_some(),"storehouse blocked for {faction}");
  assert_eq!(world.faction_metal(faction),Some(800));
 }
 world.tick_settlements(120).unwrap();
 assert_eq!(world.buildings().len(),9);
 assert!(world.buildings().iter().all(|b|b.complete));
 let before=world.buildings();drop(world);
 let after=World::load(&path).unwrap().buildings(); for (a,b) in after.iter().zip(&before) { if a!=b {eprintln!("height bits {}: {} != {}",a.id,a.ground_height.to_bits(),b.ground_height.to_bits());} } assert_eq!(after,before);
}
