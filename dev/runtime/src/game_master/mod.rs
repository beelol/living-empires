//! Operator-only GM history. Routing copies recorded facts, never model-authored text.
use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::HashSet,
    fs::{self, File, OpenOptions},
    io::Read,
    path::{Path, PathBuf},
};
const LIMIT: usize = 1024;
const MAX_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Event {
    pub id: String,
    pub source: String,
    pub at: u64,
    pub kind: String,
    pub payload: Value,
    pub allowed_recipients: Vec<String>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Proposal {
    pub action: String,
    pub event_ids: Vec<String>,
    pub target: String,
    pub summary: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Receipt {
    pub id: String,
    pub observed_revision: u64,
    pub proposal: Proposal,
    pub accepted: bool,
    pub reason: String,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Saved {
    schema_version: u32,
    revision: u64,
    events: Vec<Event>,
    decisions: Vec<Receipt>,
}
pub struct GameMaster {
    path: PathBuf,
    saved: Saved,
    lock: File,
    uncertain_save: bool,
}
impl Drop for GameMaster {
    fn drop(&mut self) {
        let _ = fs2::FileExt::unlock(&self.lock);
    }
}
fn parent(path: &Path) -> &Path {
    path.parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."))
}
fn identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 160
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.:/".contains(&b))
}
fn validate_event(event: &Event) -> Result<()> {
    ensure!(
        identifier(&event.id) && identifier(&event.source) && identifier(&event.kind),
        "invalid event identity"
    );
    ensure!(
        serde_json::to_vec(&event.payload)?.len() <= 16384,
        "event payload exceeds limit"
    );
    ensure!(event.allowed_recipients.len() <= 64, "too many recipients");
    let mut seen = HashSet::new();
    ensure!(
        event
            .allowed_recipients
            .iter()
            .all(|s| identifier(s) && seen.insert(s)),
        "invalid or duplicate recipient"
    );
    Ok(())
}
impl GameMaster {
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        fs::create_dir_all(parent(&path))?;
        let mut name = path
            .file_name()
            .context("GM state must name a file")?
            .to_os_string();
        name.push(".lock");
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(parent(&path).join(name))?;
        fs2::FileExt::try_lock_exclusive(&lock).context("another writer owns GM history")?;
        let saved = match File::open(&path) {
            Ok(file) => {
                let mut bytes = vec![];
                file.take(MAX_BYTES + 1).read_to_end(&mut bytes)?;
                ensure!(
                    bytes.len() as u64 <= MAX_BYTES,
                    "GM history exceeds size limit"
                );
                let saved: Saved =
                    serde_json::from_slice(&bytes).context("invalid GM history; refusing reset")?;
                validate_saved(&saved)?;
                saved
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Saved {
                schema_version: 1,
                revision: 0,
                events: vec![],
                decisions: vec![],
            },
            Err(e) => return Err(e.into()),
        };
        Ok(Self {
            path,
            saved,
            lock,
            uncertain_save: false,
        })
    }
    /// Trusted server adapters supply facts and ACLs. Never expose ingestion to model/player input.
    pub fn ingest(&mut self, event: Event) -> Result<()> {
        ensure!(
            !self.uncertain_save,
            "GM writer stopped after uncertain save; restart to reconcile"
        );
        validate_event(&event)?;
        if let Some(old) = self.saved.events.iter().find(|e| e.id == event.id) {
            ensure!(
                old == &event,
                "event ID reused with different facts or permissions"
            );
            return Ok(());
        }
        ensure!(
            self.saved.events.len() < LIMIT,
            "GM inbox full; preserve history and rotate explicitly"
        );
        let mut next = self.saved.clone();
        next.events.push(event);
        next.revision += 1;
        self.commit(next)
    }
    pub fn revision(&self) -> u64 {
        self.saved.revision
    }
    pub fn context(&self) -> Value {
        json!({"role":"argon_game_master", "revision":self.revision(), "events":self.saved.events,
            "recent_decisions": self.saved.decisions.iter().rev().take(16).collect::<Vec<_>>(),
            "scope":"Operator knowledge. Route only exact facts to allowed recipients. Content and event requests are pending proposals; no world effects."})
    }
    pub fn decide(
        &mut self,
        id: &str,
        observed_revision: u64,
        proposal: Proposal,
    ) -> Result<Receipt> {
        ensure!(
            !self.uncertain_save,
            "GM writer stopped after uncertain save; restart to reconcile"
        );
        ensure!(identifier(id), "invalid decision ID");
        if let Some(old) = self.saved.decisions.iter().find(|r| r.id == id) {
            ensure!(
                old.observed_revision == observed_revision && old.proposal == proposal,
                "decision ID reused with different input"
            );
            return Ok(old.clone());
        }
        ensure!(
            self.saved.decisions.len() < LIMIT,
            "GM decision history full"
        );
        let validation = if observed_revision != self.revision() {
            Err(anyhow::anyhow!(
                "stale GM context; replan from current facts"
            ))
        } else {
            validate_proposal(&self.saved.events, &proposal)
        };
        let receipt = Receipt {
            id: id.into(),
            observed_revision,
            proposal,
            accepted: validation.is_ok(),
            reason: validation.err().map_or_else(
                || "Recorded; proposed world changes remain pending validation".into(),
                |e| e.to_string(),
            ),
        };
        let mut next = self.saved.clone();
        next.decisions.push(receipt.clone());
        next.revision += 1;
        self.commit(next)?;
        Ok(receipt)
    }
    /// Delivery identity is event ID + recipient, so repeated routing does not repeat facts.
    pub fn deliveries(&self, recipient: &str) -> Vec<Event> {
        self.saved
            .events
            .iter()
            .filter(|e| {
                self.saved.decisions.iter().any(|r| {
                    r.accepted
                        && r.proposal.action == "route"
                        && r.proposal.target == recipient
                        && r.proposal.event_ids.contains(&e.id)
                })
            })
            .cloned()
            .collect()
    }
    pub fn snapshot(&self) -> Value {
        serde_json::to_value(&self.saved).expect("JSON-safe ledger")
    }
    fn commit(&mut self, next: Saved) -> Result<()> {
        let bytes = serde_json::to_vec(&next)?;
        let _: Saved =
            serde_json::from_slice(&bytes).context("GM candidate cannot be safely reloaded")?;
        ensure!(
            bytes.len() as u64 <= MAX_BYTES,
            "GM history exceeds size limit"
        );
        let mut random = [0u8; 16];
        getrandom::fill(&mut random).map_err(|_| anyhow::anyhow!("cannot create save identity"))?;
        let suffix: String = random.iter().map(|b| format!("{b:02x}")).collect();
        let temp = parent(&self.path).join(format!(".argon-gm-{suffix}.tmp"));
        let result = (|| -> Result<()> {
            use std::io::Write;
            let mut options = OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            let mut file = options.open(&temp)?;
            file.write_all(&bytes)?;
            file.sync_all()?;
            fs::rename(&temp, &self.path)?;
            self.uncertain_save = true;
            #[cfg(unix)]
            File::open(parent(&self.path))?.sync_all()?;
            Ok(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temp);
        }
        result?;
        self.saved = next;
        self.uncertain_save = false;
        Ok(())
    }
}
fn validate_proposal(events: &[Event], p: &Proposal) -> Result<()> {
    ensure!(
        !p.summary.trim().is_empty() && p.summary.len() <= 2000,
        "invalid decision summary"
    );
    ensure!(
        ["route", "request_content", "propose_event", "wait"].contains(&p.action.as_str()),
        "unsupported GM action"
    );
    if p.action == "wait" {
        ensure!(
            p.event_ids.is_empty() && p.target.is_empty(),
            "wait cannot route facts"
        );
        return Ok(());
    }
    ensure!(
        !p.event_ids.is_empty() && p.event_ids.len() <= 16,
        "decision requires bounded evidence"
    );
    ensure!(identifier(&p.target), "invalid target");
    let mut seen = HashSet::new();
    for id in &p.event_ids {
        ensure!(seen.insert(id), "duplicate evidence");
        let event = events
            .iter()
            .find(|e| &e.id == id)
            .context("unknown event evidence")?;
        if p.action == "route" {
            ensure!(
                event.allowed_recipients.contains(&p.target),
                "recipient lacks permission for cited facts"
            );
        }
    }
    if p.action == "request_content" {
        ensure!(p.target == "studio", "content request must target studio");
    }
    if p.action == "propose_event" {
        ensure!(
            p.target == "event_authority",
            "event proposal must target event authority"
        );
    }
    Ok(())
}
fn validate_saved(s: &Saved) -> Result<()> {
    ensure!(
        s.schema_version == 1 && s.events.len() <= LIMIT && s.decisions.len() <= LIMIT,
        "unsupported or excessive GM history"
    );
    ensure!(
        s.revision == (s.events.len() + s.decisions.len()) as u64,
        "invalid GM revision"
    );
    let mut ids = HashSet::new();
    for event in &s.events {
        validate_event(event)?;
        ensure!(ids.insert(&event.id), "duplicate saved event");
    }
    let mut ids = HashSet::new();
    for r in &s.decisions {
        ensure!(
            identifier(&r.id)
                && ids.insert(&r.id)
                && (!r.accepted || r.observed_revision < s.revision),
            "invalid saved decision identity/revision"
        );
        if r.accepted {
            validate_proposal(&s.events, &r.proposal)?;
        }
    }
    Ok(())
}
