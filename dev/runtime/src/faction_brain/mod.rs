//! Pure, bounded faction planning. Inputs come from authority; outputs grant no effects.
use anyhow::{ensure, Context, Result};
use mlua::{Function, HookTriggers, Lua, LuaOptions, StdLib, Table, Value as LuaValue, VmState};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet, HashSet},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};

const DEFAULT_POLICY: &str = include_str!("policy.lua");

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FactionView {
    pub schema_version: u32,
    pub faction_id: String,
    pub revision: u64,
    pub available_metal: u64,
    pub spending_limit: u64,
    pub priorities: BTreeMap<String, u32>,
    pub observations: Vec<Observation>,
    pub needs: Vec<Need>,
    pub candidates: Vec<Candidate>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Observation {
    pub id: String,
    pub allowed_recipients: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Need {
    pub id: String,
    pub role: String,
    #[serde(default)]
    pub category: NeedCategory,
    pub urgency: u32,
    pub evidence_ids: Vec<String>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NeedCategory {
    #[default]
    Unit,
    Building,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Candidate {
    pub id: String,
    pub need_id: String,
    pub action: Action,
    /// Authoritative reason codes, e.g. no_factory or design_not_active.
    pub blockers: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Action {
    RequestDesign,
    BuildUnit {
        design_id: String,
        /// Reference only: the construction authority verifies actual approved bytes.
        approval_sha256: String,
        metal_cost: u64,
    },
    BuildStructure {
        design_id: String,
        approval_sha256: String,
        metal_cost: u64,
        /// Opaque authority-issued site reference; the planner cannot approve placement.
        site_id: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct SelectedAction {
    pub faction_id: String,
    pub observed_revision: u64,
    pub candidate_id: String,
    pub need_id: String,
    pub role: String,
    pub category: NeedCategory,
    pub score: u64,
    pub action: Action,
}

/// Matches the agreed GM v1 envelope; only trusted adapters may ingest it.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct NeedReport {
    pub id: String,
    pub source: String,
    pub at: u64,
    pub kind: String,
    pub payload: Value,
    pub allowed_recipients: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Decision {
    pub action: Option<SelectedAction>,
    pub report: Option<NeedReport>,
    pub policy_version: String,
    pub policy_rejected: bool,
}

/// Evaluate one authoritative snapshot. The caller owns scheduling and durable IDs.
pub fn plan(actor: &str, view: &FactionView, report_id: &str, at: u64) -> Result<Decision> {
    Planner::new(actor)?.plan(actor, view, report_id, at)
}

/// Host-owned instance. Stage source at reload; evaluate/promote it at a decision boundary.
/// Policies see only feasible candidate IDs/roles/scores and get a fresh VM per call.
pub struct Planner {
    faction_id: String,
    active: Policy,
    staged: Option<Policy>,
}

struct Policy {
    source: String,
    version: String,
}

type Feasible<'a> = (&'a Need, &'a Candidate, u64);

impl Planner {
    pub fn new(faction_id: &str) -> Result<Self> {
        ensure!(identifier(faction_id), "invalid planner faction");
        Ok(Self {
            faction_id: faction_id.into(),
            active: Policy::load(DEFAULT_POLICY)?,
            staged: None,
        })
    }

    /// Failed compilation leaves both the active and any previously staged version intact.
    pub fn stage_policy(&mut self, source: &str) -> Result<()> {
        self.staged = Some(Policy::load(source)?);
        Ok(())
    }

    pub fn plan(
        &mut self,
        actor: &str,
        view: &FactionView,
        report_id: &str,
        at: u64,
    ) -> Result<Decision> {
        ensure!(
            actor == self.faction_id,
            "planner belongs to another faction"
        );
        validate(actor, view, report_id)?;
        let weighted: Vec<_> = view
            .needs
            .iter()
            .filter_map(|need| {
                let weight = view.priorities.get(&need.role).copied().unwrap_or(0);
                let score = u64::from(weight) * u64::from(need.urgency);
                (score > 0).then_some((need, score))
            })
            .collect();
        let feasible: Vec<_> = weighted
            .iter()
            .flat_map(|(need, score)| {
                view.candidates
                    .iter()
                    .filter(move |candidate| {
                        candidate.need_id == need.id && blockers(candidate, view).is_empty()
                    })
                    .map(move |candidate| (*need, candidate, *score))
            })
            .collect();
        let mut policy_rejected = false;
        let selected = if feasible.is_empty() {
            None
        } else {
            let staged_choice =
                self.staged
                    .take()
                    .and_then(|candidate| match candidate.select(&feasible) {
                        Ok(index) => {
                            self.active = candidate;
                            Some(index)
                        }
                        Err(_) => {
                            policy_rejected = true;
                            None
                        }
                    });
            let index = match staged_choice {
                Some(index) => index,
                None => match self.active.select(&feasible) {
                    Ok(index) => index,
                    Err(_) => {
                        // A previously valid script can fail on new inputs. Fall back to
                        // the packaged policy, never to an unchecked model/order response.
                        policy_rejected = true;
                        self.active = Policy::load(DEFAULT_POLICY)?;
                        self.active.select(&feasible)?
                    }
                },
            };
            Some(feasible[index])
        };
        let policy_version = self.active.version.clone();
        let (need, action, status, reasons) = if let Some((need, candidate, score)) = selected {
            (
                Some(need),
                Some(SelectedAction {
                    faction_id: actor.into(),
                    observed_revision: view.revision,
                    candidate_id: candidate.id.clone(),
                    need_id: need.id.clone(),
                    role: need.role.clone(),
                    category: need.category,
                    score,
                    action: candidate.action.clone(),
                }),
                "proposed",
                BTreeSet::new(),
            )
        } else if let Some((need, _)) = weighted
            .iter()
            .min_by(|(a, sa), (b, sb)| sb.cmp(sa).then_with(|| a.id.cmp(&b.id)))
        {
            let mut reasons: BTreeSet<String> = view
                .candidates
                .iter()
                .filter(|candidate| candidate.need_id == need.id)
                .flat_map(|candidate| blockers(candidate, view))
                .collect();
            if reasons.is_empty() {
                reasons.insert("no_supported_action".into());
            }
            (Some(*need), None, "blocked", reasons)
        } else {
            (None, None, "", BTreeSet::new())
        };
        let report = need.map(|need| NeedReport {
            id: report_id.into(),
            source: "faction-brain".into(),
            at,
            kind: match (need.category, action.is_some()) {
                (NeedCategory::Unit, true) => "unit_need",
                (NeedCategory::Unit, false) => "blocked_need",
                (NeedCategory::Building, true) => "building_need",
                (NeedCategory::Building, false) => "blocked_building_need",
            }.into(),
            allowed_recipients: vec![actor.into()],
            payload: json!({
                "schema_version":1, "faction_id":actor, "observed_revision":view.revision,
                "need_id":need.id, "role":need.role, "category":need.category, "urgency":need.urgency,
                "evidence_ids":need.evidence_ids, "status":status, "proposal":action,
                "blockers":reasons, "available_metal":view.available_metal,
                "spending_limit":view.spending_limit, "policy_version":policy_version,
                "policy_rejected":policy_rejected,
            }),
        });
        if let Some(report) = &report {
            ensure!(
                serde_json::to_vec(&report.payload)?.len() <= 16384,
                "need report exceeds GM payload limit"
            );
        }
        Ok(Decision {
            action,
            report,
            policy_version,
            policy_rejected,
        })
    }
}

fn identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 96
        && !value.contains("..")
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.:".contains(&b))
}

fn validate(actor: &str, view: &FactionView, report_id: &str) -> Result<()> {
    ensure!(
        identifier(actor) && identifier(report_id),
        "invalid actor or report identity"
    );
    ensure!(view.schema_version == 1, "unsupported faction view version");
    ensure!(view.faction_id == actor, "context does not belong to actor");
    ensure!(
        view.needs.len() <= 128
            && view.candidates.len() <= 128
            && view.observations.len() <= 256
            && view.priorities.len() <= 32,
        "faction view exceeds turn limits"
    );
    ensure!(
        view.priorities.keys().all(|role| identifier(role)),
        "invalid priority role"
    );
    let mut observations = HashSet::new();
    for observation in &view.observations {
        ensure!(
            identifier(&observation.id) && observations.insert(observation.id.as_str()),
            "invalid or duplicate observation"
        );
        let mut recipients = HashSet::new();
        ensure!(
            observation.allowed_recipients.len() <= 64
                && observation
                    .allowed_recipients
                    .iter()
                    .all(|id| identifier(id) && recipients.insert(id)),
            "invalid observation recipients"
        );
        ensure!(
            observation.allowed_recipients.iter().any(|id| id == actor),
            "observation is not allowed for actor"
        );
    }
    let mut needs = HashSet::new();
    for need in &view.needs {
        ensure!(
            identifier(&need.id) && identifier(&need.role) && needs.insert(need.id.as_str()),
            "invalid or duplicate need"
        );
        let mut evidence = HashSet::new();
        ensure!(
            !need.evidence_ids.is_empty()
                && need.evidence_ids.len() <= 32
                && need
                    .evidence_ids
                    .iter()
                    .all(|id| observations.contains(id.as_str()) && evidence.insert(id)),
            "need lacks distinct allowed evidence"
        );
    }
    let mut candidates = HashSet::new();
    for candidate in &view.candidates {
        ensure!(
            identifier(&candidate.id)
                && candidates.insert(candidate.id.as_str())
                && needs.contains(candidate.need_id.as_str()),
            "invalid, duplicate or unrelated candidate"
        );
        ensure!(
            candidate.blockers.len() <= 8
                && candidate.blockers.iter().all(|reason| identifier(reason)),
            "invalid candidate blockers"
        );
        let need = view
            .needs
            .iter()
            .find(|need| need.id == candidate.need_id)
            .context("candidate has no need")?;
        match &candidate.action {
            Action::BuildUnit { .. } => ensure!(
                need.category == NeedCategory::Unit,
                "unit recipe cannot fulfill a building need"
            ),
            Action::BuildStructure { site_id, .. } => ensure!(
                need.category == NeedCategory::Building && identifier(site_id),
                "building requires a building need and authority site reference"
            ),
            Action::RequestDesign => {}
        }
        if let Action::BuildUnit {
            design_id,
            approval_sha256,
            metal_cost,
        }
        | Action::BuildStructure {
            design_id,
            approval_sha256,
            metal_cost,
            ..
        } = &candidate.action
        {
            ensure!(
                identifier(design_id)
                    && *metal_cost > 0
                    && approval_sha256.len() == 64
                    && approval_sha256
                        .bytes()
                        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
                "build requires a paid recipe and exact approval reference"
            );
        }
    }
    Ok(())
}

fn blockers(candidate: &Candidate, view: &FactionView) -> BTreeSet<String> {
    let mut reasons: BTreeSet<_> = candidate.blockers.iter().cloned().collect();
    if let Action::BuildUnit { metal_cost, .. } | Action::BuildStructure { metal_cost, .. } =
        candidate.action
    {
        if metal_cost > view.available_metal {
            reasons.insert("insufficient_metal".into());
        }
        if metal_cost > view.spending_limit {
            reasons.insert("spending_limit".into());
        }
    }
    reasons
}

impl Policy {
    fn load(source: &str) -> Result<Self> {
        ensure!(source.len() <= 65536, "policy exceeds 64 KiB");
        policy_vm(source)?;
        Ok(Self {
            source: source.into(),
            version: format!("{:x}", Sha256::digest(source.as_bytes())),
        })
    }

    fn select(&self, options: &[Feasible<'_>]) -> Result<usize> {
        // No globals, closures or memory survive between factions or turns.
        let (lua, choose, budget) = policy_vm(&self.source)?;
        let rows = lua.create_table()?;
        for (index, (need, candidate, score)) in options.iter().enumerate() {
            let row = lua.create_table()?;
            row.set("id", candidate.id.as_str())?;
            row.set("role", need.role.as_str())?;
            row.set(
                "category",
                match need.category {
                    NeedCategory::Unit => "unit",
                    NeedCategory::Building => "building",
                },
            )?;
            // Exact lexical order avoids Lua signed overflow and f64 rounding.
            row.set("score_key", format!("{score:020}"))?;
            rows.set(index + 1, row)?;
        }
        budget.store(0, Ordering::Relaxed);
        let id: String = choose
            .call(rows)
            .context("policy must choose an allowed candidate ID")?;
        options
            .iter()
            .position(|(_, candidate, _)| candidate.id == id)
            .context("policy selected an unavailable candidate")
    }
}

fn policy_vm(source: &str) -> Result<(Lua, Function, Arc<AtomicUsize>)> {
    let lua = Lua::new_with(StdLib::NONE, LuaOptions::default())?;
    lua.set_memory_limit(2 * 1024 * 1024)?;
    for name in [
        "dofile",
        "loadfile",
        "load",
        "collectgarbage",
        "pcall",
        "xpcall",
        "print",
    ] {
        lua.globals().set(name, LuaValue::Nil)?;
    }
    let budget = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&budget);
    lua.set_hook(
        HookTriggers::new().every_nth_instruction(1000),
        move |_, _| {
            if counter.fetch_add(1, Ordering::Relaxed) >= 50 {
                return Err(mlua::Error::RuntimeError(
                    "policy instruction limit exceeded".into(),
                ));
            }
            Ok(VmState::Continue)
        },
    )?;
    let module: Table = lua.load(source).set_name("faction_policy").eval()?;
    ensure!(
        module.get::<u32>("schema_version")? == 1,
        "unsupported policy schema"
    );
    let choose: Function = module.get("choose")?;
    Ok((lua, choose, budget))
}
