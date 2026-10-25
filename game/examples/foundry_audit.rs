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
}

#[derive(Resource)]
struct AuditState {
    out: PathBuf,
    frame: u32,
    view: usize,
    completed: usize,
    outcome: Arc<AtomicUsize>,
}

const VIEWS: [(&str, Vec3); 3] = [
    ("front", Vec3::new(0.0, 7.5, 15.5)),
    ("three-quarter", Vec3::new(13.0, 10.0, 13.0)),
    ("rear", Vec3::new(-4.0, 8.0, -16.0)),
];

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
    ensure!(
        (4..=48).contains(&package.recipe.parts.len()),
        "recipe must contain 4–48 parts"
    );
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
            part.position
                .iter()
                .all(|v| v.is_finite() && v.abs() <= 10.0),
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
    commands.insert_resource(ClearColor(
        parse_hex(&package.theme.sky, "sky").expect("validated"),
    ));
    let ground_color = parse_hex(&package.theme.palette.ground, "ground").expect("validated");
    let rock_color = parse_hex(&package.theme.palette.rock, "rock").expect("validated");
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(32.0, 32.0))),
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
        (Vec3::new(0.0, -0.12, -12.0), Vec3::new(26.0, 0.24, 0.5)),
        (Vec3::new(0.0, -0.12, 12.0), Vec3::new(26.0, 0.24, 0.5)),
        (Vec3::new(-12.0, -0.12, 0.0), Vec3::new(0.5, 0.24, 24.0)),
        (Vec3::new(12.0, -0.12, 0.0), Vec3::new(0.5, 0.24, 24.0)),
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
        Transform::from_xyz(0.0, 7.5, 15.5).looking_at(Vec3::new(0.0, 1.5, 0.0), Vec3::Y),
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
    if state.completed == VIEWS.len() {
        let exit = if state.outcome.load(Ordering::Acquire) == 1 {
            AppExit::Success
        } else {
            AppExit::error()
        };
        commands.write_message(exit);
        return;
    }
    state.frame += 1;
    if state.view >= VIEWS.len() || state.frame < 18 {
        return;
    }
    let (name, offset) = VIEWS[state.view];
    for mut transform in &mut camera {
        *transform =
            Transform::from_translation(offset).looking_at(Vec3::new(0.0, 1.5, 0.0), Vec3::Y);
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
        if state.completed == VIEWS.len() {
            let result = (|| -> Result<()> {
                for (index, (name, _)) in VIEWS.iter().enumerate() {
                    let image = state.out.join(format!("view-{}-{name}.png", index + 1));
                    ensure!(
                        image.is_file() && fs::metadata(&image)?.len() > 0,
                        "capture file is missing or empty: {}",
                        image.display()
                    );
                }
                write_manifest(&state.out, &package.0)
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

fn write_manifest(path: &Path, package: &Package) -> Result<()> {
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
        "coordinateMapping": "recipe +Y up/+Z front; recipe x/y/z maps directly to Bevy x/y/z; x/z form horizontal footprint",
        "views": VIEWS.iter().enumerate().map(|(i, (name, _))| serde_json::json!({"index": i + 1, "name": name, "file": format!("view-{}-{name}.png", i + 1)})).collect::<Vec<_>>(),
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
    ensure!(
        !out.join("capture-manifest.json").exists()
            && VIEWS.iter().enumerate().all(|(index, (name, _))| {
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
    use super::cylinder_radii;

    #[test]
    fn cylinder_uses_distinct_top_and_bottom_recipe_dimensions() {
        assert_eq!(cylinder_radii([2.0, 4.0, 6.0]), (1.0, 3.0));
    }
}
