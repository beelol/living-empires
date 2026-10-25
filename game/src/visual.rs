//! Native terrain presentation and development controls. Release art still requires independent review.
#[path = "terrain/mod.rs"]
mod terrain;
use super::{ClientState, network_tick};
use bevy::{
    input::mouse::{MouseScrollUnit, MouseWheel},
    light::CascadeShadowConfigBuilder,
    prelude::*,
    text::FontSize,
    window::PrimaryWindow,
};
use std::{
    collections::HashSet,
    time::{Duration, Instant},
};

const LEGACY_SCALE: f32 = 0.1;

pub(super) fn install(app: &mut App) {
    terrain::install(app);
    app.insert_resource(ClearColor(Color::srgb(0.18, 0.27, 0.30)))
        .insert_resource(ViewState::default())
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (
                legacy_ground,
                camera_control,
                player_input,
                reconcile_units,
                update_overlay,
                capture_frame,
            )
                .chain()
                .after(network_tick),
        );
}

#[derive(Resource)]
struct ViewState {
    focus: Vec3,
    yaw: f32,
    pitch: f32,
    centered: bool,
    scene: Option<String>,
    distance: f32,
    target: Option<Vec2>,
    last_input: Instant,
    previous_position: Option<Vec2>,
    previous_magnitude: f32,
    step: f32,
}

impl Default for ViewState {
    fn default() -> Self {
        Self {
            focus: Vec3::new(0.0, 0.0, -4.0),
            yaw: 0.0,
            pitch: 0.80,
            centered: false,
            scene: None,
            distance: 62.0,
            target: None,
            last_input: Instant::now(),
            previous_position: None,
            previous_magnitude: 0.0,
            step: 12.0,
        }
    }
}

#[derive(Component)]
struct ArenaCamera;
#[derive(Component)]
struct Unit(String);
#[derive(Component)]
struct StatusText;
#[derive(Component)]
struct TargetMarker;
#[derive(Component)]
#[require(Visibility)]
struct LegacyGround;

/// Cached geometry stays usable across transport reconnects only while it matches
/// the current manifest. An in-progress native download is never a legacy world.
fn active_terrain(state: &ClientState) -> Option<&argon_dev::terrain::Terrain> {
    state.terrain.as_ref().filter(|terrain| {
        state
            .terrain_info
            .as_ref()
            .is_some_and(|info| info.fingerprint == terrain.info().fingerprint)
    })
}
fn active_scene(state: &ClientState) -> Option<String> {
    active_terrain(state)
        .map(|terrain| terrain.info().fingerprint)
        .or_else(|| (state.ready && state.terrain_info.is_none()).then(|| "legacy".into()))
}

fn position(state: &ClientState, x: f64, y: f64) -> Vec3 {
    if let Some(terrain) = &state.terrain {
        Vec3::new(
            x as f32,
            terrain.sample(x, y).map_or(0., |s| s.height),
            y as f32,
        )
    } else {
        Vec3::new(
            (x as f32 - 500.0) * LEGACY_SCALE,
            0.0,
            (y as f32 - 300.0) * LEGACY_SCALE,
        )
    }
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn((
        DirectionalLight {
            illuminance: 18000.0,
            color: Color::srgb(1.0, 0.90, 0.74),
            shadow_maps_enabled: true,
            ..default()
        },
        CascadeShadowConfigBuilder {
            first_cascade_far_bound: 32.,
            maximum_distance: 700.,
            ..default()
        }
        .build(),
        Transform::from_xyz(-30.0, 50.0, 20.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    commands.spawn((
        Camera3d::default(),
        DistanceFog {
            color: Color::srgb(0.18, 0.27, 0.30),
            falloff: FogFalloff::Linear {
                start: 380.,
                end: 800.,
            },
            ..default()
        },
        ArenaCamera,
        Transform::from_xyz(0.0, 50.0, 50.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    commands.spawn((
        Mesh3d(meshes.add(Cylinder::new(0.5, 0.025))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.2, 0.95, 0.8),
            emissive: LinearRgba::rgb(0.15, 0.7, 0.5),
            ..default()
        })),
        Transform::from_xyz(0.0, 0.04, 0.0),
        Visibility::Hidden,
        TargetMarker,
    ));
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px(22),
                top: px(20),
                padding: UiRect::all(px(16)),
                max_width: px(640),
                ..default()
            },
            BackgroundColor(Color::srgba(0.025, 0.045, 0.06, 0.92)),
        ))
        .with_child((
            Text::new("LIVING EMPIRES\nConnecting..."),
            TextFont {
                font_size: FontSize::Px(18.0),
                ..default()
            },
            TextColor(Color::srgb(0.87, 0.93, 0.92)),
            StatusText,
        ));
    commands.spawn((
        Text::new("WASD  Move commander     Right click  Destination     Space  Stop\nArrows  Pan     Q / E  Rotate     Scroll  Zoom     Home  Recenter\nPage Up / Down  Tilt     F12  Capture native view | Host owns terrain and movement"),
        TextFont { font_size: FontSize::Px(15.0), ..default() },
        TextColor(Color::srgb(0.72, 0.82, 0.82)),
        Node { position_type: PositionType::Absolute, left: px(22), bottom: px(20), padding: UiRect::all(px(14)), ..default() },
        BackgroundColor(Color::srgba(0.025, 0.045, 0.06, 0.92)),
    ));
}

