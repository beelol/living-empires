//! Bounded city layout proposals; only the caller's authority check grants a site.
//! No inventory, world persistence, scheduling or asset approval lives here.
pub mod source;
use anyhow::{ensure, Context, Result};
use mlua::{Function, HookTriggers, Lua, LuaOptions, StdLib, Table, Value, VmState};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};

const DEFAULT_POLICY: &str = include_str!("../../../content/city_layout.lua");
const MAX_ATTEMPTS: usize = 128;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Species {
    Organic,
    Cyborg,
    Robot,
}
impl Species {
    fn name(self) -> &'static str {
        match self {
            Self::Organic => "organic",
            Self::Cyborg => "cyborg",
            Self::Robot => "robot",
        }
    }
}

/// Reservations include accepted footprints, entrances and access corridors.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Rect {
    pub min: [f64; 2],
    pub max: [f64; 2],
}
impl Rect {
    pub fn overlaps(self, other: Self) -> bool {
        (0..2).all(|i| self.min[i] < other.max[i] && self.max[i] > other.min[i])
    }
    fn valid(self) -> bool {
        (0..2).all(|i| {
            self.min[i].is_finite() && self.max[i].is_finite() && self.min[i] < self.max[i]
        })
    }
}

/// Trusted faction-local input from World. Anchors must already connect to the
/// settlement network. The authority must check their current reachability again.
#[derive(Clone, Debug)]
pub struct LayoutRequest {
    pub faction_id: String,
    pub species: Species,
    pub center: [f64; 2],
    pub founded: bool,
    pub half_extents: [f64; 2],
    pub anchors: Vec<[f64; 2]>,
    pub reserved: Vec<Rect>,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pose {
    pub center: [f64; 2],
    pub anchor: [f64; 2],
}

/// A site is the exact value returned by the authority, never a city-issued token.
/// The caller still revalidates current revision, rights and payment at commit.
#[derive(Debug)]
pub struct LayoutDecision<T> {
    pub site: Option<T>,
    pub pose: Option<Pose>,
    pub policy_version: String,
    pub policy_rejected: bool,
    pub attempted: usize,
    pub blockers: Vec<String>,
}

pub struct CityPlanner {
    faction_id: String,
    active: Policy,
    staged: Option<Policy>,
}
struct Policy {
    source: String,
    version: String,
}

impl CityPlanner {
    pub fn new(faction_id: &str) -> Result<Self> {
        ensure!(valid_id(faction_id), "invalid city faction");
        Ok(Self {
            faction_id: faction_id.into(),
            active: Policy::load(DEFAULT_POLICY)?,
            staged: None,
        })
    }

    pub fn active_policy_version(&self) -> &str {
        &self.active.version
    }

    /// Promote only at an authority tick, including while no build is requested.
    /// Validation exercises all species/founding branches without world effects.
    pub fn activate_staged(&mut self, request: &LayoutRequest) -> Result<bool> {
        ensure!(
            request.faction_id == self.faction_id,
            "city planner belongs to another faction"
        );
        validate(request)?;
        let Some(policy) = self.staged.take() else {
            return Ok(false);
        };
        policy.checked_offsets(request)?;
        self.active = policy;
        Ok(true)
    }

    /// The host reads a changed file and stages it; the next proposal boundary
    /// promotes a valid result. Failed loads leave active and staged rules intact.
    pub fn stage_policy(&mut self, source: &str) -> Result<()> {
        self.staged = Some(Policy::load(source)?);
        Ok(())
    }

