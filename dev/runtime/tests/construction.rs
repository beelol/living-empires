#[path = "../src/construction.rs"]
mod construction;

use construction::{BuildRequest, Construction, Recipe, Status};

fn request() -> BuildRequest {
    BuildRequest {
        actor: "faction-a".into(),
        request_id: "build-1".into(),
        recipe: Recipe {
            id: "core".into(),
            content_hash: "a".repeat(64),
            metal: 101,
            work_ticks: 10,
        },
    }
}

#[test]
fn retries_and_rejected_requests_never_charge_twice() {
    let mut state = Construction::new("faction-a", 150);
    let id = state.start(request()).unwrap();
    assert_eq!(state.metal(), 49);
    assert_eq!(state.start(request()).unwrap(), id);
    let before = serde_json::to_string(&state).unwrap();
    let mut changed = request();
    changed.recipe.metal = 100;
    assert!(state.start(changed).is_err());
    let mut other = request();
    other.request_id = "build-2".into();
    assert!(state.start(other).is_err());
    let mut hostile = request();
    hostile.actor = "faction-b".into();
    assert!(state.start(hostile).is_err());
    assert_eq!(serde_json::to_string(&state).unwrap(), before);
}

#[test]
fn paused_work_and_cancel_refund_only_unconsumed_cost() {
    let mut state = Construction::new("faction-a", 150);
    let id = state.start(request()).unwrap();
    state.advance(&id, 4, false).unwrap();
    assert_eq!(state.order(&id).unwrap().completed_ticks, 0);
    state.advance(&id, 4, true).unwrap();
    // Ceiling(101 * 4 / 10) = 41 consumed, 60 refundable.
    assert_eq!(state.cancel("faction-a", &id).unwrap(), 60);
    assert_eq!(state.metal(), 109);
    assert_eq!(state.cancel("faction-a", &id).unwrap(), 0);
    state.advance(&id, 100, true).unwrap();
    assert_eq!(state.order(&id).unwrap().status, Status::Cancelled);
    assert_eq!(state.start(request()).unwrap(), id);
    assert_eq!(state.metal(), 109);
}

#[test]
fn complete_work_survives_roundtrip_without_refund_or_new_entity() {
    let mut state = Construction::new("faction-a", 150);
    let id = state.start(request()).unwrap();
    state.advance(&id, u64::MAX, true).unwrap();
    let encoded = serde_json::to_vec(&state).unwrap();
    let mut restored: Construction = serde_json::from_slice(&encoded).unwrap();
    assert_eq!(restored.start(request()).unwrap(), id);
    assert!(restored.cancel("faction-a", &id).is_err());
    assert_eq!(restored.order(&id).unwrap().status, Status::Complete);
    assert_eq!(
        restored.order(&id).unwrap().request.recipe.content_hash,
        "a".repeat(64)
    );
    assert_eq!(restored.metal(), 49);
}

#[test]
fn invalid_recipe_and_unauthorized_cancel_leave_state_unchanged() {
    let mut state = Construction::new("faction-a", u64::MAX);
    for field in 0..4 {
        let mut bad = request();
        match field {
            0 => bad.recipe.work_ticks = 0,
            1 => bad.recipe.content_hash = "unreviewed".into(),
            2 => bad.request_id.clear(),
            _ => bad.recipe.id.clear(),
        }
        assert!(state.start(bad).is_err());
        assert_eq!(state.metal(), u64::MAX);
    }
    let mut max_cost = request();
    max_cost.recipe.metal = u64::MAX;
    let id = state.start(max_cost).unwrap();
    assert!(state.cancel("faction-b", &id).is_err());
    state.advance(&id, 1, true).unwrap();
    let refund = state.cancel("faction-a", &id).unwrap();
    assert_eq!(refund, u64::MAX - (u64::MAX as u128).div_ceil(10) as u64);
    assert_eq!(state.metal(), refund);
}
