use crate::{
    content::ContentRuntime,
    world::{World, HEIGHT, WIDTH},
};
use anyhow::{Context, Result};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    io::{Read, Write},
    net::{Shutdown, TcpListener, TcpStream},
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc::{self, SyncSender},
        Arc,
    },
    thread,
    time::{Duration, Instant},
};

const BODY_LIMIT: usize = 1024;
const HEADER_LIMIT: usize = 8192;
const NETWORK_WORKERS: usize = 32;
const NETWORK_DEADLINE: Duration = Duration::from_secs(1);
const RELOAD_INTERVAL: Duration = Duration::from_millis(100);

pub struct HostConfig {
    pub bind: String,
    pub content: PathBuf,
    pub state: PathBuf,
    pub release: String,
    pub activation_file: Option<PathBuf>,
}

pub struct Host {
    listener: TcpListener,
    content: ContentRuntime,
    world: World,
    release: String,
    reload_error: Option<String>,
    activation_file: Option<PathBuf>,
}

impl Host {
    pub fn bind(config: HostConfig) -> Result<Self> {
        let content = ContentRuntime::load(&config.content)?;
        let world = World::load(&config.state)?;
        let listener = TcpListener::bind(&config.bind)
            .with_context(|| format!("cannot bind {}", config.bind))?;
        listener.set_nonblocking(true)?;
        println!(
            "Argon development playground listening at http://{}",
            listener.local_addr()?
        );
        println!(
            "Content {} · saved world {}",
            content.version(),
            config.state.display()
        );
        println!("Trusted LAN development only. This is not the full game.");
        Ok(Self {
            listener,
            content,
            world,
            release: config.release,
            reload_error: None,
            activation_file: config.activation_file,
        })
    }

    pub fn browser_url(&self) -> String {
        let address = self.listener.local_addr().expect("bound HTTP host");
        if address.ip().is_unspecified() {
            let host = if address.is_ipv4() {
                "127.0.0.1"
            } else {
                "[::1]"
            };
            format!("http://{host}:{}", address.port())
        } else {
            format!("http://{address}")
        }
    }

