//! Public event presentation. Survey drones are explicitly labeled fixture units.
//! This module is a child of client, beside visual, and reads verified snapshots.
use super::{ClientState, network_tick};
use argon_dev::events::{Phase, PublicEvent};
use bevy::{
    asset::RenderAssetUsages, mesh::Indices, prelude::*,
    render::render_resource::PrimitiveTopology, text::FontSize,
};
use std::collections::HashSet;

pub(super) fn install(app: &mut App) {
    app.add_systems(Startup, setup).add_systems(
        Update,
        (reconcile, warning_trails, bulletin).after(network_tick),
    );
}

#[derive(Component)]
struct EventMarker {
    id: u64,
    phase: Phase,
    mesh: Handle<Mesh>,
}
#[derive(Component)]
struct Drone(String);
#[derive(Component)]
struct Bulletin;
#[derive(Resource)]
struct Palette {
    warning: Handle<StandardMaterial>,
    impact: Handle<StandardMaterial>,
    cooled: Handle<StandardMaterial>,
    drone_mesh: Handle<Mesh>,
    faction: [Handle<StandardMaterial>; 3],
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let luminous = |color: Color| StandardMaterial {
        base_color: color,
        emissive: LinearRgba::from(color) * 1.4,
        perceptual_roughness: 0.8,
        ..default()
    };
    commands.insert_resource(Palette {
        warning: materials.add(luminous(Color::srgb(1.0, 0.42, 0.08))),
        impact: materials.add(StandardMaterial {
            base_color: Color::srgb(0.13, 0.055, 0.025),
            emissive: LinearRgba::rgb(0.7, 0.1, 0.015),
            perceptual_roughness: 0.98,
            ..default()
        }),
        cooled: materials.add(StandardMaterial {
            base_color: Color::srgb(0.09, 0.10, 0.11),
            perceptual_roughness: 0.98,
            ..default()
        }),
        drone_mesh: meshes.add(Sphere::new(1.0).mesh().ico(1).unwrap()),
        faction: [
            Color::srgb(1.0, 0.77, 0.27),
            Color::srgb(0.2, 0.8, 0.95),
            Color::srgb(0.95, 0.3, 0.22),
        ]
        .map(|c| materials.add(luminous(c))),
    });
    commands.spawn((
        Text::new(""),
        TextFont {
            font_size: FontSize::Px(16.0),
            ..default()
        },
        TextColor(Color::srgb(1.0, 0.85, 0.65)),
        Node {
            position_type: PositionType::Absolute,
            right: px(22),
            top: px(20),
            max_width: px(380),
            padding: UiRect::all(px(16)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.045, 0.035, 0.025, 0.92)),
        Visibility::Hidden,
        Bulletin,
    ));
}

fn position(state: &ClientState, x: f64, y: f64) -> Vec3 {
    if let Some(terrain) = &state.terrain {
        Vec3::new(
            x as f32,
            terrain.sample(x, y).map_or(0.0, |s| s.height),
            y as f32,
        )
    } else {
        Vec3::new((x as f32 - 500.0) * 0.1, 0.0, (y as f32 - 300.0) * 0.1)
    }
}

fn scar(state: &ClientState, e: &PublicEvent) -> Mesh {
    let scale = if state.terrain.is_some() { 1.0 } else { 0.1 };
    let mut points = vec![];
    let mut colors = vec![];
    let mut indices = vec![];
    for ring in 0..=3 {
        let radius = [0.0, 0.72, 0.91, 1.0][ring];
        for segment in 0..=48 {
            let angle = segment as f64 * std::f64::consts::TAU / 48.0;
            let x = e.x + angle.cos() * e.radius * radius;
            let y = e.y + angle.sin() * e.radius * radius;
            let lift = if e.phase == Phase::Warning {
                0.25
            } else {
                [0.10, 0.3, 1.4, 0.12][ring]
            };
            let p = position(state, x, y) + Vec3::Y * lift * scale;
            points.push(p.to_array());
            let bright = match e.phase {
                Phase::Warning => {
                    if ring < 2 {
                        0.0
                    } else {
                        1.0
                    }
                }
                Phase::Impact => [0.10, 0.25, 1.0, 0.16][ring],
                Phase::Cooled => [0.35, 0.5, 1.0, 0.65][ring],
            };
            colors.push([bright, bright, bright, 1.0]);
        }
    }
    let first_ring = if e.phase == Phase::Warning { 2 } else { 0 };
    for ring in first_ring..3 {
        for segment in 0..48 {
            let a = ring * 49 + segment;
            indices.extend_from_slice(&[a, a + 50, a + 49, a, a + 1, a + 50]);
        }
    }
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, points)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 1.0, 0.0]; 196])
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors)
    .with_inserted_indices(Indices::U32(indices))
}

