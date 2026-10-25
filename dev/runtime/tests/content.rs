use argon_dev::content::ContentRuntime;
use std::fs;
use tempfile::TempDir;

fn assert_validation_rejects_promptly(dir: &TempDir) {
    use std::{
        process::{Command, Stdio},
        thread,
        time::{Duration, Instant},
    };
    let mut child = Command::new(env!("CARGO_BIN_EXE_argon"))
        .args(["check-content", "--content"])
        .arg(dir.path())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let start = Instant::now();
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            assert!(!status.success(), "invalid content activated");
            break;
        }
        if start.elapsed() > Duration::from_secs(1) {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("content validation blocked for more than one second");
        }
        thread::sleep(Duration::from_millis(10));
    }
}

fn pack() -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join("manifest.json"),
        r#"{"schema_version":1,"entry":"rules.lua","files":["rules.lua"]}"#,
    )
    .unwrap();
    rules(&dir, "return {schema_version=1, world_name='Test', step=function(x,y,dx,dy) return x+dx*2,y+dy*2 end}");
    dir
}
fn rules(dir: &TempDir, source: &str) {
    fs::write(dir.path().join("rules.lua"), source).unwrap();
}

#[test]
fn executes_lua_and_reloads_without_recreating_the_runtime() {
    let dir = pack();
    let mut runtime = ContentRuntime::load(dir.path()).unwrap();
    assert_eq!(runtime.step(10., 20., 1., 0.).unwrap(), (12., 20.));
    let version = runtime.version().to_string();
    assert!(!runtime.reload().unwrap());
    rules(&dir, "return {schema_version=1,world_name='Changed',step=function(x,y,dx,dy) return x+dx*3,y+dy*3 end}");
    assert!(runtime.reload().unwrap());
    assert_ne!(runtime.version(), version);
    assert_eq!(runtime.world_name(), "Changed");
    assert_eq!(runtime.step(12., 20., 1., 0.).unwrap(), (15., 20.));
}

#[test]
fn rejects_failed_reload_and_keeps_last_working_rules() {
    let dir = pack();
    let mut runtime = ContentRuntime::load(dir.path()).unwrap();
    let version = runtime.version().to_string();
    rules(&dir, "this is broken lua");
    assert!(runtime.reload().is_err());
    assert_eq!(runtime.version(), version);
    assert_eq!(runtime.step(0., 0., 1., 0.).unwrap(), (2., 0.));
    rules(&dir, "return {schema_version=1,world_name='Repaired',step=function(x,y,dx,dy) return x+dx,y+dy end}");
    assert!(runtime.reload().unwrap());
}

#[test]
fn missing_files_and_unknown_formats_never_activate() {
    let dir = pack();
    let mut runtime = ContentRuntime::load(dir.path()).unwrap();
    fs::remove_file(dir.path().join("rules.lua")).unwrap();
    assert!(runtime.reload().is_err());
    assert_eq!(runtime.step(0., 0., 1., 0.).unwrap(), (2., 0.));
    fs::write(
        dir.path().join("manifest.json"),
        r#"{"schema_version":2,"entry":"rules.lua","files":["rules.lua"]}"#,
    )
    .unwrap();
    assert!(ContentRuntime::load(dir.path()).is_err());
}

#[test]
fn rejects_traversal_and_duplicate_manifest_entries() {
    for files in [
        r#"["../rules.lua"]"#,
        r#"["/rules.lua"]"#,
        r#"["rules.lua","rules.lua"]"#,
    ] {
        let dir = pack();
        fs::write(
            dir.path().join("manifest.json"),
            format!("{{\"schema_version\":1,\"entry\":\"rules.lua\",\"files\":{files}}}"),
        )
        .unwrap();
        assert!(ContentRuntime::load(dir.path()).is_err());
    }
}

#[test]
fn loads_only_declared_local_modules() {
    let dir = pack();
    fs::write(
        dir.path().join("manifest.json"),
        r#"{"schema_version":1,"entry":"rules.lua","files":["rules.lua","speed.lua"]}"#,
    )
    .unwrap();
    fs::write(dir.path().join("speed.lua"), "return 7").unwrap();
    rules(&dir,"local speed=require('speed'); return {schema_version=1,world_name='Modules',step=function(x,y,dx,dy) return x+dx*speed,y+dy*speed end}");
    let mut runtime = ContentRuntime::load(dir.path()).unwrap();
    assert_eq!(runtime.step(0., 0., 1., 0.).unwrap(), (7., 0.));
    fs::write(dir.path().join("speed.lua"), "return 8").unwrap();
    assert!(runtime.reload().unwrap());
    assert_eq!(runtime.step(0., 0., 1., 0.).unwrap(), (8., 0.));
}

