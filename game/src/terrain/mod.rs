//! Native presentation of validated host geometry. Decorative plants have no game authority.
mod geometry;
use super::{ClientState, ViewState};
use argon_dev::terrain::{Terrain, TerrainChunk};
use bevy::{
    asset::RenderAssetUsages,
    image::{ImageAddressMode, ImageSampler, ImageSamplerDescriptor},
    mesh::Indices,
    prelude::*,
    render::render_resource::{Extent3d, PrimitiveTopology, TextureDimension, TextureFormat},
};
use geometry::{Geometry, grid_indices, normal_at, visible_chunks};
use std::collections::HashMap;

const CHUNKS_PER_FRAME: usize = 2;
const RADIUS: i32 = 5; // 121 chunks, at most 605 chunk meshes; materials shared for the whole world.

#[derive(Resource, Default)]
struct Landscape {
    fingerprint: String,
    chunks: HashMap<(u32, u32), Resident>,
    ocean: Option<(Entity, Handle<Mesh>)>,
}
struct Resident {
    entities: Vec<Entity>,
    meshes: Vec<Handle<Mesh>>,
}
#[derive(Resource)]
struct Palette {
    ground: Handle<StandardMaterial>,
    bark: Handle<StandardMaterial>,
    foliage: Handle<StandardMaterial>,
    stone: Handle<StandardMaterial>,
    glow: Handle<StandardMaterial>,
    water: Handle<StandardMaterial>,
}

pub(super) fn install(app: &mut App) {
    app.init_resource::<Landscape>()
        .add_systems(Startup, setup)
        .add_systems(Update, stream.after(super::camera_control));
}
fn setup(
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    // A repeatable mineral-grain albedo texture, authored here; no external asset dependencies.
    let mut bytes = Vec::with_capacity(128 * 128 * 4);
    for z in 0..128 {
        for x in 0..128 {
            let grain = hash(x, z, 51);
            let band = ((x as f32 * 0.23 + (z as f32 * 0.13).sin() * 2.).sin() * 9.) as i32;
            let v = (204 + (grain * 34.) as i32 + band).clamp(0, 255) as u8;
            bytes.extend_from_slice(&[v, v, v, 255]);
        }
    }
    let mut texture = Image::new(
        Extent3d {
            width: 128,
            height: 128,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        bytes,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    texture.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        ..ImageSamplerDescriptor::linear()
    });
    let texture = images.add(texture);
    let ground = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        base_color_texture: Some(texture),
        perceptual_roughness: 0.97,
        ..default()
    });
    let bark = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        perceptual_roughness: 0.88,
        ..default()
    });
    let foliage = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        perceptual_roughness: 0.84,
        cull_mode: None,
        ..default()
    });
    let stone = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        perceptual_roughness: 0.8,
        metallic: 0.08,
        ..default()
    });
    let glow = materials.add(StandardMaterial {
        base_color: Color::srgb(0.35, 0.9, 0.85),
        emissive: LinearRgba::rgb(0.25, 1.7, 1.2),
        perceptual_roughness: 0.45,
        cull_mode: None,
        ..default()
    });
    let water = materials.add(StandardMaterial {
        base_color: Color::srgb(0.045, 0.17, 0.19),
        perceptual_roughness: 0.24,
        metallic: 0.22,
        reflectance: 0.6,
        ..default()
    });
    commands.insert_resource(Palette {
        ground,
        bark,
        foliage,
        stone,
        glow,
        water,
    });
}

