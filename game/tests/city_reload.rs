//! Real running host: one Lua edit changes future paid sites; invalid edits retain policy.
use serde_json::Value;
use std::{
    fs,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};
struct Host(Child);
impl Drop for Host {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
fn health(port: u16) -> Option<Value> {
    let mut socket = TcpStream::connect_timeout(
        &format!("127.0.0.1:{port}").parse().ok()?,
        Duration::from_millis(200),
    )
    .ok()?;
    socket
        .set_read_timeout(Some(Duration::from_millis(300)))
        .ok()?;
    write!(
        socket,
        "GET /health HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n"
    )
    .ok()?;
    let mut data = String::new();
    socket.read_to_string(&mut data).ok()?;
    serde_json::from_str(data.split_once("\r\n\r\n")?.1).ok()
}
fn until(port: u16, mut check: impl FnMut(&Value) -> bool) -> Value {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if let Some(h) = health(port) && check(&h) {
            return h;
        }
        assert!(
            Instant::now() < deadline,
            "host did not reach requested reload state"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}
#[test]
fn live_city_lua_changes_paid_layout_without_restart_and_rejects_bad_edits() {
    let tmp = tempfile::tempdir().unwrap();
    let content = tmp.path().join("content");
    fs::create_dir(&content).unwrap();
    fs::write(
        content.join("manifest.json"),
        r#"{"schema_version":1,"entry":"rules.lua","files":["rules.lua","city_layout.lua"]}"#,
    )
    .unwrap();
    fs::write(content.join("rules.lua"),"return {schema_version=1,world_name='Reload proof',step=function(x,y,dx,dy) return x+dx,y+dy end}").unwrap();
    let baseline = include_str!("../../dev/content/city_layout.lua");
    fs::write(content.join("city_layout.lua"), baseline).unwrap();
    let port = TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let path = tmp.path().join("world.json");
    let mut host = Host(
        Command::new(env!("CARGO_BIN_EXE_empires"))
            .args([
                "host",
                "--bind",
                &format!("127.0.0.1:{port}"),
                "--seed",
                "72913",
                "--state",
            ])
            .arg(&path)
            .arg("--content")
            .arg(&content)
            .env("ARGON_ENV_FILE", tmp.path().join("no-provider.env"))
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap(),
    );
    let pid = host.0.id();
    let first = until(port, |h| {
        h["buildings"] == 3 && h["city_policy_error"].is_null()
    });
    let old = first["city_policy_active"]["helios"].clone();
    let edited = baseline.replace("{{-20,0},{20,0}", "{{20,0},{-20,0}");
    assert_ne!(edited, baseline);
    fs::write(content.join("city_layout.lua"), &edited).unwrap();
    let started = Instant::now();
    let changed = until(port, |h| {
        h["city_policy_active"]["helios"] != old && h["city_policy_error"].is_null()
    });
    assert!(started.elapsed() < Duration::from_secs(5));
    let activation_elapsed = started.elapsed();
    let active = changed["city_policy_active"].clone();
    until(port, |h| h["buildings"] == 9);
    let deadline = Instant::now() + Duration::from_secs(15);
    let saved = loop {
        let saved: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        if saved["settlements"]["buildings"]
            .as_array()
            .unwrap()
            .iter()
            .all(|b| b["view"]["complete"] == true)
        {
            break saved;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(30));
    };
    let buildings = saved["settlements"]["buildings"].as_array().unwrap();
    let generator = buildings
        .iter()
        .find(|b| b["view"]["faction_id"] == "helios" && b["view"]["role"] == "generator")
        .unwrap();
    assert_eq!(
        generator["view"]["center"],
        serde_json::json!([1460.0, 1536.0])
    );
    for invalid in [
        "this is invalid Lua",
        "return {schema_version=1,offsets=function() while true do end end}",
    ] {
        let observed = health(port).unwrap()["city_policy_observed"].clone();
        fs::write(content.join("city_layout.lua"), invalid).unwrap();
        let rejected = until(port, |h| {
            h["city_policy_observed"] != observed
                && !h["city_policy_error"].is_null()
                && (!invalid.contains("while true")
                    || h["city_policy_error"]
                        .as_str()
                        .is_some_and(|e| e.contains("Rejected city policy")))
        });
        assert_eq!(rejected["city_policy_active"], active);
        assert_eq!(rejected["buildings"], 9);
        assert_eq!(host.0.id(), pid);
        assert!(host.0.try_wait().unwrap().is_none());
    }
    let after: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    assert_eq!(after, saved);
    eprintln!(
        "Live reload proof: PID {pid} retained, valid policy active in {:?}, nine paid buildings retained after invalid and looping edits",
        activation_elapsed
    );
}