fn legacy_ground(
    mut commands: Commands,
    state: Res<ClientState>,
    mut created: Local<bool>,
    mut ground: Query<&mut Visibility, With<LegacyGround>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let show = active_scene(&state).as_deref() == Some("legacy");
    for mut visibility in &mut ground {
        *visibility = if show {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
    if *created || !show {
        return;
    }
    *created = true;
    commands.spawn((
        LegacyGround,
        Mesh3d(meshes.add(Plane3d::default().mesh().size(100.0, 60.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.12, 0.17, 0.18),
            perceptual_roughness: 0.95,
            ..default()
        })),
    ));
    // The physical movement boundary is owned by the host, at 12..988 / 12..588.
    let border = materials.add(Color::srgb(0.3, 0.4, 0.4));
    for (dimensions, location) in [
        (Vec3::new(97.6, 0.06, 0.08), Vec3::new(0.0, 0.035, -28.8)),
        (Vec3::new(97.6, 0.06, 0.08), Vec3::new(0.0, 0.035, 28.8)),
        (Vec3::new(0.08, 0.06, 57.6), Vec3::new(-48.8, 0.035, 0.0)),
        (Vec3::new(0.08, 0.06, 57.6), Vec3::new(48.8, 0.035, 0.0)),
    ] {
        commands.spawn((
            LegacyGround,
            Mesh3d(meshes.add(Cuboid::from_size(dimensions))),
            MeshMaterial3d(border.clone()),
            Transform::from_translation(location),
        ));
    }
    // Ground marks give scale without pretending to implement terrain or collision.
    let mark = materials.add(Color::srgb(0.16, 0.22, 0.23));
    for x in -4..=4 {
        commands.spawn((
            LegacyGround,
            Mesh3d(meshes.add(Cuboid::new(0.025, 0.015, 56.0))),
            MeshMaterial3d(mark.clone()),
            Transform::from_xyz(x as f32 * 10.0, 0.012, 0.0),
        ));
    }
    for z in -2..=2 {
        commands.spawn((
            LegacyGround,
            Mesh3d(meshes.add(Cuboid::new(96.0, 0.015, 0.025))),
            MeshMaterial3d(mark.clone()),
            Transform::from_xyz(0.0, 0.012, z as f32 * 10.0),
        ));
    }
}

fn camera_control(
    mut view: ResMut<ViewState>,
    state: Res<ClientState>,
    keys: Res<ButtonInput<KeyCode>>,
    mut scroll: MessageReader<MouseWheel>,
    time: Res<Time>,
    mut camera: Query<(&mut Transform, &mut DistanceFog), With<ArenaCamera>>,
) {
    let scene = active_scene(&state);
    if view.scene != scene {
        view.scene = scene;
        view.centered = false;
        view.target = None;
        view.previous_position = None;
        view.previous_magnitude = 0.;
    }
    if view.scene.is_none() {
        return;
    }
    let dt = time.delta_secs().min(0.1);
    let native = active_terrain(&state).is_some();
    if !view.centered
        && state.ready
        && let Some(player) = &state.player
    {
        view.focus = position(&state, player.x, player.y);
        view.distance = if native { 150. } else { 62. };
        view.centered = true;
    }
    let mut pan = Vec3::ZERO;
    if keys.pressed(KeyCode::ArrowLeft) {
        pan.x -= 1.0;
    }
    if keys.pressed(KeyCode::ArrowRight) {
        pan.x += 1.0;
    }
    if keys.pressed(KeyCode::ArrowUp) {
        pan.z -= 1.0;
    }
    if keys.pressed(KeyCode::ArrowDown) {
        pan.z += 1.0;
    }
    let yaw = view.yaw;
    let speed = view.distance * 0.6;
    view.focus += Quat::from_rotation_y(yaw) * pan * dt * speed;
    if keys.pressed(KeyCode::KeyQ) {
        view.yaw += dt;
    }
    if keys.pressed(KeyCode::KeyE) {
        view.yaw -= dt;
    }
    for event in scroll.read() {
        let amount = match event.unit {
            MouseScrollUnit::Line => event.y,
            MouseScrollUnit::Pixel => event.y * 0.04,
        };
        view.distance = (view.distance * (1.0 - amount * 0.08)).clamp(
            if native { 14. } else { 8. },
            if native { 600. } else { 120. },
        );
    }
    if keys.just_pressed(KeyCode::Home)
        && let Some(player) = &state.player
    {
        view.focus = position(&state, player.x, player.y);
    }
    if keys.pressed(KeyCode::PageUp) {
        view.pitch = (view.pitch + dt * 0.6).min(1.35);
    }
    if keys.pressed(KeyCode::PageDown) {
        view.pitch = (view.pitch - dt * 0.6).max(0.30);
    }
    if let Some(terrain) = &state.terrain {
        let info = terrain.info();
        let extent = info.cells as f32 * info.cell_size;
        view.focus.x = view.focus.x.clamp(0., extent);
        view.focus.z = view.focus.z.clamp(0., extent);
        view.focus.y = terrain
            .sample(view.focus.x as f64, view.focus.z as f64)
            .map_or(0., |s| s.height.max(0.));
    } else {
        view.focus.x = view.focus.x.clamp(-55.0, 55.0);
        view.focus.z = view.focus.z.clamp(-35.0, 35.0);
    }
    for (mut transform, mut fog) in &mut camera {
        fog.falloff = FogFalloff::Linear {
            start: if native { view.distance + 220. } else { 150. },
            end: if native { view.distance + 540. } else { 250. },
        };
        let offset = Quat::from_rotation_y(view.yaw)
            * Vec3::new(0.0, view.pitch.sin(), view.pitch.cos())
            * view.distance;
        let mut eye = view.focus + offset;
        if let Some(terrain) = &state.terrain
            && let Some(surface) = terrain.sample(eye.x as f64, eye.z as f64)
        {
            eye.y = eye.y.max(surface.height + 3.);
        }
        *transform = Transform::from_translation(eye).looking_at(view.focus, Vec3::Y);
    }
}

fn player_input(
    mut state: ResMut<ClientState>,
    mut view: ResMut<ViewState>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    window: Query<&Window, With<PrimaryWindow>>,
    camera: Query<(&Camera, &GlobalTransform), With<ArenaCamera>>,
    mut marker: Query<(&mut Transform, &mut Visibility), With<TargetMarker>>,
) {
    if active_scene(&state).is_none() {
        view.target = None;
        for (_, mut visibility) in &mut marker {
            *visibility = Visibility::Hidden;
        }
        return;
    }
    if state.movement_error.take().is_some() {
        view.target = None;
    }
    if keys.just_pressed(KeyCode::Space) {
        view.target = None;
    }
    if mouse.just_pressed(MouseButton::Right)
        && let Ok(window) = window.single()
        && let Some(cursor) = window.cursor_position()
        && let Ok((camera, transform)) = camera.single()
        && let Ok(ray) = camera.viewport_to_world(transform, cursor)
    {
        if let Some(ground) = &state.terrain {
            if let Some(point) = terrain::pick(ground, ray) {
                view.target = Some(Vec2::new(point.x, point.z));
            }
        } else if state.ready
            && state.terrain_info.is_none()
            && let Some(point) =
                ray.plane_intersection_point(Vec3::ZERO, InfinitePlane3d::new(Vec3::Y))
            && point.x.abs() <= 50.
            && point.z.abs() <= 30.
        {
            view.target = Some(Vec2::new(
                (point.x / LEGACY_SCALE + 500.).clamp(12., 988.),
                (point.z / LEGACY_SCALE + 300.).clamp(12., 588.),
            ));
        }
    }
    for (mut transform, mut visible) in &mut marker {
        if let Some(target) = view.target {
            transform.translation =
                position(&state, target.x as f64, target.y as f64) + Vec3::Y * 0.04;
            *visible = Visibility::Visible;
        } else {
            *visible = Visibility::Hidden;
        }
    }
    let Some(player) = &state.player else {
        return;
    };
    let current = Vec2::new(player.x as f32, player.y as f32);
    if let Some(previous) = view.previous_position
        && previous != current
        && view.previous_magnitude > 0.0
    {
        view.step = (previous.distance(current) / view.previous_magnitude).clamp(0.01, 128.0);
        view.previous_magnitude = 0.0;
    }
    view.previous_position = Some(current);
    if view.last_input.elapsed() < Duration::from_millis(80) {
        return;
    }
    let mut direction = Vec2::ZERO;
    if keys.pressed(KeyCode::KeyW) {
        direction.y -= 1.0;
    }
    if keys.pressed(KeyCode::KeyS) {
        direction.y += 1.0;
    }
    if keys.pressed(KeyCode::KeyA) {
        direction.x -= 1.0;
    }
    if keys.pressed(KeyCode::KeyD) {
        direction.x += 1.0;
    }
    if direction.length_squared() > 0.0 {
        direction = direction.normalize();
        view.target = None;
    } else if let Some(target) = view.target {
        let delta = target - current;
        if delta.length() < 0.1 {
            view.target = None;
        } else {
            direction = delta / view.step.max(delta.length());
        }
    }
    if direction.length_squared() > 0.0 && state.queue_move(direction.x as f64, direction.y as f64)
    {
        view.last_input = Instant::now();
        view.previous_magnitude = direction.length();
    }
}

fn reconcile_units(
    mut commands: Commands,
    state: Res<ClientState>,
    time: Res<Time>,
    mut units: Query<(Entity, &Unit, &mut Transform, &mut Visibility)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    if active_scene(&state).is_none() {
        for (_, _, _, mut visibility) in &mut units {
            *visibility = Visibility::Hidden;
        }
        return;
    }
    let mut existing = HashSet::new();
    for (entity, unit, mut transform, mut visibility) in &mut units {
        *visibility = Visibility::Visible;
        if let Some(player) = state.players.iter().find(|p| p.id == unit.0) {
            let target = position(&state, player.x, player.y);
            transform.translation = transform
                .translation
                .lerp(target, (time.delta_secs() * 18.0).min(1.0));
            if let Some(terrain) = &state.terrain
                && let Some(surface) = terrain.sample(
                    transform.translation.x as f64,
                    transform.translation.z as f64,
                )
            {
                transform.translation.y = surface.height;
            }
            existing.insert(unit.0.clone());
        } else {
            commands.entity(entity).despawn();
        }
    }
    for player in state.players.iter().filter(|p| !existing.contains(&p.id)) {
        let own = state.player.as_ref().is_some_and(|own| own.id == player.id);
        let accent = if own {
            Color::srgb(0.15, 0.85, 0.72)
        } else {
            Color::srgb(0.95, 0.5, 0.24)
        };
        let ceramic = materials.add(StandardMaterial {
            base_color: Color::srgb(0.8, 0.84, 0.81),
            metallic: 0.15,
            perceptual_roughness: 0.6,
            ..default()
        });
        let graphite = materials.add(StandardMaterial {
            base_color: Color::srgb(0.08, 0.1, 0.12),
            metallic: 0.6,
            perceptual_roughness: 0.4,
            ..default()
        });
        let identity = materials.add(StandardMaterial {
            base_color: accent,
            emissive: LinearRgba::from(accent) * 0.3,
            ..default()
        });
        commands
            .spawn((
                Unit(player.id.clone()),
                Transform::from_translation(position(&state, player.x, player.y)),
                Visibility::default(),
            ))
            .with_children(|root| {
                for x in [-0.34, 0.34] {
                    root.spawn((
                        Mesh3d(meshes.add(Cuboid::new(0.45, 0.8, 0.55))),
                        MeshMaterial3d(graphite.clone()),
                        Transform::from_xyz(x, 0.4, 0.0),
                    ));
                }
                root.spawn((
                    Mesh3d(meshes.add(Cuboid::new(1.25, 0.85, 0.75))),
                    MeshMaterial3d(ceramic),
                    Transform::from_xyz(0.0, 1.2, 0.0),
                ));
                root.spawn((
                    Mesh3d(meshes.add(Cuboid::new(0.7, 0.45, 0.65))),
                    MeshMaterial3d(graphite),
                    Transform::from_xyz(0.0, 1.82, -0.02),
                ));
                root.spawn((
                    Mesh3d(meshes.add(Cuboid::new(0.5, 0.11, 0.045))),
                    MeshMaterial3d(identity.clone()),
                    Transform::from_xyz(0.0, 1.85, -0.36),
                ));
                root.spawn((
                    Mesh3d(meshes.add(Cylinder::new(0.85, 0.035))),
                    MeshMaterial3d(identity),
                    Transform::from_xyz(0.0, 0.03, 0.0),
                ));
            });
    }
}

fn update_overlay(state: Res<ClientState>, mut text: Query<&mut Text, With<StatusText>>) {
    let who = state
        .player
        .as_ref()
        .map(|p| format!("{}  |  {:.0}, {:.0}", p.name, p.x, p.y))
        .unwrap_or_else(|| "Waiting for commander...".into());
    let terrain_status = if let Some(terrain) = &state.terrain {
        state
            .player
            .as_ref()
            .and_then(|p| terrain.sample(p.x, p.y))
            .map(|surface| {
                use argon_dev::terrain::Biome;
                let name = match surface.biome {
                    Biome::Coast => "Coast",
                    Biome::Grassland => "Grassland",
                    Biome::Wetland => "Wetland",
                    Biome::Jungle => "Jungle",
                    Biome::LuminousGrove => "Luminous grove",
                };
                format!(
                    "\n{name}  |  movement {:.0}%  |  elevation {:.1} m  |  slope {:.0}%",
                    surface.movement_multiplier * 100.,
                    surface.height,
                    surface.slope * 100.
                )
            })
            .unwrap_or_default()
    } else if state.terrain_info.is_some() {
        "\nReceiving and validating saved terrain…".into()
    } else {
        String::new()
    };
    let short = |value: &str| value.chars().take(12).collect::<String>();
    let message = state.fatal.as_deref().or(state.content_error.as_deref());
    let error = message
        .map(|error| format!("\n{error}"))
        .unwrap_or_default();
    for mut text in &mut text {
        text.0 = format!(
            "LIVING EMPIRES  /  {}\n{}\n{}  |  {} commanders\nLua {}  |  host {}{}{}",
            if state.world_name.is_empty() {
                "DEVELOPMENT"
            } else {
                &state.world_name
            },
            state.status,
            who,
            state.players.len(),
            short(&state.content_version),
            short(&state.release),
            terrain_status,
            error
        );
    }
}

fn capture_frame(mut commands: Commands, keys: Res<ButtonInput<KeyCode>>, mut count: Local<u32>) {
    if keys.just_pressed(KeyCode::F12) {
        use bevy::render::view::screenshot::{Screenshot, save_to_disk};
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let path = format!("/tmp/argon-terrain-{stamp}-{}.png", *count);
        *count += 1;
        info!("Capturing actual native view to {path}");
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(path));
    }
}