fn stream(
    mut commands: Commands,
    state: Res<ClientState>,
    view: Res<ViewState>,
    mut landscape: ResMut<Landscape>,
    palette: Res<Palette>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let terrain = super::active_terrain(&state);
    let fingerprint = terrain.map(|terrain| terrain.info().fingerprint);
    if fingerprint.as_deref() != Some(landscape.fingerprint.as_str()) {
        for (_, resident) in landscape.chunks.drain() {
            retire(&mut commands, &mut meshes, resident);
        }
        if let Some((entity, mesh)) = landscape.ocean.take() {
            commands.entity(entity).despawn();
            meshes.remove(mesh.id());
        }
        landscape.fingerprint.clear();
    }
    let Some(terrain) = terrain else {
        return;
    };
    let info = terrain.info();
    if landscape.fingerprint != info.fingerprint {
        landscape.fingerprint = info.fingerprint;
        let size = info.cells as f32 * info.cell_size;
        let mesh = meshes.add(Plane3d::default().mesh().size(size * 3., size * 3.));
        let entity = commands
            .spawn((
                Mesh3d(mesh.clone()),
                MeshMaterial3d(palette.water.clone()),
                Transform::from_xyz(size * 0.5, info.sea_level - 0.03, size * 0.5),
            ))
            .id();
        landscape.ocean = Some((entity, mesh));
    }
    let size = info.chunk_cells as f32 * info.cell_size;
    let wanted = visible_chunks(
        view.focus.x,
        view.focus.z,
        size,
        info.cells / info.chunk_cells,
        RADIUS,
    );
    let evict: Vec<_> = landscape
        .chunks
        .keys()
        .filter(|key| !wanted.contains(key))
        .copied()
        .collect();
    for key in evict {
        if let Some(resident) = landscape.chunks.remove(&key) {
            retire(&mut commands, &mut meshes, resident);
        }
    }
    let pending: Vec<_> = wanted
        .into_iter()
        .filter(|key| !landscape.chunks.contains_key(key))
        .take(CHUNKS_PER_FRAME)
        .collect();
    for (x, z) in pending {
        let Some(chunk) = terrain.chunk(x, z) else {
            continue;
        };
        let mut resident = Resident {
            entities: Vec::new(),
            meshes: Vec::new(),
        };
        let ground = ground_geometry(terrain, &chunk);
        add_mesh(
            &mut commands,
            &mut meshes,
            &mut resident,
            ground,
            palette.ground.clone(),
        );
        let [bark, foliage, stone, glow] = decoration(terrain, &chunk);
        for (geometry, material) in [
            (bark, &palette.bark),
            (foliage, &palette.foliage),
            (stone, &palette.stone),
            (glow, &palette.glow),
        ] {
            if !geometry.positions.is_empty() {
                add_mesh(
                    &mut commands,
                    &mut meshes,
                    &mut resident,
                    geometry,
                    material.clone(),
                );
            }
        }
        landscape.chunks.insert((x, z), resident);
    }
}
fn retire(commands: &mut Commands, meshes: &mut Assets<Mesh>, resident: Resident) {
    for entity in resident.entities {
        commands.entity(entity).despawn();
    }
    for mesh in resident.meshes {
        meshes.remove(mesh.id());
    }
}
fn add_mesh(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    resident: &mut Resident,
    geometry: Geometry,
    material: Handle<StandardMaterial>,
) {
    let uv: Vec<[f32; 2]> = geometry
        .positions
        .iter()
        .map(|p| [p[0] / 24., p[2] / 24.])
        .collect();
    let colors: Vec<[f32; 4]> = geometry
        .colors
        .iter()
        .map(|c| {
            let linear = LinearRgba::from(Srgba::new(c[0], c[1], c[2], c[3]));
            [linear.red, linear.green, linear.blue, linear.alpha]
        })
        .collect();
    let mesh = meshes.add(
        Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, geometry.positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, geometry.normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uv)
        .with_inserted_indices(Indices::U32(geometry.indices)),
    );
    let entity = commands
        .spawn((Mesh3d(mesh.clone()), MeshMaterial3d(material)))
        .id();
    resident.entities.push(entity);
    resident.meshes.push(mesh);
}
fn ground_geometry(terrain: &Terrain, chunk: &TerrainChunk) -> Geometry {
    let info = terrain.info();
    let cells = info.chunk_cells;
    let extent = info.cells as f32 * info.cell_size;
    let mut geometry = Geometry {
        indices: grid_indices(cells),
        ..default()
    };
    for z in 0..=cells {
        for x in 0..=cells {
            let i = (z * (cells + 1) + x) as usize;
            let wx = (chunk.chunk_x * cells + x) as f32 * info.cell_size;
            let wz = (chunk.chunk_y * cells + z) as f32 * info.cell_size;
            let h = chunk.heights[i];
            geometry.positions.push([wx, h, wz]);
            let normal = normal_at(wx, wz, extent, info.cell_size, |x, z| {
                terrain
                    .sample(x as f64, z as f64)
                    .expect("validated terrain grid")
                    .height
            });
            geometry.normals.push(normal);
            geometry.colors.push(ground_color(
                chunk.biome_weights[i],
                h,
                chunk.geology[i],
                normal[1],
                wx,
                wz,
            ));
        }
    }
    geometry
}
fn ground_color(weights: [f32; 5], height: f32, geology: f32, up: f32, x: f32, z: f32) -> [f32; 4] {
    // Warm salt/sand, sage grassland, olive marsh, deep jade jungle, teal mineral grove.
    let colors = [
        [0.46, 0.40, 0.26],
        [0.20, 0.28, 0.12],
        [0.13, 0.19, 0.11],
        [0.07, 0.16, 0.105],
        [0.12, 0.24, 0.22],
    ];
    let mut c = [0.; 4];
    for i in 0..5 {
        for (channel, value) in c.iter_mut().enumerate().take(3) {
            *value += colors[i][channel] * weights[i];
        }
    }
    let rock = ((1. - up) * 3.8).clamp(0., 0.85);
    let variation = 0.92 + 0.08 * (x * 0.071 + (z * 0.019).sin()).sin() + 0.05 * (z * 0.047).cos();
    for (i, value) in c.iter_mut().enumerate().take(3) {
        *value = (*value * (1. - rock) + [0.31, 0.29, 0.25][i] * rock)
            * (0.88 + geology * 0.16)
            * variation;
    }
    if height < 0.6 {
        for value in c.iter_mut().take(3) {
            *value *= 0.66;
        }
    }
    c[3] = 1.;
    c
}
fn hash(x: u32, z: u32, salt: u32) -> f32 {
    let mut n =
        x.wrapping_mul(374761393) ^ z.wrapping_mul(668265263) ^ salt.wrapping_mul(1442695041);
    n = (n ^ (n >> 13)).wrapping_mul(1274126177);
    n ^= n >> 16;
    (n >> 8) as f32 / 16777216.
}