    pub fn propose<T>(
        &mut self,
        request: &LayoutRequest,
        mut check: impl FnMut(Pose) -> Result<T>,
    ) -> Result<LayoutDecision<T>> {
        ensure!(
            request.faction_id == self.faction_id,
            "city planner belongs to another faction"
        );
        validate(request)?;
        let mut rejected = false;
        let staged = self
            .staged
            .take()
            .and_then(|policy| match policy.checked_offsets(request) {
                Ok(offsets) => {
                    self.active = policy;
                    Some(offsets)
                }
                Err(_) => {
                    rejected = true;
                    None
                }
            });
        let offsets = match staged {
            Some(offsets) => offsets,
            None => match self.active.offsets(request) {
                Ok(offsets) => offsets,
                Err(_) => {
                    rejected = true;
                    self.active = Policy::load(DEFAULT_POLICY)?;
                    self.active.offsets(request)?
                }
            },
        };
        let mut decision = LayoutDecision {
            site: None,
            pose: None,
            policy_version: self.active.version.clone(),
            policy_rejected: rejected,
            attempted: 0,
            blockers: Vec::new(),
        };
        let mut blockers = BTreeSet::new();
        'sites: for offset in offsets {
            let center = [request.center[0] + offset[0], request.center[1] + offset[1]];
            // A metre of service space supplements exact authoritative collision.
            let footprint = Rect {
                min: std::array::from_fn(|i| center[i] - request.half_extents[i] - 1.0),
                max: std::array::from_fn(|i| center[i] + request.half_extents[i] + 1.0),
            };
            if request.reserved.iter().any(|r| footprint.overlaps(*r)) {
                blockers.insert("existing footprint, entrance or route needs clearance".into());
                continue;
            }
            for &anchor in &request.anchors {
                if decision.attempted == MAX_ATTEMPTS {
                    blockers.insert("bounded city candidate search exhausted".into());
                    break 'sites;
                }
                decision.attempted += 1;
                let pose = Pose { center, anchor };
                match check(pose) {
                    Ok(site) => {
                        decision.site = Some(site);
                        decision.pose = Some(pose);
                        break 'sites;
                    }
                    Err(error) => {
                        // Bound exposed diagnostics independently of authority error size.
                        blockers.insert(format!("{error:#}").chars().take(240).collect());
                    }
                }
            }
        }
        if decision.site.is_none() && blockers.is_empty() {
            blockers.insert("no legal city candidate available".into());
        }
        decision.blockers = blockers.into_iter().take(16).collect();
        Ok(decision)
    }
}
fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 96
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.:".contains(&b))
}
fn validate(request: &LayoutRequest) -> Result<()> {
    ensure!(valid_id(&request.faction_id), "invalid city faction");
    ensure!(
        request.center.iter().all(|n| n.is_finite()),
        "invalid city center"
    );
    ensure!(
        request
            .half_extents
            .iter()
            .all(|n| n.is_finite() && *n > 0.0 && *n <= 64.0),
        "invalid building dimensions"
    );
    ensure!(
        !request.anchors.is_empty() && request.anchors.len() <= 16,
        "city requires 1..16 connected anchors"
    );
    ensure!(
        request.anchors.iter().flatten().all(|n| n.is_finite()),
        "invalid access anchor"
    );
    ensure!(
        request.reserved.len() <= 1024 && request.reserved.iter().all(|r| r.valid()),
        "invalid city reservations"
    );
    Ok(())
}
impl Policy {
    fn load(source: &str) -> Result<Self> {
        ensure!(source.len() <= 65536, "city policy exceeds 64 KiB");
        vm(source)?;
        Ok(Self {
            source: source.into(),
            version: format!("{:x}", Sha256::digest(source.as_bytes())),
        })
    }
    fn checked_offsets(&self, request: &LayoutRequest) -> Result<Vec<[f64; 2]>> {
        // Lua sees only these two finite inputs. Validate the complete input
        // family before promotion, so a founding edit cannot hide broken growth.
        for species in [Species::Organic, Species::Cyborg, Species::Robot] {
            for founded in [false, true] {
                let mut variant = request.clone();
                variant.species = species;
                variant.founded = founded;
                self.offsets(&variant)?;
            }
        }
        self.offsets(request)
    }
    fn offsets(&self, request: &LayoutRequest) -> Result<Vec<[f64; 2]>> {
        let (_lua, function, budget) = vm(&self.source)?;
        budget.store(0, Ordering::Relaxed);
        let rows: Table = function
            .call((request.species.name(), request.founded))
            .context("city policy must return offsets")?;
        let count = rows.raw_len();
        ensure!(
            (1..=32).contains(&count),
            "city policy requires 1..32 offsets"
        );
        let keys: Vec<_> = rows
            .clone()
            .pairs::<Value, Value>()
            .collect::<mlua::Result<_>>()?;
        ensure!(
            keys.len() == count
                && keys.iter().all(
                    |(key, _)| matches!(key,Value::Integer(i) if *i>=1 && (*i as usize)<=count)
                ),
            "city offsets must be a dense sequence"
        );
        let mut result = Vec::with_capacity(count);
        for i in 1..=count {
            let row: Table = rows.raw_get(i)?;
            let x: f64 = row.raw_get(1)?;
            let y: f64 = row.raw_get(2)?;
            ensure!(
                x.is_finite() && y.is_finite() && x.hypot(y) <= 64.0,
                "city offset outside bounded claim layout"
            );
            ensure!(!result.contains(&[x, y]), "duplicate city offset");
            result.push([x, y]);
        }
        ensure!(
            request.founded || result == vec![[0.0, 0.0]],
            "founding center is fixed by authority"
        );
        Ok(result)
    }
}
fn vm(source: &str) -> Result<(Lua, Function, Arc<AtomicUsize>)> {
    let lua = Lua::new_with(StdLib::NONE, LuaOptions::default())?;
    lua.set_memory_limit(2 * 1024 * 1024)?;
    for name in [
        "setmetatable",
        "getmetatable",
        "dofile",
        "loadfile",
        "load",
        "collectgarbage",
        "pcall",
        "xpcall",
        "print",
    ] {
        lua.globals().set(name, Value::Nil)?;
    }
    let budget = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&budget);
    lua.set_hook(
        HookTriggers::new().every_nth_instruction(1000),
        move |_, _| {
            if counter.fetch_add(1, Ordering::Relaxed) >= 50 {
                return Err(mlua::Error::RuntimeError(
                    "city policy instruction limit exceeded".into(),
                ));
            }
            Ok(VmState::Continue)
        },
    )?;
    let module: Table = lua.load(source).set_name("city_layout").eval()?;
    ensure!(
        module.get::<u32>("schema_version")? == 1,
        "unsupported city policy schema"
    );
    let function = module.get("offsets")?;
    Ok((lua, function, budget))
}
