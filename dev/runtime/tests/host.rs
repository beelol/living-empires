use argon_dev::{content::ContentRuntime, world::World};
use std::{
    fs,
    io::{BufRead, Read, Write},
    net::{TcpListener, TcpStream},
    path::Path,
    process::{Child, Command, Stdio},
    thread,
    time::Duration,
};

struct Host(Child);
impl Drop for Host {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn request(port: u16, method: &str, path: &str, body: &str) -> (u16, serde_json::Value) {
    request_with_headers(
        port,
        method,
        path,
        body,
        "Content-Type: application/json\r\n",
    )
}

fn request_with_headers(
    port: u16,
    method: &str,
    path: &str,
    body: &str,
    extra: &str,
) -> (u16, serde_json::Value) {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    write!(stream, "{method} {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n{extra}Content-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    let (headers, data) = response.split_once("\r\n\r\n").unwrap();
    let status = headers.split_whitespace().nth(1).unwrap().parse().unwrap();
    (status, serde_json::from_str(data).unwrap())
}

fn spawn_host(content: &Path, state: &Path) -> (Host, u16) {
    spawn_host_with_gate(content, state, None)
}

fn spawn_host_with_gate(
    content: &Path,
    state: &Path,
    activation_file: Option<&Path>,
) -> (Host, u16) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    let mut command = Command::new(env!("CARGO_BIN_EXE_argon"));
    command.arg("host");
    if let Some(path) = activation_file {
        command.arg("--activate-file").arg(path);
    }
    let mut child = Host(
        command
            .args(["--bind", &format!("127.0.0.1:{port}"), "--content"])
            .arg(content)
            .arg("--state")
            .arg(state)
            .env("ARGON_RELEASE", "integration-test")
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap(),
    );
    for _ in 0..100 {
        assert!(
            child.0.try_wait().unwrap().is_none(),
            "host exited before listening"
        );
        if TcpStream::connect(("127.0.0.1", port)).is_ok() {
            return (child, port);
        }
        thread::sleep(Duration::from_millis(20));
    }
    panic!("host did not listen within two seconds");
}

fn content_at(path: &Path, speed: u32) -> ContentRuntime {
    fs::create_dir_all(path).unwrap();
    fs::write(
        path.join("manifest.json"),
        r#"{"schema_version":1,"entry":"rules.lua","files":["rules.lua"]}"#,
    )
    .unwrap();
    fs::write(path.join("rules.lua"), format!("return {{ schema_version = 1, world_name = 'Test world', step = function(x, y, dx, dy) return x + dx * {speed}, y + dy * {speed} end }}")).unwrap();
    ContentRuntime::load(path).unwrap()
}

#[test]
fn acknowledged_positions_and_sessions_survive_restart() {
    let tmp = tempfile::tempdir().unwrap();
    let runtime = content_at(&tmp.path().join("content"), 2);
    let state = tmp.path().join("nested/world.json");
    let mut world = World::load(&state).unwrap();
    let joined = world.join(None).unwrap();
    let moved = world
        .move_player(&joined.token, 1, 1.0, 0.0, &runtime)
        .unwrap();
    assert_eq!(moved.x, joined.player.x + 2.0);
    drop(world);
    let mut restored = World::load(&state).unwrap();
    let resumed = restored.join(Some(&joined.token)).unwrap();
    assert_eq!(resumed.player.id, joined.player.id);
    assert_eq!(resumed.player.x, moved.x);
    assert_eq!(restored.snapshot().len(), 1);
}

#[test]
fn identifiers_are_not_session_capabilities_and_public_state_hides_tokens() {
    let tmp = tempfile::tempdir().unwrap();
    let runtime = content_at(&tmp.path().join("content"), 1);
    let mut world = World::load(tmp.path().join("world.json")).unwrap();
    let alice = world.join(None).unwrap();
    let bob = world.join(None).unwrap();
    assert!(world
        .move_player(&alice.player.id, 1, 1.0, 0.0, &runtime)
        .is_err());
    assert!(world.join(Some("invalid-session")).is_err());
    world
        .move_player(&bob.token, 1, 1.0, 0.0, &runtime)
        .unwrap();
    assert_eq!(world.snapshot()[0].x, alice.player.x);
    let public = serde_json::to_string(&world.snapshot()).unwrap();
    assert!(!public.contains(&alice.token));
    assert!(!public.contains(&bob.token));
    assert!(!public.contains("token"));
}

#[test]
fn rejects_nonfinite_and_unbounded_moves_without_changing_saved_state() {
    let tmp = tempfile::tempdir().unwrap();
    let runtime = content_at(&tmp.path().join("content"), 1);
    let path = tmp.path().join("world.json");
    let mut world = World::load(&path).unwrap();
    let joined = world.join(None).unwrap();
    let before = fs::read(&path).unwrap();
    for (dx, dy) in [
        (f64::NAN, 0.0),
        (0.0, f64::INFINITY),
        (2.0, 0.0),
        (1.0, 1.0),
    ] {
        assert!(world
            .move_player(&joined.token, 1, dx, dy, &runtime)
            .is_err());
        assert_eq!(fs::read(&path).unwrap(), before);
    }
}

#[test]
fn content_reload_changes_rules_without_resetting_players() {
    let tmp = tempfile::tempdir().unwrap();
    let content = tmp.path().join("content");
    let mut runtime = content_at(&content, 1);
    let mut world = World::load(tmp.path().join("world.json")).unwrap();
    let joined = world.join(None).unwrap();
    let first = world
        .move_player(&joined.token, 1, 1.0, 0.0, &runtime)
        .unwrap();
    let old_version = runtime.version().to_string();
    let _ = content_at(&content, 3);
    assert!(runtime.reload().unwrap());
    assert_ne!(runtime.version(), old_version);
    let second = world
        .move_player(&joined.token, 2, 1.0, 0.0, &runtime)
        .unwrap();
    assert_eq!(second.id, first.id);
    assert_eq!(second.x, first.x + 3.0);
    let good_version = runtime.version().to_string();
    fs::write(content.join("rules.lua"), "this is not Lua").unwrap();
    assert!(runtime.reload().is_err());
    assert_eq!(runtime.version(), good_version);
    let third = world
        .move_player(&joined.token, 3, 1.0, 0.0, &runtime)
        .unwrap();
    assert_eq!(third.x, second.x + 3.0);
}

#[test]
fn corrupt_save_is_not_silently_replaced() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("world.json");
    fs::write(&path, "broken saved world").unwrap();
    assert!(World::load(&path).is_err());
    assert_eq!(fs::read_to_string(path).unwrap(), "broken saved world");
}

