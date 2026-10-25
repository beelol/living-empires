//! One authoritative Bevy host with Lightyear UDP commands and persisted state.
use crate::{
    admin::{Admin, Health},
    protocol::{
        ClientCommand, DEV_KEY, PROTOCOL_ID, PROTOCOL_VERSION, ProtocolPlugin, ServerUpdate,
        UpdateChannel,
    },
};
use anyhow::{Result, bail};
use argon_dev::{content::ContentRuntime, diplomacy_loop::DiplomacyLoop, world::World};
use bevy::prelude::*;
use lightyear::prelude::{server, *};
use serde_json::json;
use std::{
    collections::{HashMap, HashSet},
    net::SocketAddr,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

const CONTENT_POLL: Duration = Duration::from_millis(100);
const SNAPSHOT_INTERVAL: Duration = Duration::from_millis(100);

pub struct HostConfig {
    pub bind: SocketAddr,
    pub advertise: Vec<SocketAddr>,
    pub content: PathBuf,
    pub state: PathBuf,
    pub seed: Option<u64>,
    pub release: String,
    pub activation_file: Option<PathBuf>,
}

struct Authority {
    world: World,
    content: ContentRuntime,
    release: String,
    activation_file: Option<PathBuf>,
    reload_error: Option<String>,
    last_reload: Instant,
    last_snapshot: Instant,
    city_steps: u64,
    city_error: Option<String>,
    city_halted: bool,
    sessions: HashMap<Entity, String>,
    health: Health,
}

impl Authority {
    fn accepting_commands(&self) -> bool {
        self.activation_file
            .as_ref()
            .is_none_or(|path| path.is_file())
    }

    fn snapshot(&self) -> ServerUpdate {
        // This development arena has no hidden world facts. Faction visibility
        // must be introduced at this boundary before adding private gameplay.
        ServerUpdate::Snapshot {
            players: self.world.snapshot(),
            buildings: self.world.buildings(),
            content_version: self.content.version().to_owned(),
            world_name: self.content.world_name().to_owned(),
            release: self.release.clone(),
            content_error: self.reload_error.clone(),
        }
    }

    fn publish_health(&self) {
        self.health.update(json!({
            "ok":true,
            "transport":"lightyear-udp",
            "protocol_version":PROTOCOL_VERSION,
            "accepting_commands":self.accepting_commands(),
            "content_version":self.content.version(),
            "world_name":self.content.world_name(),
            "release":self.release,
            "content_error":self.reload_error,
            "city_error":self.city_error,
            "city_halted":self.city_halted,
            "buildings":self.world.buildings().len(),
        }));
    }

    fn command(&mut self, peer: Entity, command: ClientCommand) -> ServerUpdate {
        if !self.accepting_commands() {
            return rejected(
                "Host is awaiting deployment activation; reconnect shortly",
                true,
            );
        }
        match command {
            ClientCommand::RequestTerrain { fingerprint, x, y } => {
                if !self.sessions.contains_key(&peer) {
                    return rejected("Join a session before requesting terrain", true);
                }
                let Some(terrain) = self.world.terrain() else {
                    return rejected("This saved world uses the legacy arena", false);
                };
                if fingerprint != terrain.info().fingerprint {
                    return rejected("Terrain version changed; reconnect to synchronize", true);
                }
                match terrain.chunk(x, y) {
                    Some(chunk) => ServerUpdate::TerrainChunk { fingerprint, chunk },
                    None => rejected("Terrain chunk is outside the saved world", false),
                }
            }
            ClientCommand::Hello { protocol, token } => {
                if protocol != PROTOCOL_VERSION {
                    return rejected(
                        "Client protocol is incompatible; rebuild your client",
                        false,
                    );
                }
                if let Some(existing) = self.sessions.get(&peer)
                    && token.as_ref().is_some_and(|token| token != existing)
                {
                    return rejected("Disconnect before changing commander sessions", false);
                }
                // Duplicate Hello is idempotent even when its original token was
                // None. A connected peer cannot mint commanders by resending it.
                let token = self.sessions.get(&peer).cloned().or(token);
                match self.world.join(token.as_deref()) {
                    Ok(joined) => {
                        self.sessions.insert(peer, joined.token.clone());
                        ServerUpdate::Welcome {
                            token: joined.token,
                            player: joined.player,
                            next_sequence: joined.next_sequence,
                        }
                    }
                    Err(error) => rejected(&format!("Join was not accepted: {error:#}"), false),
                }
            }
            ClientCommand::Move { sequence, dx, dy } => {
                let Some(token) = self.sessions.get(&peer) else {
                    return rejected("Join a commander session before sending moves", true);
                };
                match self
                    .world
                    .move_player(token, sequence, dx, dy, &self.content)
                {
                    Ok(player) => ServerUpdate::Moved {
                        player,
                        next_sequence: sequence + 1,
                    },
                    Err(error) => {
                        let reason = format!("Move rejected: {error:#}");
                        match self.world.join(Some(token)) {
                            Ok(joined) => ServerUpdate::MoveRejected {
                                reason,
                                next_sequence: joined.next_sequence,
                            },
                            Err(_) => {
                                rejected("Commander session could not synchronize; reconnect", true)
                            }
                        }
                    }
                }
            }
        }
    }
}

fn rejected(reason: &str, reconnect: bool) -> ServerUpdate {
    ServerUpdate::Rejected {
        reason: reason.to_owned(),
        reconnect,
    }
}

type ConnectedPeers<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static mut MessageReceiver<ClientCommand>,
        &'static mut MessageSender<ServerUpdate>,
    ),
    With<Connected>,
