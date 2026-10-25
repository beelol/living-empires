//! Server-only placement checks. A valid fingerprint is not release approval.
//! World must authenticate the actor, resolve approved recipes, and atomically
//! revalidate, debit and persist the resulting building with its durable outbox.
use crate::terrain::{Terrain, CELL_SIZE, WORLD_SIZE};
use anyhow::{ensure, Context, Result};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bounds {
    pub min: [f64; 2],
    pub max: [f64; 2],
}
impl Bounds {
    fn validate(self) -> Result<()> {
        ensure!(
            (0..2).all(|i| self.min[i].is_finite()
                && self.max[i].is_finite()
                && self.min[i] >= 0.0
                && self.max[i] <= f64::from(WORLD_SIZE)
                && self.min[i] < self.max[i]),
            "invalid or out-of-world bounds"
        );
        Ok(())
    }
    fn overlaps(self, other: Self) -> bool {
        (0..2).all(|i| self.min[i] < other.max[i] && self.max[i] > other.min[i])
    }
    fn corners(self) -> [[f64; 2]; 4] {
        [
            self.min,
            [self.max[0], self.min[1]],
            self.max,
            [self.min[0], self.max[1]],
        ]
    }
    fn inside_claim(self, claim: &Claim) -> bool {
        self.corners()
            .iter()
            .all(|p| distance(*p, claim.center) <= claim.radius)
    }
    fn intersects_claim(self, claim: &Claim) -> bool {
        distance(
            [
                claim.center[0].clamp(self.min[0], self.max[0]),
                claim.center[1].clamp(self.min[1], self.max[1]),
            ],
            claim.center,
        ) < claim.radius
    }
}
#[derive(Clone, Debug)]
pub struct Claim {
    pub owner: String,
    pub center: [f64; 2],
    pub radius: f64,
}
#[derive(Clone, Debug)]
pub struct FootprintRecipe {
    pub id: String,
    pub content_hash: String,
    pub half_extents: [f64; 2],
    pub entrance_width: f64,
    pub entrance_length: f64,
    pub max_slope: f64,
    pub support_depth: f64,
    pub founding_core: bool,
}
#[derive(Clone, Debug)]
pub struct SiteRequest {
    pub actor: String,
    pub center: [f64; 2],
    pub anchor: [f64; 2],
}
pub struct PlacementContext<'a> {
    pub terrain: &'a Terrain,
    pub world_revision: u64,
    pub claims: &'a [Claim],
    pub obstacles: &'a [Bounds],
}
/// Deliberately not Deserialize and with no public constructor or writable fields.
#[derive(Clone, Debug)]
pub struct LegalSite {
    request: SiteRequest,
    recipe: FootprintRecipe,
    revision: u64,
    terrain_hash: String,
    footprint: Bounds,
    entrance: Bounds,
    approach_bounds: Bounds,
    ground_height: f64,
}
impl LegalSite {
    pub fn actor(&self) -> &str {
        &self.request.actor
    }
    pub fn recipe_id(&self) -> &str {
        &self.recipe.id
    }
    pub fn content_hash(&self) -> &str {
        &self.recipe.content_hash
    }
    pub fn world_revision(&self) -> u64 {
        self.revision
    }
    pub fn footprint(&self) -> Bounds {
        self.footprint
    }
    pub fn entrance(&self) -> Bounds {
        self.entrance
    }
    pub fn approach_bounds(&self) -> Bounds {
        self.approach_bounds
    }
    pub fn ground_height(&self) -> f64 {
        self.ground_height
    }
    pub fn center(&self) -> [f64; 2] {
        self.request.center
    }
    pub fn revalidate(&self, context: &PlacementContext<'_>) -> Result<Self> {
        ensure!(
            self.revision == context.world_revision,
            "site belongs to an older world revision"
        );
        ensure!(
            self.terrain_hash == context.terrain.info().fingerprint,
            "site terrain changed"
        );
        validate_site(context, &self.request, &self.recipe)
    }
}
fn distance(a: [f64; 2], b: [f64; 2]) -> f64 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}
fn valid_id(s: &str) -> bool {
    !s.is_empty() && s.len() <= 128 && !s.chars().any(char::is_control)
}

