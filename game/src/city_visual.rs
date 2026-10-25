//! Simple authored starter settlements rendered only from authoritative snapshots.
//! Models stay within their paid collision footprint; asset Foundry can replace
//! their appearance later without changing city placement or entity identity.
use super::{ClientState, network_tick};
use argon_dev::settlement::{BuildingView, Role, Species};
use bevy::prelude::*;
use std::collections::HashMap;

pub(super) fn install(app: &mut App) {
    app.init_resource::<StarterModels>()
        .add_systems(Update, reconcile.after(network_tick));
}

#[derive(Component)]
struct CityBuilding(BuildingView);

#[derive(Resource)]
struct StarterModels {
    cube: Handle<Mesh>,
    sphere: Handle<Mesh>,
    cylinder: Handle<Mesh>,
    shell: [Handle<StandardMaterial>; 3],
    dark: Handle<StandardMaterial>,
    light: [Handle<StandardMaterial>; 3],
    ghost: Handle<StandardMaterial>,
}
impl FromWorld for StarterModels {
    fn from_world(world: &mut World) -> Self {
        let (cube, sphere, cylinder) = {
            let mut meshes = world.resource_mut::<Assets<Mesh>>();
            (
                meshes.add(Cuboid::new(1., 1., 1.)),
                meshes.add(Sphere::new(0.5).mesh().uv(20, 12)),
                meshes.add(Cylinder::new(0.5, 1.)),
            )
        };
        let mut materials = world.resource_mut::<Assets<StandardMaterial>>();
        let shell = [
            Color::srgb(0.73, 0.76, 0.69),
            Color::srgb(0.43, 0.56, 0.45),
            Color::srgb(0.59, 0.63, 0.66),
        ]
        .map(|color| {
            materials.add(StandardMaterial {
                base_color: color,
                perceptual_roughness: 0.68,
                ..default()
            })
        });
        let light = [
            Color::srgb(0.15, 0.67, 0.86),
            Color::srgb(0.58, 0.86, 0.33),
            Color::srgb(0.90, 0.51, 0.24),
        ]
        .map(|color| {
            materials.add(StandardMaterial {
                base_color: color,
                emissive: LinearRgba::from(color) * 1.7,
                ..default()
            })
        });
        Self {
            cube,
            sphere,
            cylinder,
            shell,
            light,
            dark: materials.add(StandardMaterial {
                base_color: Color::srgb(0.12, 0.16, 0.17),
                metallic: 0.65,
                perceptual_roughness: 0.55,
                ..default()
            }),
            ghost: materials.add(StandardMaterial {
                base_color: Color::srgba(0.38, 0.72, 0.86, 0.45),
                alpha_mode: AlphaMode::Blend,
                ..default()
            }),
        }
    }
}
fn family(species: Species) -> usize {
    match species {
        Species::Robot => 0,
        Species::Organic => 1,
        Species::Cyborg => 2,
    }
}

