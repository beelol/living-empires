use crate::content::ContentRuntime;
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
    next_id: u64,
    players: Vec<SavedPlayer>,
}

pub struct World {
    path: PathBuf,
    saved: SavedWorld,
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
                next_id: 1,
                players: Vec::new(),
            },
            Err(error) => return Err(error).context("cannot open saved world"),
        };
        Ok(Self {
            path,
            saved,
            _lock: lock,
        })
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
        let player = Player {
            id: format!("commander-{id}"),
            name: format!("Commander {id}"),
            x: 440.0 + ((id - 1) % 6) as f64 * 24.0,
            y: 230.0 + (((id - 1) / 6) % 6) as f64 * 24.0,
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
        let mut next = self.saved.clone();
        let moved = &mut next.players[index].player;
        moved.x = x.clamp(12.0, WIDTH - 12.0);
        moved.y = y.clamp(12.0, HEIGHT - 12.0);
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
            file.write_all(&serde_json::to_vec_pretty(&next)?)?;
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
        saved.schema_version == 1,
        "unsupported saved-world schema; refusing to reset it"
    );
    ensure!(
        saved.next_id > 0 && saved.players.len() <= MAX_PLAYERS,
        "invalid saved-world identity state"
    );
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
            || !(12.0..=WIDTH - 12.0).contains(&player.x)
            || !(12.0..=HEIGHT - 12.0).contains(&player.y)
        {
            bail!("invalid saved commander position");
        }
    }
    Ok(())
}
