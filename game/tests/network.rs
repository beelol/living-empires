//! Real UDP protocol checks against the executable, with no HTTP gameplay path.
use bevy::prelude::*;
use lightyear::prelude::{client, *};
use living_empires::protocol::*;
use std::{
    io::{Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    path::PathBuf,
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

struct Host {
    process: Child,
    _directory: tempfile::TempDir,
    content: PathBuf,
    state: PathBuf,
    gate: PathBuf,
    address: SocketAddr,
}
impl Host {
    fn start() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let content = directory.path().join("content");
        std::fs::create_dir(&content).unwrap();
        let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../dev/content");
        for name in ["manifest.json", "rules.lua"] {
            std::fs::copy(source.join(name), content.join(name)).unwrap();
        }
        let state = directory.path().join("world.json");
        let gate = directory.path().join("active");
        let address = TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap();
        let process = Self::spawn(address, &content, &state, &gate);
        let host = Self {
            process,
            _directory: directory,
            content,
            state,
            gate,
            address,
        };
        host.wait_health();
        host
    }
    fn spawn(address: SocketAddr, content: &PathBuf, state: &PathBuf, gate: &PathBuf) -> Child {
        Command::new(env!("CARGO_BIN_EXE_empires"))
            .args(["host", "--bind", &address.to_string(), "--content"])
            .arg(content)
            .arg("--state")
            .arg(state)
            .arg("--activate-file")
            .arg(gate)
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap()
    }
    fn health(&self) -> Option<serde_json::Value> {
        let mut stream =
            TcpStream::connect_timeout(&self.address, Duration::from_millis(100)).ok()?;
        stream
            .set_read_timeout(Some(Duration::from_millis(200)))
            .ok()?;
        stream
            .write_all(b"GET /health HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
            .ok()?;
        let mut data = String::new();
        stream.read_to_string(&mut data).ok()?;
        serde_json::from_str(data.split_once("\r\n\r\n")?.1).ok()
    }
    fn wait_health(&self) {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if let Some(value) = self.health()
                && value["ok"] == true
            {
                assert_eq!(value["transport"], "lightyear-udp");
                break;
            }
            assert!(Instant::now() < deadline, "server never became healthy");
            std::thread::sleep(Duration::from_millis(20));
        }
    }
    fn restart(&mut self) {
        self.process.kill().unwrap();
        self.process.wait().unwrap();
        self.process = Self::spawn(self.address, &self.content, &self.state, &self.gate);
        self.wait_health();
    }
}
impl Drop for Host {
    fn drop(&mut self) {
        let _ = self.process.kill();
        let _ = self.process.wait();
    }
}

struct Peer {
    app: App,
    entity: Entity,
}
#[derive(Resource, Default)]
struct Inbox(Vec<ServerUpdate>);
fn collect_updates(mut peers: Query<&mut MessageReceiver<ServerUpdate>>, mut inbox: ResMut<Inbox>) {
    for mut receiver in &mut peers {
        inbox.0.extend(receiver.receive());
    }
}
impl Peer {
    fn connect(address: SocketAddr, id: u64) -> Self {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(client::ClientPlugins::default())
            .add_plugins(ProtocolPlugin)
            .init_resource::<Inbox>()
            .add_systems(Update, collect_updates);
        app.finish();
        app.cleanup();
        let entity = app
            .world_mut()
            .spawn((
                client::NetcodeClient::new(
                    Authentication::Manual {
                        server_addr: address,
                        client_id: id,
                        private_key: DEV_KEY,
                        protocol_id: PROTOCOL_ID,
                    },
                    client::NetcodeConfig::default(),
                )
                .unwrap(),
                LocalAddr("127.0.0.1:0".parse().unwrap()),
                UdpIo::default(),
            ))
            .id();
        app.world_mut().trigger(Connect { entity });
        let deadline = Instant::now() + Duration::from_secs(10);
        while app.world().get::<Connected>(entity).is_none() {
            assert!(Instant::now() < deadline, "Lightyear connection timed out");
            app.update();
            std::thread::sleep(Duration::from_millis(5));
        }
        Self { app, entity }
    }
    fn send(&mut self, message: ClientCommand) {
        self.app
            .world_mut()
            .get_mut::<MessageSender<ClientCommand>>(self.entity)
            .unwrap()
            .send::<CommandChannel>(message);
    }
    fn until(&mut self, predicate: impl Fn(&ServerUpdate) -> bool) -> ServerUpdate {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            self.app.update();
            // Lightyear clears unread receivers in Last; collect during Update.
            for message in self.app.world_mut().resource_mut::<Inbox>().0.drain(..) {
                if predicate(&message) {
                    return message;
                }
            }
            assert!(Instant::now() < deadline, "server reply timed out");
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    fn rejected(&mut self) {
        assert!(matches!(
            self.until(|m| matches!(m, ServerUpdate::Rejected { .. })),
            ServerUpdate::Rejected { .. }
        ));
    }
    fn hello(&mut self, token: Option<String>) -> (String, argon_dev::world::Player, u64) {
        self.send(ClientCommand::Hello {
            protocol: PROTOCOL_VERSION,
            token,
        });
        match self.until(|m| matches!(m, ServerUpdate::Welcome { .. })) {
            ServerUpdate::Welcome {
                token,
                player,
                next_sequence,
            } => (token, player, next_sequence),
            _ => unreachable!(),
        }
    }
    fn moved(&mut self) -> (argon_dev::world::Player, u64) {
        match self.until(|m| matches!(m, ServerUpdate::Moved { .. })) {
            ServerUpdate::Moved {
                player,
                next_sequence,
            } => (player, next_sequence),
            _ => unreachable!(),
        }
    }
}

#[test]
fn activation_authority_idempotency_reload_and_crash_recovery_over_lightyear() {
    let mut host = Host::start();
    assert_eq!(host.health().unwrap()["accepting_commands"], false);
    let mut a = Peer::connect(host.address, 1001);
    a.send(ClientCommand::Hello {
        protocol: PROTOCOL_VERSION,
        token: None,
    });
    a.rejected();
    std::fs::write(&host.gate, "").unwrap();
    std::thread::sleep(Duration::from_millis(100));
    a.send(ClientCommand::Move {
        sequence: 1,
        dx: 1.,
        dy: 0.,
    });
    a.rejected();
    a.send(ClientCommand::Hello {
        protocol: PROTOCOL_VERSION + 1,
        token: None,
    });
    a.rejected();
    a.send(ClientCommand::Hello {
        protocol: PROTOCOL_VERSION,
        token: Some("not-a-capability".into()),
    });
    a.rejected();
    let (token, initial, sequence) = a.hello(None);
    let (_, same, again) = a.hello(None);
    assert_eq!(initial, same);
    assert_eq!(
        sequence, again,
        "repeated Hello must not create a second player"
    );
    let mut b = Peer::connect(host.address, 1002);
    let (_, other, _) = b.hello(None);
    assert_ne!(initial.id, other.id);
    a.send(ClientCommand::Move {
        sequence,
        dx: 1.,
        dy: 0.,
    });
    let (moved, next) = a.moved();
    assert_eq!(moved.x, initial.x + 12.);
    assert_eq!(next, sequence + 1);
    a.send(ClientCommand::Move {
        sequence,
        dx: 1.,
        dy: 0.,
    });
    let (duplicate, _) = a.moved();
    assert_eq!(duplicate, moved);
    a.send(ClientCommand::Move {
        sequence: next + 1,
        dx: 1.,
        dy: 0.,
    });
    a.until(|m| matches!(m,ServerUpdate::MoveRejected{next_sequence,..} if *next_sequence==next));
    let snapshot=b.until(|m|matches!(m,ServerUpdate::Snapshot{players,..} if players.iter().any(|p|p.id==moved.id && p.x==moved.x)));
    if let ServerUpdate::Snapshot { players, .. } = snapshot {
        assert_eq!(players.len(), 2);
    }
    let before = host.health().unwrap()["content_version"].clone();
    let rules = host.content.join("rules.lua");
    let original = std::fs::read_to_string(&rules).unwrap();
    std::fs::write(&rules, "invalid lua !!!").unwrap();
    std::thread::sleep(Duration::from_millis(250));
    assert_eq!(host.health().unwrap()["content_version"], before);
    std::fs::write(
        &rules,
        original.replace("local distance = 12", "local distance = 17"),
    )
    .unwrap();
    a.until(|m|matches!(m,ServerUpdate::Snapshot{content_version,..} if content_version!=before.as_str().unwrap()));
    a.send(ClientCommand::Move {
        sequence: next,
        dx: 1.,
        dy: 0.,
    });
    let (updated, _) = a.moved();
    assert_eq!(updated.x, moved.x + 17.);
    host.restart();
    let mut resumed = Peer::connect(host.address, 1003);
    let (_, restored, next_after) = resumed.hello(Some(token));
    assert_eq!(restored, updated);
    assert_eq!(next_after, next + 1);
    resumed.send(ClientCommand::Move {
        sequence: next,
        dx: 1.,
        dy: 0.,
    });
    assert_eq!(resumed.moved().0, updated);
}

#[test]
fn persistent_client_recovers_after_restart_and_lost_acknowledgement() {
    use living_empires::client::{ClientConfig, HeadlessClient};
    let mut host = Host::start();
    std::fs::write(&host.gate, "").unwrap();
    let mut client = HeadlessClient::new(ClientConfig {
        server: host.address,
        session: host._directory.path().join("client.json"),
    })
    .unwrap();
    fn ready(client: &mut HeadlessClient, x: Option<f64>) -> living_empires::client::ProbeResult {
        let deadline = Instant::now() + Duration::from_secs(12);
        loop {
            client.update().unwrap();
            if let Some(result) = client.result()
                && x.is_none_or(|x| result.player.x == x)
            {
                return result;
            }
            assert!(
                Instant::now() < deadline,
                "persistent client did not recover"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    let initial = ready(&mut client, None);
    host.restart();
    // Queue input on the still-running client; only a new connection can deliver it.
    assert!(client.queue_move(1., 0.));
    let recovered = ready(&mut client, Some(initial.player.x + 12.));
    assert_eq!(recovered.token, initial.token);
    assert_eq!(recovered.player.id, initial.player.id);
    assert_eq!(recovered.next_sequence, initial.next_sequence + 1);
    // Send once, stop receiving before the server acknowledgement, then discard
    // the socket. The same client keeps its pending command through the restart.
    assert!(client.queue_move(1., 0.));
    client.update().unwrap();
    let expected = recovered.player.x + 12.;
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let saved: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&host.state).unwrap()).unwrap();
        if saved["players"][0]["player"]["x"].as_f64() == Some(expected) {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "move was not durably accepted before interrupting the acknowledgement"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    client.reconnect();
    host.restart();
    let final_state = ready(&mut client, Some(expected));
    assert_eq!(final_state.token, initial.token);
    assert_eq!(final_state.next_sequence, recovered.next_sequence + 1);
    assert_eq!(
        final_state.players.len(),
        1,
        "reconnect must not mint another identity"
    );
}