pub fn validate_site(
    context: &PlacementContext<'_>,
    request: &SiteRequest,
    recipe: &FootprintRecipe,
) -> Result<LegalSite> {
    ensure!(
        valid_id(&request.actor) && valid_id(&recipe.id),
        "invalid actor or recipe ID"
    );
    ensure!(
        recipe.content_hash.len() == 64
            && recipe.content_hash.bytes().all(|b| b.is_ascii_hexdigit()),
        "invalid recipe fingerprint"
    );
    ensure!(
        request
            .center
            .iter()
            .chain(request.anchor.iter())
            .all(|v| v.is_finite()),
        "invalid position"
    );
    ensure!(
        recipe
            .half_extents
            .iter()
            .all(|v| v.is_finite() && *v > 0.0 && *v <= 64.0),
        "invalid footprint dimensions"
    );
    ensure!(
        recipe.entrance_width.is_finite()
            && recipe.entrance_width >= 1.0
            && recipe.entrance_width <= recipe.half_extents[0] * 2.0
            && recipe.entrance_length.is_finite()
            && (1.0..=32.0).contains(&recipe.entrance_length),
        "invalid entrance dimensions"
    );
    ensure!(
        recipe.max_slope.is_finite()
            && (0.0..=0.7).contains(&recipe.max_slope)
            && recipe.support_depth.is_finite()
            && (0.0..=32.0).contains(&recipe.support_depth),
        "invalid ground limits"
    );
    let c = request.center;
    let half = recipe.half_extents;
    let footprint = Bounds {
        min: [c[0] - half[0], c[1] - half[1]],
        max: [c[0] + half[0], c[1] + half[1]],
    };
    let entrance = Bounds {
        min: [
            c[0] - recipe.entrance_width / 2.0,
            footprint.min[1] - recipe.entrance_length,
        ],
        max: [c[0] + recipe.entrance_width / 2.0, footprint.min[1]],
    };
    footprint.validate()?;
    entrance.validate()?;
    let approach = [c[0], entrance.min[1]];
    ensure!(
        distance(request.anchor, approach) <= 128.0,
        "builder or anchor is beyond supported approach range"
    );
    let clearance = recipe.entrance_width / 2.0;
    let route = Bounds {
        min: [
            request.anchor[0].min(approach[0]) - clearance,
            request.anchor[1].min(approach[1]) - clearance,
        ],
        max: [
            request.anchor[0].max(approach[0]) + clearance,
            request.anchor[1].max(approach[1]) + clearance,
        ],
    };
    route.validate()?;
    ensure!(
        !route.overlaps(footprint),
        "approach crosses the proposed building"
    );
    for obstacle in context.obstacles {
        obstacle.validate()?;
        ensure!(
            !obstacle.overlaps(footprint),
            "building footprint overlaps an obstacle"
        );
        ensure!(
            !obstacle.overlaps(entrance) && !obstacle.overlaps(route),
            "entrance or approach is blocked"
        );
    }
    for claim in context.claims {
        ensure!(
            valid_id(&claim.owner)
                && claim
                    .center
                    .iter()
                    .all(|v| v.is_finite() && *v >= 0.0 && *v <= f64::from(WORLD_SIZE))
                && claim.radius.is_finite()
                && claim.radius > 0.0
                && claim.radius <= f64::from(WORLD_SIZE),
            "invalid authoritative claim"
        );
        if recipe.founding_core {
            ensure!(
                distance(c, claim.center) >= 80.0 + claim.radius,
                "new settlement claim overlaps an existing claim"
            );
        }
        if claim.owner != request.actor {
            ensure!(
                !footprint.intersects_claim(claim)
                    && !entrance.intersects_claim(claim)
                    && !route.intersects_claim(claim),
                "missing construction or passage rights"
            );
        }
    }
    if recipe.founding_core {
        let claim = Claim {
            owner: request.actor.clone(),
            center: c,
            radius: 80.0,
        };
        ensure!(
            footprint.inside_claim(&claim) && entrance.inside_claim(&claim),
            "core and entrance exceed the new claim"
        );
    } else {
        ensure!(
            context
                .claims
                .iter()
                .any(|claim| claim.owner == request.actor
                    && footprint.inside_claim(claim)
                    && entrance.inside_claim(claim)),
            "building requires an owned construction site"
        );
    }
    let heights = |x, y| context.terrain.sample(x, y).map(|s| f64::from(s.height));
    let (_, ground_height) = check_ground(
        footprint,
        recipe.max_slope,
        Some(recipe.support_depth),
        &heights,
    )?;
    check_ground(entrance, 0.7, None, &heights).context("entrance ground is not usable")?;
    check_ground(route, 0.7, None, &heights).context("approach ground is not usable")?;
    ensure!(
        context.terrain.can_traverse(request.anchor, approach),
        "no connected straight approach"
    );
    Ok(LegalSite {
        request: request.clone(),
        recipe: recipe.clone(),
        revision: context.world_revision,
        terrain_hash: context.terrain.info().fingerprint,
        footprint,
        entrance,
        approach_bounds: route,
        ground_height,
    })
}

