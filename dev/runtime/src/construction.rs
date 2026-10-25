//! Deterministic construction accounting, embedded in the authority's transaction.
//!
//! This module does not approve recipes, authenticate network users, validate sites,
//! or acknowledge durable writes. Its caller must do those checks and persist the
//! resulting state together with the rest of the world before acknowledging it.
use anyhow::{bail, ensure, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Recipe {
    pub id: String,
    pub content_hash: String,
    pub metal: u64,
    pub work_ticks: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct BuildRequest {
    pub actor: String,
    pub request_id: String,
    pub recipe: Recipe,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum Status {
    Building,
    Complete,
    Cancelled,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Order {
    pub request: BuildRequest,
    pub completed_ticks: u64,
    pub status: Status,
}

/// One faction's construction escrow. Persist this inside the world transaction;
/// never keep a second writable balance elsewhere for these same resources.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Construction {
    owner: String,
    metal: u64,
    orders: BTreeMap<String, Order>,
}

fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 128 && !id.chars().any(char::is_control)
}

impl Construction {
    pub fn new(owner: &str, metal: u64) -> Self {
        Self {
            owner: owner.into(),
            metal,
            orders: BTreeMap::new(),
        }
    }

    pub fn metal(&self) -> u64 {
        self.metal
    }

    pub fn order(&self, id: &str) -> Option<&Order> {
        self.orders.get(id)
    }

    /// Caller supplies an authenticated actor and a server-approved recipe.
    /// Returns the stable request ID; it is faction-scoped, not a world entity ID.
    pub fn start(&mut self, request: BuildRequest) -> Result<String> {
        ensure!(
            valid_id(&request.actor) && request.actor == self.owner,
            "not the construction owner"
        );
        ensure!(valid_id(&request.request_id), "invalid request ID");
        if let Some(previous) = self.orders.get(&request.request_id) {
            ensure!(
                previous.request == request,
                "request ID already used with different terms"
            );
            return Ok(request.request_id);
        }
        ensure!(valid_id(&request.recipe.id), "invalid recipe ID");
        ensure!(
            request.recipe.work_ticks > 0,
            "work duration must be positive"
        );
        ensure!(
            request.recipe.content_hash.len() == 64
                && request
                    .recipe
                    .content_hash
                    .bytes()
                    .all(|b| b.is_ascii_hexdigit()),
            "invalid content fingerprint"
        );
        ensure!(
            self.orders.len() < 100_000,
            "construction history capacity reached"
        );
        let balance = self
            .metal
            .checked_sub(request.recipe.metal)
            .ok_or_else(|| anyhow::anyhow!("insufficient metal"))?;
        let id = request.request_id.clone();
        self.orders.insert(
            id.clone(),
            Order {
                request,
                completed_ticks: 0,
                status: Status::Building,
            },
        );
        self.metal = balance;
        Ok(id)
    }

    /// Authority-only simulation advance. `can_work` includes builder range and
    /// required power. Network retries must never call this method directly.
    pub fn advance(&mut self, id: &str, ticks: u64, can_work: bool) -> Result<()> {
        let order = self
            .orders
            .get_mut(id)
            .ok_or_else(|| anyhow::anyhow!("unknown construction"))?;
        if order.status != Status::Building || !can_work {
            return Ok(());
        }
        order.completed_ticks = order
            .completed_ticks
            .saturating_add(ticks)
            .min(order.request.recipe.work_ticks);
        if order.completed_ticks == order.request.recipe.work_ticks {
            order.status = Status::Complete;
        }
        Ok(())
    }

    /// Returns the amount refunded by this call. Repeated cancellation returns 0.
    pub fn cancel(&mut self, actor: &str, id: &str) -> Result<u64> {
        ensure!(actor == self.owner, "not the construction owner");
        let order = self
            .orders
            .get_mut(id)
            .ok_or_else(|| anyhow::anyhow!("unknown construction"))?;
        match order.status {
            Status::Cancelled => return Ok(0),
            Status::Complete => bail!("completed construction cannot be cancelled"),
            Status::Building => {}
        }
        ensure!(
            order.request.recipe.work_ticks > 0
                && order.completed_ticks <= order.request.recipe.work_ticks,
            "invalid saved construction progress"
        );
        let consumed = (u128::from(order.request.recipe.metal) * u128::from(order.completed_ticks))
            .div_ceil(u128::from(order.request.recipe.work_ticks)) as u64;
        let refund = order.request.recipe.metal - consumed;
        let balance = self
            .metal
            .checked_add(refund)
            .ok_or_else(|| anyhow::anyhow!("metal overflow"))?;
        order.status = Status::Cancelled;
        self.metal = balance;
        Ok(refund)
    }
}