// Each patch is a single combined mesh per material. No individual plant entities,
// no new materials per chunk, and no decorative placement affects game collision.
fn decoration(terrain: &Terrain, chunk: &TerrainChunk) -> [Geometry; 4] {
    let info = terrain.info();
    let size = info.chunk_cells as f32 * info.cell_size;
    let mut bark = Geometry::default();
    let mut leaves = Geometry::default();
    let mut rock = Geometry::default();
    let mut glow = Geometry::default();
    for iz in 0..8 {
        for ix in 0..8 {
            let gx = chunk.chunk_x * 8 + ix;
            let gz = chunk.chunk_y * 8 + iz;
            let x = chunk.chunk_x as f32 * size
                + (ix as f32 + 0.12 + hash(gx, gz, 1) * 0.76) * size / 8.;
            let z = chunk.chunk_y as f32 * size
                + (iz as f32 + 0.12 + hash(gx, gz, 2) * 0.76) * size / 8.;
            let Some(surface) = terrain.sample(x as f64, z as f64) else {
                continue;
            };
            if surface.water_depth > 0.1 || surface.slope > 0.8 {
                continue;
            }
            let weights = surface.biome_weights;
            let density = weights[0] * 0.06
                + weights[1] * 0.25
                + weights[2] * 0.55
                + weights[3] * 0.92
                + weights[4] * 0.7;
            let r = hash(gx, gz, 3);
            let base = Vec3::new(x, surface.height - 0.25, z);
            if r < density {
                let jungle = weights[3];
                let luminous = weights[4];
                let wet = weights[2];
                let height = 3.0 + hash(gx, gz, 4) * 3.0 + jungle * 10. + luminous * 7.;
                if jungle + luminous > 0.25 {
                    let lean = Vec3::new(
                        (hash(gx, gz, 5) - 0.5) * 2.,
                        height,
                        (hash(gx, gz, 6) - 0.5) * 2.,
                    );
                    let top = base + lean;
                    branch(
                        &mut bark,
                        base,
                        top,
                        0.28 + height * 0.035,
                        0.12,
                        [0.19, 0.22, 0.17, 1.],
                    );
                    for j in 0..5 {
                        let angle = j as f32 * 2.399 + hash(gx, gz, 7) * std::f32::consts::TAU;
                        let radial = Vec3::new(angle.cos(), 0., angle.sin());
                        let stem = base + lean * (0.55 + j as f32 * 0.065);
                        let end = stem + radial * (height * 0.3) + Vec3::Y * (height * 0.15);
                        branch(&mut bark, stem, end, 0.19, 0.04, [0.19, 0.22, 0.17, 1.]);
                        let crown_color = [
                            0.055 + luminous * 0.06,
                            0.23 + luminous * 0.12,
                            0.12 + luminous * 0.19,
                            1.,
                        ];
                        fronds(
                            &mut leaves,
                            end,
                            height * 0.26,
                            crown_color,
                            hash(gx, gz, j + 21),
                        );
                        if luminous > 0.25 {
                            // Seed stalks protrude above the canopy rather than hiding inside it.
                            let light_tip = end + Vec3::Y * (height * 0.18);
                            branch(
                                &mut bark,
                                end,
                                light_tip,
                                0.055,
                                0.025,
                                [0.16, 0.23, 0.20, 1.],
                            );
                            crown(
                                &mut glow,
                                light_tip,
                                Vec3::new(0.18, 0.5, 0.18),
                                [1., 1., 1., 1.],
                                0.5,
                            );
                        }
                    }
                    fronds(
                        &mut leaves,
                        top,
                        height * 0.28,
                        [0.08, 0.28, 0.18, 1.],
                        hash(gx, gz, 8),
                    );
                } else {
                    // Reed fans and low leathery rosettes retain readable travel space.
                    for j in 0..5 {
                        let angle = j as f32 * 2.399 + hash(gx, gz, 9) * std::f32::consts::TAU;
                        let tip = base
                            + Vec3::new(
                                angle.cos() * height * 0.28,
                                height * (0.3 + wet * 0.4),
                                angle.sin() * height * 0.28,
                            );
                        branch(
                            &mut leaves,
                            base,
                            tip,
                            0.12 + wet * 0.1,
                            0.015,
                            [0.19, 0.28, 0.12, 1.],
                        );
                        if wet > 0.3 {
                            crown(
                                &mut leaves,
                                tip,
                                Vec3::new(0.3, 0.65, 0.3),
                                [0.32, 0.27, 0.13, 1.],
                                0.2,
                            );
                        }
                    }
                }
            } else if hash(gx, gz, 10) < 0.11 + surface.geology * 0.10 {
                let scale = 1.2 + hash(gx, gz, 11) * 3.;
                crown(
                    &mut rock,
                    base + Vec3::Y * scale * 0.18,
                    Vec3::new(scale, scale * 0.65, scale * 0.8),
                    [0.27, 0.25, 0.22, 1.],
                    hash(gx, gz, 12),
                );
            }
        }
    }
    [bark, leaves, rock, glow]
}
/// Curved fronds grow from connected branch tips, with raised ribs and tapered edges.
fn fronds(mesh: &mut Geometry, center: Vec3, radius: f32, color: [f32; 4], variation: f32) {
    for leaf in 0..7 {
        let angle = leaf as f32 * std::f32::consts::TAU / 7. + variation * 6.;
        let forward = Vec3::new(angle.cos(), 0., angle.sin());
        let side = Vec3::new(-angle.sin(), 0., angle.cos());
        let length = radius * (0.82 + 0.18 * (angle * 3. + variation).sin());
        let first = mesh.positions.len() as u32;
        for segment in 0..=6 {
            let t = segment as f32 / 6.;
            let width = (std::f32::consts::PI * t).sin().max(0.).powf(0.75) * radius * 0.24;
            let lift = radius * (0.35 * (std::f32::consts::PI * t).sin() - 0.12 * t * t);
            let slope = (0.35 * std::f32::consts::PI * (std::f32::consts::PI * t).cos() - 0.24 * t)
                * radius
                / length;
            for edge in [-1_f32, 0., 1.] {
                let p = center
                    + forward * (t * length)
                    + side * width * edge
                    + Vec3::Y * (lift - width * 0.24 * edge.abs());
                let normal = (Vec3::Y - forward * slope + side * edge * 0.24).normalize();
                let shade = if edge == 0. { 1.12 } else { 0.86 };
                mesh.positions.push(p.to_array());
                mesh.normals.push(normal.to_array());
                mesh.colors.push([
                    color[0] * shade,
                    color[1] * shade,
                    color[2] * shade,
                    color[3],
                ]);
            }
        }
        for segment in 0..6 {
            for rail in 0..2 {
                let a = first + segment * 3 + rail;
                mesh.indices
                    .extend_from_slice(&[a, a + 3, a + 4, a, a + 4, a + 1]);
            }
        }
    }
}

