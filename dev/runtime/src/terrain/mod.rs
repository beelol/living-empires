//! Baked terrain v1. Map x/y become renderer x/z; all distances are metres.
use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{HashSet, VecDeque},
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::Path,
};

pub const WORLD_SIZE: f32 = 3072.0;
pub const CELL_SIZE: f32 = 8.0;
pub const CELLS: u32 = 384;
pub const CHUNK_CELLS: u32 = 16;
const STRIDE: usize = CELLS as usize + 1;
const VERTICES: usize = STRIDE * STRIDE;
const MAX_SLOPE: f32 = 0.7;
const MIN_LAND_HEIGHT: f32 = 0.2;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, Hash)]
pub enum Biome {
    Coast,
    Grassland,
    Wetland,
    Jungle,
    LuminousGrove,
}
const BIOMES: [Biome; 5] = [
    Biome::Coast,
    Biome::Grassland,
    Biome::Wetland,
    Biome::Jungle,
    Biome::LuminousGrove,
];

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct TerrainInfo {
    pub schema_version: u32,
    pub generator_version: u32,
    pub cells: u32,
    pub cell_size: f32,
    pub chunk_cells: u32,
    pub sea_level: f32,
    pub fingerprint: String,
}
impl TerrainInfo {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.schema_version == 1 && self.generator_version > 0,
            "unsupported terrain version"
        );
        ensure!(
            self.cells == CELLS
                && self.cell_size == CELL_SIZE
                && self.chunk_cells == CHUNK_CELLS
                && self.sea_level == 0.0,
            "unsupported terrain dimensions"
        );
        ensure!(
            self.fingerprint.len() == 64 && self.fingerprint.bytes().all(|b| b.is_ascii_hexdigit()),
            "invalid terrain fingerprint"
        );
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct TerrainChunk {
    pub chunk_x: u32,
    pub chunk_y: u32,
    pub heights: Vec<f32>,
    pub moisture: Vec<f32>,
    pub geology: Vec<f32>,
    pub biome_weights: Vec<[f32; 5]>,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq)]
