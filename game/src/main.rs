use anyhow::{Context, Result, bail};
use argon_dev::content::ContentRuntime;
use living_empires::{
    client::{self, ClientConfig},
    server::{self, HostConfig},
};
#[cfg(feature = "native")]
use std::{
    io::{Read, Write},
    net::{IpAddr, Ipv4Addr, TcpStream},
    process::{Child, Command, Stdio},
    time::Instant,
};
use std::{
    net::{SocketAddr, ToSocketAddrs, UdpSocket},
    path::PathBuf,
    time::Duration,
};

fn main() {
    if let Err(error) = execute() {
        eprintln!("empires: {error:#}");
        std::process::exit(1);
    }
}

#[derive(Debug)]
struct Options {
    bind: SocketAddr,
    advertise: Vec<SocketAddr>,
    content: PathBuf,
    state: PathBuf,
    session: Option<PathBuf>,
    activation_file: Option<PathBuf>,
    moves: Vec<(f64, f64)>,
    timeout: Duration,
}

fn options(command: &str, args: impl Iterator<Item = String>) -> Result<Options> {
    let mut value = Options {
        bind: "0.0.0.0:4000".parse()?,
        advertise: Vec::new(),
        content: "dev/content".into(),
        state: ".local/world.json".into(),
        session: None,
        activation_file: None,
        moves: Vec::new(),
        timeout: Duration::from_secs(15),
    };
    let mut args = args;
    let mut seen = std::collections::HashSet::new();
    while let Some(flag) = args.next() {
        if !matches!(flag.as_str(), "--advertise" | "--move") && !seen.insert(flag.clone()) {
            bail!("option {flag} supplied more than once");
        }
        let text = args
            .next()
            .with_context(|| format!("missing value for {flag}"))?;
        match (command, flag.as_str()) {
            ("host" | "play" | "check-content", "--content") => value.content = text.into(),
            ("host" | "play", "--bind") => {
                value.bind = text.parse().context("--bind must be an IP:PORT")?;
                if value.bind.port() == 0 {
                    bail!("use a nonzero port");
                }
            }
            ("host" | "play", "--advertise") => value.advertise.push(resolve(&text)?),
            ("host" | "play", "--state") => value.state = text.into(),
            ("host", "--activate-file") => value.activation_file = Some(text.into()),
            ("play" | "join" | "probe-client", "--session") => value.session = Some(text.into()),
            ("probe-client", "--move") => {
                let (x, y) = text.split_once(',').context("--move needs dx,dy")?;
                let (x, y): (f64, f64) = (x.parse()?, y.parse()?);
                if !x.is_finite() || !y.is_finite() {
                    bail!("movement must be finite");
                }
                value.moves.push((x, y));
            }
            ("probe-client", "--timeout") => {
                let seconds: f64 = text.parse()?;
                if !seconds.is_finite() || !(0.1..=300.0).contains(&seconds) {
                    bail!("timeout must be between 0.1 and 300 seconds");
                }
                value.timeout = Duration::from_secs_f64(seconds);
            }
            _ => bail!("unknown option {flag} for {command}"),
        }
    }
    Ok(value)
}

fn resolve(address: &str) -> Result<SocketAddr> {
    let values: Vec<_> = address
        .to_socket_addrs()
        .context("host must be HOST:PORT (UDP), for example 192.168.1.10:4000")?
        .collect();
    let value = values
        .iter()
        .find(|a| a.is_ipv4())
        .or(values.first())
        .context("host resolved to no address")?;
    if value.port() == 0 || value.ip().is_unspecified() {
        bail!("join requires a reachable host and nonzero port");
    }
    Ok(*value)
}

fn session(options: &Options, server: SocketAddr) -> ClientConfig {
    ClientConfig {
        server,
        session: options.session.clone().unwrap_or_else(|| {
            PathBuf::from(".local").join(format!(
                "session-{}.json",
                server.to_string().replace(':', "_")
            ))
        }),
    }
}

fn host_config(mut options: Options) -> HostConfig {
    // UDP connect chooses an interface without sending a packet. Other interfaces
    // (for example a VPN) can be declared with repeated --advertise addresses.
    if options.bind.ip().is_unspecified()
        && options.bind.is_ipv4()
        && let Ok(socket) = UdpSocket::bind("0.0.0.0:0")
        && socket.connect("192.0.2.1:9").is_ok()
        && let Ok(local) = socket.local_addr()
    {
        options
            .advertise
            .push(SocketAddr::new(local.ip(), options.bind.port()));
    }
    options.advertise.sort();
    options.advertise.dedup();
    for address in &options.advertise {
        eprintln!("Join from another device: empires join {address}");
    }
    HostConfig {
        bind: options.bind,
        advertise: options.advertise,
        content: options.content,
        state: options.state,
        release: std::env::var("ARGON_RELEASE").unwrap_or_else(|_| "local-dev".into()),
        activation_file: options.activation_file,
    }
}

