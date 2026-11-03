extern crate nat20_core;

use nat20_core::{
    components::{
        ability::Ability,
        d20::{D20CheckKind, D20CheckOutcome},
        exhaustion::{Exhaustion, Hunger},
        health::life_state::LifeState,
        saving_throw::SavingThrowKind,
        skill::Skill,
        species::CreatureSize,
    },
    systems::{
        self,
        exhaustion::{Meal, end_of_day, meal},
        time::RestKind,
    },
    test_utils::scenario::Scenario,
};
use uom::si::length::foot;

/// One pound of food, in quarter pounds.
const POUND: u32 = 4;

fn fighter() -> Scenario {
    let mut scenario = Scenario::new();
    scenario.spawn("fighter", "hero.fighter").level(2).spawn();
    scenario
}

fn exhaustion(scenario: &Scenario) -> Exhaustion {
    systems::exhaustion::exhaustion(&scenario.engine_state.world, scenario.entity("fighter"))
}

fn gain(scenario: &mut Scenario, levels: u8) -> u8 {
    let fighter = scenario.entity("fighter");
    systems::exhaustion::gain(&mut scenario.engine_state, fighter, levels)
}

fn eat(scenario: &mut Scenario, quarter_lb: u32) -> Meal {
    let fighter = scenario.entity("fighter");
    end_of_day(&mut scenario.engine_state, fighter, quarter_lb)
}

fn perception(scenario: &Scenario) -> i32 {
    systems::d20::passive_score(
        &scenario.engine_state,
        scenario.entity("fighter"),
        Skill::Perception,
    )
}

fn speed_ft(scenario: &mut Scenario) -> f32 {
    scenario.probe("fighter").movement_speed().get::<foot>()
}

#[test]
fn each_level_takes_2_from_d20_tests_and_5_feet_from_speed() {
    let mut scenario = fighter();
    let (perception_0, speed_0) = (perception(&scenario), speed_ft(&mut scenario));
    assert_eq!(gain(&mut scenario, 1), 1);
    assert_eq!(perception(&scenario), perception_0 - 2);
    assert_eq!(speed_ft(&mut scenario), speed_0 - 5.0);
    // The levels swap their effect, so the penalties never stack twice.
    assert_eq!(gain(&mut scenario, 2), 3);
    assert_eq!(perception(&scenario), perception_0 - 6);
    assert_eq!(speed_ft(&mut scenario), speed_0 - 15.0);
}

#[test]
fn a_creature_dies_at_level_6() {
    let mut scenario = fighter();
    assert_eq!(gain(&mut scenario, 9), 6);
    assert!(!scenario.probe("fighter").is_alive());
    let fighter = scenario.entity("fighter");
    assert_eq!(
        *scenario
            .engine_state
            .world
            .get::<&LifeState>(fighter)
            .unwrap(),
        LifeState::Dead
    );
}

#[test]
fn a_long_rest_removes_one_level() {
    let mut scenario = fighter();
    let perception_0 = perception(&scenario);
    gain(&mut scenario, 2);
    scenario.probe("fighter").rest(RestKind::Long);
    assert_eq!(exhaustion(&scenario).level, 1);
    assert_eq!(perception(&scenario), perception_0 - 2);
}

#[test]
fn food_needs_follow_the_creature_size() {
    assert_eq!(meal(&CreatureSize::Medium, 0), Meal::Nothing);
    assert_eq!(meal(&CreatureSize::Medium, 1), Meal::LessThanHalf);
    assert_eq!(meal(&CreatureSize::Medium, 2), Meal::AtLeastHalf);
    assert_eq!(meal(&CreatureSize::Medium, POUND), Meal::Full);
    assert_eq!(meal(&CreatureSize::Tiny, 1), Meal::Full);
    assert_eq!(meal(&CreatureSize::Large, POUND), Meal::LessThanHalf);
    assert_eq!(meal(&CreatureSize::Large, 2 * POUND), Meal::AtLeastHalf);
    assert_eq!(meal(&CreatureSize::Gargantuan, 64 * POUND), Meal::Full);
}

#[test]
fn less_than_half_rations_call_for_a_constitution_save() {
    let mut scenario = fighter();
    scenario.probe("fighter").d20_force_outcome(
        D20CheckKind::SavingThrow(SavingThrowKind::Ability(Ability::Constitution)),
        D20CheckOutcome::Success,
    );
    assert_eq!(eat(&mut scenario, 1), Meal::LessThanHalf);
    assert_eq!(exhaustion(&scenario).level, 0);

    scenario.probe("fighter").d20_force_outcome(
        D20CheckKind::SavingThrow(SavingThrowKind::Ability(Ability::Constitution)),
        D20CheckOutcome::Failure,
    );
    eat(&mut scenario, 1);
    assert_eq!(
        exhaustion(&scenario),
        Exhaustion {
            level: 1,
            from_malnutrition: 1
        }
    );
}

