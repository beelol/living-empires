//! Read-only adapter for the existing GM worker. Authority owns publication and retry storage.
use super::{validate, FactionView, NeedReport, Planner};
use anyhow::{ensure, Context, Result};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, HashSet},
    fs::OpenOptions,
    io::Read,
    path::{Path, PathBuf},
};

const VIEW_LIMIT: usize = 2 * 1024 * 1024;
const POLICY_LIMIT: usize = 64 * 1024;

/// Published atomically by the world owner from permitted actual state.
/// observed_at is persisted source UTC seconds, never the worker's poll time.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Views {
    schema_version: u32,
    observed_at: u64,
    views: Vec<FactionView>,
}

pub struct LivePlanner {
    policy_path: Option<PathBuf>,
    planners: BTreeMap<String, Planner>,
    policy_error: Option<String>,
}

impl LivePlanner {
    pub fn new(policy_path: Option<PathBuf>) -> Self {
        Self {
            policy_path,
            planners: BTreeMap::new(),
            policy_error: None,
        }
    }

    /// The host can expose this diagnostic without altering immutable GM facts.
    pub fn policy_error(&self) -> Option<&str> {
        self.policy_error.as_deref()
    }

    /// No scheduler, network calls, state writes, approvals or world effects.
    /// Repeated reports are intentional: the durable GM intake deduplicates by ID.
    /// Caller must preserve/retry the full envelope until durable ingest acknowledges.
    pub fn poll(&mut self, view_file: &Path, now: u64) -> Result<Vec<NeedReport>> {
        let Some(bytes) = read_regular(view_file, VIEW_LIMIT)? else {
            return Ok(Vec::new());
        };
        let envelope: Views = serde_json::from_slice(&bytes).context("invalid faction views")?;
        ensure!(envelope.schema_version == 1, "unsupported views schema");
        ensure!(
            envelope.observed_at <= now,
            "source observation is in the future"
        );
        ensure!(envelope.views.len() <= 3, "too many starter faction views");
        // Validate the whole batch before changing any faction's policy state.
        let mut factions = HashSet::new();
        for view in &envelope.views {
            ensure!(
                factions.insert(view.faction_id.clone()),
                "duplicate faction view"
            );
            validate(&view.faction_id, view, "live-validation")?;
        }
        self.planners.retain(|actor, _| factions.contains(actor));
        self.policy_error = None;
        let source = match self.policy_path.as_deref() {
            Some(path) => match read_regular(path, POLICY_LIMIT) {
                Ok(Some(bytes)) => match String::from_utf8(bytes) {
                    Ok(source) => Some(source),
                    Err(_) => {
                        self.policy_error =
                            Some("policy must be UTF-8; retaining last good".into());
                        None
                    }
                },
                Ok(None) => {
                    self.policy_error = Some("policy file missing; retaining last good".into());
                    None
                }
                Err(_) => {
                    self.policy_error =
                        Some("policy file unreadable or invalid; retaining last good".into());
                    None
                }
            },
            None => None,
        };
        let mut reports = Vec::new();
        for view in envelope.views {
            let actor = &view.faction_id;
            if !self.planners.contains_key(actor) {
                self.planners.insert(actor.clone(), Planner::new(actor)?);
            }
            let planner = self
                .planners
                .get_mut(actor)
                .context("missing faction planner")?;
            if let Some(source) = source.as_deref() {
                if planner.stage_policy(source).is_err() {
                    self.policy_error =
                        Some("policy validation failed; retaining last good".into());
                }
            }
            let decision = planner.plan(actor, &view, "pending", envelope.observed_at)?;
            if decision.policy_rejected {
                self.policy_error = Some("policy choice rejected; using last good rules".into());
            }
            if let Some(mut report) = decision.report {
                // Hash the ENTIRE immutable envelope with a fixed placeholder ID.
                // Including at and policy_rejected prevents changed retries after restart.
                let hash = Sha256::digest(serde_json::to_vec(&report)?);
                report.id = format!("brain-{hash:x}");
                reports.push(report);
            }
        }
        reports.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(reports)
    }
}

fn read_regular(path: &Path, limit: usize) -> Result<Option<Vec<u8>>> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NONBLOCK | libc::O_NOFOLLOW);
    }
    let file = match options.open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error).context("cannot open faction input"),
    };
    ensure!(
        file.metadata()?.is_file(),
        "faction input must be a regular file"
    );
    let mut bytes = Vec::new();
    file.take(limit as u64 + 1).read_to_end(&mut bytes)?;
    ensure!(bytes.len() <= limit, "faction input exceeds byte limit");
    Ok(Some(bytes))
}
