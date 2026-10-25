//! Small read-only HTTP status surface. Gameplay travels through Lightyear.
use anyhow::{Context, Result};
use serde_json::{Value, json};
use std::{
    io::{Read, Write},
    net::{Shutdown, SocketAddr, TcpListener, TcpStream},
    sync::{
        Arc, RwLock,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

const MAX_WORKERS: usize = 8;
const MAX_HEADER: usize = 8192;
const DEADLINE: Duration = Duration::from_secs(1);

struct Snapshot {
    value: Value,
    updated: Instant,
}

/// Only public development status is copied here; never session capabilities.
#[derive(Clone)]
pub struct Health(Arc<RwLock<Snapshot>>);

impl Health {
    pub fn new(value: Value) -> Self {
        Self(Arc::new(RwLock::new(Snapshot {
            value,
            updated: Instant::now(),
        })))
    }

    pub fn update(&self, value: Value) {
        if let Ok(mut snapshot) = self.0.write() {
            *snapshot = Snapshot {
                value,
                updated: Instant::now(),
            };
        }
    }

    fn read(&self) -> (u16, Value) {
        match self.0.read() {
            Ok(snapshot) if snapshot.updated.elapsed() < Duration::from_secs(2) => {
                (200, snapshot.value.clone())
            }
            _ => (
                503,
                json!({"ok":false,"error":"Authoritative host is not responding"}),
            ),
        }
    }
}

pub struct Admin {
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl Admin {
    pub fn bind(address: SocketAddr) -> Result<TcpListener> {
        let listener = TcpListener::bind(address)
            .with_context(|| format!("cannot bind host status at {address}"))?;
        listener.set_nonblocking(true)?;
        Ok(listener)
    }

    /// Start only after UDP startup and authoritative state validation succeeded.
    pub fn start(listener: TcpListener, health: Health) -> Self {
        Self::start_with_diplomacy(listener, health, None)
    }

    pub fn start_with_diplomacy(
        listener: TcpListener,
        health: Health,
        diplomacy: Option<Arc<RwLock<Value>>>,
    ) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let running = Arc::clone(&stop);
        let handle = thread::spawn(move || {
            let workers = Arc::new(AtomicUsize::new(0));
            while !running.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        if workers.load(Ordering::Relaxed) >= MAX_WORKERS {
                            let _ = stream.shutdown(Shutdown::Both);
                            continue;
                        }
                        workers.fetch_add(1, Ordering::Relaxed);
                        let guard = WorkerGuard(workers.clone());
                        let health = health.clone();
                        let diplomacy = diplomacy.clone();
                        thread::spawn(move || {
                            let _guard = guard;
                            let _ = serve(stream, &health, diplomacy.as_ref());
                        });
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(_) => break,
                }
            }
        });
        Self {
            stop,
            handle: Some(handle),
        }
    }
}

impl Drop for Admin {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

struct WorkerGuard(Arc<AtomicUsize>);
impl Drop for WorkerGuard {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::Relaxed);
    }
}