fn branch(mesh: &mut Geometry, start: Vec3, end: Vec3, r0: f32, r1: f32, color: [f32; 4]) {
    let axis = (end - start).normalize();
    let tangent = axis
        .cross(if axis.y.abs() > 0.9 { Vec3::X } else { Vec3::Y })
        .normalize();
    let bitangent = axis.cross(tangent);
    let first = mesh.positions.len() as u32;
    for (center, radius) in [(start, r0), (end, r1)] {
        for i in 0..7 {
            let angle = i as f32 * std::f32::consts::TAU / 7.;
            let normal = tangent * angle.cos() + bitangent * angle.sin();
            mesh.positions.push((center + normal * radius).to_array());
            mesh.normals.push(normal.to_array());
            mesh.colors.push(color);
        }
    }
    for i in 0..7 {
        let j = (i + 1) % 7;
        mesh.indices.extend_from_slice(&[
            first + i,
            first + j,
            first + 7 + j,
            first + i,
            first + 7 + j,
            first + 7 + i,
        ]);
    }
}
fn crown(mesh: &mut Geometry, center: Vec3, scale: Vec3, color: [f32; 4], variation: f32) {
    let first = mesh.positions.len() as u32;
    for ring in 0..5 {
        for segment in 0..9 {
            let latitude = std::f32::consts::PI * ring as f32 / 4.;
            let angle = std::f32::consts::TAU * segment as f32 / 9.;
            let normal = Vec3::new(
                latitude.sin() * angle.cos(),
                latitude.cos(),
                latitude.sin() * angle.sin(),
            );
            let wobble =
                1. + 0.16 * (angle * 3. + variation * std::f32::consts::TAU).sin() * latitude.sin();
            mesh.positions
                .push((center + normal * scale * wobble).to_array());
            mesh.normals.push((normal / scale).normalize().to_array());
            let shade = 0.88 + 0.12 * normal.y;
            mesh.colors.push([
                color[0] * shade,
                color[1] * shade,
                color[2] * shade,
                color[3],
            ]);
        }
    }
    for ring in 0..4 {
        for segment in 0..9 {
            let a = first + ring * 9 + segment;
            let b = first + ring * 9 + (segment + 1) % 9;
            mesh.indices
                .extend_from_slice(&[a, b, b + 9, a, b + 9, a + 9]);
        }
    }
}

