extern crate nat20_core;

use nat20_core::{
    components::{damage::DamageType, time::TimeStep},
    systems::{
        self,
        time::{LONG_REST_WAIT_SECONDS, RestError, RestKind},
    },
    test_utils::scenario::Scenario,
};

fn fighter() -> Scenario {
    let mut scenario = Scenario::new();
    scenario.spawn("fighter", "hero.fighter").level(2).spawn();
    scenario
}

fn start_long_rest(scenario: &mut Scenario) -> Result<(), RestError> {
    let fighter = scenario.entity("fighter");
    systems::time::start_rest(&mut scenario.engine_state, vec![fighter], &RestKind::Long)
}

fn pass_hours(scenario: &mut Scenario, hours: f32) {
    let fighter = scenario.entity("fighter");
    systems::time::advance_time(
        &mut scenario.engine_state,
        fighter,
        TimeStep::RealTime {
            delta_seconds: hours * 60.0 * 60.0,
        },
    );
}

fn wait_left(scenario: &Scenario) -> f32 {
    systems::time::long_rest_wait_left(&scenario.engine_state.world, scenario.entity("fighter"))
}

#[test]
fn a_long_rest_needs_at_least_one_hit_point() {
    let mut scenario = fighter();
    scenario.probe("fighter").kill();
    assert!(matches!(
        start_long_rest(&mut scenario),
        Err(RestError::NoHitPoints { entities }) if entities == vec![scenario.entity("fighter")]
    ));
}

#[test]
fn a_short_rest_needs_no_hit_points() {
    let mut scenario = fighter();
    scenario.probe("fighter").kill();
    let fighter = scenario.entity("fighter");
    assert!(
        systems::time::start_rest(&mut scenario.engine_state, vec![fighter], &RestKind::Short)
            .is_ok()
    );
}

#[test]
fn another_long_rest_waits_16_hours_after_the_last_one_ends() {
    let mut scenario = fighter();
    assert_eq!(wait_left(&scenario), 0.0);
    scenario.probe("fighter").rest(RestKind::Long);
    assert_eq!(wait_left(&scenario), LONG_REST_WAIT_SECONDS);

    pass_hours(&mut scenario, 15.0);
    assert_eq!(wait_left(&scenario), 60.0 * 60.0);
    assert!(matches!(
        start_long_rest(&mut scenario),
        Err(RestError::TooSoon { entities }) if entities == vec![scenario.entity("fighter")]
    ));

    pass_hours(&mut scenario, 1.0);
    assert_eq!(wait_left(&scenario), 0.0);
    assert!(start_long_rest(&mut scenario).is_ok());
}

#[test]
fn a_short_rest_does_not_start_the_16_hour_wait() {
    let mut scenario = fighter();
    scenario.probe("fighter").rest(RestKind::Short);
    assert_eq!(wait_left(&scenario), 0.0);
    assert!(start_long_rest(&mut scenario).is_ok());
}

#[test]
fn a_long_rest_still_heals_a_wounded_creature() {
    let mut scenario = fighter();
    let max = scenario.probe("fighter").max_hp();
    scenario
        .probe("fighter")
        .damage(5, DamageType::Slashing)
        .rest(RestKind::Long);
    assert_eq!(scenario.probe("fighter").hp(), max);
}