#[test]
fn rejects_broken_unused_modules_before_activation() {
    let dir = pack();
    fs::write(
        dir.path().join("manifest.json"),
        r#"{"schema_version":1,"entry":"rules.lua","files":["rules.lua","unused.lua"]}"#,
    )
    .unwrap();
    fs::write(dir.path().join("unused.lua"), "invalid lua !!").unwrap();
    assert!(ContentRuntime::load(dir.path()).is_err());
}

#[test]
fn native_pattern_work_cannot_stall_loading_or_movement() {
    let dir = pack();
    for code in [
        "string.find(string.rep('a',100000),'a*a*a*a*b'); return {}",
        "return {schema_version=1,world_name='Bad',step=function() string.find(string.rep('a',100000),'a*a*a*a*b'); return 0,0 end}",
    ] {
        rules(&dir,code);
        assert_validation_rejects_promptly(&dir);
    }
}

#[test]
fn native_collection_operations_cannot_bypass_instruction_limits() {
    let dir = pack();
    for operation in [
        "table.move({},1,1000000000000,1)",
        "string.rep('',1000000000000)",
        "table.insert(setmetatable({}, {__len=function() return 1000000000000 end}),1,1)",
        "table.remove(setmetatable({}, {__len=function() return 1000000000000 end}),1)",
    ] {
        for code in [operation.to_string(),format!("return {{schema_version=1,world_name='Bad',step=function() {operation}; return 0,0 end}}") ] {
            rules(&dir, &code);
            assert_validation_rejects_promptly(&dir);
        }
    }
}

#[test]
fn bounded_collection_helpers_remain_usable() {
    let dir = pack();
    rules(&dir, "local t={1,2}; table.insert(t,3); table.remove(t,1); table.move(t,1,2,1,t); table.sort(t); local a,b=table.unpack(t); return {schema_version=1,world_name=table.concat({'T','est'})..string.rep('!',2),step=function(x,y,dx,dy) return x+dx*(a+b),y end}");
    let runtime = ContentRuntime::load(dir.path()).unwrap();
    assert_eq!(runtime.world_name(), "Test!!");
    assert_eq!(runtime.step(0., 0., 1., 0.).unwrap(), (5., 0.));
}

#[cfg(unix)]
#[test]
fn special_files_cannot_block_content_loading() {
    let dir = pack();
    fs::remove_file(dir.path().join("rules.lua")).unwrap();
    assert!(std::process::Command::new("mkfifo")
        .arg(dir.path().join("rules.lua"))
        .status()
        .unwrap()
        .success());
    assert_validation_rejects_promptly(&dir);
}

#[test]
fn blocks_filesystem_process_and_unbounded_execution() {
    for code in [
        "while true do end",
        "return io.open('/etc/passwd')",
        "return os.execute('true')",
        "return require('outside')",
        "return loadfile('/etc/passwd')",
        "return {schema_version=1,world_name='Loop',step=function() while true do end end}",
    ] {
        let dir = pack();
        rules(&dir, code);
        assert!(ContentRuntime::load(dir.path()).is_err(), "accepted {code}");
    }
}

#[test]
fn rejects_invalid_movement_from_callers_and_lua() {
    let dir = pack();
    let runtime = ContentRuntime::load(dir.path()).unwrap();
    assert!(runtime.step(0., 0., f64::NAN, 0.).is_err());
    assert!(runtime.step(0., 0., 1000., 0.).is_err());
    for result in ["0/0,0", "math.huge,0", "x+10000,y"] {
        rules(&dir,&format!("return {{schema_version=1,world_name='Invalid',step=function(x,y,dx,dy) return {result} end}}"));
        assert!(ContentRuntime::load(dir.path()).is_err());
    }
}

#[cfg(unix)]
#[test]
fn rejects_file_symlinks_and_follows_atomic_pack_switches() {
    use std::os::unix::fs::symlink;
    let dir = pack();
    let other = pack();
    let pointer = tempfile::tempdir().unwrap();
    symlink(dir.path(), pointer.path().join("current")).unwrap();
    let mut runtime = ContentRuntime::load(pointer.path().join("current")).unwrap();
    rules(&other,"return {schema_version=1,world_name='New pack',step=function(x,y,dx,dy) return x+dx*4,y+dy*4 end}");
    symlink(other.path(), pointer.path().join("next")).unwrap();
    fs::rename(pointer.path().join("next"), pointer.path().join("current")).unwrap();
    assert!(runtime.reload().unwrap());
    assert_eq!(runtime.step(0., 0., 1., 0.).unwrap(), (4., 0.));
    fs::remove_file(other.path().join("rules.lua")).unwrap();
    symlink(dir.path().join("rules.lua"), other.path().join("rules.lua")).unwrap();
    assert!(runtime.reload().is_err());
}
