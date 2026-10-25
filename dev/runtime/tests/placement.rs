#[allow(dead_code)]
#[path = "../src/placement.rs"]
mod placement;
use argon_dev::terrain;
use argon_dev::terrain::Terrain;
use placement::*;
use std::sync::OnceLock;

fn terrain() -> &'static Terrain {
    static TERRAIN: OnceLock<Terrain> = OnceLock::new();
    TERRAIN.get_or_init(|| Terrain::generate(20261025))
}
fn recipe(core: bool) -> FootprintRecipe {
    FootprintRecipe {
        id: "starter-core".into(),
        content_hash: "a".repeat(64),
        half_extents: [4.0, 4.0],
        entrance_width: 2.0,
        entrance_length: 4.0,
        max_slope: 0.3,
        support_depth: 3.0,
        founding_core: core,
    }
}
fn request() -> SiteRequest {
    let center = terrain().spawn();
    SiteRequest {
        actor: "robots".into(),
        center,
        anchor: [center[0], center[1] - 24.0],
    }
}
fn context<'a>(claims: &'a [Claim], obstacles: &'a [Bounds]) -> PlacementContext<'a> {
    PlacementContext {
        terrain: terrain(),
        world_revision: 12,
        claims,
        obstacles,
    }
}

#[test]
fn core_site_binds_current_world_and_revalidates_before_commit() {
    let site = validate_site(&context(&[], &[]), &request(), &recipe(true)).unwrap();
    assert_eq!(site.actor(), "robots");
    assert_eq!(site.recipe_id(), "starter-core");
    assert_eq!(site.content_hash(), "a".repeat(64));
    assert_eq!(site.world_revision(), 12);
    assert!(site.ground_height().is_finite());
    site.revalidate(&context(&[], &[])).unwrap();
    let mut stale = context(&[], &[]);
    stale.world_revision += 1;
    assert!(site.revalidate(&stale).is_err());
    let blocker = [site.footprint()];
    assert!(site.revalidate(&context(&[], &blocker)).is_err());
}

#[test]
fn claims_prevent_free_ownership_and_overlapping_foundations() {
    let center = request().center;
    assert!(validate_site(&context(&[], &[]), &request(), &recipe(false)).is_err());
    let mut claim = Claim {
        owner: "aliens".into(),
        center,
        radius: 80.0,
    };
    assert!(validate_site(&context(&[claim.clone()], &[]), &request(), &recipe(false)).is_err());
    claim.owner = "robots".into();
    validate_site(&context(&[claim.clone()], &[]), &request(), &recipe(false)).unwrap();
    assert!(validate_site(&context(&[claim.clone()], &[]), &request(), &recipe(true)).is_err());
    claim.center[0] += 159.0;
    assert!(validate_site(&context(&[claim], &[]), &request(), &recipe(true)).is_err());
}

#[test]
fn entrance_obstacles_and_malformed_geometry_cannot_be_approved() {
    let c = request().center;
    let obstacle = Bounds {
        min: [c[0] - 0.5, c[1] - 7.0],
        max: [c[0] + 0.5, c[1] - 5.0],
    };
    assert!(validate_site(&context(&[], &[obstacle]), &request(), &recipe(true)).is_err());
    let mut bad = request();
    bad.center[0] = f64::NAN;
    assert!(validate_site(&context(&[], &[]), &bad, &recipe(true)).is_err());
    let mut bad_recipe = recipe(true);
    bad_recipe.half_extents[0] = -1.0;
    assert!(validate_site(&context(&[], &[]), &request(), &bad_recipe).is_err());
    bad_recipe = recipe(true);
    bad_recipe.content_hash.clear();
    assert!(validate_site(&context(&[], &[]), &request(), &bad_recipe).is_err());
}