    pub fn run(mut self, stop: Arc<AtomicBool>) -> Result<()> {
        let (send, receive) = mpsc::sync_channel::<Work>(NETWORK_WORKERS);
        let workers = Arc::new(AtomicUsize::new(0));
        let mut last_reload = Instant::now();
        while !stop.load(Ordering::Relaxed) {
            if last_reload.elapsed() >= RELOAD_INTERVAL {
                match self.content.reload() {
                    Ok(true) => {
                        println!("Activated content {}", self.content.version());
                        self.reload_error = None;
                    }
                    Ok(false) => {}
                    Err(error) => {
                        let message = format!("{error:#}");
                        if self.reload_error.as_ref() != Some(&message) {
                            eprintln!(
                                "Content rejected; keeping {}: {message}",
                                self.content.version()
                            );
                        }
                        self.reload_error = Some(message);
                    }
                }
                last_reload = Instant::now();
            }
            // Network reads and writes never run on the authoritative world loop.
            for _ in 0..NETWORK_WORKERS {
                match self.listener.accept() {
                    Ok((stream, _)) => {
                        if workers.load(Ordering::Relaxed) >= NETWORK_WORKERS {
                            let _ = stream.shutdown(Shutdown::Both);
                            continue;
                        }
                        workers.fetch_add(1, Ordering::Relaxed);
                        let guard = WorkerGuard(workers.clone());
                        let send = send.clone();
                        thread::spawn(move || {
                            let _guard = guard;
                            serve_connection(stream, send);
                        });
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => break,
                    Err(error) => return Err(error).context("cannot accept HTTP connection"),
                }
            }
            match receive.recv_timeout(Duration::from_millis(10)) {
                Ok((request, reply)) => {
                    let _ = reply.send(self.handle(&request));
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
        println!("Host stopped. Acknowledged player state is saved.");
        Ok(())
    }

    fn handle(&mut self, request: &Request) -> Reply {
        let path = request.path.split('?').next().unwrap_or("");
        if request.method == "POST"
            && matches!(path, "/api/join" | "/api/move")
            && !self.accepting_commands()
        {
            return Reply::error(
                503,
                "This candidate host is awaiting deployment activation. Reconnect shortly.",
            );
        }
        match (request.method.as_str(), path) {
            ("GET", "/") => Reply::html(include_str!("client.html")),
            ("GET", "/health") => Reply::json(200, self.health()),
            ("GET", "/api/state") => {
                let mut state = self.health();
                state["players"] = json!(self.world.snapshot());
                state["bounds"] = json!({"width":WIDTH,"height":HEIGHT});
                Reply::json(200, state)
            }
            ("POST", "/api/join") => {
                let input: JoinRequest = match read_json(request) {
                    Ok(input) => input,
                    Err(reply) => return reply,
                };
                if input
                    .token
                    .as_deref()
                    .is_some_and(|token| !self.world.has_session(token))
                {
                    return Reply::error(401, "Saved session is not present in this world. Join as a new commander to continue.");
                }
                match self.world.join(input.token.as_deref()) {
                    Ok(joined) => Reply::json(200, json!(joined)),
                    Err(error) => Reply::error(500, &format!("Join was not saved: {error:#}")),
                }
            }
            ("POST", "/api/move") => {
                let input: MoveRequest = match read_json(request) {
                    Ok(input) => input,
                    Err(reply) => return reply,
                };
                if !self.world.has_session(&input.token) {
                    return Reply::error(401, "Invalid commander session. Reconnect to continue.");
                }
                match self.world.move_player(
                    &input.token,
                    input.sequence,
                    input.dx,
                    input.dy,
                    &self.content,
                ) {
                    Ok(player) => Reply::json(
                        200,
                        json!({"player":player,"next_sequence":input.sequence + 1}),
                    ),
                    Err(error) => Reply::error(400, &format!("Move rejected: {error:#}")),
                }
            }
            (_, "/api/join" | "/api/move" | "/api/state" | "/health" | "/") => {
                Reply::error(405, "Method not allowed")
            }
            _ => Reply::error(404, "Not found"),
        }
    }

    fn accepting_commands(&self) -> bool {
        self.activation_file
            .as_ref()
            .is_none_or(|path| path.is_file())
    }

    fn health(&self) -> Value {
        json!({"ok":true,"accepting_commands":self.accepting_commands(),"content_version":self.content.version(),"world_name":self.content.world_name(),"release":self.release,"content_error":self.reload_error})
    }
}

pub fn run(config: HostConfig, stop: Arc<AtomicBool>) -> Result<()> {
    Host::bind(config)?.run(stop)
}

type Work = (Request, SyncSender<Reply>);
struct WorkerGuard(Arc<AtomicUsize>);
impl Drop for WorkerGuard {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::Relaxed);
    }
}

fn serve_connection(mut stream: TcpStream, send: SyncSender<Work>) {
    // Some platforms inherit the listener's nonblocking flag on accepted sockets.
    // Workers use explicit deadlines, so a packet arriving later must wait for it.
    if stream.set_nonblocking(false).is_err() {
        return;
    }
    let reply = match parse_request(&mut stream) {
        Ok(request) => {
            let (reply_send, reply_receive) = mpsc::sync_channel(1);
            match send.try_send((request, reply_send)) {
                Ok(()) => reply_receive
                    .recv_timeout(Duration::from_secs(2))
                    .unwrap_or_else(|_| {
                        Reply::error(503, "Host is busy. Reconnect to check the saved result.")
                    }),
                Err(_) => Reply::error(503, "Host request queue is full. Retry shortly."),
            }
        }
        Err(reply) => reply,
    };
    let _ = stream.set_write_timeout(Some(NETWORK_DEADLINE));
    let _ = respond(&mut stream, reply);
    let _ = stream.shutdown(Shutdown::Both);
}

struct Request {
    method: String,
    path: String,
    headers: HashMap<String, String>,
    body: Vec<u8>,
}

fn parse_request(stream: &mut TcpStream) -> std::result::Result<Request, Reply> {
    let deadline = Instant::now() + NETWORK_DEADLINE;
    let mut bytes = Vec::new();
    let header_end = loop {
        if let Some(index) = bytes.windows(4).position(|part| part == b"\r\n\r\n") {
            break index + 4;
        }
        if bytes.len() >= HEADER_LIMIT {
            return Err(Reply::error(431, "Request headers are too large"));
        }
        let mut buffer = [0; 1024];
        let limit = buffer.len().min(HEADER_LIMIT - bytes.len());
        let count = read_before_deadline(stream, &mut buffer[..limit], deadline)?;
        bytes.extend_from_slice(&buffer[..count]);
    };
    let text = std::str::from_utf8(&bytes[..header_end])
        .map_err(|_| Reply::error(400, "Invalid request headers"))?;
    let mut lines = text.split("\r\n");
    let start: Vec<_> = lines.next().unwrap_or("").split_whitespace().collect();
    if start.len() != 3 || start[2] != "HTTP/1.1" || !start[1].starts_with('/') {
        return Err(Reply::error(
            400,
            "Use an HTTP/1.1 request with an origin-relative path",
        ));
    }
    let method = start[0].to_owned();
    let path = start[1].to_owned();
    let mut headers = HashMap::new();
    for line in lines.take_while(|line| !line.is_empty()) {
        let (name, value) = line
            .split_once(':')
            .ok_or_else(|| Reply::error(400, "Invalid request header"))?;
        if name.is_empty()
            || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
            || value.bytes().any(|b| b.is_ascii_control() && b != b'\t')
        {
            return Err(Reply::error(400, "Invalid request header"));
        }
        if headers
            .insert(name.to_ascii_lowercase(), value.trim().to_owned())
            .is_some()
        {
            return Err(Reply::error(400, "Duplicate request header"));
        }
    }
    if headers.get("host").is_none_or(|host| host.is_empty()) {
        return Err(Reply::error(400, "Host header is required"));
    }
    if headers.contains_key("transfer-encoding") {
        return Err(Reply::error(
            400,
            "Chunked and streamed request bodies are not supported",
        ));
    }
    if headers.contains_key("expect") {
        return Err(Reply::error(417, "Expect requests are not supported"));
    }
    if headers.contains_key("upgrade")
        || headers
            .get("connection")
            .is_some_and(|s| s.to_ascii_lowercase().contains("upgrade"))
    {
        return Err(Reply::error(400, "Connection upgrades are not supported"));
    }
    let length = match headers.get("content-length") {
        Some(raw) if !raw.is_empty() && raw.bytes().all(|b| b.is_ascii_digit()) => raw
            .parse::<usize>()
            .map_err(|_| Reply::error(413, "Request body is too large"))?,
        Some(_) => return Err(Reply::error(400, "Invalid Content-Length")),
        None if method == "POST" => return Err(Reply::error(411, "Content-Length is required")),
        None => 0,
    };
    if length > BODY_LIMIT {
        return Err(Reply::error(413, "Request body is too large"));
    }
    let mut body = bytes[header_end..]
        .iter()
        .take(length)
        .copied()
        .collect::<Vec<_>>();
    while body.len() < length {
        let mut buffer = [0; BODY_LIMIT];
        let count = read_before_deadline(stream, &mut buffer[..length - body.len()], deadline)?;
        body.extend_from_slice(&buffer[..count]);
    }
    Ok(Request {
        method,
        path,
        headers,
        body,
    })
}

fn read_before_deadline(
    stream: &mut TcpStream,
    buffer: &mut [u8],
    deadline: Instant,
) -> std::result::Result<usize, Reply> {
    let remaining = deadline
        .checked_duration_since(Instant::now())
        .filter(|d| !d.is_zero())
        .ok_or_else(|| Reply::error(408, "Request deadline exceeded"))?;
    stream
        .set_read_timeout(Some(remaining))
        .map_err(|_| Reply::error(400, "Cannot read request"))?;
    match stream.read(buffer) {
        Ok(0) => Err(Reply::error(400, "Incomplete request")),
        Ok(count) => Ok(count),
        Err(error)
            if matches!(
                error.kind(),
                std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
            ) =>
        {
            Err(Reply::error(408, "Request deadline exceeded"))
        }
        Err(_) => Err(Reply::error(400, "Cannot read request")),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct JoinRequest {
    token: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MoveRequest {
    token: String,
    sequence: u64,
    dx: f64,
    dy: f64,
}

fn read_json<T: serde::de::DeserializeOwned>(request: &Request) -> std::result::Result<T, Reply> {
    let content_type = request
        .headers
        .get("content-type")
        .map(String::as_str)
        .unwrap_or("")
        .split(';')
        .next()
        .unwrap_or("")
        .trim();
    if !content_type.eq_ignore_ascii_case("application/json") {
        return Err(Reply::error(415, "Use Content-Type: application/json"));
    }
    if let Some(origin) = request.headers.get("origin") {
        let expected = format!(
            "http://{}",
            request
                .headers
                .get("host")
                .map(String::as_str)
                .unwrap_or("")
        );
        if origin != &expected {
            return Err(Reply::error(403, "Cross-origin commands are not allowed"));
        }
    }
    serde_json::from_slice(&request.body)
        .map_err(|error| Reply::error(400, &format!("Invalid request: {error}")))
}

struct Reply {
    status: u16,
    content_type: &'static str,
    body: Vec<u8>,
}
impl Reply {
    fn html(body: &str) -> Self {
        Self {
            status: 200,
            content_type: "text/html; charset=utf-8",
            body: body.as_bytes().to_vec(),
        }
    }
    fn json(status: u16, body: Value) -> Self {
        Self {
            status,
            content_type: "application/json",
            body: body.to_string().into_bytes(),
        }
    }
    fn error(status: u16, message: &str) -> Self {
        Self::json(status, json!({"error":message}))
    }
}

fn respond(stream: &mut TcpStream, reply: Reply) -> std::io::Result<()> {
    let reason = match reply.status {
        200 => "OK",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        408 => "Request Timeout",
        411 => "Length Required",
        413 => "Content Too Large",
        415 => "Unsupported Media Type",
        417 => "Expectation Failed",
        431 => "Request Header Fields Too Large",
        500 => "Internal Server Error",
        503 => "Service Unavailable",
        _ => "Error",
    };
    write!(stream, "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nReferrer-Policy: no-referrer\r\nContent-Security-Policy: default-src 'self'; script-src 'self' 'unsafe-inline'; style-src 'self' 'unsafe-inline'; connect-src 'self'; frame-ancestors 'none'; base-uri 'none'\r\n\r\n", reply.status, reason, reply.content_type, reply.body.len())?;
    stream.write_all(&reply.body)
}
