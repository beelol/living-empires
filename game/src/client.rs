//! The native view and the headless probe use the same Lightyear connection path.
use crate::protocol::{
    ClientCommand, CommandChannel, DEV_KEY, PROTOCOL_ID, PROTOCOL_VERSION, ProtocolPlugin,
    ServerUpdate,
};
use anyhow::{Context, Result, bail, ensure};
use argon_dev::world::Player;
use bevy::prelude::{App, Entity, MinimalPlugins, Resource, Update, World};
#[cfg(feature = "native")]
use bevy::prelude::{DefaultPlugins, PluginGroup, Window, WindowPlugin, default};
use lightyear::prelude::{client, *};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    net::SocketAddr,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

#[cfg(feature = "native")]
#[path = "visual.rs"]
mod visual;

#[derive(Clone, Debug)]
pub struct ClientConfig {
    pub server: SocketAddr,
    pub session: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProbeResult {
    pub token: String,
    pub player: Player,
    pub next_sequence: u64,
    pub players: Vec<Player>,
    pub content_version: String,
    pub world_name: String,
    pub release: String,
    pub content_error: Option<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SavedSession {
    schema_version: u32,
    server: SocketAddr,
    token: String,
}

fn load_session(config: &ClientConfig) -> Result<Option<String>> {
    let file = match File::open(&config.session) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error).with_context(|| "cannot open client session"),
    };
    let mut bytes = Vec::new();
    file.take(4097).read_to_end(&mut bytes)?;
    ensure!(bytes.len() <= 4096, "client session exceeds size limit");
    let saved: SavedSession = serde_json::from_slice(&bytes)
        .with_context(|| "cannot read client session; refusing to replace its identity")?;
    ensure!(
        saved.schema_version == 1,
        "unsupported client session format"
    );
    ensure!(
        saved.server == config.server,
        "session belongs to another server; choose a different --session file"
    );
    ensure!(valid_token(&saved.token), "invalid saved session token");
    Ok(Some(saved.token))
}

fn valid_token(token: &str) -> bool {
    token.len() == 64 && token.bytes().all(|b| b.is_ascii_hexdigit())
}