#[test]
fn failed_persistence_does_not_acknowledge_or_mutate_world() {
    let tmp = tempfile::tempdir().unwrap();
    let runtime = content_at(&tmp.path().join("content"), 1);
    let path = tmp.path().join("world.json");
    let mut world = World::load(&path).unwrap();
    let joined = world.join(None).unwrap();
    fs::remove_file(&path).unwrap();
    fs::create_dir(&path).unwrap();
    assert!(world
        .move_player(&joined.token, 1, 1.0, 0.0, &runtime)
        .is_err());
    assert_eq!(world.snapshot()[0].x, joined.player.x);
    assert!(world.join(None).is_err());
    assert_eq!(world.snapshot().len(), 1);
}

#[test]
fn save_has_only_one_writer_and_releases_lock_when_dropped() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("world.json");
    let world = World::load(&path).unwrap();
    assert!(World::load(&path).is_err());
    drop(world);
    assert!(World::load(&path).is_ok());
}

#[test]
fn replayed_move_is_applied_once_even_after_restart() {
    let tmp = tempfile::tempdir().unwrap();
    let runtime = content_at(&tmp.path().join("content"), 5);
    let path = tmp.path().join("world.json");
    let mut world = World::load(&path).unwrap();
    let joined = world.join(None).unwrap();
    let moved = world
        .move_player(&joined.token, 1, 1.0, 0.0, &runtime)
        .unwrap();
    assert_eq!(
        world
            .move_player(&joined.token, 1, 1.0, 0.0, &runtime)
            .unwrap(),
        moved
    );
    assert!(world
        .move_player(&joined.token, 1, -1.0, 0.0, &runtime)
        .is_err());
    drop(world);
    let mut world = World::load(&path).unwrap();
    assert_eq!(world.join(Some(&joined.token)).unwrap().next_sequence, 2);
    assert_eq!(
        world
            .move_player(&joined.token, 1, 1.0, 0.0, &runtime)
            .unwrap(),
        moved
    );
    assert_eq!(
        world
            .move_player(&joined.token, 2, 1.0, 0.0, &runtime)
            .unwrap()
            .x,
        moved.x + 5.0
    );
    assert!(world
        .move_player(&joined.token, 1, 1.0, 0.0, &runtime)
        .is_err());
}

