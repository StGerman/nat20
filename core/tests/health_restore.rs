extern crate nat20_core;

use nat20_core::{
    components::health::{hit_points::HitPoints, life_state::LifeState},
    systems,
    test_utils::scenario::Scenario,
};

fn captain() -> Scenario {
    let mut scenario = Scenario::new();
    scenario.spawn("guard", "stat_block.guard_captain").spawn();
    scenario
}

fn hit_points(scenario: &mut Scenario) -> (u32, u32) {
    let mut hp = (0, 0);
    scenario
        .probe("guard")
        .component::<HitPoints>(|h| hp = (h.current(), h.max()));
    hp
}

#[test]
fn a_saved_hit_point_total_comes_back_between_1_and_the_maximum() {
    let mut scenario = captain();
    let guard = scenario.entity("guard");
    let (_, max) = hit_points(&mut scenario);
    assert!(max > 3);

    systems::health::restore(&mut scenario.engine_state.world, guard, 3);
    assert_eq!(hit_points(&mut scenario), (3, max));

    systems::health::restore(&mut scenario.engine_state.world, guard, max + 10);
    assert_eq!(hit_points(&mut scenario).0, max, "never above the maximum");

    systems::health::restore(&mut scenario.engine_state.world, guard, 0);
    assert_eq!(
        hit_points(&mut scenario).0,
        1,
        "a restored creature is alive"
    );
    let life = *scenario
        .engine_state
        .world
        .get::<&LifeState>(guard)
        .unwrap();
    assert_eq!(life, LifeState::Normal);
}

#[test]
fn a_restored_creature_wakes_from_any_life_state() {
    let mut scenario = captain();
    let guard = scenario.entity("guard");
    *scenario
        .engine_state
        .world
        .get::<&mut LifeState>(guard)
        .unwrap() = LifeState::Stable;

    systems::health::restore(&mut scenario.engine_state.world, guard, 5);
    assert_eq!(hit_points(&mut scenario).0, 5);
    let life = *scenario
        .engine_state
        .world
        .get::<&LifeState>(guard)
        .unwrap();
    assert_eq!(life, LifeState::Normal, "not left Stable at 5 HP");
}
