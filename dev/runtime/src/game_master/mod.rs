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
struct Attempt {
    id: String,
    model: String,
    at: u64,
    context: Value,
    finished: bool,
    #[serde(default)]
    faction_turn: Option<u64>,
    error: Option<String>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Saved {
    schema_version: u32,
    revision: u64,
    events: Vec<Event>,
    decisions: Vec<Receipt>,
    #[serde(default)]
    attempts: Vec<Attempt>,
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
                attempts: vec![],
            },
            Err(e) => return Err(e.into()),
        };
        let mut gm = Self {
            path,
            saved,
            lock,
            uncertain_save: false,
        };
        if gm.saved.attempts.iter().any(|a| !a.finished) {
            let mut next = gm.saved.clone();
            for a in &mut next.attempts {
                if !a.finished {
                    a.finished = true;
                    a.error = Some("Request interrupted by restart; no proposal applied".into());
                }
            }
            gm.commit(next)?;
        }
        Ok(gm)
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
    /// Persist spending evidence before calling the provider. Only one request may be pending.
    pub fn begin_attempt(&mut self, id: &str, model: &str, at: u64) -> Result<Value> {
        self.begin_for_turn(id, model, at, None)
    }
    pub fn begin_scheduled_attempt(
        &mut self,
        id: &str,
        model: &str,
        at: u64,
        faction_turn: u64,
    ) -> Result<Value> {
        self.begin_for_turn(id, model, at, Some(faction_turn))
    }
    pub fn last_faction_turn(&self) -> Option<u64> {
        self.saved.attempts.last().and_then(|a| a.faction_turn)
    }
    fn begin_for_turn(
        &mut self,
        id: &str,
        model: &str,
        at: u64,
        faction_turn: Option<u64>,
    ) -> Result<Value> {
        ensure!(
            !self.uncertain_save,
            "GM writer stopped after uncertain save; restart to reconcile"
        );
        ensure!(
            identifier(id) && identifier(model),
            "invalid model request identity"
        );
        ensure!(
            self.saved.attempts.len() < LIMIT && self.saved.decisions.len() < LIMIT,
            "GM request budget capacity exhausted"
        );
        ensure!(
            !self
                .saved
                .attempts
                .iter()
                .any(|a| a.id == id || !a.finished)
                && !self.saved.decisions.iter().any(|r| r.id == id),
            "duplicate or concurrent GM request"
        );
        let context = self.context();
        let mut next = self.saved.clone();
        next.attempts.push(Attempt {
            id: id.into(),
            model: model.into(),
            at,
            context: context.clone(),
            finished: false,
            faction_turn,
            error: None,
        });
        self.commit(next)?;
        Ok(context)
    }
    pub fn attempt_count(&self) -> u64 {
        self.saved.attempts.len() as u64
    }
    /// Save the response and accepted/pending result in the same transaction.
    pub fn complete_attempt(
        &mut self,
        id: &str,
        outcome: std::result::Result<Proposal, &str>,
    ) -> Result<()> {
        ensure!(
            !self.uncertain_save,
            "GM writer stopped after uncertain save; restart to reconcile"
        );
        let index = self
            .saved
            .attempts
            .iter()
            .position(|a| a.id == id)
            .context("unknown GM request")?;
        ensure!(
            !self.saved.attempts[index].finished,
            "GM request already finished; late result rejected"
        );
        let mut next = self.saved.clone();
        match outcome {
            Ok(proposal) => {
                ensure!(
                    next.decisions.len() < LIMIT && !next.decisions.iter().any(|r| r.id == id),
                    "GM receipt capacity/identity conflict"
                );
                let observed_revision = next.attempts[index].context["revision"]
                    .as_u64()
                    .context("invalid request revision")?;
                let validation = if observed_revision == next.revision {
                    validate_new_proposal(&next, &proposal)
                } else {
                    Err(anyhow::anyhow!(
                        "stale GM context; replan from current facts"
                    ))
                };
                next.decisions.push(Receipt {
                    id: id.into(),
                    observed_revision,
                    proposal,
                    accepted: validation.is_ok(),
                    reason: validation.err().map_or_else(
                        || "Recorded; proposed world changes remain pending validation".into(),
                        |e| e.to_string(),
                    ),
                });
                next.revision += 1;
            }
            Err(error) => {
                ensure!(error.len() <= 2000, "provider error exceeds evidence limit");
                next.attempts[index].error = Some(error.into());
            }
        }
        next.attempts[index].finished = true;
        self.commit(next)
    }
    /// Read a trusted local Studio export. A malformed/conflicting batch changes nothing.
    pub fn import_file(&mut self, path: &Path) -> Result<()> {
        ensure!(
            !self.uncertain_save,
            "GM writer stopped after uncertain save; restart to reconcile"
        );
        let mut bytes = Vec::new();
        File::open(path)?.take(1_048_577).read_to_end(&mut bytes)?;
        ensure!(bytes.len() <= 1_048_576, "GM intake file exceeds one MiB");
        let events: Vec<Event> =
            serde_json::from_slice(&bytes).context("invalid GM intake event array")?;
        ensure!(events.len() <= LIMIT, "GM intake batch exceeds limit");
        self.ingest_batch(events)
    }
    fn ingest_batch(&mut self, events: Vec<Event>) -> Result<()> {
        ensure!(
            !self.uncertain_save,
            "GM writer stopped after uncertain save; restart to reconcile"
        );
        let mut next = self.saved.clone();
        for event in events {
            validate_event(&event)?;
            if let Some(old) = next.events.iter().find(|e| e.id == event.id) {
                ensure!(
                    old == &event,
                    "event ID reused with different facts or permissions"
                );
            } else {
                ensure!(next.events.len() < LIMIT, "GM inbox full");
                next.events.push(event);
                next.revision += 1;
            }
        }
        if next.revision != self.saved.revision {
            self.commit(next)?;
        }
        Ok(())
    }
    /// Consume only committed delivered history; omit full operator contexts and summaries.
    pub fn observe_diplomacy(&mut self, snapshot: &Value) -> Result<()> {
        #[derive(Serialize, Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Delivered {
            actor: String,
            target: String,
            action: String,
            message: String,
            offer_id: String,
            at: u64,
        }
        ensure!(
            snapshot["schema_version"] == 1,
            "unsupported diplomacy event snapshot"
        );
        let history = snapshot["history"]
            .as_array()
            .context("missing diplomacy event history")?;
        ensure!(
            history.len() <= LIMIT,
            "diplomacy event history exceeds limit"
        );
        let mut events = Vec::new();
        for (index, value) in history.iter().enumerate() {
            let e: Delivered = serde_json::from_value(value.clone())
                .context("invalid delivered diplomacy event")?;
            let mut recipients = vec![e.actor.clone()];
            if !e.target.is_empty() && e.target != e.actor {
                recipients.push(e.target.clone());
            }
            events.push(Event {
                id: format!("diplomacy-history-{}", index + 1),
                source: "diplomacy".into(),
                at: e.at,
                kind: format!("diplomacy.{}", e.action),
                payload: serde_json::to_value(e)?,
                allowed_recipients: recipients,
            });
        }
        self.ingest_batch(events)
    }
    pub fn revision(&self) -> u64 {
        self.saved.revision
    }
    pub fn context(&self) -> Value {
        let mut value = json!({"role":"argon_game_master", "revision":self.revision(), "events":self.saved.events.iter().rev().take(64).collect::<Vec<_>>(),
            "recent_decisions": self.saved.decisions.iter().rev().take(16).collect::<Vec<_>>(),
            "scope":"Operator knowledge. Route only exact facts to allowed recipients. Content and event requests are pending proposals; no world effects."});
        while value.to_string().len() > 48000 {
            if value["events"].as_array().is_some_and(|v| v.len() > 1) {
                value["events"].as_array_mut().unwrap().pop();
            } else if value["recent_decisions"]
                .as_array()
                .is_some_and(|v| !v.is_empty())
            {
                value["recent_decisions"].as_array_mut().unwrap().pop();
            } else {
                break;
            }
        }
        value
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
            validate_new_proposal(&self.saved, &proposal)
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
fn validate_new_proposal(saved: &Saved, proposal: &Proposal) -> Result<()> {
    validate_proposal(&saved.events, proposal)?;
    if proposal.action == "request_content" {
        ensure!(
            !saved.decisions.iter().any(|r| r.accepted
                && r.proposal.action == "request_content"
                && r.proposal.target == proposal.target
                && r.proposal.event_ids.len() == proposal.event_ids.len()
                && r.proposal
                    .event_ids
                    .iter()
                    .all(|id| proposal.event_ids.contains(id))),
            "content work for these recorded needs was already requested"
        );
    }
    Ok(())
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
        s.schema_version == 1
            && s.events.len() <= LIMIT
            && s.decisions.len() <= LIMIT
            && s.attempts.len() <= LIMIT,
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
    let mut attempt_ids = HashSet::new();
    let mut pending = 0;
    for a in &s.attempts {
        ensure!(
            identifier(&a.id) && identifier(&a.model) && attempt_ids.insert(&a.id),
            "invalid saved GM request"
        );
        ensure!(
            a.context["role"] == "argon_game_master"
                && a.context["revision"]
                    .as_u64()
                    .is_some_and(|r| r <= s.revision)
                && a.context.to_string().len() <= 48000
                && a.error.as_ref().is_none_or(|e| e.len() <= 2000),
            "invalid saved GM input"
        );
        if !a.finished {
            pending += 1;
        }
    }
    ensure!(pending <= 1, "multiple pending GM requests");
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
