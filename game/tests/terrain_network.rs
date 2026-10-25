//! Terrain proof over real UDP; independent clients receive only public geometry.
use living_empires::client::{ClientConfig, HeadlessClient};
use std::{
    net::TcpListener,
    path::PathBuf,
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

struct Host {
    process: Child,
    directory: tempfile::TempDir,
    address: std::net::SocketAddr,
}
impl Host {
    fn start() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let address = TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap();
        let process = Self::spawn(directory.path(), address);
        Self {
            process,
            directory,
            address,
        }
    }
    fn spawn(directory: &std::path::Path, address: std::net::SocketAddr) -> Child {
        Command::new(env!("CARGO_BIN_EXE_empires"))
            .args([
                "host",
                "--bind",
                &address.to_string(),
                "--seed",
                "8170321",
                "--state",
            ])
            .arg(directory.join("world.json"))
            .arg("--content")
            .arg(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../dev/content"))
            .env("ARGON_ENV_FILE", directory.join("no-provider.env"))
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap()
    }
    fn client(&self, name: &str) -> HeadlessClient {
        HeadlessClient::new(ClientConfig {
            server: self.address,
            session: self.directory.path().join(name),
        })
        .unwrap()
    }
    fn restart(&mut self) {
        self.process.kill().unwrap();
        self.process.wait().unwrap();
        self.process = Self::spawn(self.directory.path(), self.address);
    }
}
impl Drop for Host {
    fn drop(&mut self) {
        let _ = self.process.kill();
        let _ = self.process.wait();
    }
}

fn until(
    a: &mut HeadlessClient,
    b: &mut HeadlessClient,
    predicate: impl Fn(&HeadlessClient, &HeadlessClient) -> bool,
) {
    let deadline = Instant::now() + Duration::from_secs(40);
    loop {
        a.update().unwrap();
        b.update().unwrap();
        if predicate(a, b) {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "two clients did not converge on saved terrain"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn two_clients_verify_identical_terrain_move_and_reconnect_without_reset() {
    let mut host = Host::start();
    let mut a = host.client("a.json");
    let mut b = host.client("b.json");
    until(&mut a, &mut b, |a, b| {
        a.result().is_some() && b.result().is_some()
    });
    let initial = a.result().unwrap();
    let other = b.result().unwrap();
    assert_ne!(initial.player.id, other.player.id);
    assert!(initial.terrain_fingerprint.is_some());
    assert_eq!(initial.terrain_fingerprint, other.terrain_fingerprint);
    let authoritative =
        argon_dev::terrain::Terrain::load(host.directory.path().join("world.json.terrain.json"))
            .unwrap();
    assert_eq!(
        initial.terrain_fingerprint.as_deref(),
        Some(authoritative.info().fingerprint.as_str())
    );
    assert!(!authoritative.deposits().is_empty());
    assert!(a.terrain().unwrap().deposits().is_empty());
    assert!(b.terrain().unwrap().deposits().is_empty());
    for point in [
        [1536.0, 1536.0],
        [128.0, 128.0],
        [2048.5, 1888.25],
        [3072.0, 3072.0],
    ] {
        let expected = authoritative.sample(point[0], point[1]);
        assert_eq!(a.terrain().unwrap().sample(point[0], point[1]), expected);
        assert_eq!(b.terrain().unwrap().sample(point[0], point[1]), expected);
    }
    let speed = f64::from(
        authoritative
            .sample(initial.player.x, initial.player.y)
            .unwrap()
            .movement_multiplier,
    );
    let expected_x = initial.player.x + 12.0 * speed;
    assert!(a.queue_move(1.0, 0.0));
    until(&mut a, &mut b, |a, b| {
        a.result().is_some_and(|r| r.player.x == expected_x)
            && b.result().is_some_and(|r| {
                r.players
                    .iter()
                    .any(|p| p.id == initial.player.id && p.x == expected_x)
            })
    });
    host.restart();
    a.reconnect();
    b.reconnect();
    until(&mut a, &mut b, |a, b| {
        a.result().is_some() && b.result().is_some()
    });
    let restored = a.result().unwrap();
    assert_eq!(restored.player.x, expected_x);
    assert_eq!(restored.token, initial.token);
    assert_eq!(restored.terrain_fingerprint, initial.terrain_fingerprint);
    assert_eq!(restored.next_sequence, initial.next_sequence + 1);
    assert_eq!(restored.players.len(), 2);
    let mut late = host.client("late.json");
    until(&mut a, &mut late, |a, b| {
        a.result().is_some() && b.result().is_some()
    });
    assert_eq!(
        late.result().unwrap().terrain_fingerprint,
        initial.terrain_fingerprint
    );
}
