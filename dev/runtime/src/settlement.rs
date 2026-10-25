//! Authority-owned state embedded in World's transaction, never a separate save.
use crate::{
    construction::{BuildRequest, Construction, Recipe, Status},
    placement::{self, Bounds, Claim, FootprintRecipe, PlacementContext, SiteRequest},
    terrain::Terrain,
};
use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

pub const STARTER_METAL: u64 = 1000;

/// A visual package reference supplied only after the caller independently checks
/// an exact activation receipt. Its hash is separate from the gameplay recipe hash.
/// This type carries the caller's approval result; it does not perform approval.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ApprovedVisualRef {
    asset_id: String,
    version: u32,
    package_sha256: String,
}

impl ApprovedVisualRef {
    /// Construct at the boundary that has already verified the exact approved
    /// package and activation receipt. This checks shape only, not provenance.
    pub fn from_verified_activation(
        asset_id: impl Into<String>,
        version: u32,
        package_sha256: impl Into<String>,
    ) -> Result<Self> {
        let value = Self {
            asset_id: asset_id.into(),
            version,
            package_sha256: package_sha256.into(),
        };
        value.validate()?;
        Ok(value)
    }

    pub fn asset_id(&self) -> &str { &self.asset_id }
    pub fn version(&self) -> u32 { self.version }
    pub fn package_sha256(&self) -> &str { &self.package_sha256 }