fn reconcile(
    mut commands: Commands,
    state: Res<ClientState>,
    palette: Res<Palette>,
    mut meshes: ResMut<Assets<Mesh>>,
    markers: Query<(Entity, &EventMarker)>,
    mut drones: Query<(Entity, &Drone, &mut Transform)>,
    time: Res<Time>,
) {
    let scale = if state.terrain.is_some() { 1.0 } else { 0.1 };
    let mut existing = HashSet::new();
    for (entity, marker) in &markers {
        if state
            .events
            .events
            .iter()
            .any(|e| e.id == marker.id && e.phase == marker.phase)
        {
            existing.insert(marker.id);
        } else {
            commands.entity(entity).despawn();
            meshes.remove(marker.mesh.id());
        }
    }
    for e in &state.events.events {
        if !existing.contains(&e.id) {
            let mesh = meshes.add(scar(&state, e));
            let material = match e.phase {
                Phase::Warning => &palette.warning,
                Phase::Impact => &palette.impact,
                Phase::Cooled => &palette.cooled,
            };
            commands.spawn((
                EventMarker {
                    id: e.id,
                    phase: e.phase,
                    mesh: mesh.clone(),
                },
                Mesh3d(mesh),
                MeshMaterial3d(material.clone()),
                Transform::default(),
            ));
        }
    }
    let mut present = HashSet::new();
    for (entity, drone, mut transform) in &mut drones {
        if let Some(unit) = state.events.units.iter().find(|u| u.faction == drone.0) {
            let p = position(&state, unit.x, unit.y)
                + Vec3::Y * (5.0 + time.elapsed_secs().sin() * 0.4) * scale;
            transform.translation = transform
                .translation
                .lerp(p, (time.delta_secs() * 10.0).min(1.0));
            transform.rotate_y(time.delta_secs());
            present.insert(drone.0.clone());
        } else {
            commands.entity(entity).despawn();
        }
    }
    for unit in state
        .events
        .units
        .iter()
        .filter(|u| !present.contains(&u.faction))
    {
        let i = argon_dev::events::FACTIONS
            .iter()
            .position(|f| *f == unit.faction)
            .unwrap_or(0);
        commands.spawn((
            Drone(unit.faction.clone()),
            Mesh3d(palette.drone_mesh.clone()),
            MeshMaterial3d(palette.faction[i].clone()),
            Transform::from_translation(position(&state, unit.x, unit.y) + Vec3::Y * 5.0 * scale)
                .with_scale(Vec3::new(2.0, 0.6, 2.0) * scale),
        ));
    }
}

fn warning_trails(state: Res<ClientState>, mut gizmos: Gizmos) {
    let scale = if state.terrain.is_some() { 1.0 } else { 0.1 };
    for e in state
        .events
        .events
        .iter()
        .filter(|e| e.phase == Phase::Warning)
    {
        let ground = position(&state, e.x, e.y);
        let seconds = e.impact_at.saturating_sub(state.events.now) as f32;
        let height = seconds.min(20.0) * 5.0 * scale;
        gizmos.line(
            ground + Vec3::Y * height,
            ground + Vec3::new(-12.0, 25.0, 8.0) * scale + Vec3::Y * height,
            Color::srgb(1.0, 0.7, 0.2),
        );
    }
}

fn bulletin(
    state: Res<ClientState>,
    mut text: Query<(&mut Text, &mut Visibility), With<Bulletin>>,
) {
    for (mut text, mut visible) in &mut text {
        if state.events.schema_version == 0 {
            *visible = Visibility::Hidden;
            continue;
        }
        *visible = Visibility::Visible;
        let latest = state.events.events.last();
        let message=latest.map(|e|match e.phase {
            Phase::Warning=>format!("METEOR WARNING\nImpact in {}s | {:.0}, {:.0}\nClear the marked perimeter.",e.impact_at.saturating_sub(state.events.now),e.x,e.y),
            Phase::Impact=>format!("METEOR IMPACT\nPassage blocked | {:.0}m radius\nCools in {}s; travel around the rim.",e.radius,e.clear_at.saturating_sub(state.events.now)),
            Phase::Cooled=>"IMPACT COOLED\nPassage restored. The scar remains.".into(),
        }).unwrap_or_else(||format!("LIVING WORLD DEMO\nOrbital watch active | {}s",state.events.now));
        let responses = latest
            .map(|e| {
                state
                    .events
                    .responses
                    .iter()
                    .filter(|r| r.event_id == e.id)
                    .map(|r| {
                        format!(
                            "{} | {}",
                            r.faction,
                            if r.completed {
                                "survey arrived"
                            } else {
                                "survey dispatched"
                            }
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .unwrap_or_default();
        text.0 =
            format!("{message}\n\n{responses}\nFixture aerial survey units | public broadcast");
    }
}