/// Exact triangle intersection, using the same diagonal and height vertices as authority.
/// The grid DDA visits only cells crossed by the ray's map projection.
pub(super) fn pick(terrain: &Terrain, ray: Ray3d) -> Option<Vec3> {
    let info = terrain.info();
    let spacing = info.cell_size;
    let size = info.cells as f32 * spacing;
    let origin = ray.origin;
    let dir = *ray.direction;
    let mut enter = 0_f32;
    let mut exit = 10000_f32;
    for (o, d) in [(origin.x, dir.x), (origin.z, dir.z)] {
        if d.abs() < 1e-7 {
            if o < 0. || o > size {
                return None;
            }
        } else {
            let a = -o / d;
            let b = (size - o) / d;
            enter = enter.max(a.min(b));
            exit = exit.min(a.max(b));
        }
    }
    if enter > exit {
        return None;
    }
    let p = origin + dir * enter;
    let cell = |v: f32, d: f32| {
        let grid = v / spacing;
        let index = grid.floor() as i32;
        (if d < 0. && (grid - grid.round()).abs() < 0.00001 {
            index - 1
        } else {
            index
        })
        .clamp(0, info.cells as i32 - 1)
    };
    let mut x = cell(p.x, dir.x);
    let mut z = cell(p.z, dir.z);
    let step_x = if dir.x > 0. { 1 } else { -1 };
    let step_z = if dir.z > 0. { 1 } else { -1 };
    for _ in 0..(info.cells * 2 + 4) {
        if x < 0 || z < 0 || x >= info.cells as i32 || z >= info.cells as i32 {
            break;
        }
        let point = |dx: i32, dz: i32| {
            let wx = (x + dx) as f32 * spacing;
            let wz = (z + dz) as f32 * spacing;
            Vec3::new(
                wx,
                terrain
                    .sample(wx as f64, wz as f64)
                    .expect("validated terrain vertex")
                    .height,
                wz,
            )
        };
        let a = point(0, 0);
        let b = point(0, 1);
        let c = point(1, 1);
        let d = point(1, 0);
        for tri in [[a, b, c], [a, c, d]] {
            if let Some(hit) = triangle_hit(origin, dir, tri)
                && hit >= enter - 0.001
                && hit <= exit + 0.001
            {
                return Some(origin + dir * hit);
            }
        }
        let next_x = if dir.x.abs() < 1e-7 {
            f32::INFINITY
        } else {
            (((if dir.x > 0. { x + 1 } else { x }) as f32 * spacing) - origin.x) / dir.x
        };
        let next_z = if dir.z.abs() < 1e-7 {
            f32::INFINITY
        } else {
            (((if dir.z > 0. { z + 1 } else { z }) as f32 * spacing) - origin.z) / dir.z
        };
        let next = next_x.min(next_z);
        if !next.is_finite() || next > exit + 0.001 {
            break;
        }
        if next_x <= next_z {
            x += step_x;
        }
        if next_z <= next_x {
            z += step_z;
        }
    }
    None
}
fn triangle_hit(origin: Vec3, dir: Vec3, vertices: [Vec3; 3]) -> Option<f32> {
    let [a, b, c] = vertices;
    let e1 = b - a;
    let e2 = c - a;
    let p = dir.cross(e2);
    let det = e1.dot(p);
    if det.abs() < 1e-7 {
        return None;
    }
    let inverse = 1. / det;
    let delta = origin - a;
    let u = delta.dot(p) * inverse;
    let q = delta.cross(e1);
    let v = dir.dot(q) * inverse;
    if u < -1e-5 || v < -1e-5 || u + v > 1.00001 {
        return None;
    }
    let t = e2.dot(q) * inverse;
    (t >= 0.).then_some(t)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn transition_fixture() -> (App, tempfile::TempDir, String) {
        use crate::client::{ClientConfig, ClientState};
        let directory = tempfile::tempdir().unwrap();
        let mut state = ClientState::new(ClientConfig {
            server: "127.0.0.1:4000".parse().unwrap(),
            session: directory.path().join("session.json"),
        })
        .unwrap();
        let terrain = Terrain::generate(72913);
        let fingerprint = terrain.info().fingerprint;
        state.terrain_info = Some(terrain.info());
        state.terrain = Some(terrain);
        state.ready = true;
        state.welcomed = true;
        state.terrain_manifest_received = true;
        state.player = Some(argon_dev::world::Player {
            id: "camera-test".into(),
            name: "Camera test".into(),
            x: 1536.,
            y: 1536.,
        });
        let mut app = App::new();
        app.init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<StandardMaterial>>()
            .init_resource::<Assets<Image>>()
            .init_resource::<Landscape>()
            .init_resource::<ViewState>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<Time>()
            .add_message::<bevy::input::mouse::MouseWheel>()
            .insert_resource(state)
            .add_systems(Startup, setup)
            .add_systems(
                Update,
                (
                    super::super::legacy_ground,
                    super::super::camera_control,
                    stream,
                )
                    .chain(),
            );
        app.world_mut().spawn((
            super::super::ArenaCamera,
            Transform::default(),
            DistanceFog::default(),
        ));
        app.update();
        (app, directory, fingerprint)
    }
    fn resident_handles(app: &App) -> Vec<Handle<Mesh>> {
        let landscape = app.world().resource::<Landscape>();
        let mut meshes: Vec<_> = landscape
            .chunks
            .values()
            .flat_map(|r| r.meshes.iter().cloned())
            .collect();
        meshes.push(landscape.ocean.as_ref().unwrap().1.clone());
        meshes
    }
    #[test]
    fn manifest_replacement_retires_a_while_b_downloads_and_recenters_on_b() {
        let (mut app, _directory, a) = transition_fixture();
        assert_eq!(app.world().resource::<Landscape>().fingerprint, a);
        let old_meshes = resident_handles(&app);
        let focus_a = app.world().resource::<ViewState>().focus;
        let b = Terrain::generate(72914);
        let b_fingerprint = b.info().fingerprint;
        app.world_mut()
            .resource_mut::<ClientState>()
            .accept(crate::protocol::ServerUpdate::TerrainManifest {
                info: Some(b.info()),
            })
            .unwrap();
        app.update();
        let landscape = app.world().resource::<Landscape>();
        assert!(
            landscape.chunks.is_empty(),
            "old native chunks remained during replacement download"
        );
        assert!(
            landscape.ocean.is_none(),
            "old ocean remained during replacement download"
        );
        for mesh in old_meshes {
            assert!(
                app.world().resource::<Assets<Mesh>>().get(&mesh).is_none(),
                "old geometry retained in Assets"
            );
        }
        let view = app.world().resource::<ViewState>();
        assert_eq!(
            view.focus, focus_a,
            "native download clamped the camera into legacy coordinates"
        );
        assert!(
            !view.centered,
            "replacement geometry must get a fresh center"
        );
        {
            let mut state = app.world_mut().resource_mut::<ClientState>();
            state.player.as_mut().unwrap().x = 1720.;
            state.player.as_mut().unwrap().y = 1660.;
            state.terrain = Some(b);
            state.ready = true;
        }
        app.update();
        assert_eq!(
            app.world().resource::<Landscape>().fingerprint,
            b_fingerprint
        );
        let view = app.world().resource::<ViewState>();
        assert!(view.centered);
        assert_eq!((view.focus.x, view.focus.z), (1720., 1660.));
        assert_eq!(view.distance, 150.);
    }
    #[test]
    fn native_to_legacy_retires_native_geometry_and_recenters_at_legacy_scale() {
        let (mut app, _directory, _) = transition_fixture();
        let old_meshes = resident_handles(&app);
        {
            let mut state = app.world_mut().resource_mut::<ClientState>();
            state
                .accept(crate::protocol::ServerUpdate::TerrainManifest { info: None })
                .unwrap();
            state.player.as_mut().unwrap().x = 440.;
            state.player.as_mut().unwrap().y = 230.;
        }
        app.update();
        let landscape = app.world().resource::<Landscape>();
        assert!(landscape.chunks.is_empty());
        assert!(landscape.ocean.is_none());
        for mesh in old_meshes {
            assert!(app.world().resource::<Assets<Mesh>>().get(&mesh).is_none());
        }
        let view = app.world().resource::<ViewState>();
        assert_eq!((view.focus.x, view.focus.z), (-6., -7.));
        assert_eq!(view.distance, 62.);
        // A later native manifest must also hide the retained legacy arena.
        let b = Terrain::generate(72914);
        app.world_mut()
            .resource_mut::<ClientState>()
            .accept(crate::protocol::ServerUpdate::TerrainManifest {
                info: Some(b.info()),
            })
            .unwrap();
        app.update();
        let world = app.world_mut();
        let mut query = world.query_filtered::<&Visibility, With<super::super::LegacyGround>>();
        let visibility: Vec<_> = query.iter(world).collect();
        assert!(!visibility.is_empty());
        assert!(visibility.into_iter().all(|v| *v == Visibility::Hidden));
    }
    #[test]
    fn mesh_triangles_match_authoritative_height_and_adjacent_normals() {
        let terrain = Terrain::generate(72913);
        let a = ground_geometry(&terrain, &terrain.chunk(11, 11).unwrap());
        let b = ground_geometry(&terrain, &terrain.chunk(12, 11).unwrap());
        for row in 0..17 {
            assert_eq!(a.positions[row * 17 + 16], b.positions[row * 17]);
            assert_eq!(a.normals[row * 17 + 16], b.normals[row * 17]);
            assert_eq!(a.colors[row * 17 + 16], b.colors[row * 17]);
        }
        for tri in a.indices.chunks_exact(3) {
            let points: Vec<Vec3> = tri
                .iter()
                .map(|i| Vec3::from(a.positions[*i as usize]))
                .collect();
            let p = points[0] * 0.2 + points[1] * 0.3 + points[2] * 0.5;
            let expected = terrain.sample(p.x as f64, p.z as f64).unwrap().height;
            assert!(
                (p.y - expected).abs() < 0.001,
                "render triangle {p:?} vs authority {expected}"
            );
        }
    }
    #[test]
    fn ray_picking_hits_actual_triangles_from_vertical_and_oblique_views() {
        let terrain = Terrain::generate(72913);
        for target in [
            Vec2::new(1536., 1536.),
            Vec2::new(1707.3, 1492.7),
            Vec2::new(128., 128.),
            Vec2::new(0., 0.),
            Vec2::new(3072., 3072.),
        ] {
            let height = terrain
                .sample(target.x as f64, target.y as f64)
                .unwrap()
                .height;
            let target = Vec3::new(target.x, height, target.y);
            for offset in [Vec3::new(0., 150., 0.), Vec3::new(-130., 180., 160.)] {
                let ray = Ray3d::new(target + offset, Dir3::new(-offset).unwrap());
                let hit = pick(&terrain, ray).unwrap_or_else(|| {
                    panic!("ground ray missed target {target:?}, offset {offset:?}")
                });
                assert!(
                    hit.distance(target) < 0.04,
                    "ray hit {hit:?}, expected {target:?}"
                );
            }
        }
        assert!(pick(&terrain, Ray3d::new(Vec3::new(-1., 100., -1.), Dir3::NEG_Y)).is_none());
        assert!(pick(&terrain, Ray3d::new(Vec3::new(1536., 500., 1536.), Dir3::Y)).is_none());
    }
    #[test]
    fn decorative_geometry_is_deterministic_finite_and_bounded() {
        let terrain = Terrain::generate(72913);
        for key in [(12, 12), (14, 11), (19, 12)] {
            let chunk = terrain.chunk(key.0, key.1).unwrap();
            let first = decoration(&terrain, &chunk);
            let second = decoration(&terrain, &chunk);
            assert!(first.iter().map(|g| g.positions.len()).sum::<usize>() <= 100000);
            for (mesh, repeat) in first.iter().zip(second) {
                assert_eq!(mesh.positions, repeat.positions);
                assert_eq!(mesh.positions.len(), mesh.normals.len());
                assert_eq!(mesh.positions.len(), mesh.colors.len());
                assert!(mesh.positions.iter().flatten().all(|v| v.is_finite()));
                assert!(
                    mesh.indices
                        .iter()
                        .all(|i| (*i as usize) < mesh.positions.len())
                );
                assert!(
                    mesh.normals
                        .iter()
                        .all(|n| (Vec3::from(*n).length() - 1.).abs() < 0.001)
                );
            }
        }
    }
    #[test]
    fn biome_material_transition_has_no_palette_step() {
        for i in 0..4 {
            let mut previous = None;
            for step in 0..=100 {
                let mut weights = [0.; 5];
                weights[i] = 1. - step as f32 / 100.;
                weights[i + 1] = step as f32 / 100.;
                let color = ground_color(weights, 10., 0.5, 1., 1700., 1600.);
                if let Some(last) = previous {
                    let last: [f32; 4] = last;
                    assert!(color.iter().zip(last).all(|(a, b)| (a - b).abs() < 0.01));
                }
                previous = Some(color);
            }
        }
    }
}