pub struct Surface {
    pub height: f32,
    /// Rise/run of the exact triangle; not degrees.
    pub slope: f32,
    pub water_depth: f32,
    pub biome: Biome,
    pub biome_weights: [f32; 5],
    pub moisture: f32,
    pub geology: f32,
    pub movement_multiplier: f32,
    pub traversable: bool,
}
#[derive(Clone, Debug)]
pub struct Terrain {
    seed: Option<u64>,
    deposits: Vec<Deposit>,
    info: TerrainInfo,
    heights: Vec<f32>,
    moisture: Vec<f32>,
    geology: Vec<f32>,
    biome_weights: Vec<[f32; 5]>,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub enum DepositKind {
    Metal,
    Crystal,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Deposit {
    pub id: u64,
    pub x: f64,
    pub y: f64,
    pub kind: DepositKind,
    pub initial_quantity: u64,
    pub remaining: u64,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct SavedTerrain {
    seed: u64,
    info: TerrainInfo,
    heights: Vec<f32>,
    moisture: Vec<f32>,
    geology: Vec<f32>,
    biome_weights: Vec<[f32; 5]>,
    deposits: Vec<Deposit>,
}
const MAX_SAVE_BYTES: u64 = 64 * 1024 * 1024;
impl Terrain {
    pub fn deposits(&self) -> &[Deposit] {
        &self.deposits
    }
    /// Authority primitive only: caller must enforce ownership, extraction costs,
    /// idempotency and durable commit before acknowledging a gameplay action.
    pub fn deplete_deposit(&mut self, id: u64, quantity: u64) -> Result<()> {
        let deposit = self
            .deposits
            .iter_mut()
            .find(|d| d.id == id)
            .context("unknown deposit")?;
        ensure!(
            quantity <= deposit.remaining,
            "deposit has insufficient remaining quantity"
        );
        deposit.remaining -= quantity;
        Ok(())
    }
    /// Atomic replace and fsync. A failed or absent load NEVER generates terrain.
    /// Public reconstructed geometry cannot overwrite an authoritative save.
    pub fn save(&self, path: impl AsRef<Path>) -> Result<()> {
        let seed = self
            .seed
            .context("public geometry cannot be saved as authoritative terrain")?;
        self.validate()?;
        let payload = serde_json::to_vec(&SavedTerrain {
            seed,
            info: self.info.clone(),
            heights: self.heights.clone(),
            moisture: self.moisture.clone(),
            geology: self.geology.clone(),
            biome_weights: self.biome_weights.clone(),
            deposits: self.deposits.clone(),
        })?;
        ensure!(
            payload.len() as u64 <= MAX_SAVE_BYTES,
            "terrain exceeds save limit"
        );
        let path = path.as_ref();
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        fs::create_dir_all(parent)?;
        let mut nonce = [0u8; 8];
        getrandom::fill(&mut nonce)
            .map_err(|e| anyhow::anyhow!("terrain temporary filename: {e}"))?;
        let name = path
            .file_name()
            .context("terrain path must name a file")?
            .to_string_lossy();
        let temporary = parent.join(format!(".{name}.{:016x}.tmp", u64::from_le_bytes(nonce)));
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        let result = (|| -> Result<()> {
            file.write_all(b"ARGTRN01")?;
            file.write_all(&(payload.len() as u64).to_le_bytes())?;
            file.write_all(&Sha256::digest(&payload))?;
            file.write_all(&payload)?;
            file.sync_all()?;
            fs::rename(&temporary, path)?;
            File::open(parent)?.sync_all()?;
            Ok(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        result
    }
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let mut file =
            File::open(path.as_ref()).context("cannot open terrain; refusing to regenerate")?;
        let mut header = [0u8; 48];
        file.read_exact(&mut header)
            .context("truncated terrain header")?;
        ensure!(
            &header[..8] == b"ARGTRN01",
            "unsupported terrain file format"
        );
        let length = u64::from_le_bytes(header[8..16].try_into().unwrap());
        ensure!(length <= MAX_SAVE_BYTES, "terrain save exceeds size limit");
        let mut payload = Vec::new();
        file.take(length + 1).read_to_end(&mut payload)?;
        ensure!(
            payload.len() as u64 == length,
            "truncated or trailing terrain data"
        );
        ensure!(
            Sha256::digest(&payload)[..] == header[16..48],
            "terrain checksum mismatch; refusing to regenerate"
        );
        let saved: SavedTerrain =
            serde_json::from_slice(&payload).context("invalid baked terrain records")?;
        let terrain = Self {
            seed: Some(saved.seed),
            info: saved.info,
            heights: saved.heights,
            moisture: saved.moisture,
            geology: saved.geology,
            biome_weights: saved.biome_weights,
            deposits: saved.deposits,
        };
        terrain.validate()?;
        Ok(terrain)
    }
    pub fn from_chunks(info: TerrainInfo, chunks: Vec<TerrainChunk>) -> Result<Self> {
        info.validate()?;
        ensure!(chunks.len() == 576, "incomplete terrain chunks");
        // Validate all records before allocating the world-size arrays.
        for chunk in &chunks {
            chunk.validate(&info)?;
        }
        let mut terrain = Self {
            seed: None,
            deposits: Vec::new(),
            info,
            heights: vec![0.0; VERTICES],
            moisture: vec![0.0; VERTICES],
            geology: vec![0.0; VERTICES],
            biome_weights: vec![[0.0; 5]; VERTICES],
        };
        let mut seen = vec![false; VERTICES];
        let mut seen_chunks = [false; 576];
        for chunk in chunks {
            let chunk_index = (chunk.chunk_y * 24 + chunk.chunk_x) as usize;
            ensure!(!seen_chunks[chunk_index], "duplicate terrain chunk");
            seen_chunks[chunk_index] = true;
            for row in 0..17 {
                for col in 0..17 {
                    let source = row * 17 + col;
                    let i = (chunk.chunk_y as usize * 16 + row) * STRIDE
                        + chunk.chunk_x as usize * 16
                        + col;
                    if seen[i] {
                        ensure!(
                            terrain.heights[i] == chunk.heights[source]
                                && terrain.moisture[i] == chunk.moisture[source]
                                && terrain.geology[i] == chunk.geology[source]
                                && terrain.biome_weights[i] == chunk.biome_weights[source],
                            "terrain chunk seam mismatch"
                        );
                    } else {
                        terrain.heights[i] = chunk.heights[source];
                        terrain.moisture[i] = chunk.moisture[source];
                        terrain.geology[i] = chunk.geology[source];
                        terrain.biome_weights[i] = chunk.biome_weights[source];
                        seen[i] = true;
                    }
                }
            }
        }
        terrain.validate()?;
        Ok(terrain)
    }
    fn validate(&self) -> Result<()> {
        self.info.validate()?;
        validate_samples(
            VERTICES,
            &self.heights,
            &self.moisture,
            &self.geology,
            &self.biome_weights,
        )?;
        ensure!(
            self.geometry_fingerprint() == self.info.fingerprint,
            "terrain geometry fingerprint mismatch"
        );
        ensure!(self.deposits.len() <= 4096, "too many deposits");
        let mut ids = HashSet::new();
        for deposit in &self.deposits {
            ensure!(
                deposit.id > 0 && ids.insert(deposit.id),
                "duplicate or invalid deposit identity"
            );
            ensure!(
                deposit.remaining <= deposit.initial_quantity
                    && deposit.initial_quantity <= 1_000_000,
                "invalid deposit quantity"
            );
            ensure!(
                self.sample(deposit.x, deposit.y)
                    .is_some_and(|s| s.traversable),
                "deposit outside traversable terrain"
            );
        }
        Ok(())
    }
    pub fn generate(seed: u64) -> Self {
        let mut terrain = Self {
            seed: Some(seed),
            deposits: Vec::new(),
            info: TerrainInfo {
                schema_version: 1,
                generator_version: 1,
                cells: CELLS,
                cell_size: CELL_SIZE,
                chunk_cells: CHUNK_CELLS,
                sea_level: 0.0,
                fingerprint: String::new(),
            },
            heights: Vec::with_capacity(VERTICES),
            moisture: Vec::with_capacity(VERTICES),
            geology: Vec::with_capacity(VERTICES),
            biome_weights: Vec::with_capacity(VERTICES),
        };
        for gy in 0..=CELLS {
            for gx in 0..=CELLS {
                let x = f64::from(gx) * 8.0;
                let y = f64::from(gy) * 8.0;
                let dx = x - 1536.0;
                let dy = y - 1536.0;
                let angle = dy.atan2(dx);
                let phase = random(seed, 0, 0) * std::f64::consts::TAU;
                let shore_radius = 1380.0
                    + 65.0 * (angle * 3.0 + phase).sin()
                    + 35.0 * (angle * 7.0 - phase).sin();
                let inland = shore_radius - dx.hypot(dy);
                let relief = 32.0
                    + 52.0 * noise(seed, x / 510.0, y / 510.0)
                    + 17.0 * noise(seed ^ 91, x / 155.0, y / 155.0);
                let coastal = (inland / 180.0).clamp(0.0, 1.0);
                let height = if inland < 0.0 {
                    (inland * 0.13).max(-48.0)
                } else {
                    inland.min(40.0) * 0.18 + relief * smooth(coastal)
                } as f32;
                let moisture = (0.15
                    + 0.68 * (x / 3072.0)
                    + 0.18 * (noise(seed ^ 19, x / 380.0, y / 380.0) - 0.5)
                    + 0.06 * (y / 3072.0)) as f32;
                let geology = noise(seed ^ 71, x / 330.0, y / 330.0) as f32;
                let grove_x = 1830.0 + 180.0 * (random(seed, 4, 1) - 0.5);
                let grove_y = 1500.0 + 300.0 * (random(seed, 4, 2) - 0.5);
                let grove_distance = ((x - grove_x) / 140.0).hypot((y - grove_y) / 210.0);
                let grove = smooth(((1.35 - grove_distance) / 0.6).clamp(0.0, 1.0)) as f32;
                let coast = (1.0 - smooth((f64::from(height) / 19.0).clamp(0.0, 1.0))) as f32;
                let wet = smooth(((f64::from(moisture) - 0.43) / 0.15).clamp(0.0, 1.0)) as f32;
                let jungle = smooth(((f64::from(moisture) - 0.61) / 0.13).clamp(0.0, 1.0)) as f32;
                let inner = (1.0 - coast) * (1.0 - grove);
                terrain.heights.push(height);
                terrain.moisture.push(moisture);
                terrain.geology.push(geology);
                terrain.biome_weights.push([
                    coast,
                    inner * (1.0 - wet),
                    inner * wet * (1.0 - jungle),
                    inner * wet * jungle,
                    (1.0 - coast) * grove,
                ]);
            }
        }
        // Jittered geology-linked deposits remain private authoritative records.
        // Their identities and remaining quantities are baked, never regenerated on load.
        for y in 1..19 {
            for x in 1..19 {
                let px = x as f64 * 154.0 + (random(seed ^ 307, x, y) - 0.5) * 70.0;
                let py = y as f64 * 154.0 + (random(seed ^ 311, x, y) - 0.5) * 70.0;
                if let Some(surface) = terrain.sample(px, py).filter(|s| s.traversable) {
                    let kind = if surface.geology > 0.65 || surface.biome == Biome::LuminousGrove {
                        DepositKind::Crystal
                    } else {
                        DepositKind::Metal
                    };
                    let quantity = if kind == DepositKind::Metal {
                        2400
                    } else {
                        700
                    };
                    terrain.deposits.push(Deposit {
                        id: y * 19 + x,
                        x: px,
                        y: py,
                        kind,
                        initial_quantity: quantity,
                        remaining: quantity,
                    });
                }
            }
        }
        terrain.info.fingerprint = terrain.geometry_fingerprint();
        terrain
    }
    pub fn info(&self) -> TerrainInfo {
        self.info.clone()
    }
    pub fn sample(&self, x: f64, y: f64) -> Option<Surface> {
        sample_grid(
            x,
            y,
            0.0,
            0.0,
            CELLS as usize,
            &self.heights,
            &self.moisture,
            &self.geology,
            &self.biome_weights,
        )
    }
    pub fn chunk(&self, x: u32, y: u32) -> Option<TerrainChunk> {
        if x >= CELLS / CHUNK_CELLS || y >= CELLS / CHUNK_CELLS {
            return None;
        }
        let mut chunk = TerrainChunk {
            chunk_x: x,
            chunk_y: y,
            heights: Vec::with_capacity(289),
            moisture: Vec::with_capacity(289),
            geology: Vec::with_capacity(289),
            biome_weights: Vec::with_capacity(289),
        };
        for row in 0..=CHUNK_CELLS {
            for col in 0..=CHUNK_CELLS {
                let i =
                    (y * CHUNK_CELLS + row) as usize * STRIDE + (x * CHUNK_CELLS + col) as usize;
                chunk.heights.push(self.heights[i]);
                chunk.moisture.push(self.moisture[i]);
                chunk.geology.push(self.geology[i]);
                chunk.biome_weights.push(self.biome_weights[i]);
            }
        }
        Some(chunk)
    }
    /// Check every crossed triangle, including zero-length and boundary moves.
    /// Water height is linear within a triangle and slope constant, so endpoints
    /// and an interior point prove the complete segment without sampling gaps.
    pub fn can_traverse(&self, from: [f64; 2], to: [f64; 2]) -> bool {
        if !self.sample(from[0], from[1]).is_some_and(|s| s.traversable)
            || !self.sample(to[0], to[1]).is_some_and(|s| s.traversable)
        {
            return false;
        }
        let mut cuts = vec![0.0, 1.0];
        for (a, b) in [
            (from[0], to[0]),
            (from[1], to[1]),
            (from[0] - from[1], to[0] - to[1]),
        ] {
            if a == b {
                continue;
            }
            let low = (a.min(b) / 8.0).ceil() as i32;
            let high = (a.max(b) / 8.0).floor() as i32;
            for k in low..=high {
                let t = (f64::from(k) * 8.0 - a) / (b - a);
                if t > 0.0 && t < 1.0 {
                    cuts.push(t);
                }
            }
        }
        cuts.sort_by(f64::total_cmp);
        cuts.dedup();
        let safe = |t: f64| {
            self.sample(
                from[0] + (to[0] - from[0]) * t,
                from[1] + (to[1] - from[1]) * t,
            )
            .is_some_and(|s| s.traversable)
        };
        cuts.iter().copied().all(safe) && cuts.windows(2).all(|w| safe((w[0] + w[1]) * 0.5))
    }
    /// Conservative area: count only full cells whose two triangles are safe,
    /// and only the largest four-neighbour connected component.
    pub fn connected_land_area(&self) -> f64 {
        let n = CELLS as usize;
        let mut safe = vec![false; n * n];
        for y in 0..n {
            for x in 0..n {
                let i = y * STRIDE + x;
                let h = [
                    self.heights[i],
                    self.heights[i + 1],
                    self.heights[i + STRIDE],
                    self.heights[i + STRIDE + 1],
                ];
                safe[y * n + x] = h.iter().all(|h| *h >= MIN_LAND_HEIGHT)
                    && ((h[1] - h[0]) / 8.0).hypot((h[3] - h[1]) / 8.0) <= MAX_SLOPE
                    && ((h[3] - h[2]) / 8.0).hypot((h[2] - h[0]) / 8.0) <= MAX_SLOPE;
            }
        }
        let mut largest = 0;
        let mut queue = VecDeque::new();
        for start in 0..safe.len() {
            if !safe[start] {
                continue;
            }
            safe[start] = false;
            queue.push_back(start);
            let mut area = 0;
            while let Some(i) = queue.pop_front() {
                area += 1;
                let x = i % n;
                let y = i / n;
                for next in [
                    x.checked_sub(1).map(|v| y * n + v),
                    (x + 1 < n).then_some(i + 1),
                    y.checked_sub(1).map(|v| v * n + x),
                    (y + 1 < n).then_some(i + n),
                ]
                .into_iter()
                .flatten()
                {
                    if safe[next] {
                        safe[next] = false;
                        queue.push_back(next);
                    }
                }
            }
            largest = largest.max(area);
        }
        largest as f64 * 64.0
    }
    pub fn spawn(&self) -> [f64; 2] {
        [1536.0, 1536.0]
    }
    fn geometry_fingerprint(&self) -> String {
        let mut hash = Sha256::new();
        hash.update(b"argon-terrain-geometry-v1");
        hash.update(self.info.generator_version.to_le_bytes());
        for values in [&self.heights, &self.moisture, &self.geology] {
            for value in values {
                hash.update(value.to_bits().to_le_bytes());
            }
        }
        for weights in &self.biome_weights {
            for value in weights {
                hash.update(value.to_bits().to_le_bytes());
            }
        }
        format!("{:x}", hash.finalize())
    }
}
impl TerrainChunk {
    pub fn validate(&self, info: &TerrainInfo) -> Result<()> {
        info.validate()?;
        ensure!(
            self.chunk_x < CELLS / CHUNK_CELLS && self.chunk_y < CELLS / CHUNK_CELLS,
            "chunk coordinate out of range"
        );
        validate_samples(
            289,
            &self.heights,
            &self.moisture,
            &self.geology,
            &self.biome_weights,
        )
    }
    pub fn sample(&self, x: f64, y: f64) -> Option<Surface> {
        sample_grid(
            x,
            y,
            f64::from(self.chunk_x) * 128.0,
            f64::from(self.chunk_y) * 128.0,
            16,
            &self.heights,
            &self.moisture,
            &self.geology,
            &self.biome_weights,
        )
    }
}
fn validate_samples(
    count: usize,
    heights: &[f32],
    moisture: &[f32],
    geology: &[f32],
    weights: &[[f32; 5]],
) -> Result<()> {
    ensure!(
        heights.len() == count
            && moisture.len() == count
            && geology.len() == count
            && weights.len() == count,
        "invalid terrain sample count"
    );
    ensure!(
        heights
            .iter()
            .all(|h| h.is_finite() && (-256.0..=512.0).contains(h)),
        "invalid terrain height"
    );
    ensure!(
        moisture
            .iter()
            .chain(geology)
            .all(|v| v.is_finite() && (0.0..=1.0).contains(v)),
        "invalid terrain field"
    );
    ensure!(
        weights.iter().all(
            |w| w.iter().all(|v| v.is_finite() && (0.0..=1.0).contains(v))
                && (w.iter().sum::<f32>() - 1.0).abs() < 0.0001
        ),
        "invalid biome weights"
    );
    Ok(())
}
#[allow(clippy::too_many_arguments)]
fn sample_grid(
    x: f64,
    y: f64,
    origin_x: f64,
    origin_y: f64,
    cells: usize,
    heights: &[f32],
    moisture: &[f32],
    geology: &[f32],
    weights: &[[f32; 5]],
) -> Option<Surface> {
    let x = (x - origin_x) / 8.0;
    let y = (y - origin_y) / 8.0;
    if !x.is_finite()
        || !y.is_finite()
        || x < 0.0
        || y < 0.0
        || x > cells as f64
        || y > cells as f64
    {
        return None;
    }
    let stride = cells + 1;
    let col = (x.floor() as usize).min(cells - 1);
    let row = (y.floor() as usize).min(cells - 1);
    let u = (x - col as f64) as f32;
    let v = (y - row as f64) as f32;
    let i = row * stride + col;
    let (indices, bary, dx, dy) = if u >= v {
        (
            [i, i + 1, i + stride + 1],
            [1.0 - u, u - v, v],
            (*heights.get(i + 1)? - *heights.get(i)?) / 8.0,
            (*heights.get(i + stride + 1)? - *heights.get(i + 1)?) / 8.0,
        )
    } else {
        (
            [i, i + stride, i + stride + 1],
            [1.0 - v, v - u, u],
            (*heights.get(i + stride + 1)? - *heights.get(i + stride)?) / 8.0,
            (*heights.get(i + stride)? - *heights.get(i)?) / 8.0,
        )
    };
    let interp = |values: &[f32]| -> Option<f32> {
        Some(
            *values.get(indices[0])? * bary[0]
                + *values.get(indices[1])? * bary[1]
                + *values.get(indices[2])? * bary[2],
        )
    };
    let height = interp(heights)?;
    let mut blend = [0.0; 5];
    for (j, weight) in blend.iter_mut().enumerate() {
        for k in 0..3 {
            *weight += weights.get(indices[k])?[j] * bary[k];
        }
    }
    let index = (0..5)
        .max_by(|a, b| blend[*a].total_cmp(&blend[*b]))
        .unwrap();
    let slope = dx.hypot(dy);
    Some(Surface {
        height,
        slope,
        water_depth: (-height).max(0.0),
        biome: BIOMES[index],
        biome_weights: blend,
        moisture: interp(moisture)?,
        geology: interp(geology)?,
        movement_multiplier: blend
            .iter()
            .zip([0.9, 1.0, 0.62, 0.78, 0.85])
            .map(|(w, s)| w * s)
            .sum(),
        traversable: height >= MIN_LAND_HEIGHT && slope <= MAX_SLOPE,
    })
}
fn smooth(t: f64) -> f64 {
    t * t * (3.0 - 2.0 * t)
}
fn random(seed: u64, x: u64, y: u64) -> f64 {
    let mut z = seed
        .wrapping_add(x.wrapping_mul(0x9e3779b97f4a7c15))
        .wrapping_add(y.wrapping_mul(0xbf58476d1ce4e5b9));
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
    z ^= z >> 31;
    (z >> 11) as f64 / (1u64 << 53) as f64
}
fn noise(seed: u64, x: f64, y: f64) -> f64 {
    let ix = x.floor() as u64;
    let iy = y.floor() as u64;
    let u = smooth(x.fract());
    let v = smooth(y.fract());
    let a = random(seed, ix, iy) * (1.0 - u) + random(seed, ix + 1, iy) * u;
    let b = random(seed, ix, iy + 1) * (1.0 - u) + random(seed, ix + 1, iy + 1) * u;
    a * (1.0 - v) + b * v
}