fn save_session(config: &ClientConfig, token: &str) -> Result<()> {
    ensure!(
        valid_token(token),
        "server returned an invalid session token"
    );
    let parent = config
        .session
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    fs::create_dir_all(parent).with_context(|| "cannot create client session directory")?;
    let temp = parent.join(format!(
        ".empires-session-{}-{}.tmp",
        std::process::id(),
        random_id()?
    ));
    let result = (|| -> Result<()> {
        let mut options = OpenOptions::new();
        options.create_new(true).write(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&temp)?;
        file.write_all(&serde_json::to_vec(&SavedSession {
            schema_version: 1,
            server: config.server,
            token: token.to_owned(),
        })?)?;
        file.sync_all()?;
        fs::rename(&temp, &config.session).with_context(|| "cannot save client identity")?;
        #[cfg(unix)]
        File::open(parent)?.sync_all()?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(temp);
    }
    result
}

fn random_id() -> Result<u64> {
    let mut bytes = [0_u8; 8];
    getrandom::fill(&mut bytes)
        .map_err(|e| anyhow::anyhow!("cannot create connection identity: {e}"))?;
    Ok(u64::from_le_bytes(bytes))
}

#[derive(Clone)]
struct PendingMove {
    sequence: u64,
    dx: f64,
    dy: f64,
    sent: bool,
}

#[derive(Resource)]
struct ClientState {
    config: ClientConfig,
    connection: Option<Entity>,
    connect_started: Instant,
    retry_after: Instant,
    last_received: Instant,
    hello_sent: bool,
    ready: bool,
    token: Option<String>,
    player: Option<Player>,
    next_sequence: u64,
    pending: Option<PendingMove>,
    movement_error: Option<String>,
    players: Vec<Player>,
    content_version: String,
    world_name: String,
    release: String,
    content_error: Option<String>,
    status: String,
    fatal: Option<String>,
}

impl ClientState {
    fn new(config: ClientConfig) -> Result<Self> {
        let token = load_session(&config)?;
        Ok(Self {
            config,
            connection: None,
            connect_started: Instant::now(),
            retry_after: Instant::now(),
            last_received: Instant::now(),
            hello_sent: false,
            ready: false,
            token,
            player: None,
            next_sequence: 1,
            pending: None,
            movement_error: None,
            players: Vec::new(),
            content_version: String::new(),
            world_name: String::new(),
            release: String::new(),
            content_error: None,
            status: "Connecting to host...".into(),
            fatal: None,
        })
    }

    fn queue_move(&mut self, dx: f64, dy: f64) -> bool {
        if !self.ready || self.pending.is_some() || self.fatal.is_some() {
            return false;
        }
        if !dx.is_finite() || !dy.is_finite() || dx.hypot(dy) > 1.000_001 {
            return false;
        }
        self.pending = Some(PendingMove {
            sequence: self.next_sequence,
            dx,
            dy,
            sent: false,
        });
        self.movement_error = None;
        true
    }

    fn accept(&mut self, update: ServerUpdate) -> Result<bool> {
        self.last_received = Instant::now();
        match update {
            ServerUpdate::Welcome {
                token,
                player,
                next_sequence,
            } => {
                if self.token.as_ref() != Some(&token) {
                    save_session(&self.config, &token)?;
                }
                self.token = Some(token);
                if let Some(pending) = &mut self.pending {
                    if next_sequence == pending.sequence {
                        pending.sent = false;
                    } else if next_sequence == pending.sequence + 1 {
                        self.pending = None;
                    } else {
                        bail!("host movement history changed; unresolved input was not replayed");
                    }
                }
                self.player = Some(player);
                self.next_sequence = next_sequence;
                self.ready = true;
                self.status = "Connected | authoritative host".into();
            }
            ServerUpdate::Moved {
                player,
                next_sequence,
            } => {
                if let Some(pending) = &self.pending {
                    ensure!(
                        next_sequence == pending.sequence + 1,
                        "unexpected movement acknowledgement"
                    );
                    self.pending = None;
                }
                self.player = Some(player);
                self.next_sequence = next_sequence;
                self.status = "Connected | authoritative host".into();
            }
            ServerUpdate::MoveRejected {
                reason,
                next_sequence,
            } => {
                self.pending = None;
                self.next_sequence = next_sequence;
                self.status = format!("Move blocked: {reason}");
                self.movement_error = Some(reason);
            }
            ServerUpdate::Snapshot {
                players,
                content_version,
                world_name,
                release,
                content_error,
            } => {
                self.players = players;
                self.content_version = content_version;
                self.world_name = world_name;
                self.release = release;
                self.content_error = content_error;
            }
            ServerUpdate::Rejected { reason, reconnect } => {
                self.status = reason.clone();
                if reconnect {
                    return Ok(true);
                }
                self.pending = None;
                self.fatal = Some(reason);
                self.ready = false;
            }
        }
        Ok(false)
    }

    fn result(&self) -> Option<ProbeResult> {
        let player = self.player.as_ref()?;
        // A movement acknowledgement alone does not prove snapshots caught up.
        if !self.ready || self.pending.is_some() || !self.players.iter().any(|p| p == player) {
            return None;
        }
        Some(ProbeResult {
            token: self.token.clone()?,
            player: player.clone(),
            next_sequence: self.next_sequence,
            players: self.players.clone(),
            content_version: self.content_version.clone(),
            world_name: self.world_name.clone(),
            release: self.release.clone(),
            content_error: self.content_error.clone(),
        })
    }
}

fn configure(app: &mut App, config: ClientConfig) -> Result<()> {
    app.add_plugins(client::ClientPlugins::default())
        .add_plugins(ProtocolPlugin)
        .insert_resource(ClientState::new(config)?)
        .add_systems(Update, network_tick);
    Ok(())
}

fn disconnect(world: &mut World, state: &mut ClientState) {
    if let Some(entity) = state.connection.take() {
        world.trigger(Disconnect { entity });
        // Lightyear observers queue lifecycle component changes. Apply them while
        // the entity still exists before retiring the old connection.
        world.flush();
        let _ = world.despawn(entity);
    }
    state.ready = false;
    state.hello_sent = false;
    if let Some(pending) = &mut state.pending {
        pending.sent = false;
    }
    state.retry_after = Instant::now() + Duration::from_millis(250);
}

fn connect(world: &mut World, state: &mut ClientState) -> Result<()> {
    let client = client::NetcodeClient::new(
        Authentication::Manual {
            server_addr: state.config.server,
            client_id: random_id()?,
            private_key: DEV_KEY,
            protocol_id: PROTOCOL_ID,
        },
        client::NetcodeConfig::default(),
    )?;
    let local: SocketAddr = if state.config.server.is_ipv6() {
        "[::]:0"
    } else {
        "0.0.0.0:0"
    }
    .parse()?;
    let entity = world
        .spawn((client, LocalAddr(local), UdpIo::default()))
        .id();
    world.trigger(Connect { entity });
    // NetcodeClient initially requires Disconnected; Connect removes that marker
    // through deferred observers. Do not mistake it for a failed connection.
    world.flush();
    state.connection = Some(entity);
    state.connect_started = Instant::now();
    state.last_received = Instant::now();
    state.status = format!("Connecting to {}...", state.config.server);
    Ok(())
}

fn network_tick(world: &mut World) {
    // Exclusive access keeps connection lifecycle, acknowledgements and input state
    // coherent without making Lightyear's lazily-added receiver a required query.
    let Some(mut state) = world.remove_resource::<ClientState>() else {
        return;
    };
    let result = network_step(world, &mut state);
    if let Err(error) = result {
        state.fatal = Some(format!("{error:#}"));
        state.status = format!("Cannot continue: {error:#}");
        disconnect(world, &mut state);
    }
    world.insert_resource(state);
}

fn network_step(world: &mut World, state: &mut ClientState) -> Result<()> {
    if state.fatal.is_some() {
        return Ok(());
    }
    if state.connection.is_none() && Instant::now() >= state.retry_after {
        connect(world, state)?;
    }
    let Some(entity) = state.connection else {
        return Ok(());
    };
    let connected = world.get::<Connected>(entity).is_some();
    let updates: Vec<_> = world
        .get_mut::<MessageReceiver<ServerUpdate>>(entity)
        .map(|mut receiver| receiver.receive().collect())
        .unwrap_or_default();
    for update in updates {
        if state.accept(update)? {
            disconnect(world, state);
            return Ok(());
        }
    }
    if state.fatal.is_some() {
        disconnect(world, state);
        return Ok(());
    }
    let timed_out = if connected && state.ready {
        state.last_received.elapsed() > Duration::from_secs(2)
    } else {
        state.connect_started.elapsed() > Duration::from_secs(4)
    };
    if world.get::<Disconnected>(entity).is_some() || timed_out {
        state.status = "Connection interrupted; reconnecting with saved identity...".into();
        disconnect(world, state);
        return Ok(());
    }
    if connected && let Some(mut sender) = world.get_mut::<MessageSender<ClientCommand>>(entity) {
        if !state.hello_sent {
            sender.send::<CommandChannel>(ClientCommand::Hello {
                protocol: PROTOCOL_VERSION,
                token: state.token.clone(),
            });
            state.hello_sent = true;
        }
        if state.ready
            && let Some(pending) = &mut state.pending
            && !pending.sent
        {
            sender.send::<CommandChannel>(ClientCommand::Move {
                sequence: pending.sequence,
                dx: pending.dx,
                dy: pending.dy,
            });
            pending.sent = true;
        }
    }
    Ok(())
}

/// A persistent, windowless client using exactly the native client's networking
/// systems. Call `update` regularly; no background thread silently drives it.
pub struct HeadlessClient {
    app: App,
}

impl HeadlessClient {
    pub fn new(config: ClientConfig) -> Result<Self> {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        configure(&mut app, config)?;
        app.finish();
        app.cleanup();
        Ok(Self { app })
    }

    pub fn update(&mut self) -> Result<()> {
        self.app.update();
        let state = self.app.world().resource::<ClientState>();
        if let Some(error) = &state.fatal {
            bail!("{error}");
        }
        if let Some(error) = &state.movement_error {
            bail!("movement rejected: {error}");
        }
        Ok(())
    }

    pub fn queue_move(&mut self, dx: f64, dy: f64) -> bool {
        self.app
            .world_mut()
            .resource_mut::<ClientState>()
            .queue_move(dx, dy)
    }

    pub fn result(&self) -> Option<ProbeResult> {
        self.app.world().resource::<ClientState>().result()
    }

    /// Interrupt transport while keeping identity and pending input. The ordinary
    /// reconnect system resumes it with fresh Netcode authentication.
    pub fn reconnect(&mut self) {
        let world = self.app.world_mut();
        let mut state = world
            .remove_resource::<ClientState>()
            .expect("client state exists");
        disconnect(world, &mut state);
        state.status = "Reconnecting with saved identity...".into();
        world.insert_resource(state);
    }
}

/// Connect through real UDP, perform acknowledged moves, and return a synchronized
/// authoritative snapshot. `wait` bounds the entire attempt, including reconnects.
pub fn probe(config: ClientConfig, moves: Vec<(f64, f64)>, wait: Duration) -> Result<ProbeResult> {
    for (dx, dy) in &moves {
        ensure!(
            dx.is_finite() && dy.is_finite() && dx.hypot(*dy) <= 1.000_001,
            "invalid probe movement direction"
        );
    }
    let mut client = HeadlessClient::new(config)?;
    let started = Instant::now();
    let mut moves = moves.into_iter().peekable();
    loop {
        client.update()?;
        if let Some(&(dx, dy)) = moves.peek() {
            if client.queue_move(dx, dy) {
                moves.next();
            }
        } else if let Some(result) = client.result() {
            return Ok(result);
        }
        if started.elapsed() >= wait {
            bail!(
                "client probe timed out after {:.2}s: {}",
                wait.as_secs_f64(),
                client.app.world().resource::<ClientState>().status
            );
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[cfg(feature = "native")]
pub fn run(config: ClientConfig) -> Result<()> {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "Living Empires | development world".into(),
            resolution: (1280, 800).into(),
            ..default()
        }),
        ..default()
    }));
    configure(&mut app, config)?;
    visual::install(&mut app);
    app.run();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(directory: &Path) -> ClientState {
        ClientState::new(ClientConfig {
            server: "127.0.0.1:4000".parse().unwrap(),
            session: directory.join("session.json"),
        })
        .unwrap()
    }

    fn welcome(next_sequence: u64) -> ServerUpdate {
        ServerUpdate::Welcome {
            token: "ab".repeat(32),
            player: Player {
                id: "commander-1".into(),
                name: "Commander 1".into(),
                x: 440.0,
                y: 230.0,
            },
            next_sequence,
        }
    }

    #[test]
    fn reconnect_resolves_a_lost_ack_without_replaying_the_committed_move() {
        let directory = tempfile::tempdir().unwrap();
        let mut client = state(directory.path());
        client.accept(welcome(1)).unwrap();
        assert!(client.queue_move(1.0, 0.0));
        client.pending.as_mut().unwrap().sent = true;
        client.ready = false;
        client.accept(welcome(2)).unwrap();
        assert!(client.pending.is_none());
        assert!(client.queue_move(0.0, 1.0));
        assert_eq!(client.pending.as_ref().unwrap().sequence, 2);
    }

    #[test]
    fn reconnect_retries_an_uncommitted_move_with_its_original_sequence() {
        let directory = tempfile::tempdir().unwrap();
        let mut client = state(directory.path());
        client.accept(welcome(1)).unwrap();
        assert!(client.queue_move(0.5, -0.5));
        client.pending.as_mut().unwrap().sent = true;
        client.ready = false;
        client.accept(welcome(1)).unwrap();
        let retry = client.pending.as_ref().unwrap();
        assert_eq!(
            (retry.sequence, retry.dx, retry.dy, retry.sent),
            (1, 0.5, -0.5, false)
        );
    }

    #[test]
    fn rejected_move_is_reported_and_allows_a_new_command() {
        let directory = tempfile::tempdir().unwrap();
        let mut client = state(directory.path());
        client.accept(welcome(1)).unwrap();
        client.queue_move(1.0, 0.0);
        client
            .accept(ServerUpdate::MoveRejected {
                reason: "blocked by current rule".into(),
                next_sequence: 1,
            })
            .unwrap();
        assert!(client.ready);
        assert!(client.pending.is_none());
        assert_eq!(
            client.movement_error.as_deref(),
            Some("blocked by current rule")
        );
        assert!(client.queue_move(0.0, 1.0));
        assert_eq!(client.pending.as_ref().unwrap().sequence, 1);
        assert!(client.movement_error.is_none());
    }

    #[test]
    fn session_survives_restart_and_is_scoped_to_its_server() {
        let directory = tempfile::tempdir().unwrap();
        let mut config = ClientConfig {
            server: "127.0.0.1:4000".parse().unwrap(),
            session: directory.path().join("client/session.json"),
        };
        let token = "ab".repeat(32);
        assert!(load_session(&config).unwrap().is_none());
        save_session(&config, &token).unwrap();
        assert_eq!(load_session(&config).unwrap(), Some(token));
        config.server = "127.0.0.1:4001".parse().unwrap();
        assert!(load_session(&config).is_err());
    }

    #[test]
    fn corrupt_session_does_not_silently_create_a_new_identity() {
        let directory = tempfile::tempdir().unwrap();
        let config = ClientConfig {
            server: "127.0.0.1:4000".parse().unwrap(),
            session: directory.path().join("session.json"),
        };
        std::fs::write(&config.session, "not JSON").unwrap();
        assert!(load_session(&config).is_err());
        assert_eq!(
            std::fs::read_to_string(&config.session).unwrap(),
            "not JSON"
        );
    }
}
