//! SRD 5.2.1, Actions > Dash: extra movement equal to the Speed for the current turn, and each
//! Dash adds it again (a Fighter's Action Surge spent on a second Dash).

extern crate nat20_core;

use nat20_core::{components::time::TimeMode, systems, test_utils::scenario::Scenario};
use uom::si::length::foot;

const DASH: &str = "effect.dash";

fn scenario() -> Scenario {
    let mut scenario = Scenario::new();
    scenario
        .spawn("fighter", "hero.fighter")
        .level(2)
        .time_mode(TimeMode::TurnBased { encounter_id: None })
        .spawn();
    scenario
}

fn total_speed_ft(scenario: &Scenario) -> f32 {
    systems::movement::speed(&scenario.engine_state, scenario.entity("fighter"))
        .total_speed()
        .get::<foot>()
}

fn assert_feet(actual: f32, expected: f32) {
    assert!(
        (actual - expected).abs() < 0.01,
        "expected {expected} feet, got {actual}"
    );
}

#[test]
fn one_dash_doubles_the_movement() {
    let mut scenario = scenario();
    assert_feet(total_speed_ft(&scenario), 30.0);
    scenario.probe("fighter").apply_effect(DASH);
    assert_feet(total_speed_ft(&scenario), 60.0);
}

#[test]
fn a_second_dash_in_the_same_turn_adds_the_speed_again() {
    let mut scenario = scenario();
    scenario
        .probe("fighter")
        .apply_effect(DASH)
        .apply_effect(DASH);
    assert_feet(total_speed_ft(&scenario), 90.0);
}

#[test]
fn dash_action_surge_dash_triples_the_movement_until_the_next_turn() {
    let mut scenario = scenario();
    scenario.probe("fighter").act("action.dash").perform();
    scenario
        .probe("fighter")
        .act("action.fighter.action_surge")
        .perform();
    scenario.probe("fighter").act("action.dash").perform();
    assert_feet(total_speed_ft(&scenario), 90.0);

    scenario.probe("fighter").end_turn();
    assert_feet(total_speed_ft(&scenario), 30.0);
}
