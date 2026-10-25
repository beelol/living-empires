use crate::{content::ContentRuntime, terrain::Terrain};
use anyhow::{bail, ensure, Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::HashSet,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};

pub const WIDTH: f64 = 1000.0;
pub const HEIGHT: f64 = 600.0;
const MAX_PLAYERS: usize = 256;
const MAX_SAVE_BYTES: u64 = 1_048_576;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Player {
    pub id: String,
    pub name: String,
    pub x: f64,
    pub y: f64,
}

#[derive(Serialize)]
pub struct Joined {
    pub token: String,
    pub player: Player,
    pub next_sequence: u64,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct SavedPlayer {
    player: Player,
    token_hash: String,
    last_move: Option<SavedMove>,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct SavedMove {
    sequence: u64,
    dx: f64,
    dy: f64,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct SavedWorld {
    schema_version: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    settlements: Option<crate::settlement::SettlementState>,
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    builders: std::collections::BTreeMap<String, [f64; 2]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    terrain: Option<SavedTerrain>,
    next_id: u64,
    players: Vec<SavedPlayer>,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct SavedTerrain {
    seed: u64,
    fingerprint: String,
}

pub struct World {
    path: PathBuf,
    saved: SavedWorld,
    terrain: Option<Terrain>,
    _lock: File,
}

impl Drop for World {
    fn drop(&mut self) {
        // Explicit unlock also releases the lock while a concurrently spawned child
        // briefly holds an inherited descriptor before exec closes it.
        let _ = fs2::FileExt::unlock(&self._lock);
    }
}

impl World {
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        fs::create_dir_all(parent).context("cannot create state directory")?;
        let mut lock_name = path
            .file_name()
            .context("state must name a file")?
            .to_os_string();
        lock_name.push(".lock");
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(parent.join(lock_name))?;
        fs2::FileExt::try_lock_exclusive(&lock)
            .context("another host already owns this saved world")?;
        let saved = match File::open(&path) {
            Ok(file) => {
                let mut bytes = Vec::new();
                file.take(MAX_SAVE_BYTES + 1).read_to_end(&mut bytes)?;
                ensure!(
                    bytes.len() as u64 <= MAX_SAVE_BYTES,
                    "saved world exceeds size limit"
                );
                let saved: SavedWorld = serde_json::from_slice(&bytes).with_context(|| {
                    format!(
                        "cannot read saved world {}; refusing to reset it",
                        path.display()
                    )
                })?;
                validate_saved(&saved)?;
                saved
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => SavedWorld {
                schema_version: 1,
                settlements: None,
                builders: Default::default(),
                terrain: None,
                next_id: 1,
                players: Vec::new(),
            },
            Err(error) => return Err(error).context("cannot open saved world"),
        };
        let terrain = saved
            .terrain
            .as_ref()
            .map(|record| -> Result<Terrain> {
                let terrain = Terrain::load(terrain_path(&path)).context(
                    "saved terrain is missing or corrupt; refusing to regenerate geography",
                )?;
                ensure!(
                    terrain.info().fingerprint == record.fingerprint,
                    "terrain fingerprint differs from saved world; refusing changed geography"
                );
                Ok(terrain)
            })
            .transpose()?;
        Ok(Self {
            path,
            saved,
            terrain,
            _lock: lock,
        })
    }

    /// Create terrain only for a new native world. Established v1 arenas retain
    /// their old geography; an explicit seed cannot silently reinterpret them.
    pub fn load_terrain(path: impl AsRef<Path>, seed: Option<u64>) -> Result<Self> {
        let path = path.as_ref();
        let existed = path.try_exists().context("cannot inspect world save")?;
        let mut world = Self::load(path)?;
        if existed {
            if let Some(requested) = seed {
                let record = world.saved.terrain.as_ref().context(
                    "this saved world is a legacy arena; choose a new --state path for terrain",
                )?;
                ensure!(record.seed == requested,
                    "seed differs from saved world; choose a new --state path instead of changing geography");
            }
            return Ok(world);
        }
        let sidecar = terrain_path(path);
        ensure!(!sidecar.try_exists()?,
            "terrain exists without its world save; restore the world save or use a new --state path");
        let seed = seed.unwrap_or(20261025);
        let terrain = Terrain::generate(seed);
        terrain.save(&sidecar)?;
        let mut next = world.saved.clone();
        next.schema_version = 2;
        next.terrain = Some(SavedTerrain {
            seed,
            fingerprint: terrain.info().fingerprint,
        });
        // Geometry is durable before a world can reference it or acknowledge a join.
        world.commit(next)?;
        world.terrain = Some(terrain);
        Ok(world)
    }

    pub fn terrain(&self) -> Option<&Terrain> {
        self.terrain.as_ref()
    }

    pub fn buildings(&self) -> Vec<crate::settlement::BuildingView> {
        self.saved
            .settlements
            .as_ref()
            .map_or_else(Vec::new, |s| s.snapshot())
    }

    pub fn faction_metal(&self, faction: &str) -> Option<u64> {
        self.saved
            .settlements
            .as_ref()
            .and_then(|s| s.metal(faction))
    }

    /// Explicit authored scenario setup; never reinterpret a legacy arena.
    pub fn initialize_settlements(&mut self) -> Result<()> {
        if self.saved.settlements.is_some() {
            return Ok(());
        }
        let terrain = self
            .terrain
            .as_ref()
            .context("settlements require a terrain world")?;
        let spawn = terrain.spawn();
        let mut next = self.saved.clone();
        let mut state = crate::settlement::SettlementState::new();
        for (id, offset) in [
            ("helios", [-96.0, 0.0]),
            ("tide", [96.0, 0.0]),
            ("cinder", [0.0, 160.0]),
        ] {
            let mut anchor_found = None;
            'search: for radius in 0i32..16 {
                for y in -radius..=radius {
                    for x in -radius..=radius {
                        if x.abs() != radius && y.abs() != radius {
                            continue;
                        }
                        let center = [
                            spawn[0] + offset[0] + f64::from(x) * 8.0,
                            spawn[1] + offset[1] + f64::from(y) * 8.0,
                        ];
                        let anchor = [center[0], center[1] - 24.0];
                        if state
                            .start(
                                terrain,
                                id,
                                "starter-core",
                                crate::settlement::Role::Core,
                                center,
                                anchor,
                                state.revision(),
                            )
                            .is_ok()
                        {
                            anchor_found = Some(anchor);
                            break 'search;
                        }
                    }
                }
            }
            next.builders.insert(
                id.into(),
                anchor_found.context("no legal starter core site; world unchanged")?,
            );
        }
        state.validate()?;
        next.schema_version = 3;
        next.settlements = Some(state);
        self.commit(next)
    }

    /// Trusted faction policy entry point; never pass a client-selected faction.
    pub fn plan_settlement(
        &mut self,
        faction: &str,
        request_id: &str,
        role: crate::settlement::Role,
    ) -> Result<Option<String>> {
        use crate::city::{CityPlanner, LayoutRequest, Rect, Species};
        let state = self
            .saved
            .settlements
            .as_ref()
            .context("settlements not initialized")?;
        if let Some(previous) = state.request(faction, request_id) {
            ensure!(previous.role == role, "request ID reused with another role");
            return Ok(Some(previous.id.clone()));
        }
        let snapshot = state.snapshot();
        let core = snapshot
            .iter()
            .find(|b| b.faction_id == faction && b.role == crate::settlement::Role::Core)
            .context("unknown faction core")?;
        ensure!(core.complete, "core still under construction");
        let anchor = *self
            .saved
            .builders
            .get(faction)
            .context("missing authoritative builder")?;
        let terrain = self.terrain.as_ref().context("missing terrain")?;
        let mut planner = CityPlanner::new(faction)?;
        let recipe = role.recipe();
        let request = LayoutRequest {
            faction_id: faction.into(),
            species: match core.species {
                crate::settlement::Species::Robot => Species::Robot,
                crate::settlement::Species::Organic => Species::Organic,
                crate::settlement::Species::Cyborg => Species::Cyborg,
            },
            center: core.center,
            founded: true,
            half_extents: recipe.half_extents,
            anchors: vec![anchor],
            reserved: snapshot
                .iter()
                .flat_map(|b| {
                    [
                        Rect {
                            min: [
                                b.center[0] - b.half_extents[0],
                                b.center[1] - b.half_extents[1],
                            ],
                            max: [
                                b.center[0] + b.half_extents[0],
                                b.center[1] + b.half_extents[1],
                            ],
                        },
                        Rect {
                            min: b.entrance.min,
                            max: b.entrance.max,
                        },
                        Rect {
                            min: b.approach_bounds.min,
                            max: b.approach_bounds.max,
                        },
                    ]
                })
                .collect(),
        };
        // Each rejected proposal is an isolated clone; no charge or history escapes.
        let decision = planner.propose(&request, |pose| {
            let mut candidate = state.clone();
            let id = candidate.start(
                terrain,
                faction,
                request_id,
                role,
                pose.center,
                anchor,
                state.revision(),
            )?;
            Ok((candidate, id))
        })?;
        let Some((state, id)) = decision.site else {
            return Ok(None);
        };
        let mut next = self.saved.clone();
        next.settlements = Some(state);
        self.commit(next)?;
        Ok(Some(id))
    }

    /// Called only from the server's fixed simulation schedule.
    pub fn tick_settlements(&mut self, ticks: u64) -> Result<()> {
        let mut next = self.saved.clone();
        let Some(state) = next.settlements.as_mut() else {
            return Ok(());
        };
        let changed = state.advance(ticks, |b| {
            next.builders
                .get(&b.faction_id)
                .is_some_and(|a| (a[0] - b.center[0]).hypot(a[1] - b.center[1]) <= 128.0)
        })?;
        if changed {
            self.commit(next)?;
        }
        Ok(())
    }

    pub fn snapshot(&self) -> Vec<Player> {
        self.saved
            .players
            .iter()
            .map(|p| p.player.clone())
            .collect()
    }

    pub fn has_session(&self, token: &str) -> bool {
        self.session_index(token).is_some()
    }

    pub fn join(&mut self, token: Option<&str>) -> Result<Joined> {
        if let Some(token) = token {
            let index = self
                .session_index(token)
                .context("session not found; join as a new commander")?;
            let record = &self.saved.players[index];
            return Ok(Joined {
                token: token.to_owned(),
                player: record.player.clone(),
                next_sequence: record.last_move.as_ref().map_or(1, |m| m.sequence + 1),
            });
        }
        ensure!(
            self.saved.players.len() < MAX_PLAYERS,
            "this development world is full (256 commanders)"
        );
        let token = random_token()?;
        let id = self.saved.next_id;
        let (x, y) = if let Some(terrain) = &self.terrain {
            let spawn = terrain.spawn();
            let x = spawn[0] + ((id - 1) % 6) as f64 * 4.0;
            let y = spawn[1] + (((id - 1) / 6) % 6) as f64 * 4.0;
            ensure!(
                terrain.sample(x, y).is_some_and(|s| s.traversable),
                "no traversable commander landing is available"
            );
            (x, y)
        } else {
            (
                440.0 + ((id - 1) % 6) as f64 * 24.0,
                230.0 + (((id - 1) / 6) % 6) as f64 * 24.0,
            )
        };
        let player = Player {
            id: format!("commander-{id}"),
            name: format!("Commander {id}"),
            x,
            y,
        };
        let mut next = self.saved.clone();
        next.next_id = id
            .checked_add(1)
            .context("commander identity limit reached")?;
        next.players.push(SavedPlayer {
            player: player.clone(),
            token_hash: hash_token(&token),
            last_move: None,
        });
        self.commit(next)?;
        Ok(Joined {
            token,
            player,
            next_sequence: 1,
        })
    }

    pub fn move_player(
        &mut self,
        token: &str,
        sequence: u64,
        dx: f64,
        dy: f64,
        runtime: &ContentRuntime,
    ) -> Result<Player> {
        let index = self.session_index(token).context("invalid session")?;
        ensure!(
            dx.is_finite() && dy.is_finite() && dx.hypot(dy) <= 1.000_001,
            "movement must be a finite direction of length at most one"
        );
        ensure!(
            sequence > 0 && sequence < 9_007_199_254_740_991,
            "invalid movement sequence"
        );
        let last = &self.saved.players[index].last_move;
        if let Some(last) = last {
            if sequence == last.sequence {
                ensure!(
                    dx == last.dx && dy == last.dy,
                    "movement sequence already used for a different move"
                );
                return Ok(self.saved.players[index].player.clone());
            }
        }
        ensure!(
            sequence == last.as_ref().map_or(1, |m| m.sequence + 1),
            "stale or out-of-order movement sequence; reconnect to synchronize"
        );
        let old = &self.saved.players[index].player;
        let (x, y) = runtime.step(old.x, old.y, dx, dy)?;
        ensure!(
            x.is_finite() && y.is_finite() && (x - old.x).hypot(y - old.y) <= 128.0,
            "content returned an invalid or excessive movement step"
        );
        let (x, y) = if let Some(terrain) = &self.terrain {
            let surface = terrain
                .sample(old.x, old.y)
                .context("commander is outside saved terrain")?;
            let factor = f64::from(surface.movement_multiplier);
            let target = [old.x + (x - old.x) * factor, old.y + (y - old.y) * factor];
            ensure!(
                terrain.can_traverse([old.x, old.y], target),
                "unreachable: route crosses deep water, steep ground, or the world boundary"
            );
            (target[0], target[1])
        } else {
            (x.clamp(12.0, WIDTH - 12.0), y.clamp(12.0, HEIGHT - 12.0))
        };
        ensure!(
            !self
                .saved
                .settlements
                .as_ref()
                .is_some_and(|s| s.blocks_movement([old.x, old.y], [x, y])),
            "route crosses a reserved building footprint"
        );
        let mut next = self.saved.clone();
        let moved = &mut next.players[index].player;
        moved.x = x;
        moved.y = y;
        let moved = moved.clone();
        next.players[index].last_move = Some(SavedMove { sequence, dx, dy });
        self.commit(next)?;
        Ok(moved)
    }

    fn session_index(&self, token: &str) -> Option<usize> {
        if token.len() != 64 || !token.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        let hash = hash_token(token);
        self.saved.players.iter().position(|p| p.token_hash == hash)
    }

    fn commit(&mut self, next: SavedWorld) -> Result<()> {
        let bytes = serde_json::to_vec_pretty(&next)?;
        ensure!(
            (bytes.len() as u64) < MAX_SAVE_BYTES,
            "world save exceeds size limit"
        );
        let parent = self
            .path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        fs::create_dir_all(parent).context("cannot create state directory")?;
        // A temporary file beside the destination makes rename an atomic replacement.
        let temp = parent.join(format!(".argon-save-{}.tmp", random_token()?));
        let result = (|| -> Result<()> {
            let mut options = OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            let mut file = options
                .open(&temp)
                .context("cannot create temporary world save")?;
            file.write_all(&bytes)?;
            file.write_all(b"\n")?;
            file.sync_all().context("cannot flush world save")?;
            fs::rename(&temp, &self.path).context("cannot replace world save")?;
            #[cfg(unix)]
            File::open(parent)?
                .sync_all()
                .context("cannot flush state directory")?;
            Ok(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temp);
        }
        result?;
        self.saved = next;
        Ok(())
    }
}

fn terrain_path(world: &Path) -> PathBuf {
    let mut name = world.as_os_str().to_os_string();
    name.push(".terrain.json");
    PathBuf::from(name)
}

fn random_token() -> Result<String> {
    let mut bytes = [0_u8; 32];
    getrandom::fill(&mut bytes).map_err(|e| anyhow::anyhow!("cannot generate session: {e}"))?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

fn hash_token(token: &str) -> String {
    format!("{:x}", Sha256::digest(token.as_bytes()))
}

fn validate_saved(saved: &SavedWorld) -> Result<()> {
    ensure!(
        (saved.schema_version == 1 && saved.terrain.is_none() && saved.settlements.is_none())
            || (saved.schema_version == 2
                && saved.terrain.is_some()
                && saved.settlements.is_none())
            || (saved.schema_version == 3
                && saved.terrain.is_some()
                && saved.settlements.is_some()),
        "unsupported saved-world schema; refusing to reset it"
    );
    if let Some(state) = &saved.settlements {
        state.validate()?;
        ensure!(saved.builders.len() == 3, "invalid saved builder roster");
        for id in ["helios", "tide", "cinder"] {
            ensure!(
                saved
                    .builders
                    .get(id)
                    .is_some_and(|a| a.iter().all(|v| v.is_finite()
                        && *v >= 0.0
                        && *v <= f64::from(crate::terrain::WORLD_SIZE))),
                "invalid saved builder anchor"
            );
        }
    } else {
        ensure!(saved.builders.is_empty(), "orphan saved builders");
    }
    ensure!(
        saved.next_id > 0 && saved.players.len() <= MAX_PLAYERS,
        "invalid saved-world identity state"
    );
    if let Some(terrain) = &saved.terrain {
        ensure!(
            terrain.fingerprint.len() == 64
                && terrain.fingerprint.bytes().all(|b| b.is_ascii_hexdigit()),
            "invalid saved terrain fingerprint"
        );
    }
    let (min, width, height) = if saved.terrain.is_some() {
        (
            0.0,
            f64::from(crate::terrain::WORLD_SIZE),
            f64::from(crate::terrain::WORLD_SIZE),
        )
    } else {
        (12.0, WIDTH - 12.0, HEIGHT - 12.0)
    };
    let mut ids = HashSet::new();
    let mut tokens = HashSet::new();
    for record in &saved.players {
        let player = &record.player;
        let number = player
            .id
            .strip_prefix("commander-")
            .and_then(|n| n.parse::<u64>().ok());
        ensure!(
            number.is_some_and(|n| n > 0 && n < saved.next_id),
            "invalid saved commander identity"
        );
        ensure!(
            ids.insert(&player.id) && tokens.insert(&record.token_hash),
            "duplicate saved commander or session"
        );
        ensure!(
            record.token_hash.len() == 64
                && record.token_hash.bytes().all(|b| b.is_ascii_hexdigit()),
            "invalid saved session"
        );
        ensure!(
            player.name.len() <= 80 && !player.name.is_empty(),
            "invalid saved commander name"
        );
        if let Some(last) = &record.last_move {
            ensure!(
                last.sequence > 0
                    && last.sequence < 9_007_199_254_740_991
                    && last.dx.is_finite()
                    && last.dy.is_finite()
                    && last.dx.hypot(last.dy) <= 1.000_001,
                "invalid saved movement sequence"
            );
        }
        if !player.x.is_finite()
            || !player.y.is_finite()
            || !(min..=width).contains(&player.x)
            || !(min..=height).contains(&player.y)
        {
            bail!("invalid saved commander position");
        }
    }
    Ok(())
}

#[cfg(test)]
mod terrain_tests {
    use super::*;

    fn rules(dir: &Path) -> ContentRuntime {
        fs::write(
            dir.join("manifest.json"),
            r#"{"schema_version":1,"entry":"rules.lua","files":["rules.lua"]}"#,
        )
        .unwrap();
        fs::write(dir.join("rules.lua"), "return {schema_version=1,world_name='Terrain test',step=function(x,y,dx,dy) return x+dx*12,y+dy*12 end}").unwrap();
        ContentRuntime::load(dir).unwrap()
    }

    #[test]
    fn native_world_preserves_seed_geography_identity_and_retry_after_restart() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("world.json");
        let runtime = rules(dir.path());
        let mut world = World::load_terrain(&path, Some(928751)).unwrap();
        let fingerprint = world.terrain().unwrap().info().fingerprint;
        let joined = world.join(None).unwrap();
        assert!(
            world
                .terrain()
                .unwrap()
                .sample(joined.player.x, joined.player.y)
                .unwrap()
                .traversable
        );
        let moved = world
            .move_player(&joined.token, 1, 1.0, 0.0, &runtime)
            .unwrap();
        drop(world);
        let mut restored = World::load_terrain(&path, None).unwrap();
        assert_eq!(restored.terrain().unwrap().info().fingerprint, fingerprint);
        assert_eq!(restored.join(Some(&joined.token)).unwrap().player, moved);
        assert_eq!(
            restored
                .move_player(&joined.token, 1, 1.0, 0.0, &runtime)
                .unwrap(),
            moved
        );
        drop(restored);
        assert!(
            World::load_terrain(&path, Some(1)).is_err(),
            "a seed change must never regenerate established terrain"
        );
    }

    #[test]
    fn existing_development_world_keeps_its_geography_and_identity() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("world.json");
        let mut old = World::load(&path).unwrap();
        let joined = old.join(None).unwrap();
        drop(old);
        let mut world = World::load_terrain(&path, None).unwrap();
        assert!(world.terrain().is_none());
        assert_eq!(
            world.join(Some(&joined.token)).unwrap().player,
            joined.player
        );
        drop(world);
        assert!(
            World::load_terrain(&path, Some(55)).is_err(),
            "explicit terrain creation cannot overwrite an established legacy world"
        );
    }

    #[test]
    fn missing_or_corrupt_terrain_is_never_regenerated() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("world.json");
        let mut world = World::load_terrain(&path, Some(44019)).unwrap();
        world.join(None).unwrap();
        drop(world);
        let sidecar = dir.path().join("world.json.terrain.json");
        let terrain_bytes = fs::read(&sidecar).unwrap();
        let world_bytes = fs::read(&path).unwrap();
        fs::remove_file(&sidecar).unwrap();
        assert!(World::load_terrain(&path, None).is_err());
        assert!(!sidecar.exists());
        fs::write(&sidecar, b"{truncated terrain").unwrap();
        assert!(World::load_terrain(&path, None).is_err());
        assert_eq!(fs::read(&sidecar).unwrap(), b"{truncated terrain");
        assert_eq!(fs::read(&path).unwrap(), world_bytes);
        fs::write(&sidecar, terrain_bytes).unwrap();
        assert!(World::load_terrain(&path, None).is_ok());
    }

    #[test]
    fn rejected_water_crossing_preserves_position_and_sequence() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("world.json");
        let runtime = rules(dir.path());
        let mut world = World::load_terrain(&path, Some(8170321)).unwrap();
        let joined = world.join(None).unwrap();
        let terrain = world.terrain().unwrap();
        let x = (1..1536)
            .find(|x| terrain.sample(f64::from(*x), 1536.0).unwrap().traversable)
            .unwrap() as f64
            + 1.0;
        let mut fixture = world.saved.clone();
        fixture.players[0].player.x = x;
        fixture.players[0].player.y = 1536.0;
        world.commit(fixture).unwrap();
        let before = world.join(Some(&joined.token)).unwrap().player;
        assert!(world
            .move_player(&joined.token, 1, -1.0, 0.0, &runtime)
            .is_err());
        let after = world.join(Some(&joined.token)).unwrap();
        assert_eq!(after.player, before);
        assert_eq!(after.next_sequence, 1);
        assert!(world
            .move_player(&joined.token, 1, 1.0, 0.0, &runtime)
            .is_ok());
    }
}