fn reconcile(
    mut commands: Commands,
    state: Res<ClientState>,
    models: Res<StarterModels>,
    current: Query<(Entity, &CityBuilding)>,
) {
    // Render no native geometry against an absent or mismatched terrain manifest.
    let ready = state.terrain.as_ref().is_some_and(|terrain| {
        state
            .terrain_info
            .as_ref()
            .is_some_and(|info| info.fingerprint == terrain.info().fingerprint)
    });
    let incoming: HashMap<_, _> = state
        .buildings
        .iter()
        .filter(|b| ready && valid_view(b))
        .map(|b| (b.id.as_str(), b))
        .collect();
    let mut retained = HashMap::new();
    for (entity, rendered) in &current {
        if incoming
            .get(rendered.0.id.as_str())
            .is_some_and(|view| same_model(view, &rendered.0))
        {
            retained.insert(rendered.0.id.as_str(), entity);
        } else {
            commands.entity(entity).despawn();
        }
    }
    for view in incoming.values() {
        if retained.contains_key(view.id.as_str()) {
            continue;
        }
        spawn_building(&mut commands, &models, view);
    }
}
fn valid_view(b: &BuildingView) -> bool {
    !b.id.is_empty()
        && b.center.iter().all(|v| v.is_finite())
        && b.ground_height.is_finite()
        && b.half_extents
            .iter()
            .all(|v| v.is_finite() && *v > 0. && *v <= 64.)
        && b.work_ticks > 0
        && b.completed_ticks <= b.work_ticks
}
fn same_model(a: &BuildingView, b: &BuildingView) -> bool {
    // Four construction stages avoid rebuilding every tick. The completed model
    // uses the exact persisted pose and content hash; no local completion timer.
    let stage = |v: &BuildingView| {
        if v.complete {
            4
        } else {
            v.completed_ticks.saturating_mul(4) / v.work_ticks.max(1)
        }
    };
    a.id == b.id
        && a.faction_id == b.faction_id
        && a.species == b.species
        && a.role == b.role
        && a.center == b.center
        && a.ground_height == b.ground_height
        && a.half_extents == b.half_extents
        && a.content_hash == b.content_hash
        && a.complete == b.complete
        && stage(a) == stage(b)
}
fn spawn_building(commands: &mut Commands, models: &StarterModels, b: &BuildingView) {
    let f = family(b.species);
    let width = b.half_extents[0] as f32 * 2.;
    let depth = b.half_extents[1] as f32 * 2.;
    let shell = if b.complete {
        models.shell[f].clone()
    } else {
        models.ghost.clone()
    };
    let progress = if b.complete {
        1.
    } else {
        (b.completed_ticks as f32 / b.work_ticks as f32).clamp(0.1, 0.95)
    };
    let height = match b.role {
        Role::Core => 6.,
        Role::Generator => 7.,
        Role::Storehouse => 4.,
    } * progress;
    let root = commands
        .spawn((
            CityBuilding(b.clone()),
            Transform::from_xyz(
                b.center[0] as f32,
                b.ground_height as f32,
                b.center[1] as f32,
            ),
            Visibility::default(),
            Name::new(format!("{} {:?}", b.faction_id, b.role)),
        ))
        .id();
    commands.entity(root).with_children(|parent| {
        let mut part =
            |mesh: Handle<Mesh>, material: Handle<StandardMaterial>, location: Vec3, size: Vec3| {
                parent.spawn((
                    Mesh3d(mesh),
                    MeshMaterial3d(material),
                    Transform::from_translation(location).with_scale(size),
                ));
            };
        // The authority's starter foundation supports up to 3m of relief. Its
        // solid skirt reaches below all validated ground rather than floating.
        part(
            models.cube.clone(),
            models.dark.clone(),
            Vec3::new(0., -1.55, 0.),
            Vec3::new(width, 3.2, depth),
        );
        match b.species {
            Species::Robot => {
                part(
                    models.cube.clone(),
                    shell.clone(),
                    Vec3::new(0., height * 0.45, 0.),
                    Vec3::new(width * 0.82, height * 0.9, depth * 0.82),
                );
                part(
                    models.cube.clone(),
                    models.dark.clone(),
                    Vec3::new(0., height * 0.95, 0.),
                    Vec3::new(width * 0.96, height * 0.1, depth * 0.96),
                );
                match b.role {
                    Role::Core => part(
                        models.cube.clone(),
                        models.light[f].clone(),
                        Vec3::new(0., height * 1.08, 0.),
                        Vec3::new(width * 0.2, height * 0.18, depth * 0.2),
                    ),
                    Role::Generator => {
                        part(
                            models.cylinder.clone(),
                            models.light[f].clone(),
                            Vec3::new(0., height * 0.7, 0.),
                            Vec3::new(width * 0.5, height * 0.6, depth * 0.5),
                        );
                        for x in [-0.32, 0.32] {
                            part(
                                models.cube.clone(),
                                models.dark.clone(),
                                Vec3::new(width * x, height * 0.65, 0.),
                                Vec3::new(width * 0.1, height * 0.9, depth * 0.62),
                            );
                        }
                    }
                    Role::Storehouse => {
                        for x in [-0.25, 0., 0.25] {
                            part(
                                models.cube.clone(),
                                models.dark.clone(),
                                Vec3::new(width * x, height * 0.45, -depth * 0.42),
                                Vec3::new(width * 0.035, height * 0.9, depth * 0.03),
                            );
                        }
                    }
                }
            }
            Species::Organic => match b.role {
                Role::Core => {
                    part(
                        models.sphere.clone(),
                        shell.clone(),
                        Vec3::new(0., height * 0.42, 0.),
                        Vec3::new(width * 0.95, height * 0.95, depth * 0.85),
                    );
                    for x in [-0.3, 0.3] {
                        part(
                            models.sphere.clone(),
                            models.dark.clone(),
                            Vec3::new(width * x, height * 0.3, depth * 0.16),
                            Vec3::new(width * 0.28, height * 0.62, depth * 0.48),
                        );
                    }
                }
                Role::Generator => {
                    part(
                        models.cylinder.clone(),
                        shell.clone(),
                        Vec3::new(0., height * 0.32, 0.),
                        Vec3::new(width * 0.36, height * 0.65, depth * 0.36),
                    );
                    part(
                        models.sphere.clone(),
                        models.light[f].clone(),
                        Vec3::new(0., height * 0.75, 0.),
                        Vec3::new(width * 0.76, height * 0.5, depth * 0.76),
                    );
                }
                Role::Storehouse => {
                    for x in [-0.25, 0.25] {
                        part(
                            models.sphere.clone(),
                            shell.clone(),
                            Vec3::new(width * x, height * 0.4, 0.),
                            Vec3::new(width * 0.46, height * 0.8, depth * 0.86),
                        );
                    }
                }
            },
            Species::Cyborg => {
                part(
                    models.sphere.clone(),
                    shell.clone(),
                    Vec3::new(0., height * 0.4, 0.),
                    Vec3::new(width * 0.9, height * 0.82, depth * 0.86),
                );
                for x in [-0.34, 0.34] {
                    part(
                        models.cube.clone(),
                        models.dark.clone(),
                        Vec3::new(width * x, height * 0.45, 0.),
                        Vec3::new(width * 0.12, height * 0.9, depth * 0.95),
                    );
                }
                let tower = match b.role {
                    Role::Core => 0.3,
                    Role::Generator => 0.55,
                    Role::Storehouse => 0.12,
                };
                part(
                    models.cylinder.clone(),
                    models.light[f].clone(),
                    Vec3::new(0., height * 0.82, 0.),
                    Vec3::new(width * 0.22, height * tower, depth * 0.22),
                );
            }
        }
        // A dark, unobstructed south access face; no decorative part occupies the
        // authority's entrance rectangle outside the footprint.
        part(
            models.cube.clone(),
            models.dark.clone(),
            Vec3::new(0., height * 0.2, -depth * 0.49),
            Vec3::new(1.8, height * 0.4, depth * 0.015),
        );
        part(
            models.cube.clone(),
            models.light[f].clone(),
            Vec3::new(0., height * 0.44, -depth * 0.49),
            Vec3::new(2.0, 0.12, depth * 0.015),
        );
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn progress_stages_change_only_from_authoritative_work() {
        let mut view:BuildingView=serde_json::from_value(serde_json::json!({"id":"b1","faction_id":"helios","species":"robot","role":"core","center":[10.,10.],"ground_height":2.,"half_extents":[4.,4.],"entrance":{"min":[9.,2.],"max":[11.,6.]},"approach_bounds":{"min":[8.,0.],"max":[12.,3.]},"content_hash":"a".repeat(64),"completed_ticks":0,"work_ticks":120,"complete":false})).unwrap();
        let before = view.clone();
        view.completed_ticks = 29;
        assert!(same_model(&before, &view));
        view.completed_ticks = 30;
        assert!(!same_model(&before, &view));
        view.complete = true;
        assert!(!same_model(&before, &view));
        view.work_ticks = 0;
        assert!(!valid_view(&view));
    }
}
