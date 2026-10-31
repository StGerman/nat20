//! SRD 5.2.1, Actions > Dodge: until the start of your next turn, attack rolls against you have
//! Disadvantage and you make Dexterity saving throws with Advantage, unless you are Incapacitated
//! or your Speed is 0.

extern crate nat20_core;

use nat20_core::{
    components::{
        ability::Ability,
        d20::{AdvantageType, D20CheckKind, RollMode},
        id::EffectId,
        items::equipment::slots::EquipmentSlot,
        modifier::ModifierSource,
        saving_throw::SavingThrowKind,
        time::TimeMode,
    },
    test_utils::scenario::Scenario,
};

const DODGE: &str = "effect.dodge";

/// A Fighter with a goblin beside it.
fn scenario() -> Scenario {
    let mut scenario = Scenario::new();
    scenario
        .spawn("fighter", "hero.fighter")
        .level(2)
        .time_mode(TimeMode::TurnBased { encounter_id: None })
        .spawn();
    scenario
        .spawn("goblin", "monster.goblin_warrior")
        .level(1)
        .position([1.0, 0.0, 0.0], false)
        .spawn();
    scenario
}

fn goblin_attack_mode(scenario: &mut Scenario) -> RollMode {
    scenario
        .probe("goblin")
        .preview_attack_roll(
            "fighter",
            "action.melee_attack",
            EquipmentSlot::MeleeMainHand,
        )
        .advantage_tracker()
        .roll_mode()
}

fn dexterity_save() -> D20CheckKind {
    D20CheckKind::SavingThrow(SavingThrowKind::Ability(Ability::Dexterity))
}

fn dodge_source() -> ModifierSource {
    ModifierSource::Effect(EffectId::from(DODGE))
}

#[test]
fn dodging_imposes_disadvantage_on_attacks_and_grants_advantage_on_dexterity_saves() {
    let mut scenario = scenario();
    assert_eq!(goblin_attack_mode(&mut scenario), RollMode::Normal);

    scenario
        .probe("fighter")
        .assert_action_available("action.dodge")
        .act("action.dodge")
        .perform();

    scenario
        .probe("fighter")
        .assert_effect(DODGE)
        .assert_d20_advantage(&dexterity_save(), &dodge_source(), AdvantageType::Advantage);
    assert_eq!(goblin_attack_mode(&mut scenario), RollMode::Disadvantage);
}

#[test]
fn dodge_lasts_until_the_start_of_the_next_turn() {
    let mut scenario = scenario();
    scenario.probe("fighter").act("action.dodge").perform();
    scenario.probe("fighter").end_turn().assert_no_effect(DODGE);
    assert_eq!(goblin_attack_mode(&mut scenario), RollMode::Normal);
}

#[test]
fn an_incapacitated_creature_loses_the_benefits_of_dodging() {
    let mut scenario = scenario();
    scenario.probe("fighter").act("action.dodge").perform();
    scenario
        .probe("fighter")
        .apply_effect("effect.condition.incapacitated")
        .assert_d20_no_advantage(&dexterity_save(), &dodge_source(), AdvantageType::Advantage);
    let tracker = scenario
        .probe("goblin")
        .preview_attack_roll(
            "fighter",
            "action.melee_attack",
            EquipmentSlot::MeleeMainHand,
        )
        .advantage_tracker()
        .summary()
        .get(&dodge_source())
        .cloned();
    assert_eq!(tracker, None);
}

#[test]
fn a_creature_whose_speed_is_0_loses_the_benefits_of_dodging() {
    // Paralyzed sets Speed to 0 (and also incapacitates; no registry effect stops movement alone)
    let mut scenario = scenario();
    scenario.probe("fighter").act("action.dodge").perform();
    scenario
        .probe("fighter")
        .apply_effect("effect.condition.paralyzed")
        .assert_effect(DODGE)
        .assert_d20_no_advantage(&dexterity_save(), &dodge_source(), AdvantageType::Advantage);
    let dodge = scenario
        .probe("goblin")
        .preview_attack_roll(
            "fighter",
            "action.melee_attack",
            EquipmentSlot::MeleeMainHand,
        )
        .advantage_tracker()
        .summary()
        .get(&dodge_source())
        .cloned();
    assert_eq!(dodge, None);
}
