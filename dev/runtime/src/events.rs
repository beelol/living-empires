//! Development meteor simulation. Public broadcasts and survey drones are real
//! authoritative state; these fixture drones do not imply a paid economy exists.
use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};

pub const FACTIONS: [&str; 3] = ["helios", "tide", "cinder"];
const MAX_EVENTS: usize = 256;
const MAX_BYTES: u64 = 8 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum Phase {
    Warning,
    Impact,
    Cooled,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PublicEvent {
    pub id: u64,
    pub kind: String,
    pub warning_at: u64,
    pub impact_at: u64,
    pub clear_at: u64,
    pub x: f64,
    pub y: f64,
    pub radius: f64,
    pub phase: Phase,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Observation {
    pub event_id: u64,
    pub at: u64,
    pub source: String,
    pub phase: Phase,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SurveyUnit {
    pub faction: String,
    pub home: [f64; 2],
    pub x: f64,
    pub y: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Response {
    pub faction: String,
    pub event_id: u64,
    pub started_at: u64,
    pub completed_at: Option<u64>,
    pub goal: String,
    pub destination: [f64; 2],
    pub completed: bool,
}

/// GM v1 envelope. `at` is an actually captured UNIX receipt time; simulation
/// timestamps remain explicitly labeled inside the payload. Public facts only.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Broadcast {
    pub id: String,
    pub source: String,
    pub at: u64,
    pub kind: String,
    pub payload: serde_json::Value,
    pub allowed_recipients: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct EventSnapshot {
    pub schema_version: u32,
    pub now: u64,
    pub events: Vec<PublicEvent>,
    pub units: Vec<SurveyUnit>,
    pub responses: Vec<Response>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct EventRecord {
    event: PublicEvent,
    broadcast: bool,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Knowledge {
    faction: String,
    observations: Vec<Observation>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Saved {
    schema_version: u32,
    seed: u64,
    now: u64,
    next_warning: u64,
    center: [f64; 2],
    events: Vec<EventRecord>,
    knowledge: Vec<Knowledge>,
    units: Vec<SurveyUnit>,
    responses: Vec<Response>,
    #[serde(default)]
    namespace: String,
    #[serde(default)]
    broadcasts: Vec<Broadcast>,
}

pub struct EventWorld {
    saved: Saved,
    path: Option<PathBuf>,
    lock: Option<File>,
}

impl Drop for EventWorld {
    fn drop(&mut self) {
        if let Some(lock) = &self.lock {
            let _ = fs2::FileExt::unlock(lock);
        }
    }
}

impl EventWorld {
    pub fn new(seed: u64) -> Self {
        let homes = [[380.0, 260.0], [620.0, 260.0], [500.0, 420.0]];
        Self {
            saved: Saved {
                schema_version: 1,
                seed,
                now: 0,
                next_warning: 20,
                center: [520.0, 300.0],
                events: vec![],
                responses: vec![],
                namespace: format!("fixture-{seed:x}"),
                broadcasts: vec![],
                knowledge: FACTIONS
                    .iter()
                    .map(|f| Knowledge {
                        faction: (*f).into(),
                        observations: vec![],
                    })
                    .collect(),
                units: FACTIONS
                    .iter()
                    .zip(homes)
                    .map(|(f, home)| SurveyUnit {
                        faction: (*f).into(),
                        home,
                        x: home[0],
                        y: home[1],
                    })
                    .collect(),
            },
            path: None,
            lock: None,
        }
    }

    /// Separate sidecar keeps commander and diplomacy formats unchanged. Its
    /// clock advances only through authority ticks, including with no players.
    pub fn load(path: impl AsRef<Path>, seed: u64) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        fs::create_dir_all(parent(&path))?;
        let mut lock_path = path.as_os_str().to_os_string();
        lock_path.push(".lock");
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(lock_path)?;
        fs2::FileExt::try_lock_exclusive(&lock).context("another host owns the event world")?;
        let mut world = Self::new(seed);
        world.saved.namespace = String::new();
        match File::open(&path) {
            Ok(file) => {
                let mut bytes = vec![];
                file.take(MAX_BYTES + 1).read_to_end(&mut bytes)?;
                ensure!(bytes.len() as u64 <= MAX_BYTES, "event save exceeds limit");
                world.saved = serde_json::from_slice(&bytes)
                    .context("invalid event save; refusing to reset history")?;
                validate(&world.saved)?;
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
        if world.saved.namespace.is_empty() {
            let mut nonce = [0; 16];
            getrandom::fill(&mut nonce).map_err(|e| anyhow::anyhow!("event namespace: {e}"))?;
            world.saved.namespace = format!(
                "world-{}",
                nonce.iter().map(|b| format!("{b:02x}")).collect::<String>()
            );
        }
        world.path = Some(path);
        world.lock = Some(lock);
        world.persist(&world.saved)?;
        Ok(world)
    }

    pub fn snapshot(&self) -> EventSnapshot {
        let public = |id| {
            self.saved
                .events
                .iter()
                .any(|r| r.event.id == id && r.broadcast)
        };
        EventSnapshot {
            schema_version: 1,
            now: self.saved.now,
            events: self
                .saved
                .events
                .iter()
                .filter(|r| r.broadcast)
                .map(|r| r.event.clone())
                .collect(),
            units: self
                .saved
                .units
                .iter()
                .filter(|u| {
                    self.saved
                        .responses
                        .iter()
                        .rev()
                        .find(|r| r.faction == u.faction)
                        .is_none_or(|r| public(r.event_id))
                })
                .cloned()
                .collect(),
            responses: self
                .saved
                .responses
                .iter()
                .filter(|r| public(r.event_id))
                .cloned()
                .collect(),
        }
    }

    pub fn observations(&self, faction: &str) -> Vec<Observation> {
        self.saved
            .knowledge
            .iter()
            .find(|k| k.faction == faction)
            .map(|k| k.observations.clone())
            .unwrap_or_default()
    }

    pub fn responses(&self, faction: &str) -> Vec<Response> {
        self.saved
            .responses
            .iter()
            .filter(|r| r.faction == faction)
            .cloned()
            .collect()
    }

    /// Durable public outbox, separate from frequent client snapshots so a long
    /// history does not inflate every gameplay packet. A consumer deduplicates IDs.
    pub fn broadcasts(&self) -> Vec<Broadcast> {
        self.saved.broadcasts.clone()
    }

    /// Set a fresh world's event region to a terrain-validated demo site. Never
    /// repositions established events, drones or history on reconnect/restart.
    pub fn set_center(&mut self, center: [f64; 2]) -> Result<()> {
        if center == self.saved.center {
            return Ok(());
        }
        ensure!(
            self.saved.now == 0 && self.saved.events.is_empty(),
            "established event region cannot move"
        );
        ensure!(
            center
                .iter()
                .all(|v| v.is_finite() && (200.0..=999_000.0).contains(v)),
            "invalid event region center"
        );
        let mut next = self.saved.clone();
        let offset = [center[0] - next.center[0], center[1] - next.center[1]];
        for unit in &mut next.units {
            unit.x += offset[0];
            unit.y += offset[1];
            unit.home = [unit.x, unit.y];
        }
        next.center = center;
        self.commit(next)
    }

    /// Called only after an authoritative sensor or report has established
    /// observation. Neither a model proposal nor a player message can call this.
    pub fn observe(&mut self, faction: &str, id: u64) -> Result<()> {
        ensure!(FACTIONS.contains(&faction), "unknown observing faction");
        let mut next = self.saved.clone();
        let event = next
            .events
            .iter()
            .find(|r| r.event.id == id)
            .context("unknown event")?
            .event
            .clone();
        observe(&mut next, faction, &event, "sensor/report");
        self.commit(next)
    }

    /// Authority-only audit/setup path; random events use the same transition.
    pub fn schedule_meteor(
        &mut self,
        x: f64,
        y: f64,
        radius: f64,
        delay: u64,
        broadcast: bool,
    ) -> Result<u64> {
        ensure!(
            x.is_finite() && y.is_finite() && x.abs() <= 1_000_000.0 && y.abs() <= 1_000_000.0,
            "invalid impact position"
        );
        ensure!(
            radius.is_finite() && (4.0..=64.0).contains(&radius),
            "invalid impact radius"
        );
        ensure!((1..=3600).contains(&delay), "invalid warning duration");
        ensure!(
            self.saved.events.len() < MAX_EVENTS,
            "event history full; retain this world"
        );
        let mut next = self.saved.clone();
        let id = schedule(&mut next, x, y, radius, delay, broadcast)?;
        self.commit(next)?;
        Ok(id)
    }

    pub fn advance(&mut self, seconds: u64) -> Result<()> {
        self.advance_with_receipt(seconds, None)
    }

    pub fn advance_recorded(&mut self, seconds: u64, receipt_unix: u64) -> Result<()> {
        ensure!(
            receipt_unix > 0
                && self
                    .saved
                    .broadcasts
                    .last()
                    .is_none_or(|b| receipt_unix >= b.at),
            "invalid or backwards broadcast receipt clock"
        );
        self.advance_with_receipt(seconds, Some(receipt_unix))
    }

    fn advance_with_receipt(&mut self, seconds: u64, receipt: Option<u64>) -> Result<()> {
        ensure!(
            seconds <= 3600,
            "advance at most one simulation hour per call"
        );
        if seconds == 0 {
            return Ok(());
        }
        let mut next = self.saved.clone();
        for _ in 0..seconds {
            let previous: Vec<_> = next
                .events
                .iter()
                .map(|r| (r.event.id, r.event.phase))
                .collect();
            tick(&mut next)?;
            if let Some(at) = receipt {
                for record in next
                    .events
                    .iter()
                    .filter(|r| r.broadcast && !previous.contains(&(r.event.id, r.event.phase)))
                {
                    let phase = phase_name(record.event.phase);
                    next.broadcasts.push(Broadcast {
                        id:format!("{}/meteor/{}/{phase}",next.namespace,record.event.id),source:"world-events".into(),at,
                        kind:format!("meteor.{phase}"),
                        payload:serde_json::json!({"simulation_time":next.now,"event":record.event}),
                        allowed_recipients:FACTIONS.iter().map(|f|(*f).into()).collect(),
                    });
                }
            }
        }
        self.commit(next)
    }

    /// Test the swept segment, not just its endpoint. A commander caught inside
    /// an impact may move outward; the event never teleports or traps it.
    pub fn segment_blocked(&self, from: [f64; 2], to: [f64; 2]) -> bool {
        if !from.iter().chain(to.iter()).all(|v| v.is_finite()) {
            return true;
        }
        self.saved
            .events
            .iter()
            .filter(|r| r.event.phase == Phase::Impact)
            .any(|r| {
                let e = &r.event;
                let center = [e.x, e.y];
                let radius = e.radius + 1.0;
                if distance(from, center) < radius
                    && distance(to, center) > distance(from, center)
                    && (from[0] - center[0]) * (to[0] - from[0])
                        + (from[1] - center[1]) * (to[1] - from[1])
                        >= 0.0
                {
                    return false;
                }
                segment_distance(from, to, center) <= radius
            })
    }

    fn commit(&mut self, next: Saved) -> Result<()> {
        self.persist(&next)?;
        self.saved = next;
        Ok(())
    }

    fn persist(&self, next: &Saved) -> Result<()> {
        let Some(path) = &self.path else {
            return Ok(());
        };
        let bytes = serde_json::to_vec(next)?;
        ensure!(bytes.len() as u64 <= MAX_BYTES, "event save exceeds limit");
        let mut nonce = [0; 8];
        getrandom::fill(&mut nonce).map_err(|e| anyhow::anyhow!("event save nonce: {e}"))?;
        let temp = parent(path).join(format!(".events-{:016x}.tmp", u64::from_le_bytes(nonce)));
        let result = (|| -> Result<()> {
            let mut options = OpenOptions::new();
            options.create_new(true).write(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            let mut file = options.open(&temp)?;
            file.write_all(&bytes)?;
            file.sync_all()?;
            fs::rename(&temp, path)?;
            File::open(parent(path))?.sync_all()?;
            Ok(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(temp);
        }
        result
    }
}

fn phase_name(phase: Phase) -> &'static str {
    match phase {
        Phase::Warning => "warning",
        Phase::Impact => "impact",
        Phase::Cooled => "cooled",
    }
}

fn parent(path: &Path) -> &Path {
    path.parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."))
}
fn distance(a: [f64; 2], b: [f64; 2]) -> f64 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}
fn segment_distance(a: [f64; 2], b: [f64; 2], p: [f64; 2]) -> f64 {
    let d = [b[0] - a[0], b[1] - a[1]];
    let length = d[0] * d[0] + d[1] * d[1];
    let t = if length == 0.0 {
        0.0
    } else {
        ((p[0] - a[0]) * d[0] + (p[1] - a[1]) * d[1]) / length
    }
    .clamp(0.0, 1.0);
    distance(p, [a[0] + d[0] * t, a[1] + d[1] * t])
}
fn random(saved: &mut Saved) -> u64 {
    saved.seed = saved.seed.wrapping_add(0x9e3779b97f4a7c15);
    let mut n = saved.seed;
    n = (n ^ (n >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    n = (n ^ (n >> 27)).wrapping_mul(0x94d049bb133111eb);
    n ^ (n >> 31)
}
fn schedule(
    saved: &mut Saved,
    x: f64,
    y: f64,
    radius: f64,
    delay: u64,
    broadcast: bool,
) -> Result<u64> {
    let id = saved.events.len() as u64 + 1;
    let impact_at = saved
        .now
        .checked_add(delay)
        .context("event clock exhausted")?;
    let clear_at = impact_at
        .checked_add(120)
        .context("event clock exhausted")?;
    let event = PublicEvent {
        id,
        kind: "meteor".into(),
        warning_at: saved.now,
        impact_at,
        clear_at,
        x,
        y,
        radius,
        phase: Phase::Warning,
    };
    if broadcast {
        for faction in FACTIONS {
            observe(saved, faction, &event, "public orbital warning");
        }
    }
    saved.events.push(EventRecord { event, broadcast });
    Ok(id)
}
fn observe(saved: &mut Saved, faction: &str, event: &PublicEvent, source: &str) {
    let Some(knowledge) = saved.knowledge.iter_mut().find(|k| k.faction == faction) else {
        return;
    };
    if knowledge
        .observations
        .iter()
        .any(|o| o.event_id == event.id && o.phase == event.phase)
    {
        return;
    }
    knowledge.observations.push(Observation {
        event_id: event.id,
        at: saved.now,
        source: source.into(),
        phase: event.phase,
    });
}
fn tick(saved: &mut Saved) -> Result<()> {
    saved.now = saved.now.checked_add(1).context("event clock exhausted")?;
    if saved.now >= saved.next_warning && saved.events.len() < MAX_EVENTS {
        let x = saved.center[0] - 60.0 + (random(saved) % 120) as f64;
        let y = saved.center[1] - 60.0 + (random(saved) % 120) as f64;
        let radius = 12.0 + (random(saved) % 12) as f64;
        schedule(saved, x, y, radius, 20, true)?;
        saved.next_warning = saved.now.saturating_add(180 + random(saved) % 90);
    }
    for i in 0..saved.events.len() {
        let e = &saved.events[i].event;
        let phase = if saved.now >= e.clear_at {
            Phase::Cooled
        } else if saved.now >= e.impact_at {
            Phase::Impact
        } else {
            Phase::Warning
        };
        if phase != e.phase {
            saved.events[i].event.phase = phase;
            let event = saved.events[i].event.clone();
            for faction in FACTIONS {
                // A private warning is not continuing remote sensor coverage.
                if saved.events[i].broadcast {
                    observe(saved, faction, &event, "public orbital bulletin");
                }
            }
        }
    }
    for (index, faction) in FACTIONS.iter().enumerate() {
        let known = saved.knowledge[index]
            .observations
            .iter()
            .rev()
            .find(|o| o.phase == Phase::Impact)
            .map(|o| o.event_id);
        if let Some(id) = known.filter(|id| {
            !saved
                .responses
                .iter()
                .any(|r| r.faction == *faction && r.event_id == *id)
        }) {
            let event = &saved
                .events
                .iter()
                .find(|r| r.event.id == id)
                .unwrap()
                .event;
            if event.phase == Phase::Impact {
                let home = saved.units[index].home;
                let len = distance(home, [event.x, event.y]).max(1.0);
                let radius = event.radius + 10.0;
                saved.responses.push(Response {
                    faction: (*faction).into(),
                    event_id: id,
                    started_at: saved.now,
                    completed_at: None,
                    goal: "Survey meteor perimeter (fixture aerial drone; no resource grant)"
                        .into(),
                    destination: [
                        event.x + (home[0] - event.x) / len * radius,
                        event.y + (home[1] - event.y) / len * radius,
                    ],
                    completed: false,
                });
            }
        }
        if let Some(response) = saved
            .responses
            .iter_mut()
            .rev()
            .find(|r| r.faction == *faction && !r.completed)
        {
            let unit = &mut saved.units[index];
            let len = distance([unit.x, unit.y], response.destination);
            let t = if len == 0.0 {
                1.0
            } else {
                (12.0 / len).min(1.0)
            };
            unit.x += (response.destination[0] - unit.x) * t;
            unit.y += (response.destination[1] - unit.y) * t;
            if t == 1.0 {
                response.completed = true;
                response.completed_at = Some(saved.now);
            }
        }
    }
    Ok(())
}

fn validate(saved: &Saved) -> Result<()> {
    ensure!(
        saved.schema_version == 1 && saved.events.len() <= MAX_EVENTS,
        "unsupported event save"
    );
    ensure!(
        saved.namespace.len() <= 64
            && saved
                .namespace
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-'),
        "invalid event namespace"
    );
    ensure!(
        saved.broadcasts.len() <= MAX_EVENTS * 3,
        "broadcast outbox exceeds limit"
    );
    let mut ids = std::collections::HashSet::new();
    let mut last_at = 0;
    for broadcast in &saved.broadcasts {
        let event: PublicEvent = serde_json::from_value(broadcast.payload["event"].clone())
            .context("invalid public broadcast payload")?;
        let known = saved
            .events
            .iter()
            .find(|r| r.event.id == event.id && r.broadcast)
            .context("broadcast references nonpublic event")?;
        let mut expected = known.event.clone();
        expected.phase = event.phase;
        ensure!(
            event == expected
                && broadcast.payload["simulation_time"]
                    .as_u64()
                    .is_some_and(|at| at <= saved.now),
            "broadcast facts disagree with event history"
        );
        ensure!(
            broadcast.id
                == format!(
                    "{}/meteor/{}/{}",
                    saved.namespace,
                    event.id,
                    phase_name(event.phase)
                )
                && ids.insert(&broadcast.id),
            "invalid or duplicate broadcast identity"
        );
        ensure!(
            broadcast.at > 0
                && broadcast.at >= last_at
                && broadcast.source == "world-events"
                && broadcast.kind == format!("meteor.{}", phase_name(event.phase))
                && broadcast.allowed_recipients == FACTIONS,
            "invalid broadcast provenance"
        );
        last_at = broadcast.at;
    }
    ensure!(
        saved
            .center
            .iter()
            .all(|v| v.is_finite() && v.abs() <= 1_000_000.0),
        "invalid event region"
    );
    ensure!(
        saved.units.len() == 3
            && saved.knowledge.len() == 3
            && saved.responses.len() <= MAX_EVENTS * 3,
        "invalid event faction records"
    );
    for (index, faction) in FACTIONS.iter().enumerate() {
        let unit = &saved.units[index];
        ensure!(
            unit.faction == *faction && saved.knowledge[index].faction == *faction,
            "event faction identity mismatch"
        );
        ensure!(
            [unit.x, unit.y, unit.home[0], unit.home[1]]
                .iter()
                .all(|v| v.is_finite() && v.abs() <= 1_000_000.0),
            "invalid survey position"
        );
        ensure!(
            saved.knowledge[index].observations.len() <= MAX_EVENTS * 3,
            "observation limit exceeded"
        );
        for o in &saved.knowledge[index].observations {
            ensure!(
                o.at <= saved.now && saved.events.iter().any(|r| r.event.id == o.event_id),
                "invalid observation"
            );
        }
    }
    for (index, r) in saved.events.iter().enumerate() {
        let e = &r.event;
        ensure!(
            e.id == index as u64 + 1
                && e.kind == "meteor"
                && e.warning_at <= saved.now
                && e.warning_at < e.impact_at
                && e.impact_at.checked_add(120) == Some(e.clear_at),
            "invalid meteor history"
        );
        ensure!(
            e.x.is_finite()
                && e.y.is_finite()
                && e.radius.is_finite()
                && (4.0..=64.0).contains(&e.radius),
            "invalid meteor geometry"
        );
        let phase = if saved.now >= e.clear_at {
            Phase::Cooled
        } else if saved.now >= e.impact_at {
            Phase::Impact
        } else {
            Phase::Warning
        };
        ensure!(e.phase == phase, "event phase disagrees with saved clock");
    }
    for r in &saved.responses {
        ensure!(
            FACTIONS.contains(&r.faction.as_str())
                && saved.events.iter().any(|e| e.event.id == r.event_id)
                && r.started_at <= saved.now
                && r.destination.iter().all(|v| v.is_finite()),
            "invalid survey response"
        );
        ensure!(
            r.completed == r.completed_at.is_some()
                && r.completed_at
                    .is_none_or(|at| at >= r.started_at && at <= saved.now),
            "invalid survey completion"
        );
    }
    Ok(())
}
