//! Native mission instruments. Read-only projection of received player state.
//! No UI label confers resources, health, ownership or successful actions.
use super::{ClientState, ViewState};
use bevy::{
    asset::uuid_handle,
    pbr::{ExtendedMaterial, MaterialExtension},
    prelude::*,
    render::render_resource::AsBindGroup,
    shader::ShaderRef,
    text::FontSize,
    window::PrimaryWindow,
};
use std::collections::VecDeque;

const INK: Color = Color::srgb(0.92, 0.91, 0.85);
const MUTED: Color = Color::srgb(0.65, 0.74, 0.73);
const PANEL: Color = Color::srgba(0.025, 0.045, 0.055, 0.94);
const ACCENT: Color = Color::srgb(0.52, 0.83, 0.73);
const WARNING: Color = Color::srgb(1.0, 0.73, 0.37);

#[derive(Component)]
struct Instrument;
#[derive(Component, Clone, Copy)]
enum Field {
    Connection,
    Commander,
    Coordinates,
    Order,
    Alert,
    Performance,
    View,
    Surface,
}
#[derive(Resource)]
struct Instruments {
    refresh: Timer,
    frames: VecDeque<f32>,
    photo: bool,
    telemetry: bool,
    quality: bool,
    movement_notice: Option<(String, f32)>,
}
impl Default for Instruments {
    fn default() -> Self {
        Self {
            refresh: Timer::from_seconds(0.25, TimerMode::Repeating),
            frames: VecDeque::with_capacity(600),
            photo: false,
            telemetry: false,
            quality: true,
            movement_notice: None,
        }
    }
}

pub(super) fn install(app: &mut App) {
    app.add_plugins(MaterialPlugin::<WaterMaterial>::default());
    app.world_mut()
        .resource_mut::<Assets<Shader>>()
        .insert(
            WATER_SHADER.id(),
            Shader::from_wesl(
                include_str!("../assets/shaders/landscape.wesl"),
                "shaders/landscape.wesl",
            ),
        )
        .expect("fixed shader handle");
    app.insert_resource(GlobalAmbientLight {
        color: Color::srgb(0.65, 0.78, 0.86),
        brightness: 550.,
        ..default()
    });
    app.init_resource::<Instruments>()
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            remember_rejection
                .after(super::network_tick)
                .before(super::player_input),
        )
        .add_systems(
            Update,
            (measure, quality, update)
                .chain()
                .after(super::camera_control),
        );
}

fn panel() -> Node {
    Node {
        padding: UiRect::all(px(20)),
        flex_direction: FlexDirection::Column,
        row_gap: px(8),
        ..default()
    }
}
fn setup(mut commands: Commands, mut fonts: ResMut<Assets<Font>>) {
    let font = fonts.add(Font::from_bytes(
        include_bytes!("../assets/fonts/PublicSans.ttf").to_vec(),
    ));
    let label = |value: &str, size: f32, color: Color| {
        (
            Text::new(value),
            TextFont {
                font: font.clone().into(),
                font_size: FontSize::Px(size),
                ..default()
            },
            TextColor(color),
        )
    };
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px(24),
                right: px(24),
                top: px(20),
                height: px(60),
                padding: UiRect::axes(px(20), px(12)),
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(PANEL),
            Instrument,
        ))
        .with_children(|bar| {
            bar.spawn((Node {
                column_gap: px(24),
                align_items: AlignItems::Center,
                ..default()
            },))
                .with_children(|brand| {
                    brand.spawn(label("ARGON / EMPIRES", 22., INK));
                    brand.spawn((
                        label("Establishing connection", 14., ACCENT),
                        Field::Connection,
                    ));
                });
            bar.spawn(label("SURFACE OPERATIONS", 13., MUTED));
        });
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px(24),
                bottom: px(82),
                width: px(300),
                ..panel()
            },
            BackgroundColor(PANEL),
            Instrument,
        ))
        .with_children(|body| {
            body.spawn((label("Awaiting commander", 23., INK), Field::Commander));
            body.spawn((label("", 14., MUTED), Field::Coordinates));
            body.spawn((
                Node {
                    height: px(1),
                    margin: UiRect::vertical(px(5)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.18, 0.28, 0.29)),
            ));
            body.spawn((label("", 16., ACCENT), Field::Order));
            body.spawn((label("", 13., MUTED), Field::Surface));
        });
    commands.spawn((Node {
        position_type: PositionType::Absolute, right: px(24), bottom: px(82),
        width: px(250), ..panel()
    }, BackgroundColor(PANEL), Instrument)).with_children(|body| {
        body.spawn(label("Survey camera", 20., INK));
        body.spawn((label("", 14., ACCENT), Field::View));
        body.spawn(label("Arrows   Pan camera\nQ / E       Rotate\nScroll     Zoom\nPgUp / Dn   Tilt\nHome     Recenter", 14., MUTED));
    });
    commands.spawn((Node {
        position_type: PositionType::Absolute, left: px(24), right: px(24), bottom: px(20),
        height: px(44), padding: UiRect::axes(px(18), px(10)),
        justify_content: JustifyContent::SpaceBetween, align_items: AlignItems::Center,
        ..default()
    }, BackgroundColor(PANEL), Instrument)).with_children(|bar| {
        bar.spawn(label("WASD  Commander    /    F1  Photo    /    F2  Quality    /    F3  Performance    /    F12  Capture", 13., MUTED));
        bar.spawn((label("DEVELOPMENT BUILD", 12., MUTED), Field::Performance));
    });
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px(24),
                top: px(94),
                max_width: px(540),
                padding: UiRect::all(px(16)),
                ..default()
            },
            BackgroundColor(PANEL),
            Instrument,
            Visibility::Hidden,
        ))
        .with_child((label("", 15., WARNING), Field::Alert));
}

