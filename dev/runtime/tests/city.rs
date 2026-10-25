#[allow(dead_code)]
#[path = "../src/city/mod.rs"]
mod city;
use city::*;
use std::cell::Cell;

fn request(species: Species) -> LayoutRequest {
    LayoutRequest {
        faction_id: "test-empire".into(),
        species,
        center: [1500.0, 1500.0],
        founded: true,
        half_extents: [4.0, 4.0],
        anchors: vec![[1500.0, 1480.0]],
        reserved: vec![Rect {
            min: [1496.0, 1496.0],
            max: [1504.0, 1504.0],
        }],
    }
}

// Returning no offsets, identical layouts, or overlapping candidates breaks this.
#[test]
fn three_families_expand_in_distinct_spaces_without_replacing_the_core() {
    let mut layouts = Vec::new();
    for species in [Species::Organic, Species::Cyborg, Species::Robot] {
        let mut planner = CityPlanner::new("test-empire").unwrap();
        let mut view = request(species);
        let mut positions = Vec::new();
        for _ in 0..5 {
            let result = planner.propose(&view, Ok::<_, anyhow::Error>).unwrap();
            let pose = result
                .site
                .expect("a bounded layout has room for five buildings");
            assert!(pose.center[0] >= 1440.0 && pose.center[0] <= 1560.0);
            let footprint = Rect {
                min: [pose.center[0] - 4.0, pose.center[1] - 4.0],
                max: [pose.center[0] + 4.0, pose.center[1] + 4.0],
            };
            assert!(view.reserved.iter().all(|r| !footprint.overlaps(*r)));
            view.reserved.push(footprint);
            positions.push(pose.center);
        }
        layouts.push(positions);
    }
    assert_ne!(layouts[0], layouts[1]);
    assert_ne!(layouts[1], layouts[2]);
    assert_ne!(layouts[0], layouts[2]);
}

// Ignoring an existing access corridor would allow a building to sever the city.
#[test]
fn future_buildings_respect_existing_entrances_and_routes() {
    let mut planner = CityPlanner::new("test-empire").unwrap();
    let mut view = request(Species::Robot);
    let first = planner
        .propose(&view, Ok::<_, anyhow::Error>)
        .unwrap()
        .site
        .unwrap();
    view.reserved.push(Rect {
        min: [first.center[0] - 6.0, first.center[1] - 32.0],
        max: [first.center[0] + 6.0, first.center[1] + 6.0],
    });
    let second = planner
        .propose(&view, Ok::<_, anyhow::Error>)
        .unwrap()
        .site
        .unwrap();
    assert_ne!(first.center, second.center);
}

// Ignoring the authority rejection, or retrying indefinitely, breaks this.
#[test]
fn no_legal_candidate_returns_bounded_explicit_block_without_effects() {
    let mut planner = CityPlanner::new("test-empire").unwrap();
    let calls = Cell::new(0);
    let decision = planner
        .propose::<()>(&request(Species::Organic), |_| {
            calls.set(calls.get() + 1);
            anyhow::bail!("terrain revision blocks entrance")
        })
        .unwrap();
    assert!(decision.site.is_none());
    assert!(!decision.blockers.is_empty());
    assert!(decision
        .blockers
        .iter()
        .any(|b| b.contains("terrain revision blocks entrance")));
    assert!(calls.get() > 0 && calls.get() <= 128);
    assert_eq!(decision.attempted, calls.get());
}

// A valid edit should affect future proposals; a broken edit must not replace it.
#[test]
fn valid_reload_promotes_and_invalid_or_looping_reload_keeps_last_good() {
    let mut planner = CityPlanner::new("test-empire").unwrap();
    planner
        .stage_policy("return {schema_version=1, offsets=function(_,founded) if not founded then return {{0,0}} end return {{30,0}} end}")
        .unwrap();
    let promoted = planner
        .propose(&request(Species::Robot), Ok::<_, anyhow::Error>)
        .unwrap();
    assert_eq!(promoted.site.unwrap().center, [1530.0, 1500.0]);
    let hash = promoted.policy_version;
    assert!(planner.stage_policy("return broken lua").is_err());
    assert!(planner.stage_policy("while true do end").is_err());
    planner
        .stage_policy("return {schema_version=1, offsets=function() return {{99999,0}} end}")
        .unwrap();
    let fallback = planner
        .propose(&request(Species::Robot), Ok::<_, anyhow::Error>)
        .unwrap();
    assert!(fallback.policy_rejected);
    assert_eq!(fallback.policy_version, hash);
    assert_eq!(fallback.site.unwrap().center, [1530.0, 1500.0]);
}

// A compromised Lua result cannot move the settlement's founding anchor.
#[test]
fn founding_stays_at_authority_selected_center_and_other_factions_are_rejected() {
    let mut planner = CityPlanner::new("test-empire").unwrap();
    let mut view = request(Species::Organic);
    view.founded = false;
    view.reserved.clear();
    let proposal = planner
        .propose(&view, Ok::<_, anyhow::Error>)
        .unwrap()
        .site
        .unwrap();
    assert_eq!(proposal.center, view.center);
    view.faction_id = "another-empire".into();
    let calls = Cell::new(0);
    assert!(planner
        .propose(&view, |p| {
            calls.set(1);
            Ok(p)
        })
        .is_err());
    assert_eq!(calls.get(), 0);
}