    fn validate(&self) -> Result<()> {
        ensure!(
            !self.asset_id.is_empty()
                && self.asset_id.len() <= 128
                && self.asset_id.bytes().all(|b| b.is_ascii_alphanumeric() || b"._:-".contains(&b)),
            "invalid visual asset ID"
        );
        ensure!(self.version > 0, "visual asset version must be positive");
        ensure!(
            self.package_sha256.len() == 64
                && self.package_sha256.bytes().all(|b| b.is_ascii_hexdigit()),
            "visual package hash must be SHA-256 hex"
        );
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct VisualRevisionReceipt {
    building_id: String,
    revision: u64,
    expected: Option<ApprovedVisualRef>,
    replacement: ApprovedVisualRef,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Species {
    Robot,
    Organic,
    Cyborg,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Core,
    Generator,
    Storehouse,
}
impl Role {
    pub fn recipe(self) -> FootprintRecipe {
        let (id, half) = match self {
            Self::Core => ("authored-core-v1", [4.0, 4.0]),
            Self::Generator => ("authored-generator-v1", [3.0, 3.0]),
            Self::Storehouse => ("authored-storehouse-v1", [4.0, 3.0]),
        };
        FootprintRecipe {
            id: id.into(),
            content_hash: format!(
                "{:x}",
                Sha256::digest(format!(
                    "{id}:{}:{}:south:2:4:authored-starter",
                    half[0], half[1]
                ))
            ),
            half_extents: half,
            entrance_width: 2.0,
            entrance_length: 4.0,
            max_slope: 0.3,
            support_depth: 3.0,
            founding_core: self == Self::Core,
        }
    }
    fn cost(self) -> u64 {
        match self {
            Self::Core => 100,
            Self::Generator => 60,
            Self::Storehouse => 40,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct BuildingView {
    pub id: String,
    pub faction_id: String,
    pub species: Species,
    pub role: Role,
    pub center: [f64; 2],
    pub ground_height: f64,
    pub half_extents: [f64; 2],
    pub entrance: Bounds,
    pub approach_bounds: Bounds,
    pub content_hash: String,
    /// Approved appearance reference. `content_hash` remains the gameplay recipe.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visual_asset: Option<ApprovedVisualRef>,
    pub completed_ticks: u64,
    pub work_ticks: u64,
    pub complete: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Building {
    view: BuildingView,
    request_id: String,
    anchor: [f64; 2],
    footprint: Bounds,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Faction {
    species: Species,
    ledger: Construction,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SettlementEvent {
    pub id: String,
    pub faction_id: String,
    pub building_id: String,
    pub kind: String,
    pub revision: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SettlementState {
    revision: u64,
    next_id: u64,
    factions: BTreeMap<String, Faction>,
    buildings: Vec<Building>,
    events: Vec<SettlementEvent>,
    #[serde(default)]
    visual_revisions: BTreeMap<String, VisualRevisionReceipt>,
}
impl Default for SettlementState {
    fn default() -> Self {
        Self::new()
    }
}
impl SettlementState {
    /// Validate deserialized state before exposing it or accepting commands.
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.factions.len() == 3 && self.buildings.len() <= 4096,
            "invalid settlement capacity"
        );
        let mut ids = std::collections::BTreeSet::new();
        let mut requests = std::collections::BTreeSet::new();
        let mut cores = std::collections::BTreeSet::new();
        let mut spent = BTreeMap::<String, u64>::new();
        for b in &self.buildings {
            let v = &b.view;
            let faction = self
                .factions
                .get(&v.faction_id)
                .context("missing building faction")?;
            let recipe = v.role.recipe();
            ensure!(
                ids.insert(&v.id) && requests.insert((&v.faction_id, &b.request_id)),
                "duplicate saved construction"
            );
            let numeric =
                v.id.strip_prefix("building-")
                    .context("invalid building ID")?
                    .parse::<u64>()?;
            ensure!(
                numeric > 0 && numeric < self.next_id,
                "invalid building identity sequence"
            );
            ensure!(
                v.species == faction.species
                    && v.half_extents == recipe.half_extents
                    && v.content_hash == recipe.content_hash,
                "saved building recipe mismatch"
            );
            if let Some(asset) = &v.visual_asset { asset.validate()?; }
            ensure!(
                v.ground_height.is_finite()
                    && v.ground_height >= 0.2
                    && v.center.iter().chain(b.anchor.iter()).all(|n| n.is_finite()
                        && *n >= 0.0
                        && *n <= f64::from(crate::terrain::WORLD_SIZE)),
                "invalid saved building geometry"
            );
            ensure!(
                b.footprint.min
                    == [
                        v.center[0] - v.half_extents[0],
                        v.center[1] - v.half_extents[1]
                    ]
                    && b.footprint.max
                        == [
                            v.center[0] + v.half_extents[0],
                            v.center[1] + v.half_extents[1]
                        ],
                "saved footprint mismatch"
            );
            for bounds in [b.footprint, v.entrance, v.approach_bounds] {
                ensure!(
                    (0..2).all(|i| bounds.min[i].is_finite()
                        && bounds.max[i].is_finite()
                        && bounds.min[i] >= 0.0
                        && bounds.max[i] <= f64::from(crate::terrain::WORLD_SIZE)
                        && bounds.min[i] < bounds.max[i]),
                    "invalid saved reservation"
                );
            }
            let order = faction
                .ledger
                .order(&b.request_id)
                .context("missing saved construction order")?;
            ensure!(
                order.request.actor == v.faction_id
                    && order.request.recipe.id == recipe.id
                    && order.request.recipe.content_hash == recipe.content_hash
                    && order.request.recipe.metal == v.role.cost()
                    && order.request.recipe.work_ticks == 120
                    && v.work_ticks == 120,
                "saved construction terms mismatch"
            );
            ensure!(
                v.completed_ticks == order.completed_ticks
                    && v.completed_ticks <= 120
                    && v.complete == (order.status == Status::Complete)
                    && v.complete == (v.completed_ticks == 120)
                    && order.status != Status::Cancelled,
                "saved progress mismatch"
            );
            if v.role == Role::Core {
                ensure!(cores.insert(&v.faction_id), "duplicate faction core");
            }
            *spent.entry(v.faction_id.clone()).or_default() += v.role.cost();
        }
        for (id, species) in [
            ("helios", Species::Robot),
            ("tide", Species::Organic),
            ("cinder", Species::Cyborg),
        ] {
            let f = self.factions.get(id).context("missing starter faction")?;
            ensure!(
                f.species == species
                    && STARTER_METAL.checked_sub(*spent.get(id).unwrap_or(&0))
                        == Some(f.ledger.metal()),
                "invalid saved starter ledger"
            );
        }
        ensure!(
            self.next_id > 0 && self.events.len() <= self.buildings.len() * 2,
            "invalid settlement event history"
        );
        let mut events = std::collections::BTreeSet::new();
        for event in &self.events {
            ensure!(
                events.insert(&event.id)
                    && event.revision <= self.revision
                    && self
                        .buildings
                        .iter()
                        .any(|b| b.view.id == event.building_id
                            && b.view.faction_id == event.faction_id),
                "invalid settlement event"
            );
        }
        let mut latest_visuals = BTreeMap::<&str, (u64, &ApprovedVisualRef)>::new();
        let mut visual_revision_numbers = std::collections::BTreeSet::new();
        for (operation_id, receipt) in &self.visual_revisions {
            ensure!(
                !operation_id.is_empty()
                    && operation_id.len() <= 128
                    && operation_id.bytes().all(|b| b.is_ascii_alphanumeric() || b"._:-".contains(&b)),
                "invalid visual revision operation ID"
            );
            ensure!(
                self.buildings.iter().any(|b| b.view.id == receipt.building_id),
                "visual revision references missing building"
            );
            ensure!(receipt.revision > 0 && receipt.revision <= self.revision, "invalid visual revision order");
            ensure!(visual_revision_numbers.insert(receipt.revision), "duplicate visual revision sequence");
            receipt.replacement.validate()?;
            if let Some(old) = &receipt.expected { old.validate()?; }
            let slot = latest_visuals.entry(receipt.building_id.as_str()).or_insert((0, &receipt.replacement));
            if receipt.revision > slot.0 { *slot = (receipt.revision, &receipt.replacement); }
        }
        for building in &self.buildings {
            match (building.view.visual_asset.as_ref(), latest_visuals.get(building.view.id.as_str())) {
                (None, None) => {}
                (Some(actual), Some((_, recorded))) if actual == *recorded => {}
                _ => anyhow::bail!("saved visual binding lacks its matching revision receipt"),
            }
        }
        Ok(())
    }
    pub fn new() -> Self {
        Self {
            revision: 0,
            next_id: 1,
            factions: [
                ("helios", Species::Robot),
                ("tide", Species::Organic),
                ("cinder", Species::Cyborg),
            ]
            .into_iter()
            .map(|(id, species)| {
                (
                    id.into(),
                    Faction {
                        species,
                        ledger: Construction::new(id, STARTER_METAL),
                    },
                )
            })
            .collect(),
            buildings: vec![],
            events: vec![],
            visual_revisions: BTreeMap::new(),
        }
    }
    pub fn revision(&self) -> u64 {
        self.revision
    }
    pub fn request(&self, faction: &str, request_id: &str) -> Option<&BuildingView> {
        self.buildings
            .iter()
            .find(|b| b.view.faction_id == faction && b.request_id == request_id)
            .map(|b| &b.view)
    }
    /// Conservative authored-building collision, including reserved construction.
    /// Building interiors are not yet a traversal surface in this starter slice.
    pub fn blocks_movement(&self, from: [f64; 2], to: [f64; 2]) -> bool {
        self.buildings.iter().any(|b| {
            let mut enter: f64 = 0.0;
            let mut exit: f64 = 1.0;
            for axis in 0..2 {
                let min = b.footprint.min[axis] - 0.35;
                let max = b.footprint.max[axis] + 0.35;
                let delta = to[axis] - from[axis];
                if delta.abs() < 1e-12 {
                    if from[axis] < min || from[axis] > max {
                        return false;
                    }
                } else {
                    let a = (min - from[axis]) / delta;
                    let z = (max - from[axis]) / delta;
                    enter = enter.max(a.min(z));
                    exit = exit.min(a.max(z));
                    if enter > exit {
                        return false;
                    }
                }
            }
            true
        })
    }
    pub fn metal(&self, actor: &str) -> Option<u64> {
        self.factions.get(actor).map(|f| f.ledger.metal())
    }
    pub fn snapshot(&self) -> Vec<BuildingView> {
        self.buildings.iter().map(|b| b.view.clone()).collect()
    }
    pub fn events(&self) -> &[SettlementEvent] {
        &self.events
    }
    /// Actor is supplied by trusted World policy/session mapping, not a player field.
    /// World persists the returned modified clone before acknowledging the ID.
    #[allow(clippy::too_many_arguments)]
    pub fn start(
        &mut self,
        terrain: &Terrain,
        actor: &str,
        request_id: &str,
        role: Role,
        center: [f64; 2],
        anchor: [f64; 2],
        observed_revision: u64,
    ) -> Result<String> {
        let faction = self.factions.get(actor).context("unknown faction")?;
        if let Some(previous) = self
            .buildings
            .iter()
            .find(|b| b.view.faction_id == actor && b.request_id == request_id)
        {
            ensure!(
                previous.view.role == role
                    && previous.view.center == center
                    && previous.anchor == anchor,
                "request ID reused with different construction"
            );
            return Ok(previous.view.id.clone());
        }
        ensure!(
            observed_revision == self.revision,
            "stale settlement revision"
        );
        ensure!(self.buildings.len() < 4096, "building capacity reached");
        if role == Role::Core {
            ensure!(
                !self
                    .buildings
                    .iter()
                    .any(|b| b.view.faction_id == actor && b.view.role == Role::Core),
                "faction already has a core"
            );
        }
        let claims: Vec<_> = self
            .buildings
            .iter()
            .filter(|b| b.view.role == Role::Core)
            .map(|b| Claim {
                owner: b.view.faction_id.clone(),
                center: b.view.center,
                radius: 80.0,
            })
            .collect();
        // Reserved approaches remain walkable, but no later footprint may occupy them.
        let obstacles: Vec<_> = self.buildings.iter().map(|b| b.footprint).collect();
        let context = PlacementContext {
            terrain,
            world_revision: self.revision,
            claims: &claims,
            obstacles: &obstacles,
        };
        let recipe = role.recipe();
        let site = placement::validate_site(
            &context,
            &SiteRequest {
                actor: actor.into(),
                center,
                anchor,
            },
            &recipe,
        )?;
        for existing in &self.buildings {
            ensure!(
                !overlaps(site.footprint(), existing.view.entrance)
                    && !overlaps(site.footprint(), existing.view.approach_bounds),
                "building blocks reserved access"
            );
        }
        let mut next = self.clone();
        let id = format!("building-{}", next.next_id);
        next.next_id = next
            .next_id
            .checked_add(1)
            .context("building ID exhausted")?;
        next.revision = next.revision.checked_add(1).context("revision exhausted")?;
        next.factions
            .get_mut(actor)
            .unwrap()
            .ledger
            .start(BuildRequest {
                actor: actor.into(),
                request_id: request_id.into(),
                recipe: Recipe {
                    id: recipe.id,
                    content_hash: recipe.content_hash.clone(),
                    metal: role.cost(),
                    work_ticks: 120,
                },
            })?;
        next.buildings.push(Building {
            view: BuildingView {
                id: id.clone(),
                faction_id: actor.into(),
                species: faction.species,
                role,
                center,
                ground_height: site.ground_height(),
                half_extents: recipe.half_extents,
                entrance: site.entrance(),
                approach_bounds: site.approach_bounds(),
                content_hash: recipe.content_hash,
                visual_asset: None,
                completed_ticks: 0,
                work_ticks: 120,
                complete: false,
            },
            request_id: request_id.into(),
            anchor,
            footprint: site.footprint(),
        });
        next.events.push(SettlementEvent {
            id: format!("{id}:reserved"),
            faction_id: actor.into(),
            building_id: id.clone(),
            kind: "construction.reserved".into(),
            revision: next.revision,
        });
        *self = next;
        Ok(id)
    }

    /// CAS-update only a live building's visual reference. This method does not
    /// create entities or touch construction, collision, resources, ownership or pose.
    pub(crate) fn revise_building_visual(
        &mut self,
        building_id: &str,
        operation_id: &str,
        expected: Option<&ApprovedVisualRef>,
        replacement: ApprovedVisualRef,
    ) -> Result<bool> {
        ensure!(
            !operation_id.is_empty()
                && operation_id.len() <= 128
                && operation_id.bytes().all(|b| b.is_ascii_alphanumeric() || b"._:-".contains(&b)),
            "invalid visual revision operation ID"
        );
        replacement.validate()?;
        if let Some(old) = expected { old.validate()?; }
        let receipt = VisualRevisionReceipt {
            building_id: building_id.to_owned(),
            revision: 0,
            expected: expected.cloned(),
            replacement: replacement.clone(),
        };
        if let Some(prior) = self.visual_revisions.get(operation_id) {
            ensure!(
                prior.building_id == receipt.building_id
                    && prior.expected == receipt.expected
                    && prior.replacement == receipt.replacement,
                "visual revision operation ID reused with different terms"
            );
            return Ok(false);
        }
        let next_revision = self.revision.checked_add(1).context("revision exhausted")?;
        let receipt = VisualRevisionReceipt { revision: next_revision, ..receipt };
        let building = self.buildings.iter_mut()
            .find(|b| b.view.id == building_id)
            .context("unknown building; visual revisions cannot create entities")?;
        ensure!(
            building.view.visual_asset.as_ref() == expected,
            "stale visual reference"
        );
        building.view.visual_asset = Some(replacement);
        self.visual_revisions.insert(operation_id.to_owned(), receipt);
        self.revision = next_revision;
        Ok(true)
    }
    /// Fixed simulation ticks only. Caller determines builder/power availability.
    pub fn advance(
        &mut self,
        ticks: u64,
        can_work: impl Fn(&BuildingView) -> bool,
    ) -> Result<bool> {
        let mut next = self.clone();
        let revision = self.revision.checked_add(1).context("revision exhausted")?;
        let mut changed = false;
        for building in &mut next.buildings {
            if building.view.complete || !can_work(&building.view) {
                continue;
            }
            let ledger = &mut next
                .factions
                .get_mut(&building.view.faction_id)
                .context("missing faction")?
                .ledger;
            ledger.advance(&building.request_id, ticks, true)?;
            let order = ledger
                .order(&building.request_id)
                .context("missing order")?;
            changed |= building.view.completed_ticks != order.completed_ticks;
            building.view.completed_ticks = order.completed_ticks;
            building.view.complete = order.status == Status::Complete;
            if building.view.complete {
                next.events.push(SettlementEvent {
                    id: format!("{}:completed", building.view.id),
                    faction_id: building.view.faction_id.clone(),
                    building_id: building.view.id.clone(),
                    kind: "construction.completed".into(),
                    revision,
                });
            }
        }
        if changed {
            next.revision = revision;
            *self = next;
        }
        Ok(changed)
    }
}
fn overlaps(a: Bounds, b: Bounds) -> bool {
    (0..2).all(|i| a.min[i] < b.max[i] && a.max[i] > b.min[i])
}
