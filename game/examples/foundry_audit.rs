//! Render a schema-1 Living Empires building package in Bevy and capture three views.
//!
//! Usage: cargo run -p living-empires --example foundry_audit -- PACKAGE.json OUTPUT_DIR
//! Recipe coordinates are metres with +Y up and +Z as the front. This adapter maps
//! recipe (x, y, z) directly to Bevy (x, y, z); x/z remain the horizontal footprint.
//! It renders declared geometry only and makes no collision, gameplay, or approval claim.

use anyhow::{Context as AnyhowContext, Result, ensure};
use bevy::{
    prelude::*,
    render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk},
};
use serde::Deserialize;
use std::{
    collections::BTreeMap,
    env, fs,
    path::{Path, PathBuf},
    process::ExitCode,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Package {
    format: String,
    schema_version: u8,
    asset_type: String,
    asset_id: String,
    theme_id: String,
    theme: Theme,
    recipe: Recipe,
    manifest: Manifest,
}

#[derive(Deserialize)]
struct Theme {
    palette: Palette,
    sky: String,
}

#[derive(Deserialize)]
struct Palette {
    shell: String,
    frame: String,
    accent: String,
    ground: String,
    rock: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Recipe {
    theme_id: String,
    name: String,
    description: String,
    sensor_radius: f64,
    parts: Vec<Part>,
    #[serde(default)]
    scene: Option<RecipeScene>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RecipeScene {
    half_extent: f64,
    components: Vec<SceneComponent>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SceneComponent {
    id: String,
    name: String,
    role: String,
    part_ids: Vec<String>,
}

#[derive(Deserialize)]
struct Part {
    id: String,
    shape: String,
    role: String,
    position: [f64; 3],
    size: [f64; 3],
    material: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Manifest {
    content_sha256: String,
    rehearsal: bool,
    provider: String,
    model: String,
    #[serde(default)]
    review_half_extent: Option<f64>,
}

#[derive(Resource)]
struct AuditState {
    out: PathBuf,
    frame: u32,
    view: usize,
    completed: usize,
    outcome: Arc<AtomicUsize>,
    views: [(&'static str, Vec3); 3],
    camera_target_y: f32,
}

const LEGACY_VIEWS: [(&str, Vec3); 3] = [
    ("front", Vec3::new(0.0, 7.5, 15.5)),
    ("three-quarter", Vec3::new(13.0, 10.0, 13.0)),
    ("rear", Vec3::new(-4.0, 8.0, -16.0)),
];

fn scene_views(extent: f64) -> [(&'static str, Vec3); 3] {
    let scale = extent as f32 / 4.0;
    LEGACY_VIEWS.map(|(name, position)| (name, position * scale))
}

fn camera_extent(package: &Package) -> Result<(f64, bool)> {
    let recipe_extent = package.recipe.scene.as_ref().map(|scene| scene.half_extent);
    let review_extent = package.manifest.review_half_extent;
    for extent in [recipe_extent, review_extent].into_iter().flatten() {
        ensure!(
            extent.is_finite() && (4.0..=16.0).contains(&extent),
            "scene/review halfExtent must be 4–16 metres"
        );
    }
    if let (Some(recipe), Some(review)) = (recipe_extent, review_extent) {
        ensure!(
            (recipe - review).abs() <= 0.001,
            "manifest reviewHalfExtent differs from recipe scene halfExtent"
        );
    }
    Ok((
        review_extent.or(recipe_extent).unwrap_or(4.0),
        recipe_extent.is_some() || review_extent.is_some(),
    ))
}

fn parse_hex(value: &str, label: &str) -> Result<Color> {
    let bytes = value.as_bytes();
    ensure!(
        bytes.len() == 7 && bytes[0] == b'#' && bytes[1..].iter().all(u8::is_ascii_hexdigit),
        "{label} must be #RRGGBB"
    );
    let channel = |range| {
        AnyhowContext::with_context(u8::from_str_radix(&value[range], 16), || {
            format!("invalid {label} color {value}")
        })
    };
    Ok(Color::srgb_u8(
        channel(1..3)?,
        channel(3..5)?,
        channel(5..7)?,
    ))
}

fn validate(package: &Package) -> Result<()> {
    ensure!(
        package.format == "living-empires.asset" && package.schema_version == 1,
        "expected living-empires.asset schemaVersion 1"
    );
    ensure!(
        package.asset_type == "building",
        "native audit accepts building packages only"
    );
    ensure!(
        !package.asset_id.trim().is_empty(),
        "package assetId is required"
    );
    ensure!(
        package.recipe.theme_id == package.theme_id,
        "recipe themeId differs from package themeId"
    );
    ensure!(
        !package.recipe.name.trim().is_empty() && !package.recipe.description.trim().is_empty(),
        "recipe identity is incomplete"
    );
    ensure!(
        package.recipe.sensor_radius.is_finite()
            && (1.0..=20.0).contains(&package.recipe.sensor_radius),
        "invalid sensorRadius"
    );
    let geometry_extent = if let Some(scene) = &package.recipe.scene {
        ensure!(
            scene.half_extent.is_finite() && (4.0..=16.0).contains(&scene.half_extent),
            "scene halfExtent must be 4–16 metres"
        );
        ensure!(
            (4..=128).contains(&package.recipe.parts.len()),
            "scene recipe must contain 4–128 parts"
        );
        ensure!(
            (1..=32).contains(&scene.components.len()),
            "scene must contain 1–32 components"
        );
        let mut component_ids = std::collections::HashSet::new();
        let mut assigned_parts = std::collections::HashSet::new();
        for component in &scene.components {
            let slug = component.id.as_bytes();
            ensure!(
                !slug.is_empty()
                    && slug.len() <= 64
                    && slug[0].is_ascii_lowercase()
                    && slug
                        .iter()
                        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || *b == b'-')
                    && !component.id.ends_with('-')
                    && !component.id.contains("--")
                    && component_ids.insert(&component.id),
                "scene component IDs must be unique lowercase slugs"
            );
            ensure!(
                !component.name.trim().is_empty()
                    && component.name.len() <= 100
                    && !component.role.trim().is_empty()
                    && component.role.len() <= 80
                    && !component.part_ids.is_empty(),
                "scene component name, role, and part membership must be bounded and non-empty"
            );
            for part_id in &component.part_ids {
                ensure!(
                    assigned_parts.insert(part_id),
                    "each part must belong to exactly one scene component"
                );
            }
        }
        ensure!(
            assigned_parts.len() == package.recipe.parts.len()
                && package
                    .recipe
                    .parts
                    .iter()
                    .all(|part| assigned_parts.contains(&part.id)),
            "scene components must cover every part exactly once"
        );
        scene.half_extent
    } else {
        ensure!(
            (4..=48).contains(&package.recipe.parts.len()),
            "legacy recipe must contain 4–48 parts"
        );
        4.0
    };
    for (label, color) in [
        ("shell", &package.theme.palette.shell),
        ("frame", &package.theme.palette.frame),
        ("accent", &package.theme.palette.accent),
        ("ground", &package.theme.palette.ground),
        ("rock", &package.theme.palette.rock),
        ("sky", &package.theme.sky),
    ] {
        parse_hex(color, label)?;
    }
    ensure!(
        package.manifest.content_sha256.len() == 64
            && package
                .manifest
                .content_sha256
                .bytes()
                .all(|b| b.is_ascii_hexdigit()),
        "manifest contentSha256 is missing or invalid"
    );
    let mut ids = std::collections::HashSet::new();
    for part in &package.recipe.parts {
        ensure!(
            !part.id.is_empty() && ids.insert(&part.id),
            "part IDs must be non-empty and unique"
        );
        ensure!(
            ["box", "cylinder", "cone"].contains(&part.shape.as_str()),
            "unsupported shape for part {}",
            part.id
        );
        ensure!(
            ["wall", "support", "roof", "detail"].contains(&part.role.as_str()),
            "unsupported role for part {}",
            part.id
        );
        ensure!(
            ["shell", "frame", "accent"].contains(&part.material.as_str()),
            "unsupported material for part {}",
            part.id
        );
        ensure!(
            part.position.iter().all(|v| v.is_finite())
                && part.position[0].abs() <= 28.0
                && part.position[1].abs() <= 10.0
                && part.position[2].abs() <= 28.0,
            "invalid position for part {}",
            part.id
        );
        ensure!(
            part.size
                .iter()
                .all(|v| v.is_finite() && *v > 0.0 && *v <= 12.0),
            "invalid size for part {}",
            part.id
        );
        ensure!(
            part.position[0].abs() + part.size[0] / 2.0 <= geometry_extent + 0.05
                && part.position[2].abs() + part.size[2] / 2.0 <= geometry_extent + 0.05,
            "part {} exceeds scene halfExtent {}",
            part.id,
            geometry_extent
        );
        ensure!(
            part.position[1] - part.size[1] / 2.0 >= -0.05,
            "part {} extends below the ground",
            part.id
        );
    }
    Ok(())
}

fn material(palette: &Palette, name: &str) -> Color {
    let hex = match name {
        "shell" => &palette.shell,
        "frame" => &palette.frame,
        _ => &palette.accent,
    };
    parse_hex(hex, name).expect("palette was validated")
}

fn cylinder_radii(size: [f32; 3]) -> (f32, f32) {
    // Match Three.CylinderGeometry(topRadius, bottomRadius, height, 12).
    (size[0] / 2.0, size[2] / 2.0)
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    package: Res<PackageResource>,
    state: Res<AuditState>,
) {
    let package = &package.0;
    let (extent, scalable) = camera_extent(package).expect("validated scene extent");
    commands.insert_resource(ClearColor(
        parse_hex(&package.theme.sky, "sky").expect("validated"),
    ));
    let ground_color = parse_hex(&package.theme.palette.ground, "ground").expect("validated");
    let rock_color = parse_hex(&package.theme.palette.rock, "rock").expect("validated");
    let ground_size = if scalable {
        ((extent + 10.0) * 2.0) as f32
    } else {
        32.0
    };
    let edge = if scalable {
        (extent + 8.0) as f32
    } else {
        12.0
    };
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(ground_size, ground_size))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: ground_color,
            perceptual_roughness: 0.92,
            ..default()
        })),
    ));
    // Subtle perimeter slabs ground the object in a material palette supplied by the package.
    let rock = materials.add(StandardMaterial {
        base_color: rock_color,
        perceptual_roughness: 0.88,
        ..default()
    });
    for (pos, size) in [
        (
            Vec3::new(0.0, -0.12, -edge),
            Vec3::new(edge * 2.0 + 2.0, 0.24, 0.5),
        ),
        (
            Vec3::new(0.0, -0.12, edge),
            Vec3::new(edge * 2.0 + 2.0, 0.24, 0.5),
        ),
        (
            Vec3::new(-edge, -0.12, 0.0),
            Vec3::new(0.5, 0.24, edge * 2.0),
        ),
        (
            Vec3::new(edge, -0.12, 0.0),
            Vec3::new(0.5, 0.24, edge * 2.0),
        ),
    ] {
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::from_size(size))),
            MeshMaterial3d(rock.clone()),
            Transform::from_translation(pos),
        ));
    }
    for part in &package.recipe.parts {
        let [w, h, d] = part.size.map(|v| v as f32);
        let mesh = match part.shape.as_str() {
            "box" => meshes.add(Cuboid::from_size(Vec3::new(w, h, d))),
            "cylinder" => {
                let (top_radius, bottom_radius) = cylinder_radii([w, h, d]);
                meshes.add(
                    ConicalFrustum {
                        radius_top: top_radius,
                        radius_bottom: bottom_radius,
                        height: h,
                    }
                    .mesh()
                    .resolution(12)
                    .build(),
                )
            }
            "cone" => meshes.add(Cone::new(w / 2.0, h).mesh().resolution(6).build()),
            _ => unreachable!("validated shape"),
        };
        let color = material(&package.theme.palette, &part.material);
        let roughness = if part.role == "detail" { 0.45 } else { 0.76 };
        commands.spawn((
            Mesh3d(mesh),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: color,
                perceptual_roughness: roughness,
                metallic: if part.material == "frame" { 0.28 } else { 0.08 },
                ..default()
            })),
            Transform::from_xyz(
                part.position[0] as f32,
                part.position[1] as f32,
                part.position[2] as f32,
            ),
        ));
    }
    commands.spawn((
        DirectionalLight {
            illuminance: 14500.0,
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_xyz(-8.0, 18.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    commands.spawn((
        Camera3d::default(),
        Transform::from_translation(state.views[0].1)
            .looking_at(Vec3::new(0.0, state.camera_target_y, 0.0), Vec3::Y),
    ));
    let _ = state.out.as_path();
}

#[derive(Resource)]
struct PackageResource(Package);

fn capture_views(
    mut commands: Commands,
    mut state: ResMut<AuditState>,
    mut camera: Query<&mut Transform, With<Camera3d>>,
) {
    if state.completed == state.views.len() {
        let exit = if state.outcome.load(Ordering::Acquire) == 1 {
            AppExit::Success
        } else {
            AppExit::error()
        };
        commands.write_message(exit);
        return;
    }
    state.frame += 1;
    if state.view >= state.views.len() || state.frame < 18 {
        return;
    }
    let (name, offset) = state.views[state.view];
    for mut transform in &mut camera {
        *transform = Transform::from_translation(offset)
            .looking_at(Vec3::new(0.0, state.camera_target_y, 0.0), Vec3::Y);
    }
    if state.frame == 18 {
        let path = state
            .out
            .join(format!("view-{}-{name}.png", state.view + 1));
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_and_mark(path));
    }
    if state.completed > state.view {
        state.view += 1;
        state.frame = 0;
    }
}

fn save_and_mark(
    path: PathBuf,
) -> impl FnMut(On<ScreenshotCaptured>, ResMut<AuditState>, Res<PackageResource>) + Send + Sync + 'static
{
    let mut save = save_to_disk(path);
    move |event, mut state, package| {
        // Keep the filesystem write and completion check in one observer. Multiple observers
        // for the same capture have no ordering guarantee.
        save(event);
        state.completed += 1;
        if state.completed == state.views.len() {
            let result = (|| -> Result<()> {
                for (index, (name, _)) in state.views.iter().enumerate() {
                    let image = state.out.join(format!("view-{}-{name}.png", index + 1));
                    ensure!(
                        image.is_file() && fs::metadata(&image)?.len() > 0,
                        "capture file is missing or empty: {}",
                        image.display()
                    );
                }
                write_manifest(
                    &state.out,
                    &package.0,
                    &state.views,
                    camera_extent(&package.0)?.0,
                )
            })();
            match result {
                Ok(()) => state.outcome.store(1, Ordering::Release),
                Err(error) => {
                    eprintln!("foundry_audit: capture completion failed: {error:#}");
                    state.outcome.store(2, Ordering::Release);
                }
            }
        }
    }
}

fn write_manifest(
    path: &Path,
    package: &Package,
    views: &[(&str, Vec3); 3],
    review_half_extent: f64,
) -> Result<()> {
    let mut meta = BTreeMap::new();
    meta.insert("format", package.format.as_str());
    meta.insert("assetId", package.asset_id.as_str());
    meta.insert("assetType", package.asset_type.as_str());
    meta.insert("themeId", package.theme_id.as_str());
    meta.insert("candidateHash", package.manifest.content_sha256.as_str());
    meta.insert("provider", package.manifest.provider.as_str());
    meta.insert("model", package.manifest.model.as_str());
    let manifest = serde_json::json!({
        "package": meta,
        "rehearsal": package.manifest.rehearsal,
        "renderer": "Bevy 0.20 native",
        "reviewHalfExtent": review_half_extent,
        "coordinateMapping": "recipe +Y up/+Z front; recipe x/y/z maps directly to Bevy x/y/z; x/z form horizontal footprint",
        "views": views.iter().enumerate().map(|(i, (name, position))| serde_json::json!({"index": i + 1, "name": name, "file": format!("view-{}-{name}.png", i + 1), "cameraPosition": [position.x, position.y, position.z]})).collect::<Vec<_>>(),
        "claims": ["actual native renderer captures", "no collision or gameplay result", "no approval result"]
    });
    fs::write(
        path.join("capture-manifest.json"),
        serde_json::to_vec_pretty(&manifest)?,
    )?;
    Ok(())
}

fn run() -> Result<()> {
    let args: Vec<_> = env::args_os().skip(1).collect();
    ensure!(
        args.len() == 2,
        "usage: foundry_audit PACKAGE.json OUTPUT_DIR"
    );
    let package_path = PathBuf::from(&args[0]);
    let out = PathBuf::from(&args[1]);
    AnyhowContext::with_context(fs::create_dir_all(&out), || {
        format!("create output directory {}", out.display())
    })?;
    let raw = AnyhowContext::with_context(fs::read(&package_path), || {
        format!("read package {}", package_path.display())
    })?;
    let package: Package =
        AnyhowContext::context(serde_json::from_slice(&raw), "parse Asset Foundry package")?;
    validate(&package)?;
    let (review_extent, scalable_cameras) = camera_extent(&package)?;
    let views = if scalable_cameras {
        scene_views(review_extent)
    } else {
        LEGACY_VIEWS
    };
    ensure!(
        !out.join("capture-manifest.json").exists()
            && views.iter().enumerate().all(|(index, (name, _))| {
                !out.join(format!("view-{}-{name}.png", index + 1)).exists()
            }),
        "output directory already contains evidence; choose a fresh directory"
    );
    let outcome = Arc::new(AtomicUsize::new(0));
    let state = AuditState {
        out,
        frame: 0,
        view: 0,
        completed: 0,
        outcome: Arc::clone(&outcome),
        views,
        camera_target_y: 1.5,
    };
    let mut app = App::new();
    app.insert_resource(PackageResource(package))
        .insert_resource(state)
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Living Empires asset audit".into(),
                resolution: (1280, 900).into(),
                ..default()
            }),
            ..default()
        }))
        .add_systems(Startup, setup)
        .add_systems(Update, capture_views);
    let app_exit = app.run();
    ensure!(app_exit.is_success(), "renderer exited unsuccessfully");
    ensure!(
        outcome.load(Ordering::Acquire) == 1,
        "renderer closed before all three captures and their manifest were verified"
    );
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("foundry_audit: {error:#}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{LEGACY_VIEWS, camera_extent, cylinder_radii, scene_views};
    use bevy::prelude::Vec3;
    use serde_json::json;

    #[test]
    fn cylinder_uses_distinct_top_and_bottom_recipe_dimensions() {
        assert_eq!(cylinder_radii([2.0, 4.0, 6.0]), (1.0, 3.0));
    }

    #[test]
    fn scene_camera_positions_scale_with_frozen_half_extent() {
        assert_eq!(
            scene_views(8.0),
            [
                ("front", Vec3::new(0.0, 15.0, 31.0)),
                ("three-quarter", Vec3::new(26.0, 20.0, 26.0)),
                ("rear", Vec3::new(-8.0, 16.0, -32.0)),
            ]
        );
    }

    #[test]
    fn legacy_camera_layout_is_preserved_without_scene_or_review_extent() {
        let package: super::Package = serde_json::from_value(json!({
            "format":"living-empires.asset", "schemaVersion":1,
            "assetType":"building", "assetId":"legacy", "themeId":"argonaut",
            "theme":{"palette":{"shell":"#ffffff","frame":"#ffffff","accent":"#ffffff","ground":"#ffffff","rock":"#ffffff"},"sky":"#ffffff"},
            "recipe":{"themeId":"argonaut","name":"Outpost","description":"Legacy","sensorRadius":12,"parts":[]},
            "manifest":{"contentSha256":"0".repeat(64),"rehearsal":true,"provider":"fixture","model":"fixture"}
        })).unwrap();
        assert_eq!(camera_extent(&package).unwrap(), (4.0, false));
        assert_eq!(LEGACY_VIEWS[0].1, Vec3::new(0.0, 7.5, 15.5));
    }

    #[test]
    fn legacy_baseline_can_use_frozen_review_extent_for_camera_only() {
        let package: super::Package = serde_json::from_value(json!({
            "format":"living-empires.asset", "schemaVersion":1,
            "assetType":"building", "assetId":"legacy", "themeId":"argonaut",
            "theme":{"palette":{"shell":"#ffffff","frame":"#ffffff","accent":"#ffffff","ground":"#ffffff","rock":"#ffffff"},"sky":"#ffffff"},
            "recipe":{"themeId":"argonaut","name":"Outpost","description":"Legacy","sensorRadius":12,"parts":[]},
            "manifest":{"contentSha256":"0".repeat(64),"rehearsal":true,"provider":"fixture","model":"fixture","reviewHalfExtent":12}
        })).unwrap();
        assert_eq!(camera_extent(&package).unwrap(), (12.0, true));
    }
}