#[test]
fn http_clients_share_authoritative_state_and_resume_after_killed_host() {
    let tmp = tempfile::tempdir().unwrap();
    let content = tmp.path().join("content");
    let _runtime = content_at(&content, 4);
    let state = tmp.path().join("world.json");
    let (host, port) = spawn_host(&content, &state);
    let (status, health) = request(port, "GET", "/health", "");
    assert_eq!(status, 200);
    assert_eq!(health["release"], "integration-test");
    assert!(health["content_version"].as_str().unwrap().len() >= 16);
    let (status, alice) = request(port, "POST", "/api/join", "{}");
    assert_eq!(status, 200);
    let (_, bob) = request(port, "POST", "/api/join", "{}");
    let body = serde_json::json!({"token":alice["token"],"sequence":1,"dx":1,"dy":0}).to_string();
    let (status, moved) = request(port, "POST", "/api/move", &body);
    assert_eq!(status, 200);
    assert_eq!(
        moved["player"]["x"].as_f64().unwrap(),
        alice["player"]["x"].as_f64().unwrap() + 4.0
    );
    let (_, shared) = request(port, "GET", "/api/state", "");
    assert_eq!(shared["players"].as_array().unwrap().len(), 2);
    assert_eq!(shared["players"][0]["x"], moved["player"]["x"]);
    assert_eq!(shared["players"][1]["x"], bob["player"]["x"]);
    assert!(!shared
        .to_string()
        .contains(alice["token"].as_str().unwrap()));
    drop(host);
    let (_host, port) = spawn_host(&content, &state);
    let (status, resumed) = request(
        port,
        "POST",
        "/api/join",
        &serde_json::json!({"token":alice["token"]}).to_string(),
    );
    assert_eq!(status, 200);
    assert_eq!(resumed["player"], moved["player"]);
    let (_, shared) = request(port, "GET", "/api/state", "");
    assert_eq!(shared["players"].as_array().unwrap().len(), 2);
}

#[test]
fn http_rejects_forged_identity_nonfinite_and_oversized_input() {
    let tmp = tempfile::tempdir().unwrap();
    let content = tmp.path().join("content");
    let _runtime = content_at(&content, 1);
    let (_host, port) = spawn_host(&content, &tmp.path().join("world.json"));
    let (_, alice) = request(port, "POST", "/api/join", "{}");
    let forged =
        serde_json::json!({"token":alice["player"]["id"],"sequence":1,"dx":1,"dy":0}).to_string();
    assert_eq!(request(port, "POST", "/api/move", &forged).0, 401);
    let impersonate = serde_json::json!({"token":alice["token"],"player_id":"player-2","sequence":1,"dx":1,"dy":0}).to_string();
    assert_eq!(request(port, "POST", "/api/move", &impersonate).0, 400);
    let invalid_number = format!(
        r#"{{"token":{},"sequence":1,"dx":1e999,"dy":0}}"#,
        alice["token"]
    );
    assert_eq!(request(port, "POST", "/api/move", &invalid_number).0, 400);
    assert_eq!(request(port, "POST", "/api/join", &" ".repeat(5000)).0, 413);
    let (_, shared) = request(port, "GET", "/api/state", "");
    assert_eq!(shared["players"][0], alice["player"]);
}

#[test]
fn http_accepts_same_origin_json_and_rejects_cross_origin_or_form_commands() {
    let tmp = tempfile::tempdir().unwrap();
    let content = tmp.path().join("content");
    let _runtime = content_at(&content, 1);
    let (_host, port) = spawn_host(&content, &tmp.path().join("world.json"));
    let headers = format!("Content-Type: application/json\r\nOrigin: http://127.0.0.1:{port}\r\n");
    assert_eq!(
        request_with_headers(port, "POST", "/api/join", "{}", &headers).0,
        200
    );
    assert_eq!(
        request_with_headers(
            port,
            "POST",
            "/api/join",
            "{}",
            "Content-Type: application/json\r\nOrigin: https://unrelated.example\r\n"
        )
        .0,
        403
    );
    assert_eq!(
        request_with_headers(
            port,
            "POST",
            "/api/join",
            "{}",
            "Content-Type: text/plain\r\n"
        )
        .0,
        415
    );
    let (_, state) = request(port, "GET", "/api/state", "");
    assert_eq!(state["players"].as_array().unwrap().len(), 1);
}

#[test]
fn http_hot_reload_reports_rejection_and_keeps_the_world() {
    let tmp = tempfile::tempdir().unwrap();
    let content = tmp.path().join("content");
    let _runtime = content_at(&content, 1);
    let (_host, port) = spawn_host(&content, &tmp.path().join("world.json"));
    let (_, before) = request(port, "GET", "/health", "");
    let (_, joined) = request(port, "POST", "/api/join", "{}");
    fs::write(content.join("rules.lua"), "syntax ? broken").unwrap();
    for _ in 0..30 {
        let (_, health) = request(port, "GET", "/health", "");
        if health["content_error"].is_string() {
            break;
        }
        thread::sleep(Duration::from_millis(20));
    }
    // A rejected fingerprint is cached by the loader. Its error must stay visible.
    thread::sleep(Duration::from_millis(220));
    let (_, rejected) = request(port, "GET", "/api/state", "");
    assert!(rejected["content_error"].is_string());
    assert_eq!(rejected["content_version"], before["content_version"]);
    assert_eq!(rejected["players"][0], joined["player"]);
    let expected = content_at(&content, 3);
    for _ in 0..30 {
        let (_, health) = request(port, "GET", "/health", "");
        if health["content_version"] == expected.version() {
            break;
        }
        thread::sleep(Duration::from_millis(20));
    }
    let (_, active) = request(port, "GET", "/api/state", "");
    assert_eq!(active["content_version"], expected.version());
    assert!(active["content_error"].is_null());
    assert_eq!(active["players"][0], joined["player"]);
}

