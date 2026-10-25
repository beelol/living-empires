use anyhow::{bail, Context, Result};
use argon_dev::{
    content::ContentRuntime,
    server::{Host, HostConfig},
};
use std::{
    path::PathBuf,
    process::Command,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

fn main() {
    if let Err(error) = execute() {
        eprintln!("argon: {error:#}");
        std::process::exit(1);
    }
}

fn execute() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let command = args.next().unwrap_or_else(|| "help".into());
    if matches!(command.as_str(), "help" | "--help" | "-h") {
        println!("Argon development playground\n\n  argon play [--bind 0.0.0.0:4000] [--content dev/content] [--state .local/world.json]\n  argon host [--bind 0.0.0.0:4000] [--content dev/content] [--state .local/world.json]\n  argon join http://HOST:4000\n  argon check-content [--content dev/content]\n\nplay opens a browser and runs a LAN host. host stays in the foreground.\njoin opens an existing host in your browser. Press Ctrl-C to stop a host.\nFor managed deployment, --activate-file PATH gates commands until PATH exists.\nUse only on a trusted LAN; this is a development playground, not the full game.");
        return Ok(());
    }
    if command == "join" {
        let url = args.next().context("usage: argon join http://HOST:4000")?;
        if args.next().is_some() {
            bail!("join takes exactly one host URL");
        }
        validate_url(&url)?;
        open_browser(&url)?;
        println!("Opened {url}");
        return Ok(());
    }
    if !matches!(command.as_str(), "host" | "play" | "check-content") {
        bail!("unknown command {command}; run argon --help");
    }
    let config = parse_options(&command, args)?;
    if command == "check-content" {
        let loaded = ContentRuntime::load(config.content)?;
        println!(
            "{}",
            serde_json::json!({"ok":true,"content_version":loaded.version(),"world_name":loaded.world_name()})
        );
        return Ok(());
    }
    let stop = Arc::new(AtomicBool::new(false));
    let signal = stop.clone();
    ctrlc::set_handler(move || signal.store(true, Ordering::Relaxed))
        .context("cannot install shutdown handler")?;
    let host = Host::bind(config)?;
    if command == "play" {
        let url = host.browser_url();
        if let Err(error) = open_browser(&url) {
            eprintln!("Could not open the browser: {error:#}. Open {url} manually.");
        }
    }
    host.run(stop)
}

fn parse_options(command: &str, mut args: impl Iterator<Item = String>) -> Result<HostConfig> {
    let mut bind = "0.0.0.0:4000".to_string();
    let mut content = PathBuf::from("dev/content");
    let mut state = PathBuf::from(".local/world.json");
    let mut activation_file = None;
    let mut seen = std::collections::HashSet::new();
    while let Some(flag) = args.next() {
        if !seen.insert(flag.clone()) {
            bail!("option {flag} supplied more than once");
        }
        let value = args
            .next()
            .with_context(|| format!("missing value for {flag}"))?;
        if value.starts_with("--") {
            bail!("missing value for {flag}");
        }
        match flag.as_str() {
            "--content" => content = value.into(),
            "--state" if command != "check-content" => state = value.into(),
            "--bind" if command != "check-content" => bind = value,
            "--activate-file" if command != "check-content" => activation_file = Some(value.into()),
            _ => bail!("unknown option {flag} for {command}"),
        }
    }
    Ok(HostConfig {
        bind,
        content,
        state,
        release: std::env::var("ARGON_RELEASE").unwrap_or_else(|_| "local-dev".into()),
        activation_file,
    })
}

fn validate_url(url: &str) -> Result<()> {
    let remainder = url
        .strip_prefix("http://")
        .or_else(|| url.strip_prefix("https://"))
        .context("host URL must begin with http:// or https://")?;
    let authority = remainder.split('/').next().unwrap_or("");
    if authority.is_empty()
        || url.chars().any(|c| c.is_whitespace() || c.is_control())
        || authority.contains('@')
    {
        bail!("provide a valid host URL without embedded credentials");
    }
    Ok(())
}

fn open_browser(url: &str) -> Result<()> {
    #[cfg(target_os = "macos")]
    let status = Command::new("open").arg(url).status();
    #[cfg(all(unix, not(target_os = "macos")))]
    let status = Command::new("xdg-open").arg(url).status();
    #[cfg(target_os = "windows")]
    let status = Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            &format!("Start-Process '{}'", url.replace('\'', "''")),
        ])
        .status();
    let status = status.context("cannot launch the system browser")?;
    if !status.success() {
        bail!("browser launcher exited with {status}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::parse_options;

    #[test]
    fn play_and_host_default_to_lan_with_explicit_loopback_override() {
        for command in ["play", "host"] {
            let defaults = parse_options(command, std::iter::empty()).unwrap();
            assert_eq!(defaults.bind, "0.0.0.0:4000");
            let explicit = parse_options(
                command,
                ["--bind", "127.0.0.1:4999"].into_iter().map(str::to_owned),
            )
            .unwrap();
            assert_eq!(explicit.bind, "127.0.0.1:4999");
        }
    }
}