fn clipped(value: &str, limit: usize) -> String {
    let mut result: String = value.chars().take(limit).collect();
    if value.chars().count() > limit {
        result.push('…');
    }
    result
}
fn percentile_ms(frames: &VecDeque<f32>) -> Option<f32> {
    if frames.is_empty() {
        return None;
    }
    let mut values: Vec<_> = frames.iter().copied().collect();
    values.sort_unstable_by(f32::total_cmp);
    Some(values[((values.len() as f32 * 0.95).ceil() as usize).saturating_sub(1)] * 1000.)
}
fn measure(time: Res<Time<Real>>, keys: Res<ButtonInput<KeyCode>>, mut ui: ResMut<Instruments>) {
    let dt = time.delta_secs();
    if dt.is_finite() && dt > 0. && time.elapsed_secs() > 10. {
        if ui.frames.len() == 600 {
            ui.frames.pop_front();
        }
        ui.frames.push_back(dt);
    }
    if keys.just_pressed(KeyCode::F1) {
        ui.photo = !ui.photo;
    }
    if keys.just_pressed(KeyCode::F3) {
        ui.telemetry = !ui.telemetry;
    }
    if keys.just_pressed(KeyCode::F2) {
        ui.quality = !ui.quality;
    }
    ui.refresh.tick(time.delta());
}
fn update(
    state: Res<ClientState>,
    view: Res<ViewState>,
    ui: Res<Instruments>,
    window: Query<&Window, With<PrimaryWindow>>,
    mut scale: ResMut<UiScale>,
    mut fields: Query<(&Field, &mut Text, &mut TextColor, &ChildOf)>,
    mut panels: Query<&mut Visibility, With<Instrument>>,
) {
    if let Ok(window) = window.single() {
        // Fit instruments to logical pixels, independently of Retina density.
        scale.0 = (window.width() / 1440.)
            .min(window.height() / 810.)
            .clamp(0.65, 1.);
    }
    for mut visibility in &mut panels {
        *visibility = if ui.photo {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
    }
    let alert = state
        .fatal
        .as_deref()
        .or(state.content_error.as_deref())
        .or(ui
            .movement_notice
            .as_ref()
            .map(|(message, _)| message.as_str()));
    for (field, mut text, mut color, parent) in &mut fields {
        if matches!(field, Field::Alert)
            && let Ok(mut visible) = panels.get_mut(parent.parent())
        {
            *visible = if alert.is_some() && !ui.photo {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            };
        }
        if !ui.refresh.just_finished() {
            continue;
        }
        let next = match field {
            Field::Connection => {
                color.0 = if state.ready { ACCENT } else { WARNING };
                if state.ready {
                    format!(
                        "Connected  /  {} {}",
                        state.players.len(),
                        if state.players.len() == 1 {
                            "commander"
                        } else {
                            "commanders"
                        }
                    )
                } else if state.terrain_info.is_some() && state.terrain.is_none() {
                    "Validating saved landscape…".into()
                } else {
                    clipped(&state.status, 54)
                }
            }
            Field::Commander => state
                .player
                .as_ref()
                .map(|p| clipped(&p.name, 24))
                .unwrap_or_else(|| "Awaiting commander".into()),
            Field::Coordinates => state
                .player
                .as_ref()
                .map(|p| format!("POSITION    {:.0} E  /  {:.0} N", p.x, p.y))
                .unwrap_or_else(|| "Waiting for host state".into()),
            Field::Order => {
                if !state.ready {
                    "Controls waiting for connection".into()
                } else if view.target.is_some() {
                    "Moving to destination".into()
                } else {
                    "Ready for orders".into()
                }
            }
            Field::Alert => alert.map(|s| clipped(s, 220)).unwrap_or_default(),
            Field::Surface => state
                .terrain
                .as_ref()
                .and_then(|t| state.player.as_ref().and_then(|p| t.sample(p.x, p.y)))
                .map(|s| {
                    let name = match s.biome {
                        argon_dev::terrain::Biome::Coast => "Coast",
                        argon_dev::terrain::Biome::Grassland => "Grassland",
                        argon_dev::terrain::Biome::Wetland => "Wetland",
                        argon_dev::terrain::Biome::Jungle => "Jungle",
                        argon_dev::terrain::Biome::LuminousGrove => "Luminous grove",
                    };
                    format!(
                        "{name}  /  {:.0} m elevation\nMovement {:.0}%  /  Slope {:.0}%",
                        s.height,
                        s.movement_multiplier * 100.,
                        s.slope * 100.
                    )
                })
                .unwrap_or_else(|| {
                    if state.terrain_info.is_some() {
                        "Receiving terrain from host".into()
                    } else {
                        "Legacy development arena".into()
                    }
                }),
            Field::View => format!(
                "View distance  {:.0}  /  {:.0}°",
                view.distance,
                view.yaw.to_degrees().rem_euclid(360.)
            ),
            Field::Performance => {
                if ui.telemetry {
                    percentile_ms(&ui.frames)
                        .map(|p95| {
                            format!("P95  {p95:.1} ms  /  {} recent frames", ui.frames.len())
                        })
                        .unwrap_or_else(|| "Performance warming up…".into())
                } else {
                    format!(
                        "{} / DEVELOPMENT",
                        if ui.quality { "BALANCED" } else { "LOW COST" }
                    )
                }
            }
        };
        if text.0 != next {
            text.0 = next;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn real_frame_collection_retains_long_stalls_and_caps_history() {
        use std::time::Duration;
        let mut app = App::new();
        app.insert_resource(Time::<Real>::default())
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<Instruments>()
            .add_systems(Update, measure);
        app.world_mut()
            .resource_mut::<Time<Real>>()
            .advance_by(Duration::from_secs(11));
        app.update();
        app.world_mut().resource_mut::<Instruments>().frames.clear();
        app.world_mut()
            .resource_mut::<Time<Real>>()
            .advance_by(Duration::from_millis(1200));
        app.update();
        assert_eq!(
            percentile_ms(&app.world().resource::<Instruments>().frames),
            Some(1200.)
        );
        for _ in 0..601 {
            app.world_mut()
                .resource_mut::<Time<Real>>()
                .advance_by(Duration::from_millis(16));
            app.update();
        }
        assert_eq!(app.world().resource::<Instruments>().frames.len(), 600);
    }
    #[test]
    fn long_unicode_labels_are_bounded_without_breaking_characters() {
        assert_eq!(clipped("Argonaut", 8), "Argonaut");
        assert_eq!(clipped("世界探検隊", 3), "世界探…");
    }
    #[test]
    fn frame_measurements_include_stalls_and_use_nearest_rank() {
        let mut frames = VecDeque::from(vec![0.016; 100]);
        for value in frames.iter_mut().take(6) {
            *value = 1.2;
        }
        assert_eq!(percentile_ms(&frames), Some(1200.));
        assert_eq!(percentile_ms(&VecDeque::new()), None);
    }
}

// The extension only perturbs shading normals; physical water/terrain are unchanged.
const WATER_SHADER: Handle<Shader> = uuid_handle!("bf294361-0a2c-4db2-b458-f9146bfbcc08");
#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub(super) struct WaterSurface {
    #[uniform(100)]
    pub settings: Vec4,
}
pub(super) type WaterMaterial = ExtendedMaterial<StandardMaterial, WaterSurface>;
impl MaterialExtension for WaterSurface {
    fn fragment_shader() -> ShaderRef {
        WATER_SHADER.into()
    }
}

impl Default for WaterSurface {
    fn default() -> Self {
        Self {
            settings: Vec4::ONE,
        }
    }
}
fn quality(
    keys: Res<ButtonInput<KeyCode>>,
    ui: Res<Instruments>,
    mut lights: Query<&mut DirectionalLight>,
    mut waters: ResMut<Assets<WaterMaterial>>,
) {
    if !keys.just_pressed(KeyCode::F2) {
        return;
    }
    for mut light in &mut lights {
        light.shadow_maps_enabled = ui.quality;
    }
    for (_, water) in waters.iter_mut() {
        water.extension.settings.x = if ui.quality { 1. } else { 0. };
    }
}

fn remember_rejection(state: Res<ClientState>, time: Res<Time>, mut ui: ResMut<Instruments>) {
    if let Some(error) = &state.movement_error {
        ui.movement_notice = Some((error.clone(), time.elapsed_secs() + 6.));
    } else if ui
        .movement_notice
        .as_ref()
        .is_some_and(|(_, until)| time.elapsed_secs() > *until)
    {
        ui.movement_notice = None;
    }
}
