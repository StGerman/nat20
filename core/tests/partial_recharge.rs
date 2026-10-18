extern crate nat20_core;

use nat20_core::{
    components::resource::{ResourceAmount, ResourceMap},
    systems::{self, time::RestKind},
    test_utils::scenario::{Operator, Scenario},
};

fn hero(class: &str, level: u8) -> Scenario {
    let mut scenario = Scenario::new();
    scenario
        .spawn("hero", format!("hero.{class}"))
        .level(level)
        .spawn();
    scenario
}

fn spend(scenario: &mut Scenario, resource: &str, uses: u8) {
    let hero = scenario.entity("hero");
    systems::helpers::get_component_mut::<ResourceMap>(&mut scenario.engine_state.world, hero)
        .spend(&resource.parse().unwrap(), &ResourceAmount::Flat(uses))
        .unwrap();
}

#[test]
fn second_wind_regains_one_use_on_a_short_rest_and_all_on_a_long_rest() {
    const SECOND_WIND: &str = "resource.fighter.second_wind";
    let mut scenario = hero("fighter", 5);
    scenario
        .probe("hero")
        .assert_resource(SECOND_WIND, Operator::Equal(3));

    spend(&mut scenario, SECOND_WIND, 3);
    scenario
        .probe("hero")
        .assert_resource(SECOND_WIND, Operator::Equal(0))
        .rest(RestKind::Short)
        .assert_resource(SECOND_WIND, Operator::Equal(1))
        .rest(RestKind::Short)
        .assert_resource(SECOND_WIND, Operator::Equal(2))
        .rest(RestKind::Long)
        .assert_resource(SECOND_WIND, Operator::Equal(3));
}

#[test]
fn a_short_rest_never_goes_past_the_maximum() {
    const SECOND_WIND: &str = "resource.fighter.second_wind";
    let mut scenario = hero("fighter", 5);
    scenario
        .probe("hero")
        .rest(RestKind::Short)
        .assert_resource(SECOND_WIND, Operator::Equal(3));
}

#[test]
fn rage_regains_one_use_on_a_short_rest() {
    const RAGE: &str = "resource.barbarian.rage";
    let mut scenario = hero("barbarian", 2);
    let max = scenario.probe("hero").resource(RAGE);
    assert!(max >= 2, "{max}");

    spend(&mut scenario, RAGE, 2);
    scenario
        .probe("hero")
        .assert_resource(RAGE, Operator::Equal(max - 2))
        .rest(RestKind::Short)
        .assert_resource(RAGE, Operator::Equal(max - 1))
        .rest(RestKind::Long)
        .assert_resource(RAGE, Operator::Equal(max));
}