fn serve(
    mut stream: TcpStream,
    health: &Health,
    diplomacy: Option<&Arc<RwLock<Value>>>,
) -> Result<()> {
    stream.set_nonblocking(false)?;
    stream.set_write_timeout(Some(DEADLINE))?;
    let end = Instant::now() + DEADLINE;
    let mut bytes = Vec::with_capacity(512);
    let mut buffer = [0; 512];
    while !bytes.windows(4).any(|window| window == b"\r\n\r\n") {
        let remaining = end.saturating_duration_since(Instant::now());
        if remaining.is_zero() || bytes.len() >= MAX_HEADER {
            return Ok(());
        }
        stream.set_read_timeout(Some(remaining))?;
        let count = stream.read(&mut buffer)?;
        if count == 0 {
            return Ok(());
        }
        bytes.extend_from_slice(&buffer[..count]);
    }
    let first_line = bytes
        .split(|byte| *byte == b'\n')
        .next()
        .unwrap_or_default();
    let mut fields = first_line.split(|byte| *byte == b' ');
    let method = fields.next().unwrap_or_default();
    let path = fields.next().unwrap_or_default();
    let is_operator = matches!(path, b"/diplomacy" | b"/api/diplomacy");
    let headers = String::from_utf8_lossy(&bytes);
    let host = headers
        .lines()
        .find_map(|line| {
            line.split_once(':')
                .filter(|(name, _)| name.eq_ignore_ascii_case("host"))
                .map(|(_, value)| value.trim())
        })
        .unwrap_or("");
    let origin = headers.lines().find_map(|line| {
        line.split_once(':')
            .filter(|(name, _)| name.eq_ignore_ascii_case("origin"))
            .map(|(_, value)| value.trim())
    });
    let local_host = host
        .parse::<SocketAddr>()
        .is_ok_and(|address| address.ip().is_loopback())
        || host == "localhost"
        || host
            .strip_prefix("localhost:")
            .is_some_and(|port| port.parse::<u16>().is_ok());
    let local_access = stream.peer_addr()?.ip().is_loopback()
        && local_host
        && origin.is_none_or(|value| value == format!("http://{host}"));
    let (status, body, content_type) = match (method, path) {
        _ if is_operator && !local_access => (403, "Diplomacy operator view is available only on localhost.\n".to_owned(), "text/plain"),
        (b"GET", b"/diplomacy") => (200, include_str!("diplomacy.html").to_owned(), "text/html; charset=utf-8"),
        (b"GET", b"/api/diplomacy") => {
            let value = diplomacy.and_then(|value| value.read().ok().map(|value| value.clone()))
                .unwrap_or_else(|| json!({"phase":"unavailable","message":"Diplomacy worker is not available in this host"}));
            (200, serde_json::to_string(&value)?, "application/json")
        }
        (b"GET", b"/health") => {
            let (status, value) = health.read();
            (status, serde_json::to_string(&value)?, "application/json")
        }
        (b"GET", b"/") => (
            200,
            "Living Empires native host is running. Play with `empires join HOST:PORT`. Gameplay uses Bevy and Lightyear UDP. Status: /health. Local diplomacy demo: /diplomacy\n".to_owned(),
            "text/plain; charset=utf-8",
        ),
        (b"GET", _) => (404, "Not found\n".to_owned(), "text/plain"),
        _ => (405, "Read-only status endpoint\n".to_owned(), "text/plain"),
    };
    let reason = match status {
        200 => "OK",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        _ => "Service Unavailable",
    };
    write!(
        stream,
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nReferrer-Policy: no-referrer\r\nContent-Security-Policy: default-src 'self'; script-src 'self' 'unsafe-inline'; style-src 'self' 'unsafe-inline'; connect-src 'self'; frame-ancestors 'none'; base-uri 'none'\r\n\r\n{body}",
        body.len()
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stale_authority_is_not_healthy() {
        let health = Health::new(json!({"ok":true}));
        assert_eq!(health.read().0, 200);
        health.0.write().unwrap().updated = Instant::now() - Duration::from_secs(3);
        assert_eq!(health.read().0, 503);
        health.update(json!({"ok":true}));
        assert_eq!(health.read().0, 200);
    }

    #[test]
    fn diplomacy_page_is_local_and_rejects_cross_origin_reads() {
        let listener = Admin::bind("127.0.0.1:0".parse().unwrap()).unwrap();
        let address = listener.local_addr().unwrap();
        let _admin = Admin::start(listener, Health::new(json!({"ok":true})));
        let request = |host: &str, origin: &str| {
            let mut stream = TcpStream::connect(address).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            write!(
                stream,
                "GET /diplomacy HTTP/1.1\r\nHost: {host}\r\n{origin}Connection: close\r\n\r\n"
            )
            .unwrap();
            let mut result = String::new();
            stream.read_to_string(&mut result).unwrap();
            result
        };
        let result = request(&address.to_string(), "");
        assert!(result.starts_with("HTTP/1.1 200"), "{result}");
        assert!(result.contains("Diplomacy"));
        assert!(request("attacker.example", "").starts_with("HTTP/1.1 403"));
        assert!(
            request(&address.to_string(), "Origin: https://attacker.example\r\n")
                .starts_with("HTTP/1.1 403")
        );
    }
}