#[cfg(feature = "native")]
fn health(address: SocketAddr) -> Result<serde_json::Value> {
    let mut stream = TcpStream::connect_timeout(&address, Duration::from_millis(300))?;
    stream.set_read_timeout(Some(Duration::from_millis(500)))?;
    stream.set_write_timeout(Some(Duration::from_millis(500)))?;
    stream.write_all(b"GET /health HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")?;
    let mut bytes = Vec::new();
    stream.take(16_384).read_to_end(&mut bytes)?;
    let body = bytes
        .windows(4)
        .position(|v| v == b"\r\n\r\n")
        .context("invalid health response")?
        + 4;
    let value: serde_json::Value = serde_json::from_slice(&bytes[body..])?;
    if value.get("transport").and_then(|v| v.as_str()) != Some("lightyear-udp")
        || value["ok"] != true
    {
        bail!("port is occupied by a different or unhealthy server");
    }
    Ok(value)
}

#[cfg(feature = "native")]
struct OwnedHost(Child);
#[cfg(feature = "native")]
impl Drop for OwnedHost {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[cfg(feature = "native")]
fn play(options: Options) -> Result<()> {
    let mut address = options.bind;
    if address.ip().is_unspecified() {
        address.set_ip(if address.is_ipv4() {
            IpAddr::V4(Ipv4Addr::LOCALHOST)
        } else {
            IpAddr::V6(std::net::Ipv6Addr::LOCALHOST)
        });
    }
    let config = session(&options, address);
    let _owned = match TcpStream::connect_timeout(&address, Duration::from_millis(300)) {
        Ok(_) => {
            let result = health(address)?;
            if result["accepting_commands"] != true {
                bail!("existing host is preparing a release; retry in a moment");
            }
            eprintln!("Joining the existing host at {address}");
            None
        }
        Err(_) => {
            let mut command = Command::new(std::env::current_exe()?);
            command
                .args(["host", "--bind", &options.bind.to_string(), "--content"])
                .arg(&options.content)
                .arg("--state")
                .arg(&options.state)
                .stdin(Stdio::null());
            for advertised in &options.advertise {
                command.arg("--advertise").arg(advertised.to_string());
            }
            let mut child = OwnedHost(command.spawn().context("could not start the host")?);
            let deadline = Instant::now() + Duration::from_secs(15);
            loop {
                if let Some(status) = child.0.try_wait()? {
                    bail!("host exited during startup: {status}");
                }
                if let Ok(value) = health(address)
                    && value["accepting_commands"] == true
                {
                    break;
                }
                if Instant::now() >= deadline {
                    bail!("host did not become ready within 15 seconds");
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            Some(child)
        }
    };
    // The window stays on the main thread; a host started here stops when it closes.
    client::run(config)
}

fn execute() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let command = args.next().unwrap_or_else(|| "help".into());
    if matches!(command.as_str(), "help" | "--help" | "-h") {
        println!(
            "Living Empires — Bevy + Lightyear development foundation\n\n  empires play [--bind 0.0.0.0:4000] [--content dev/content] [--state .local/world.json]\n  empires host [same options] [--advertise LAN_OR_VPN_IP:4000]\n  empires join HOST:4000 [--session .local/player.json]\n  empires check-content [--content dev/content]\n  empires probe-client HOST:4000 [--session PATH] [--move 1,0] [--timeout 15]\n\nplay opens a native window and starts a host if none is running.\njoin uses Lightyear UDP; allow port 4000 through the host firewall.\nLua updates reload live. Native changes reconnect clients to the durable world.\nDefault builds support all commands. --no-default-features builds the headless host/probe.\nDevelopment authentication is for trusted LANs only. This is not the full game."
        );
        return Ok(());
    }
    let address = if matches!(command.as_str(), "join" | "probe-client") {
        Some(resolve(&args.next().context("provide HOST:PORT")?)?)
    } else {
        None
    };
    if !matches!(
        command.as_str(),
        "host" | "play" | "join" | "check-content" | "probe-client"
    ) {
        bail!("unknown command {command}; run empires --help");
    }
    let options = options(&command, args)?;
    match command.as_str() {
        "check-content" => {
            let loaded = ContentRuntime::load(options.content)?;
            println!(
                "{}",
                serde_json::json!({"ok":true,"content_version":loaded.version(),"world_name":loaded.world_name()})
            );
            Ok(())
        }
        "host" => server::run(host_config(options)),
        "probe-client" => {
            let result = client::probe(
                session(&options, address.unwrap()),
                options.moves,
                options.timeout,
            )?;
            println!("{}", serde_json::to_string(&result)?);
            Ok(())
        }
        #[cfg(feature = "native")]
        "join" => client::run(session(&options, address.unwrap())),
        #[cfg(feature = "native")]
        "play" => play(options),
        _ => bail!(
            "this is a headless build; run cargo build -p living-empires for the native window"
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reject_ambiguous_or_unreachable_cli_inputs() {
        assert!(resolve("0.0.0.0:4000").is_err());
        assert!(resolve("127.0.0.1:0").is_err());
        for args in [
            ["--timeout", "NaN"],
            ["--move", "NaN,1"],
            ["--bind", "127.0.0.1:4000"],
        ] {
            assert!(options("probe-client", args.into_iter().map(str::to_owned)).is_err());
        }
        assert!(
            options(
                "host",
                ["--bind", "127.0.0.1:0"].into_iter().map(str::to_owned)
            )
            .is_err()
        );
    }
}