#[test]
fn half_rations_or_more_need_no_save() {
    let mut scenario = fighter();
    scenario.probe("fighter").d20_force_outcome(
        D20CheckKind::SavingThrow(SavingThrowKind::Ability(Ability::Constitution)),
        D20CheckOutcome::Failure,
    );
    assert_eq!(eat(&mut scenario, 2), Meal::AtLeastHalf);
    assert_eq!(exhaustion(&scenario).level, 0);
}

#[test]
fn malnutrition_stays_until_a_full_days_food() {
    let mut scenario = fighter();
    scenario.probe("fighter").d20_force_outcome(
        D20CheckKind::SavingThrow(SavingThrowKind::Ability(Ability::Constitution)),
        D20CheckOutcome::Failure,
    );
    eat(&mut scenario, 1);
    scenario.probe("fighter").rest(RestKind::Long);
    assert_eq!(
        exhaustion(&scenario).level,
        1,
        "a Long Rest can't remove it yet"
    );

    assert_eq!(eat(&mut scenario, POUND), Meal::Full);
    assert_eq!(
        exhaustion(&scenario),
        Exhaustion {
            level: 1,
            from_malnutrition: 0
        }
    );
}

#[test]
fn a_full_meal_lets_the_next_long_rest_remove_malnutrition() {
    let mut scenario = fighter();
    scenario.probe("fighter").d20_force_outcome(
        D20CheckKind::SavingThrow(SavingThrowKind::Ability(Ability::Constitution)),
        D20CheckOutcome::Failure,
    );
    eat(&mut scenario, 1);
    eat(&mut scenario, POUND);
    scenario.probe("fighter").rest(RestKind::Long);
    assert_eq!(exhaustion(&scenario).level, 0);
}

#[test]
fn five_days_without_food_bring_a_level_and_each_day_after_another() {
    let mut scenario = fighter();
    for _ in 0..4 {
        assert_eq!(eat(&mut scenario, 0), Meal::Nothing);
    }
    assert_eq!(exhaustion(&scenario).level, 0);
    eat(&mut scenario, 0);
    assert_eq!(exhaustion(&scenario).level, 1);
    eat(&mut scenario, 0);
    assert_eq!(
        exhaustion(&scenario),
        Exhaustion {
            level: 2,
            from_malnutrition: 2
        }
    );
    // Eating anything breaks the count.
    eat(&mut scenario, 2);
    assert_eq!(
        systems::exhaustion::days_without_food(
            &scenario.engine_state.world,
            scenario.entity("fighter")
        ),
        0
    );
}

#[test]
fn a_saved_exhaustion_and_hunger_come_back_as_they_were() {
    let mut scenario = fighter();
    let (perception_0, speed_0) = (perception(&scenario), speed_ft(&mut scenario));
    let fighter = scenario.entity("fighter");
    systems::exhaustion::restore(
        &mut scenario.engine_state,
        fighter,
        Exhaustion {
            level: 2,
            from_malnutrition: 1,
        },
        Hunger {
            days_without_food: 3,
        },
    );
    assert_eq!(
        exhaustion(&scenario),
        Exhaustion {
            level: 2,
            from_malnutrition: 1
        }
    );
    assert_eq!(
        systems::exhaustion::days_without_food(&scenario.engine_state.world, fighter),
        3
    );
    assert_eq!(perception(&scenario), perception_0 - 4);
    assert_eq!(speed_ft(&mut scenario), speed_0 - 10.0);
    // A Long Rest takes the level that isn't from malnutrition, and no more.
    assert_eq!(
        systems::exhaustion::reduce_after_long_rest(&mut scenario.engine_state, fighter),
        1
    );
    assert_eq!(
        systems::exhaustion::reduce_after_long_rest(&mut scenario.engine_state, fighter),
        1
    );
}

#[test]
fn restored_malnutrition_never_exceeds_the_level() {
    let mut scenario = fighter();
    let fighter = scenario.entity("fighter");
    systems::exhaustion::restore(
        &mut scenario.engine_state,
        fighter,
        Exhaustion {
            level: 1,
            from_malnutrition: 3,
        },
        Hunger::default(),
    );
    assert_eq!(
        exhaustion(&scenario),
        Exhaustion {
            level: 1,
            from_malnutrition: 1
        }
    );
}