use argon_dev::terrain;
#[allow(dead_code)]
#[path = "../src/placement.rs"]
mod placement;

// This invokes the real geometry/rights validator, not an accepting fixture.
#[test]
fn real_terrain_validator_connects_paid_site_candidates_and_rejects_stale_revision() {
    use placement::*;
    let terrain = terrain::Terrain::generate(20261025);
    let center = terrain.spawn();
    for species in [Species::Organic, Species::Cyborg, Species::Robot] {
        let mut planner = CityPlanner::new("test-empire").unwrap();
        let claims = vec![Claim {
            owner: "test-empire".into(),
            center,
            radius: 80.0,
        }];
        let mut obstacles = vec![Bounds {
            min: [center[0] - 4.0, center[1] - 4.0],
            max: [center[0] + 4.0, center[1] + 4.0],
        }];
        let mut view = request(species);
        view.center = center;
        view.anchors = vec![[center[0], center[1] - 18.0]];
        view.reserved = vec![Rect {
            min: obstacles[0].min,
            max: obstacles[0].max,
        }];
        let recipe = FootprintRecipe {
            id: "fixture-generator".into(),
            content_hash: "a".repeat(64),
            half_extents: [4.0, 4.0],
            entrance_width: 2.0,
            entrance_length: 4.0,
            max_slope: 0.3,
            support_depth: 3.0,
            founding_core: false,
        };
        for _ in 0..3 {
            let context = PlacementContext {
                terrain: &terrain,
                world_revision: 7,
                claims: &claims,
                obstacles: &obstacles,
            };
            let decision = planner
                .propose(&view, |pose| {
                    validate_site(
                        &context,
                        &SiteRequest {
                            actor: view.faction_id.clone(),
                            center: pose.center,
                            anchor: pose.anchor,
                        },
                        &recipe,
                    )
                })
                .unwrap();
            eprintln!(
                "species={species:?} count={} attempts={} blockers={:?}",
                obstacles.len(),
                decision.attempted,
                decision.blockers
            );
            let site = decision.site.unwrap_or_else(|| {
                panic!("valid connected site expected: {:?}", decision.blockers)
            });
            assert!(site.ground_height() >= 0.2);
            site.revalidate(&context).unwrap();
            let changed = PlacementContext {
                world_revision: 8,
                ..context
            };
            assert!(site.revalidate(&changed).is_err());
            for bounds in [site.footprint(), site.entrance(), site.approach_bounds()] {
                view.reserved.push(Rect {
                    min: bounds.min,
                    max: bounds.max,
                });
            }
            obstacles.push(site.footprint());
            let entrance = site.entrance();
            view.anchors.push([site.center()[0], entrance.min[1]]);
        }
    }
}

// A reload validated during founding must not hide a looping growth branch.
#[test]
fn founding_reload_cannot_promote_a_broken_growth_branch() {
    let mut planner = CityPlanner::new("test-empire").unwrap();
    planner.stage_policy("return {schema_version=1, offsets=function(_,founded) if founded then return {{30,0}} else return {{0,0}} end end}").unwrap();
    let view = request(Species::Robot);
    let good = planner.propose(&view, Ok::<_, anyhow::Error>).unwrap();
    assert_eq!(good.site.unwrap().center, [1530.0, 1500.0]);
    planner.stage_policy("return {schema_version=1, offsets=function(_,founded) if founded then while true do end else return {{0,0}} end end}").unwrap();
    let mut founding = view.clone();
    founding.founded = false;
    founding.reserved.clear();
    let decision = planner.propose(&founding, Ok::<_, anyhow::Error>).unwrap();
    assert!(
        decision.policy_rejected,
        "all branches must validate before a policy replaces last good"
    );
    let grown = planner.propose(&view, Ok::<_, anyhow::Error>).unwrap();
    assert_eq!(grown.policy_version, good.policy_version);
    assert_eq!(grown.site.unwrap().center, [1530.0, 1500.0]);
}

// Lua finalizers run during VM teardown without instruction hooks. A policy must
// not register one; isolate this regression so a future failure cannot hang CI.
#[test]
fn finalizer_metatables_are_rejected_without_hanging_the_host() {
    use std::{
        process::{Command, Stdio},
        time::{Duration, Instant},
    };
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args(["--ignored", "--exact", "lua_finalizer_probe"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let started = Instant::now();
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            assert!(status.success(), "finalizer registration must be rejected");
            break;
        }
        if started.elapsed() > Duration::from_secs(3) {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("city policy finalizer stalled Lua teardown");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}
#[test]
#[ignore = "isolated child of finalizer_metatables_are_rejected_without_hanging_the_host"]
fn lua_finalizer_probe() {
    let mut planner = CityPlanner::new("test-empire").unwrap();
    assert!(planner.stage_policy("local x=setmetatable({}, {__gc=function() while true do end end}); return {schema_version=1,offsets=function() return {{0,0}} end}").is_err());
}
