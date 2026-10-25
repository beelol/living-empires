//! Development-only diplomacy ledger; no inventory, combat, or territory effects.
use anyhow::{bail, ensure, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, HashSet},
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};

pub const FACTION_IDS: [&str; 3] = ["helios", "tide", "cinder"];
pub(crate) const LIMIT: usize = 1024;
const MAX_SAVE: u64 = 64 * 1024 * 1024;
const MAX_CONTEXT: usize = 32768;

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Faction {
    id: String,
    name: String,
    leader: String,
    priorities: Vec<String>,
    stances: BTreeMap<String, String>,
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Offer {
    id: String,
    version: u64,
    proposer: String,
    recipient: String,
    kind: String,
    duration_seconds: u64,
    created_at: u64,
    expires_at: u64,
    status: String,
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Pact {
    offer_id: String,
    parties: [String; 2],
    kind: String,
    starts_at: u64,
    ends_at: u64,
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Event {
    actor: String,
    target: String,
    action: String,
    message: String,
    offer_id: String,
    at: u64,
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Receipt {
    actor: String,
    request_id: String,
    fingerprint: String,
    observed_revision: u64,
    proposal: Value,
    accepted: bool,
    result: Value,
    at: u64,
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Attempt {
    actor: String,
    request_id: String,
    context: Value,
    model: String,
    at: u64,
    error: Option<String>,
    finished_at: Option<u64>,
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ReportDelivery {
    actor: String,
    received_at: u64,
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ReceivedReport {
    event: crate::game_master::Event,
    deliveries: Vec<ReportDelivery>,
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Saved {
    schema_version: u32,
    revision: u64,
    factions: Vec<Faction>,
    offers: Vec<Offer>,
    pacts: Vec<Pact>,
    history: Vec<Event>,
    decisions: Vec<Receipt>,
    attempts: Vec<Attempt>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    reports: Vec<ReceivedReport>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Proposal {
    action: String,
    target: String,
    message: String,
    summary: String,
    offer_id: String,
    offer_version: u64,
    stance: String,
}

pub struct Diplomacy {
    path: PathBuf,
    saved: Saved,
    lock: File,
}
impl Drop for Diplomacy {
    fn drop(&mut self) {
        let _ = fs2::FileExt::unlock(&self.lock);
    }
}
impl Diplomacy {
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        fs::create_dir_all(parent(&path))?;
        let mut lock_name = path
            .file_name()
            .context("diplomacy state must name a file")?
            .to_os_string();
        lock_name.push(".lock");
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(parent(&path).join(lock_name))?;
        fs2::FileExt::try_lock_exclusive(&lock)
            .context("another process owns this diplomacy ledger")?;
        let (saved, fresh) = match File::open(&path) {
            Ok(file) => {
                let mut bytes = Vec::new();
                file.take(MAX_SAVE + 1).read_to_end(&mut bytes)?;
                ensure!(
                    bytes.len() as u64 <= MAX_SAVE,
                    "diplomacy save exceeds size limit"
                );
                let saved: Saved = serde_json::from_slice(&bytes)
                    .context("invalid diplomacy save; refusing to reset it")?;
                validate_saved(&saved)?;
                (saved, false)
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => (fixtures(), true),
            Err(error) => return Err(error).context("cannot read diplomacy save"),
        };
        let mut core = Self { path, saved, lock };
        if fresh {
            core.commit(core.saved.clone())?;
        }
        Ok(core)
    }
    pub fn revision(&self) -> u64 {
        self.saved.revision
    }
    pub fn attempts(&self) -> u64 {
        self.saved.attempts.len() as u64
    }
    /// Trusted authority only. Eligibility is not delivery; this records one actor
    /// learning exact source facts. Replays keep the original receipt time.
    pub fn ingest_report(
        &mut self,
        actor: &str,
        event: &crate::game_master::Event,
        received_at: u64,
    ) -> Result<()> {
        validate_report(event)?;
        ensure!(
            FACTION_IDS.contains(&actor) && event.allowed_recipients.iter().any(|id| id == actor),
            "report is not permitted for this faction"
        );
        if let Some(old) = self.saved.reports.iter().find(|r| r.event.id == event.id) {
            ensure!(
                old.event == *event,
                "report ID reused with different facts or permissions"
            );
            if old.deliveries.iter().any(|d| d.actor == actor) {
                return Ok(());
            }
        }
        ensure!(
            received_at >= event.at,
            "report cannot be delivered before its source time"
        );
        ensure!(
            self.saved.decisions.iter().all(|r| received_at >= r.at)
                && self
                    .saved
                    .reports
                    .iter()
                    .flat_map(|r| &r.deliveries)
                    .all(|d| received_at >= d.received_at),
            "clock moved backwards; retry with current time"
        );
        ensure!(
            delivery_count(&self.saved) < LIMIT,
            "report delivery limit reached"
        );
        let mut next = self.saved.clone();
        let delivery = ReportDelivery {
            actor: actor.into(),
            received_at,
        };
        if let Some(old) = next.reports.iter_mut().find(|r| r.event.id == event.id) {
            old.deliveries.push(delivery);
        } else {
            next.reports.push(ReceivedReport {
                event: event.clone(),
                deliveries: vec![delivery],
            });
        }
        next.revision = next
            .revision
            .checked_add(1)
            .context("revision limit reached")?;
        self.commit(next)
    }
    pub fn context(&self, actor: &str, now: u64) -> Result<Value> {
        let own = self
            .saved
            .factions
            .iter()
            .find(|f| f.id == actor)
            .context("unknown faction")?;
        let contacts: Vec<_> = self
            .saved
            .factions
            .iter()
            .filter(|f| f.id != actor)
            .map(|f| json!({"id":f.id,"name":f.name,"leader":f.leader,"attitude":f.stances[actor]}))
            .collect();
        let history: Vec<_> = self
            .saved
            .history
            .iter()
            .filter(|e| e.actor == actor || e.target == actor)
            .rev()
            .take(16)
            .cloned()
            .collect();
        let offers: Vec<_> = self
            .saved
            .offers
            .iter()
            .filter(|o| pending(o, now) && (o.proposer == actor || o.recipient == actor))
            .collect();
        let pacts: Vec<_> = self
            .saved
            .pacts
            .iter()
            .filter(|p| active(p, now) && p.parties.iter().any(|party| party == actor))
            .collect();
        let mut delivered: Vec<_> = self
            .saved
            .reports
            .iter()
            .filter_map(|r| {
                r.deliveries
                    .iter()
                    .find(|d| d.actor == actor)
                    .map(|d| (&r.event, d.received_at))
            })
            .collect();
        delivered.sort_by_key(|(_, at)| *at);
        let reports: Vec<_> = delivered
            .into_iter()
            .rev()
            .take(16)
            .map(|(event, received_at)| {
                json!({"id":event.id,"source":event.source,"at":event.at,"kind":event.kind,
                "payload":event.payload,"received_at":received_at})
            })
            .collect();
        let mut context = json!({"revision":self.revision(),"now":now,"self":own,"known_contacts":contacts,
            "history":history,"reports":reports,"pending_offers":offers,"active_pacts":pacts,
            "scope":"Development diplomacy ledger only; no resources, land, armies or combat effects."});
        // History is newest-first. Keep whole messages and all current terms;
        // only the prompt is shortened, never the authoritative saved history.
        while serde_json::to_vec(&context)?.len() > MAX_CONTEXT {
            if context["history"].as_array_mut().unwrap().pop().is_some() {
                continue;
            }
            ensure!(
                context["reports"].as_array_mut().unwrap().pop().is_some(),
                "current faction facts exceed context size limit"
            );
        }
        Ok(context)
    }
    /// Operator-only: contains private model contexts and internal summaries.
    pub fn snapshot(&self, now: u64) -> Value {
        let offers: Vec<_> = self
            .saved
            .offers
            .iter()
            .filter(|o| pending(o, now))
            .collect();
        let pacts: Vec<_> = self.saved.pacts.iter().filter(|p| active(p, now)).collect();
        json!({"schema_version":1,"revision":self.revision(),"now":now,"attempt_count":self.attempts(),
            "factions":self.saved.factions,"pending_offers":offers,"active_pacts":pacts,
            "offer_history":self.saved.offers,"pact_history":self.saved.pacts,
            "history":self.saved.history,"decisions":self.saved.decisions,"attempts":self.saved.attempts,
            "report_count":self.saved.reports.len(),"delivery_count":delivery_count(&self.saved),
            "reports":self.saved.reports.iter().rev().take(24).collect::<Vec<_>>()})
    }
    pub fn apply(
        &mut self,
        actor: &str,
        observed_revision: u64,
        request_id: &str,
        proposal: Value,
        now: u64,
    ) -> Result<Value> {
        validate_actor_request(actor, request_id)?;
        ensure!(
            serde_json::to_vec(&proposal)?.len() <= 16384,
            "proposal exceeds size limit"
        );
        let fingerprint = fingerprint(observed_revision, &proposal)?;
        if let Some(receipt) = self
            .saved
            .decisions
            .iter()
            .find(|r| r.actor == actor && r.request_id == request_id)
        {
            ensure!(
                receipt.fingerprint == fingerprint,
                "request ID already used with different input"
            );
            return receipt_result(receipt);
        }
        ensure!(
            self.saved.decisions.len() < LIMIT,
            "diplomacy ledger limit reached; retain this ledger and start a new demo"
        );
        let mut next = self.saved.clone();
        let outcome = (|| -> Result<Value> {
            ensure!(
                self.saved.decisions.iter().all(|r| now >= r.at)
                    && self
                        .saved
                        .reports
                        .iter()
                        .flat_map(|r| &r.deliveries)
                        .all(|d| now >= d.received_at),
                "clock moved backwards; retry with current time"
            );
            ensure!(
                observed_revision == next.revision,
                "world changed since this decision; request fresh context"
            );
            if let Some(attempt) = next
                .attempts
                .iter()
                .find(|a| a.actor == actor && a.request_id == request_id)
            {
                ensure!(
                    attempt.error.is_none(),
                    "attempt already failed; late response rejected"
                );
            }
            let parsed: Proposal =
                serde_json::from_value(proposal.clone()).context("invalid proposal fields")?;
            validate_proposal(actor, &parsed)?;
            enact(&mut next, actor, &parsed, now)?;
            next.revision = next
                .revision
                .checked_add(1)
                .context("revision limit reached")?;
            Ok(
                json!({"accepted":true,"action":parsed.action,"actor":actor,"target":parsed.target,"revision":next.revision,"request_id":request_id}),
            )
        })();
        let (accepted, result) = match outcome {
            Ok(value) => (true, value),
            Err(error) => {
                // Reject the entire proposal, including any partial candidate edits.
                next = self.saved.clone();
                (
                    false,
                    json!({"accepted":false,"reason":format!("{error:#}"),"revision":self.revision(),"request_id":request_id}),
                )
            }
        };
        if let Some(attempt) = next
            .attempts
            .iter_mut()
            .find(|a| a.actor == actor && a.request_id == request_id)
        {
            attempt.finished_at = Some(now);
        }
        let receipt = Receipt {
            actor: actor.into(),
            request_id: request_id.into(),
            fingerprint,
            observed_revision,
            proposal,
            accepted,
            result,
            at: now,
        };
        next.decisions.push(receipt.clone());
        self.commit(next)?;
        receipt_result(&receipt)
    }
    /// Call before spending on the provider. Every provider call requires a new ID.
    pub fn record_attempt(
        &mut self,
        actor: &str,
        request_id: &str,
        context: Value,
        model: &str,
        now: u64,
    ) -> Result<()> {
        validate_actor_request(actor, request_id)?;
        ensure!(
            !model.trim().is_empty() && model.len() <= 128,
            "invalid model name"
        );
        ensure!(
            context == self.context(actor, now)?,
            "attempt must use the current faction context"
        );
        ensure!(
            serde_json::to_vec(&context)?.len() <= MAX_CONTEXT,
            "context exceeds size limit"
        );
        ensure!(
            self.saved.attempts.len() < LIMIT && self.saved.decisions.len() < LIMIT,
            "diplomacy attempt or decision limit reached"
        );
        ensure!(
            !self
                .saved
                .attempts
                .iter()
                .any(|a| a.actor == actor && a.request_id == request_id)
                && !self
                    .saved
                    .decisions
                    .iter()
                    .any(|a| a.actor == actor && a.request_id == request_id),
            "request already attempted"
        );
        let mut next = self.saved.clone();
        next.attempts.push(Attempt {
            actor: actor.into(),
            request_id: request_id.into(),
            context,
            model: model.into(),
            at: now,
            error: None,
            finished_at: None,
        });
        self.commit(next)
    }
    pub fn record_failure(
        &mut self,
        actor: &str,
        request_id: &str,
        context: Value,
        error: &str,
        now: u64,
    ) -> Result<()> {
        validate_actor_request(actor, request_id)?;
        ensure!(
            !error.is_empty() && error.len() <= 2048,
            "invalid service error"
        );
        let mut next = self.saved.clone();
        let attempt = next
            .attempts
            .iter_mut()
            .find(|a| a.actor == actor && a.request_id == request_id)
            .context("attempt not recorded")?;
        ensure!(
            attempt.context == context,
            "failure context differs from recorded attempt"
        );
        if let Some(existing) = &attempt.error {
            ensure!(
                existing == error,
                "attempt already failed with different evidence"
            );
            return Ok(());
        }
        ensure!(attempt.finished_at.is_none(), "attempt already completed");
        attempt.error = Some(error.into());
        attempt.finished_at = Some(now);
        self.commit(next)
    }
    fn commit(&mut self, next: Saved) -> Result<()> {
        let bytes = serde_json::to_vec(&next)?;
        let _: Saved = serde_json::from_slice(&bytes)
            .context("diplomacy candidate cannot be safely reloaded")?;
        ensure!(
            bytes.len() as u64 <= MAX_SAVE,
            "diplomacy save exceeds size limit"
        );
        let mut random = [0u8; 16];
        getrandom::fill(&mut random)
            .map_err(|e| anyhow::anyhow!("cannot create save name: {e}"))?;
        let name: String = random.iter().map(|b| format!("{b:02x}")).collect();
        let temp = parent(&self.path).join(format!(".diplomacy-{name}.tmp"));
        let result = (|| -> Result<()> {
            let mut options = OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            let mut file = options
                .open(&temp)
                .context("cannot create diplomacy save")?;
            file.write_all(&bytes)?;
            file.sync_all()?;
            fs::rename(&temp, &self.path).context("cannot replace diplomacy save")?;
            #[cfg(unix)]
            File::open(parent(&self.path))?.sync_all()?;
            Ok(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temp);
        }
        result?;
        self.saved = next;
        Ok(())
    }
}
fn parent(path: &Path) -> &Path {
    path.parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."))
}
fn pending(offer: &Offer, now: u64) -> bool {
    offer.status == "pending" && offer.created_at <= now && now < offer.expires_at
}
fn active(pact: &Pact, now: u64) -> bool {
    pact.starts_at <= now && now < pact.ends_at
}
fn pair(a: &str, b: &str, c: &str, d: &str) -> bool {
    (a == c && b == d) || (a == d && b == c)
}
fn validate_actor_request(actor: &str, request: &str) -> Result<()> {
    ensure!(FACTION_IDS.contains(&actor), "unknown faction");
    ensure!(
        !request.is_empty()
            && request.len() <= 128
            && request
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-_:.".contains(&b)),
        "invalid request ID"
    );
    Ok(())
}
fn validate_proposal(actor: &str, p: &Proposal) -> Result<()> {
    ensure!(
        matches!(
            p.action.as_str(),
            "message" | "set_stance" | "propose_pact" | "accept" | "decline" | "wait"
        ),
        "unsupported action"
    );
    ensure!(
        p.message.chars().count() <= 500
            && !p.summary.trim().is_empty()
            && p.summary.chars().count() <= 500,
        "message or summary exceeds bounds"
    );
    ensure!(
        matches!(p.stance.as_str(), "neutral" | "friendly" | "wary"),
        "invalid stance"
    );
    ensure!(
        p.action == "set_stance" || p.stance == "neutral",
        "stance only applies to set_stance"
    );
    if p.action == "wait" {
        ensure!(p.target.is_empty(), "wait cannot target another faction");
    } else {
        ensure!(
            FACTION_IDS.contains(&p.target.as_str()) && p.target != actor,
            "target must be another known faction"
        );
    }
    if matches!(p.action.as_str(), "accept" | "decline") {
        ensure!(
            !p.offer_id.is_empty() && p.offer_id.len() <= 128 && p.offer_version == 1,
            "exact offer ID and version 1 required"
        );
    } else {
        ensure!(
            p.offer_id.is_empty() && p.offer_version == 0,
            "offer reference only applies to accept or decline"
        );
    }
    if matches!(p.action.as_str(), "message" | "propose_pact") {
        ensure!(!p.message.trim().is_empty(), "message is required");
    }
    Ok(())
}
fn enact(next: &mut Saved, actor: &str, p: &Proposal, now: u64) -> Result<()> {
    let mut offer_id = p.offer_id.clone();
    match p.action.as_str() {
        "set_stance" => {
            next.factions
                .iter_mut()
                .find(|f| f.id == actor)
                .unwrap()
                .stances
                .insert(p.target.clone(), p.stance.clone());
        }
        "propose_pact" => {
            ensure!(
                !next
                    .offers
                    .iter()
                    .any(|o| pending(o, now) && pair(actor, &p.target, &o.proposer, &o.recipient)),
                "a pending offer already exists between these factions"
            );
            ensure!(
                !next.pacts.iter().any(|pact| active(pact, now)
                    && pair(actor, &p.target, &pact.parties[0], &pact.parties[1])),
                "a non-aggression pact is already active"
            );
            offer_id = format!("offer-{}", next.revision + 1);
            next.offers.push(Offer {
                id: offer_id.clone(),
                version: 1,
                proposer: actor.into(),
                recipient: p.target.clone(),
                kind: "non_aggression".into(),
                duration_seconds: 300,
                created_at: now,
                expires_at: now.checked_add(120).context("clock overflow")?,
                status: "pending".into(),
            });
        }
        "accept" | "decline" => {
            let offer = next
                .offers
                .iter_mut()
                .find(|o| o.id == p.offer_id)
                .context("offer does not exist")?;
            ensure!(
                offer.recipient == actor && offer.proposer == p.target,
                "only the named recipient may answer this offer"
            );
            ensure!(
                offer.version == p.offer_version && pending(offer, now),
                "offer is expired or no longer pending"
            );
            if p.action == "accept" {
                let ends_at = now.checked_add(300).context("clock overflow")?;
                offer.status = "accepted".into();
                next.pacts.push(Pact {
                    offer_id: offer.id.clone(),
                    parties: [offer.proposer.clone(), offer.recipient.clone()],
                    kind: "non_aggression".into(),
                    starts_at: now,
                    ends_at,
                });
            } else {
                offer.status = "declined".into();
            }
        }
        _ => {}
    }
    if p.action != "wait" {
        next.history.push(Event {
            actor: actor.into(),
            target: p.target.clone(),
            action: p.action.clone(),
            message: p.message.clone(),
            offer_id,
            at: now,
        });
    }
    Ok(())
}
fn fingerprint(revision: u64, proposal: &Value) -> Result<String> {
    Ok(format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&json!([revision, proposal]))?)
    ))
}
fn receipt_result(receipt: &Receipt) -> Result<Value> {
    if receipt.accepted {
        Ok(receipt.result.clone())
    } else {
        bail!(
            "{}",
            receipt.result["reason"]
                .as_str()
                .unwrap_or("proposal rejected")
        )
    }
}
fn fixtures() -> Saved {
    let descriptions = [
        (
            "Helios Assembly",
            "Envoy Sol",
            "Build stable peaceful relationships",
        ),
        (
            "Tide Collective",
            "Envoy Current",
            "Seek reciprocal trust and patient dialogue",
        ),
        (
            "Cinder Compact",
            "Envoy Ember",
            "Protect independence and avoid hasty promises",
        ),
    ];
    let factions = FACTION_IDS
        .iter()
        .zip(descriptions)
        .map(|(id, (name, leader, priority))| Faction {
            id: (*id).into(),
            name: name.into(),
            leader: leader.into(),
            priorities: vec![priority.into()],
            stances: FACTION_IDS
                .iter()
                .filter(|other| *other != id)
                .map(|other| ((*other).into(), "neutral".into()))
                .collect(),
        })
        .collect();
    Saved {
        schema_version: 1,
        revision: 0,
        factions,
        offers: vec![],
        pacts: vec![],
        history: vec![],
        decisions: vec![],
        attempts: vec![],
        reports: vec![],
    }
}
fn delivery_count(s: &Saved) -> usize {
    s.reports.iter().map(|r| r.deliveries.len()).sum()
}
fn validate_report(event: &crate::game_master::Event) -> Result<()> {
    let identifier = |value: &str| {
        !value.is_empty()
            && value.len() <= 160
            && value
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-_.:/".contains(&b))
    };
    ensure!(
        identifier(&event.id) && identifier(&event.source) && identifier(&event.kind),
        "invalid report identity"
    );
    ensure!(
        serde_json::to_vec(&event.payload)?.len() <= 16384,
        "report payload exceeds size limit"
    );
    // Reports are nested again inside persisted model contexts. Leave enough
    // depth for those wrappers, not only for the report's own saved envelope.
    let mut pending = vec![(&event.payload, 0)];
    while let Some((value, depth)) = pending.pop() {
        ensure!(depth <= 64, "report payload nesting exceeds limit");
        match value {
            Value::Array(values) => pending.extend(values.iter().map(|v| (v, depth + 1))),
            Value::Object(values) => pending.extend(values.values().map(|v| (v, depth + 1))),
            _ => {}
        }
    }
    let mut recipients = HashSet::new();
    ensure!(
        event.allowed_recipients.len() <= FACTION_IDS.len()
            && event
                .allowed_recipients
                .iter()
                .all(|id| FACTION_IDS.contains(&id.as_str()) && recipients.insert(id)),
        "invalid report recipients"
    );
    Ok(())
}
fn validate_saved(s: &Saved) -> Result<()> {
    ensure!(
        s.schema_version == 1 && s.factions.len() == 3,
        "unsupported or invalid diplomacy save"
    );
    ensure!(
        s.decisions.len() <= LIMIT
            && s.attempts.len() <= LIMIT
            && s.offers.len() <= LIMIT
            && s.pacts.len() <= LIMIT
            && s.history.len() <= LIMIT
            && s.reports.len() <= LIMIT
            && delivery_count(s) <= LIMIT,
        "saved ledger exceeds limits"
    );
    ensure!(
        s.revision
            == (s.decisions.iter().filter(|r| r.accepted).count() + delivery_count(s)) as u64,
        "saved revision does not match receipts"
    );
    let mut report_ids = HashSet::new();
    for report in &s.reports {
        validate_report(&report.event)?;
        ensure!(
            report_ids.insert(&report.event.id),
            "duplicate saved report"
        );
        ensure!(!report.deliveries.is_empty(), "report has no delivery");
        let mut recipients = HashSet::new();
        for delivery in &report.deliveries {
            ensure!(
                report.event.allowed_recipients.contains(&delivery.actor)
                    && recipients.insert(&delivery.actor)
                    && delivery.received_at >= report.event.at,
                "invalid saved report delivery"
            );
        }
    }
    let mut ids = HashSet::new();
    for f in &s.factions {
        ensure!(
            FACTION_IDS.contains(&f.id.as_str()) && ids.insert(&f.id),
            "invalid or duplicate saved faction"
        );
        ensure!(
            !f.name.is_empty()
                && f.name.len() <= 80
                && !f.leader.is_empty()
                && f.leader.len() <= 80
                && !f.priorities.is_empty()
                && f.priorities.len() <= 8
                && f.priorities.iter().all(|p| !p.is_empty() && p.len() <= 500),
            "invalid faction identity"
        );
        ensure!(
            f.stances.len() == 2
                && f.stances
                    .iter()
                    .all(|(id, stance)| FACTION_IDS.contains(&id.as_str())
                        && id != &f.id
                        && matches!(stance.as_str(), "neutral" | "friendly" | "wary")),
            "invalid saved stances"
        );
    }
    let mut requests = HashSet::new();
    for r in &s.decisions {
        validate_actor_request(&r.actor, &r.request_id)?;
        ensure!(
            requests.insert((&r.actor, &r.request_id))
                && r.fingerprint == fingerprint(r.observed_revision, &r.proposal)?
                && r.result["accepted"] == r.accepted,
            "invalid saved receipt"
        );
    }
    requests.clear();
    for a in &s.attempts {
        validate_actor_request(&a.actor, &a.request_id)?;
        ensure!(
            requests.insert((&a.actor, &a.request_id))
                && a.context["self"]["id"] == a.actor
                && !a.model.is_empty()
                && a.model.len() <= 128
                && a.error.as_ref().is_none_or(|e| e.len() <= 2048),
            "invalid saved attempt"
        );
    }
    let mut offers = HashSet::new();
    for o in &s.offers {
        ensure!(
            offers.insert(&o.id)
                && !o.id.is_empty()
                && o.id.len() <= 128
                && FACTION_IDS.contains(&o.proposer.as_str())
                && FACTION_IDS.contains(&o.recipient.as_str())
                && o.proposer != o.recipient
                && o.version == 1
                && o.kind == "non_aggression"
                && o.duration_seconds == 300
                && o.created_at.checked_add(120) == Some(o.expires_at)
                && matches!(o.status.as_str(), "pending" | "accepted" | "declined"),
            "invalid saved offer"
        );
    }
    let mut pacts = HashSet::new();
    for p in &s.pacts {
        let offer = s
            .offers
            .iter()
            .find(|o| o.id == p.offer_id)
            .context("pact has no offer")?;
        ensure!(
            pacts.insert(&p.offer_id)
                && offer.status == "accepted"
                && p.parties == [offer.proposer.clone(), offer.recipient.clone()]
                && p.kind == "non_aggression"
                && p.starts_at.checked_add(300) == Some(p.ends_at)
                && p.starts_at < offer.expires_at
                && p.starts_at >= offer.created_at,
            "invalid saved pact"
        );
    }
    for e in &s.history {
        ensure!(
            FACTION_IDS.contains(&e.actor.as_str())
                && FACTION_IDS.contains(&e.target.as_str())
                && e.actor != e.target
                && e.message.chars().count() <= 500
                && matches!(
                    e.action.as_str(),
                    "message" | "set_stance" | "propose_pact" | "accept" | "decline"
                ),
            "invalid saved event"
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::fs;

    fn proposal(action: &str, target: &str) -> Value {
        json!({"action":action,"target":target,"message":"Let us keep peace.","summary":"A cautious diplomatic step.","offer_id":"","offer_version":0,"stance":"neutral"})
    }
    fn apply(
        core: &mut Diplomacy,
        actor: &str,
        request: &str,
        value: Value,
        now: u64,
    ) -> Result<Value> {
        core.apply(actor, core.revision(), request, value, now)
    }
    fn accept(offer: &str, version: u64) -> Value {
        let mut value = proposal("accept", "helios");
        value["offer_id"] = json!(offer);
        value["offer_version"] = json!(version);
        value
    }

    #[test]
    fn faction_context_does_not_share_private_summaries_or_undelivered_messages() {
        let dir = tempfile::tempdir().unwrap();
        let mut core = Diplomacy::load(dir.path().join("world.json")).unwrap();
        let mut value = proposal("message", "tide");
        value["message"] = json!("Private channel message.");
        value["summary"] = json!("Secret internal goal.");
        apply(&mut core, "helios", "one", value, 100).unwrap();
        let recipient = core.context("tide", 100).unwrap().to_string();
        let stranger = core.context("cinder", 100).unwrap().to_string();
        assert!(recipient.contains("Private channel message."));
        assert!(!recipient.contains("Secret internal goal."));
        assert!(!stranger.contains("Private channel message."));
        assert!(!stranger.contains("Secret internal goal."));
        assert!(core
            .snapshot(100)
            .to_string()
            .contains("Secret internal goal."));
        assert!(core.context("invented", 100).is_err());
    }

    #[test]
    fn long_private_history_keeps_next_request_usable_after_restart() {
        for text in ["🌍".repeat(500), "\0".repeat(500)] {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("world.json");
            let mut core = Diplomacy::load(&path).unwrap();
            apply(
                &mut core,
                "helios",
                "offer",
                proposal("propose_pact", "tide"),
                100,
            )
            .unwrap();
            for at in 101..117 {
                let mut message = proposal("message", "tide");
                message["message"] = json!(text);
                apply(&mut core, "helios", &format!("message-{at}"), message, at).unwrap();
            }
            for restart in 0..2 {
                let at = 117 + restart;
                let context = core.context("tide", at).unwrap();
                assert!(serde_json::to_vec(&context).unwrap().len() <= 32768);
                let history = context["history"].as_array().unwrap();
                assert!(!history.is_empty() && history.len() < 16);
                assert_eq!(history[0]["at"], 116);
                assert!(history.iter().all(|event| event["message"] == text));
                assert_eq!(context["pending_offers"][0]["id"], "offer-1");
                assert!(core.context("cinder", at).unwrap()["history"]
                    .as_array()
                    .unwrap()
                    .is_empty());
                assert_eq!(core.snapshot(at)["history"].as_array().unwrap().len(), 17);
                core.record_attempt(
                    "tide",
                    &format!("next-{restart}"),
                    context.clone(),
                    "test-model",
                    at,
                )
                .unwrap();
                core.record_failure(
                    "tide",
                    &format!("next-{restart}"),
                    context,
                    "provider offline",
                    at,
                )
                .unwrap();
                drop(core);
                core = Diplomacy::load(&path).unwrap();
            }
        }
    }

    #[test]
    fn pact_requires_exact_recipient_consent_and_replay_never_reapplies_it() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("world.json");
        let mut core = Diplomacy::load(&path).unwrap();
        apply(
            &mut core,
            "helios",
            "offer",
            proposal("propose_pact", "tide"),
            100,
        )
        .unwrap();
        let offer = core.context("tide", 100).unwrap()["pending_offers"][0]["id"]
            .as_str()
            .unwrap()
            .to_owned();
        assert_eq!(
            core.snapshot(100)["active_pacts"].as_array().unwrap().len(),
            0
        );
        assert!(apply(&mut core, "cinder", "forged", accept(&offer, 1), 101).is_err());
        assert!(apply(&mut core, "tide", "wrong-version", accept(&offer, 2), 102).is_err());
        let revision = core.revision();
        let value = accept(&offer, 1);
        let first = core
            .apply("tide", revision, "consent", value.clone(), 103)
            .unwrap();
        assert_eq!(core.snapshot(104)["active_pacts"][0]["ends_at"], 403);
        assert_eq!(
            core.apply("tide", revision, "consent", value.clone(), 200)
                .unwrap(),
            first
        );
        drop(core);
        let mut core = Diplomacy::load(&path).unwrap();
        assert_eq!(
            core.apply("tide", revision, "consent", value, 201).unwrap(),
            first
        );
        assert_eq!(
            core.snapshot(201)["active_pacts"].as_array().unwrap().len(),
            1
        );
        assert!(core
            .apply("tide", revision, "consent", proposal("wait", ""), 201)
            .is_err());
        assert_eq!(
            core.snapshot(403)["active_pacts"].as_array().unwrap().len(),
            0
        );
    }

    #[test]
    fn stale_expired_and_duplicate_offers_are_rejected_with_evidence() {
        let dir = tempfile::tempdir().unwrap();
        let mut core = Diplomacy::load(dir.path().join("world.json")).unwrap();
        let before = core.revision();
        apply(
            &mut core,
            "helios",
            "offer",
            proposal("propose_pact", "tide"),
            100,
        )
        .unwrap();
        assert!(core
            .apply("cinder", before, "stale", proposal("message", "tide"), 101)
            .is_err());
        assert!(apply(
            &mut core,
            "tide",
            "duplicate",
            proposal("propose_pact", "helios"),
            101
        )
        .is_err());
        let offer = core.context("tide", 101).unwrap()["pending_offers"][0]["id"]
            .as_str()
            .unwrap()
            .to_owned();
        assert!(apply(&mut core, "tide", "expired", accept(&offer, 1), 220).is_err());
        assert!(core.snapshot(220)["decisions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["request_id"] == "expired" && item["accepted"] == false));
        assert_eq!(
            core.snapshot(220)["active_pacts"].as_array().unwrap().len(),
            0
        );
        apply(
            &mut core,
            "tide",
            "new-offer",
            proposal("propose_pact", "helios"),
            221,
        )
        .unwrap();
    }

    #[test]
    fn unknown_fields_targets_and_unused_authority_fields_are_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let mut core = Diplomacy::load(dir.path().join("world.json")).unwrap();
        let mut injected = proposal("message", "tide");
        injected["grant_land"] = json!(true);
        assert!(apply(&mut core, "helios", "inject", injected, 1).is_err());
        assert!(apply(
            &mut core,
            "helios",
            "self",
            proposal("message", "helios"),
            1
        )
        .is_err());
        assert!(apply(
            &mut core,
            "helios",
            "unknown",
            proposal("message", "unknown"),
            1
        )
        .is_err());
        let mut invalid = proposal("message", "tide");
        invalid["stance"] = json!("friendly");
        assert!(apply(&mut core, "helios", "smuggle", invalid, 1).is_err());
        let mut stance = proposal("set_stance", "tide");
        stance["stance"] = json!("wary");
        apply(&mut core, "helios", "stance", stance, 1).unwrap();
        assert_eq!(
            core.context("helios", 1).unwrap()["self"]["stances"]["tide"],
            "wary"
        );
        assert_eq!(
            core.context("tide", 1).unwrap()["self"]["stances"]["helios"],
            "neutral"
        );
    }

    #[test]
    fn attempt_budget_and_service_failure_survive_restart_without_gameplay_change() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("world.json");
        let mut core = Diplomacy::load(&path).unwrap();
        let revision = core.revision();
        let context = core.context("helios", 100).unwrap();
        core.record_attempt("helios", "attempt-1", context.clone(), "test-model", 100)
            .unwrap();
        assert_eq!(core.attempts(), 1);
        assert_eq!(core.revision(), revision);
        assert!(core
            .record_attempt("helios", "attempt-1", context.clone(), "test-model", 100)
            .is_err());
        core.record_failure("helios", "attempt-1", context, "service unavailable", 105)
            .unwrap();
        drop(core);
        let core = Diplomacy::load(&path).unwrap();
        assert_eq!(core.attempts(), 1);
        assert_eq!(core.revision(), revision);
        assert!(core
            .snapshot(105)
            .to_string()
            .contains("service unavailable"));
    }

    #[test]
    fn full_decision_ledger_blocks_spending_before_another_provider_call() {
        let dir = tempfile::tempdir().unwrap();
        let mut core = Diplomacy::load(dir.path().join("world.json")).unwrap();
        for n in 0..1024 {
            apply(
                &mut core,
                "helios",
                &format!("manual-{n}"),
                proposal("wait", ""),
                n,
            )
            .unwrap();
        }
        let context = core.context("helios", 1024).unwrap();
        assert!(core
            .record_attempt("helios", "provider-next", context, "test-model", 1024)
            .is_err());
        assert_eq!(core.attempts(), 0);
    }

    #[test]
    fn earlier_clock_cannot_accept_future_offer_or_create_unloadable_state() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("world.json");
        let mut core = Diplomacy::load(&path).unwrap();
        apply(
            &mut core,
            "helios",
            "offer",
            proposal("propose_pact", "tide"),
            100,
        )
        .unwrap();
        let offer = core.context("tide", 100).unwrap()["pending_offers"][0]["id"]
            .as_str()
            .unwrap()
            .to_owned();
        assert!(apply(&mut core, "tide", "early", accept(&offer, 1), 99).is_err());
        assert_eq!(
            core.snapshot(100)["active_pacts"].as_array().unwrap().len(),
            0
        );
        drop(core);
        assert!(Diplomacy::load(path).is_ok());
    }

    #[test]
    fn locking_corruption_and_failed_save_cannot_reset_or_acknowledge_state() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("world.json");
        let mut core = Diplomacy::load(&path).unwrap();
        assert!(Diplomacy::load(&path).is_err());
        let before = core.snapshot(1);
        fs::remove_file(&path).unwrap();
        fs::create_dir(&path).unwrap();
        assert!(apply(&mut core, "helios", "one", proposal("message", "tide"), 1).is_err());
        assert_eq!(core.snapshot(1), before);
        drop(core);
        fs::remove_dir(&path).unwrap();
        fs::write(&path, "broken").unwrap();
        assert!(Diplomacy::load(&path).is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), "broken");
    }
}