#[test]
fn content_check_outputs_the_validated_fingerprint_and_fails_invalid_input() {
    let tmp = tempfile::tempdir().unwrap();
    let content = tmp.path().join("content");
    let runtime = content_at(&content, 1);
    let valid = Command::new(env!("CARGO_BIN_EXE_argon"))
        .args(["check-content", "--content"])
        .arg(&content)
        .output()
        .unwrap();
    assert!(valid.status.success());
    let result: serde_json::Value = serde_json::from_slice(&valid.stdout).unwrap();
    assert_eq!(result["content_version"], runtime.version());
    fs::write(content.join("rules.lua"), "not valid lua!").unwrap();
    let invalid = Command::new(env!("CARGO_BIN_EXE_argon"))
        .args(["check-content", "--content"])
        .arg(&content)
        .output()
        .unwrap();
    assert!(!invalid.status.success());
    assert!(invalid.stdout.is_empty());
}

#[test]
fn http_candidate_rejects_mutations_until_activation_is_committed() {
    let tmp = tempfile::tempdir().unwrap();
    let content = tmp.path().join("content");
    let _runtime = content_at(&content, 4);
    let path = tmp.path().join("world.json");
    let marker = tmp.path().join("activate");
    let mut world = World::load(&path).unwrap();
    let joined = world.join(None).unwrap();
    drop(world);
    let saved = fs::read(&path).unwrap();
    let (_host, port) = spawn_host_with_gate(&content, &path, Some(&marker));
    assert_eq!(
        request(port, "GET", "/health", "").1["accepting_commands"],
        false
    );
    assert_eq!(request(port, "POST", "/api/join", "{}").0, 503);
    let body = serde_json::json!({"token":joined.token,"sequence":1,"dx":1,"dy":0}).to_string();
    assert_eq!(request(port, "POST", "/api/move", &body).0, 503);
    assert_eq!(fs::read(&path).unwrap(), saved);
    fs::write(&marker, "committed").unwrap();
    assert_eq!(
        request(port, "GET", "/health", "").1["accepting_commands"],
        true
    );
    assert_eq!(request(port, "POST", "/api/move", &body).0, 200);
    assert_eq!(request(port, "POST", "/api/join", "{}").0, 200);
    assert_ne!(fs::read(&path).unwrap(), saved);
}

#[test]
fn http_slow_or_streamed_request_body_does_not_stall_authoritative_loop() {
    let tmp = tempfile::tempdir().unwrap();
    let content = tmp.path().join("content");
    let _runtime = content_at(&content, 1);
    let (_host, port) = spawn_host(&content, &tmp.path().join("world.json"));
    for framing in [
        "Content-Length: 2048",
        "Content-Length: 18446744073709551615",
        "Transfer-Encoding: chunked",
        "Content-Length: 128\r\nExpect: 100-continue",
        "Content-Length: 128\r\nConnection: upgrade",
        "Content-Length: 128",
    ] {
        let mut slow = TcpStream::connect(("127.0.0.1", port)).unwrap();
        slow.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
        write!(slow, "POST /api/join HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nContent-Type: application/json\r\n{framing}\r\n\r\n").unwrap();
        thread::sleep(Duration::from_millis(30));
        assert_eq!(request(port, "GET", "/health", "").0, 200);
        let mut response = String::new();
        std::io::BufReader::new(&mut slow)
            .read_line(&mut response)
            .unwrap();
        assert!(response.starts_with("HTTP/1.1 4"), "{response}");
    }
}

#[test]
fn http_accepts_a_body_arriving_after_headers_within_deadline() {
    let tmp = tempfile::tempdir().unwrap();
    let content = tmp.path().join("content");
    let _runtime = content_at(&content, 1);
    let (_host, port) = spawn_host(&content, &tmp.path().join("world.json"));
    let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    write!(stream, "POST /api/join HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nContent-Type: application/json\r\nContent-Length: 2\r\n\r\n").unwrap();
    thread::sleep(Duration::from_millis(100));
    stream.write_all(b"{}").unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    assert!(response.starts_with("HTTP/1.1 200"), "{response}");
}