>;

fn authoritative_update(
    mut authority: NonSendMut<Authority>,
    connected: Query<Entity, With<Connected>>,
    mut peers: ConnectedPeers,
) {
    let live: HashSet<_> = connected.iter().collect();
    authority.sessions.retain(|peer, _| live.contains(peer));
    if authority.last_reload.elapsed() >= CONTENT_POLL {
        match authority.content.reload() {
            Ok(true) => {
                println!("Activated Lua content {}", authority.content.version());
                authority.reload_error = None;
            }
            Ok(false) => {}
            Err(error) => {
                let message = format!("{error:#}");
                if authority.reload_error.as_ref() != Some(&message) {
                    eprintln!("Content rejected; preserving active rules: {message}");
                }
                authority.reload_error = Some(message);
            }
        }
        authority.last_reload = Instant::now();
    }
    let broadcast = authority.last_snapshot.elapsed() >= SNAPSHOT_INTERVAL;
    for (peer, mut receiver, mut sender) in &mut peers {
        let mut welcomed = false;
        // Bound work per peer. The supported client only keeps one pending move;
        // deliberately excessive traffic cannot monopolize the authority loop.
        for command in receiver.receive().take(8) {
            let response = authority.command(peer, command);
            welcomed |= matches!(response, ServerUpdate::Welcome { .. });
            sender.send::<UpdateChannel>(response);
        }
        if welcomed {
            sender.send::<UpdateChannel>(ServerUpdate::TerrainManifest {
                info: authority.world.terrain().map(|t| t.info()),
            });
        }
        if (welcomed || broadcast) && authority.sessions.contains_key(&peer) {
            sender.send::<UpdateChannel>(authority.snapshot());
        }
    }
    if broadcast {
        authority.last_snapshot = Instant::now();
    }
    authority.publish_health();
}

// The existing authority owns fixed construction work and a bounded authored
// starter plan. This adds no model worker, second world writer or content loop.
fn advance_city(mut authority: NonSendMut<Authority>) {
    use argon_dev::settlement::Role;
    if !authority.accepting_commands()
        || authority.world.terrain().is_none()
        || authority.city_halted
    {
        return;
    }
    authority.city_steps = authority.city_steps.saturating_add(1);
    let planning = authority.city_steps % 60 == 1;
    let outcome = (|| -> Result<Vec<String>> {
        let mut blockers = Vec::new();
        if authority.world.buildings().is_empty() {
            if !planning {
                return Ok(blockers);
            }
            authority.world.initialize_settlements()?;
        }
        authority.world.tick_settlements(1)?;
        if !planning {
            return Ok(blockers);
        }
        for faction in ["helios", "tide", "cinder"] {
            let buildings = authority.world.buildings();
            let own: Vec<_> = buildings
                .iter()
                .filter(|b| b.faction_id == faction)
                .collect();
            if !own.iter().any(|b| b.role == Role::Core && b.complete) {
                continue;
            }
            let next = if !own.iter().any(|b| b.role == Role::Generator) {
                Some((Role::Generator, "starter-generator"))
            } else if own.iter().any(|b| b.role == Role::Generator && b.complete)
                && !own.iter().any(|b| b.role == Role::Storehouse)
            {
                Some((Role::Storehouse, "starter-storehouse"))
            } else {
                None
            };
            if let Some((role, request)) = next {
                let id = authority.world.plan_settlement(faction, request, role)?;
                if id.is_none() {
                    blockers.push(format!(
                        "{faction}: {role:?} expansion blocked; no legal affordable site"
                    ));
                }
            }
        }
        Ok(blockers)
    })();
    match outcome {
        Ok(blockers) if planning => {
            let message = (!blockers.is_empty()).then(|| blockers.join("; "));
            if message != authority.city_error
                && let Some(message) = &message
            {
                eprintln!("City expansion blocked: {message}");
            }
            authority.city_error = message;
        }
        Ok(_) => {}
        Err(error) => {
            let message = format!("{error:#}");
            if authority.city_error.as_deref() != Some(&message) {
                eprintln!("City construction blocked: {message}");
            }
            authority.city_error = Some(message);
            authority.city_halted = true;
        }
    }
}

