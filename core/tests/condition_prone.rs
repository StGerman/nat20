//! SRD 5.2.1, Conditions > Prone, with standing up at the start of the turn
//! (nat20-table Milestone 5 spec D8).

extern crate nat20_core;

use nat20_core::{
    components::{
        d20::{AdvantageType, RollMode},
        id::EffectId,
        items::equipment::slots::EquipmentSlot,
        modifier::ModifierSource,
        time::TimeMode,
    },
    systems,
    test_utils::scenario::Scenario,
};
use uom::si::length::foot;

const PRONE: &str = "effect.condition.prone";
const PRONE_TARGET: &str = "effect.condition.prone.target";

/// A Fighter with a goblin beside it and one 30 feet away.
fn scenario() -> Scenario {
    let mut scenario = Scenario::new();
    scenario
        .spawn("fighter", "hero.fighter")
        .level(1)
        .time_mode(TimeMode::TurnBased { encounter_id: None })
        .spawn();
    scenario
        .spawn("near", "monster.goblin_warrior")
        .level(1)
        .position([1.0, 0.0, 0.0], false)
        .spawn();
    scenario
        .spawn("far", "monster.goblin_warrior")
        .level(1)
        .position([10.0, 0.0, 0.0], false)
        .spawn();
    scenario
}

fn attack_roll_mode(scenario: &mut Scenario, attacker: &str, target: &str) -> RollMode {
    scenario
        .probe(attacker)
        .preview_attack_roll(target, "action.melee_attack", EquipmentSlot::MeleeMainHand)
        .advantage_tracker()
        .roll_mode()
}

fn remaining_movement_ft(scenario: &Scenario, handle: &str) -> f32 {
    systems::movement::speed(&scenario.engine_state, scenario.entity(handle))
        .remaining_movement()
        .get::<foot>()
}

fn assert_feet(actual: f32, expected: f32) {
    assert!(
        (actual - expected).abs() < 0.01,
        "expected {expected} feet, got {actual}"
    );
}

#[test]
fn attacks_against_a_prone_creature_depend_on_distance() {
    let mut scenario = scenario();
    scenario.probe("fighter").apply_effect(PRONE);

    let near = scenario.probe("near").preview_attack_roll(
        "fighter",
        "action.melee_attack",
        EquipmentSlot::MeleeMainHand,
    );
    assert_eq!(
        near.advantage_tracker()
            .summary()
            .get(&ModifierSource::Effect(EffectId::from(PRONE_TARGET))),
        Some(&AdvantageType::Advantage)
    );
    assert_eq!(near.advantage_tracker().roll_mode(), RollMode::Advantage);

    assert_eq!(
        attack_roll_mode(&mut scenario, "far", "fighter"),
        RollMode::Disadvantage
    );
}

#[test]
fn a_prone_creature_has_disadvantage_on_its_attacks() {
    let mut scenario = scenario();
    assert_eq!(
        attack_roll_mode(&mut scenario, "fighter", "near"),
        RollMode::Normal
    );
    scenario.probe("fighter").apply_effect(PRONE);
    assert_eq!(
        attack_roll_mode(&mut scenario, "fighter", "near"),
        RollMode::Disadvantage
    );
}

#[test]
fn a_prone_attacker_against_a_prone_target_beside_it_rolls_straight() {
    let mut scenario = scenario();
    scenario.probe("fighter").apply_effect(PRONE);
    scenario.probe("near").apply_effect(PRONE);
    assert_eq!(
        attack_roll_mode(&mut scenario, "near", "fighter"),
        RollMode::Normal
    );
}

#[test]
fn standing_up_spends_half_the_speed() {
    let mut scenario = scenario();
    let speed = scenario.probe("fighter").movement_speed().get::<foot>();
    assert_feet(speed, 30.0);

    scenario.probe("fighter").apply_effect(PRONE);
    scenario.probe("fighter").end_turn();

    scenario.probe("fighter").assert_no_effect(PRONE);
    assert_feet(remaining_movement_ft(&scenario, "fighter"), 15.0);
}

#[test]
fn standing_up_then_dashing_leaves_45_feet() {
    let mut scenario = scenario();
    scenario.probe("fighter").apply_effect(PRONE);
    scenario.probe("fighter").end_turn();
    scenario.probe("fighter").apply_effect("effect.dash");

    assert_feet(remaining_movement_ft(&scenario, "fighter"), 45.0);
}

#[test]
fn a_creature_with_speed_0_stays_prone() {
    let mut scenario = scenario();
    scenario
        .probe("fighter")
        .apply_effect("effect.condition.paralyzed")
        .apply_effect(PRONE);
    scenario.probe("fighter").end_turn();

    scenario.probe("fighter").assert_effect(PRONE);
}