// Clip the actual linear height triangles, not just footprint corners or a sparse
// sample grid. Height extrema occur on vertices of the clipped polygon.
fn check_ground(
    bounds: Bounds,
    max_slope: f64,
    support: Option<f64>,
    height: &impl Fn(f64, f64) -> Option<f64>,
) -> Result<(f64, f64)> {
    bounds.validate()?;
    let cell = f64::from(CELL_SIZE);
    let mut low = f64::INFINITY;
    let mut high = f64::NEG_INFINITY;
    for y in (bounds.min[1] / cell).floor() as u32..(bounds.max[1] / cell).ceil() as u32 {
        for x in (bounds.min[0] / cell).floor() as u32..(bounds.max[0] / cell).ceil() as u32 {
            let mut corners = [[0.0; 3]; 4];
            for (i, (dx, dy)) in [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)]
                .into_iter()
                .enumerate()
            {
                let px = (f64::from(x) + dx) * cell;
                let py = (f64::from(y) + dy) * cell;
                let h = height(px, py).context("missing terrain vertex")?;
                ensure!(h.is_finite(), "nonfinite terrain height");
                corners[i] = [px, py, h];
            }
            for indices in [[0, 1, 2], [0, 2, 3]] {
                let tri = indices.map(|i| corners[i]);
                let mut polygon = tri.to_vec();
                for axis in 0..2 {
                    polygon = clip(polygon, axis, bounds.min[axis], true);
                    polygon = clip(polygon, axis, bounds.max[axis], false);
                }
                if polygon.len() < 3 {
                    continue;
                }
                let area: f64 = polygon
                    .iter()
                    .zip(polygon.iter().cycle().skip(1))
                    .take(polygon.len())
                    .map(|(a, b)| a[0] * b[1] - b[0] * a[1])
                    .sum();
                if area.abs() < 1e-8 {
                    continue;
                }
                let a = tri[0];
                let b = tri[1];
                let c = tri[2];
                let determinant = (b[0] - a[0]) * (c[1] - a[1]) - (c[0] - a[0]) * (b[1] - a[1]);
                let dx =
                    ((b[2] - a[2]) * (c[1] - a[1]) - (c[2] - a[2]) * (b[1] - a[1])) / determinant;
                let dy =
                    ((b[0] - a[0]) * (c[2] - a[2]) - (c[0] - a[0]) * (b[2] - a[2])) / determinant;
                ensure!(dx.hypot(dy) <= max_slope + 1e-6, "excessive ground slope");
                for vertex in polygon {
                    low = low.min(vertex[2]);
                    high = high.max(vertex[2]);
                }
            }
        }
    }
    ensure!(
        low.is_finite() && high.is_finite() && low >= 0.2,
        "footprint contains water or unsafe shoreline"
    );
    if let Some(depth) = support {
        ensure!(
            high - low <= depth,
            "foundation cannot reach all ground beneath footprint"
        );
    }
    Ok((low, high))
}
fn clip(polygon: Vec<[f64; 3]>, axis: usize, limit: f64, minimum: bool) -> Vec<[f64; 3]> {
    if polygon.is_empty() {
        return polygon;
    }
    let inside = |p: [f64; 3]| {
        if minimum {
            p[axis] >= limit
        } else {
            p[axis] <= limit
        }
    };
    let mut output = Vec::new();
    let mut previous = *polygon.last().unwrap();
    for current in polygon {
        if inside(current) != inside(previous) {
            let t = (limit - previous[axis]) / (current[axis] - previous[axis]);
            output.push(std::array::from_fn(|i| {
                previous[i] + t * (current[i] - previous[i])
            }));
        }
        if inside(current) {
            output.push(current);
        }
        previous = current;
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn interior_ridge_and_water_cannot_hide_behind_safe_corners() {
        let bounds = Bounds {
            min: [0.0, 0.0],
            max: [24.0, 24.0],
        };
        for hazard in [20.0, -3.0] {
            let height = |x, y| Some(if x == 8.0 && y == 8.0 { hazard } else { 1.0 });
            if hazard < 0.0 {
                assert!(check_ground(bounds, 100.0, None, &height).is_err());
            } else {
                assert_eq!(
                    check_ground(bounds, 100.0, None, &height).unwrap(),
                    (1.0, 20.0)
                );
                assert!(check_ground(bounds, 100.0, Some(2.0), &height).is_err());
            }
            assert!(check_ground(bounds, 0.3, Some(2.0), &height).is_err());
        }
    }
    #[test]
    fn clipped_triangle_ignores_steep_geometry_outside_footprint() {
        // First triangle is flat; only the other side of diagonal 00->11 rises.
        let height = |x, y| Some(if x == 0.0 && y == 8.0 { 50.0 } else { 1.0 });
        let bounds = Bounds {
            min: [5.0, 1.0],
            max: [7.0, 2.0],
        };
        assert_eq!(
            check_ground(bounds, 0.3, Some(0.0), &height).unwrap(),
            (1.0, 1.0)
        );
    }
}