/// Blocking headless host used by the same executable's `host` mode.
pub fn run(config: HostConfig) -> Result<()> {
    let stop = Arc::new(AtomicBool::new(false));
    let signal = Arc::clone(&stop);
    anyhow::Context::context(
        ctrlc::set_handler(move || signal.store(true, Ordering::Relaxed)),
        "cannot install host shutdown handler",
    )?;
    run_with_stop(config, stop)
}

/// Explicit stop handle supports integration tests without process-wide signals.
pub fn run_with_stop(mut config: HostConfig, stop: Arc<AtomicBool>) -> Result<()> {
    let content = ContentRuntime::load(&config.content)?;
    let world = World::load_terrain(&config.state, config.seed)?;
    let listener = Admin::bind(config.bind)?;
    config.bind = listener.local_addr()?;
    let health = Health::new(json!({"ok":false}));
    let activation_file = config.activation_file.clone();
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(bevy::log::LogPlugin::default())
        .add_plugins(server::ServerPlugins::default())
        .add_plugins(ProtocolPlugin)
        .insert_non_send(Authority {
            world,
            content,
            release: config.release,
            activation_file: config.activation_file,
            reload_error: None,
            last_reload: Instant::now(),
            last_snapshot: Instant::now(),
            city_steps: 0,
            city_error: None,
            city_halted: false,
            sessions: HashMap::new(),
            health: health.clone(),
        })
        .insert_resource(Time::<Fixed>::from_duration(crate::protocol::TICK))
        .add_systems(FixedUpdate, advance_city)
        .add_systems(Update, authoritative_update);
    app.finish();
    app.cleanup();
    let endpoint = app
        .world_mut()
        .spawn((
            server::NetcodeServer::new(
                server::NetcodeConfig::default()
                    .with_protocol_id(PROTOCOL_ID)
                    .with_key(DEV_KEY)
                    .with_max_clients(64)
                    .with_additional_expected_addresses(config.advertise),
            ),
            LocalAddr(config.bind),
            server::ServerUdpIo,
        ))
        .id();
    app.world_mut().trigger(server::Start { entity: endpoint });
    let startup = Instant::now();
    while app.world().get::<server::Started>(endpoint).is_none()
        || app.world().get::<Linked>(endpoint).is_none()
    {
        app.update();
        if app.world().get::<server::Stopped>(endpoint).is_some()
            || startup.elapsed() > Duration::from_secs(3)
        {
            bail!("Lightyear UDP host could not start at {}", config.bind);
        }
        if stop.load(Ordering::Relaxed) {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(5));
    }
    // A health probe cannot pass until the real gameplay socket has started.
    app.world().non_send::<Authority>().publish_health();
    let mut diplomacy_path = config.state.as_os_str().to_os_string();
    diplomacy_path.push(".diplomacy.json");
    let env_file = std::env::var_os("ARGON_ENV_FILE")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(".env.local"));
    let diplomacy = DiplomacyLoop::start(diplomacy_path.into(), env_file, activation_file)?;
    let _admin = Admin::start_with_diplomacy(listener, health, Some(diplomacy.status()));
    println!(
        "Living Empires Bevy/Lightyear host listening on {} (UDP gameplay, TCP status)",
        config.bind
    );
    println!(
        "Saved world: {}. Trusted LAN development only.",
        config.state.display()
    );
    println!(
        "Local diplomacy view: http://127.0.0.1:{}/diplomacy",
        config.bind.port()
    );
    while !stop.load(Ordering::Relaxed) {
        app.update();
        if app.world().get::<server::Started>(endpoint).is_none()
            || app.world().get::<Linked>(endpoint).is_none()
        {
            bail!("Lightyear UDP host stopped unexpectedly");
        }
        thread::sleep(Duration::from_millis(5));
    }
    app.world_mut().trigger(server::Stop { entity: endpoint });
    app.update();
    println!("Host stopped. Acknowledged player state is saved.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn city_activation_gates_fixed_work_and_failed_save_halts_it() {
        let directory = tempfile::tempdir().unwrap();
        std::fs::write(
            directory.path().join("manifest.json"),
            r#"{"schema_version":1,"entry":"rules.lua","files":["rules.lua"]}"#,
        )
        .unwrap();
        std::fs::write(directory.path().join("rules.lua"),"return {schema_version=1,world_name='Test',step=function(x,y,dx,dy) return x+dx,y+dy end}").unwrap();
        let path = directory.path().join("world.json");
        let gate = directory.path().join("active");
        let authority = Authority {
            world: World::load_terrain(&path, Some(72913)).unwrap(),
            content: ContentRuntime::load(directory.path()).unwrap(),
            release: "test".into(),
            activation_file: Some(gate.clone()),
            reload_error: None,
            last_reload: Instant::now(),
            last_snapshot: Instant::now(),
            city_steps: 0,
            city_error: None,
            city_halted: false,
            sessions: HashMap::new(),
            health: Health::new(json!({})),
        };
        let mut app = App::new();
        app.insert_non_send(authority)
            .add_systems(Update, advance_city);
        app.update();
        assert!(
            app.world()
                .non_send::<Authority>()
                .world
                .buildings()
                .is_empty()
        );
        std::fs::write(&gate, b"active").unwrap();
        app.update();
        let snapshot = app
            .world()
            .non_send::<Authority>()
            .world
            .buildings();
        assert_eq!(snapshot.len(), 3);
        let bytes = std::fs::read(&path).unwrap();
        std::fs::remove_file(&path).unwrap();
        std::fs::create_dir(&path).unwrap();
        app.update();
        assert!(app.world().non_send::<Authority>().city_halted);
        assert_eq!(
            app.world()
                .non_send::<Authority>()
                .world
                .buildings(),
            snapshot
        );
        std::fs::remove_dir(&path).unwrap();
        std::fs::write(&path, bytes).unwrap();
        app.update();
        assert_eq!(
            app.world()
                .non_send::<Authority>()
                .world
                .buildings(),
            snapshot
        );
    }

    #[test]
    fn rejected_move_keeps_session_usable_and_reports_authoritative_sequence() {
        let directory = tempfile::tempdir().unwrap();
        std::fs::write(
            directory.path().join("manifest.json"),
            r#"{"schema_version":1,"entry":"rules.lua","files":["rules.lua"]}"#,
        )
        .unwrap();
        std::fs::write(directory.path().join("rules.lua"),
            "return {schema_version=1,world_name='Test',step=function(x,y,dx,dy) return x+dx*12,y+dy*12 end}").unwrap();
        let state = directory.path().join("world.json");
        let mut authority = Authority {
            world: World::load(&state).unwrap(),
            content: ContentRuntime::load(directory.path()).unwrap(),
            release: "test".into(),
            activation_file: None,
            reload_error: None,
            last_reload: Instant::now(),
            last_snapshot: Instant::now(),
            city_steps: 0,
            city_error: None,
            city_halted: false,
            sessions: HashMap::new(),
            health: Health::new(json!({"ok":true})),
        };
        let peer = Entity::PLACEHOLDER;
        let ServerUpdate::Welcome { player, .. } = authority.command(
            peer,
            ClientCommand::Hello {
                protocol: PROTOCOL_VERSION,
                token: None,
            },
        ) else {
            panic!("expected welcome");
        };
        let rejected = authority.command(
            peer,
            ClientCommand::Move {
                sequence: 1,
                dx: 2.0,
                dy: 0.0,
            },
        );
        assert!(matches!(
            rejected,
            ServerUpdate::MoveRejected {
                next_sequence: 1,
                ..
            }
        ));
        assert_eq!(authority.world.snapshot(), vec![player.clone()]);
        let ServerUpdate::Moved {
            player: moved,
            next_sequence,
        } = authority.command(
            peer,
            ClientCommand::Move {
                sequence: 1,
                dx: 1.0,
                dy: 0.0,
            },
        )
        else {
            panic!("valid move after rejection must succeed");
        };
        assert_eq!(next_sequence, 2);
        assert_eq!(moved.x, player.x + 12.0);
        drop(authority);
        assert_eq!(World::load(state).unwrap().snapshot(), vec![moved]);
    }
}
