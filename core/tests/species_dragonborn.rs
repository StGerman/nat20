extern crate nat20_core;

use nat20_core::{
    components::{
        ability::{Ability, AbilityScoreMap},
        d20::{D20CheckKind, D20CheckOutcome},
        damage::{DamageComponent, DamageType},
        dice::{DiceSet, DieSize},
        modifier::{ModifierMap, ModifierSource},
        saving_throw::SavingThrowKind,
    },
    systems::time::RestKind,
    test_utils::scenario::{Operator, Scenario},
};

const USES: &str = "resource.dragonborn.breath_weapon";
const RED_CONE: &str = "action.dragonborn.breath_weapon.red.cone";
const RED_LINE: &str = "action.dragonborn.breath_weapon.red.line";
const DEX_SAVE: D20CheckKind =
    D20CheckKind::SavingThrow(SavingThrowKind::Ability(Ability::Dexterity));

/// The test Wizard is a red Dragonborn. Challenge rating 2 gives the goblin enough HP to
/// survive a 1d10.
fn breath_scenario(goblin_save: D20CheckOutcome) -> Scenario {
    let mut scenario = Scenario::new();
    scenario
        .spawn("wizard", "hero.wizard")
        .level(2)
        .position([0.0, 0.0, 0.0], true)
        .spawn();
    scenario
        .spawn("goblin", "monster.goblin_warrior")
        .level(2)
        .position([1.0, 0.0, 0.0], true)
        .spawn();
    scenario
        .probe("goblin")
        .d20_force_outcome(DEX_SAVE, goblin_save);
    scenario
}

fn breathe(scenario: &mut Scenario, action: &str) {
    scenario
        .act("wizard", action)
        .target_point([1.0, 0.0, 0.0])
        .perform();
}

fn constitution_modifier(scenario: &mut Scenario) -> i32 {
    let mut modifier = 0;
    scenario
        .probe("wizard")
        .component::<AbilityScoreMap>(|scores| {
            modifier = scores.ability_modifier(&Ability::Constitution).total();
        });
    modifier
}

fn assert_one_fire_roll(scenario: &Scenario) {
    scenario
        .event_filter()
        .actor("wizard")
        .damage_roll(DamageComponent::new(
            ModifierMap::from(ModifierSource::Base, DiceSet::new(1, DieSize::D10)),
            DamageType::Fire,
        ))
        .assert_event_count(1);
}

#[test]
fn every_dragonborn_ancestry_has_a_cone_and_a_line() {
    let mut scenario = Scenario::new();
    for (handle, hero, ancestry) in [
        ("wizard", "hero.wizard", "red"),
        ("fighter", "hero.fighter", "white"),
        ("warlock", "hero.warlock", "black"),
    ] {
        scenario.spawn(handle, hero).level(2).spawn();
        scenario
            .probe(handle)
            .assert_has_action(format!("action.dragonborn.breath_weapon.{ancestry}.cone").as_str())
            .assert_has_action(format!("action.dragonborn.breath_weapon.{ancestry}.line").as_str())
            .assert_resource(USES, Operator::Equal(2));
    }
}

#[test]
fn the_save_dc_is_8_plus_constitution_plus_proficiency() {
    for action in [RED_CONE, RED_LINE] {
        let mut scenario = breath_scenario(D20CheckOutcome::Failure);
        let dc = 8 + constitution_modifier(&mut scenario) + 2;
        breathe(&mut scenario, action);
        scenario
            .event_filter()
            .actor("goblin")
            .d20_dc(DEX_SAVE, dc)
            .assert_event_count(1);
        assert_one_fire_roll(&scenario);
    }
}

fn breath_damage(outcome: D20CheckOutcome) -> u32 {
    let mut scenario = breath_scenario(outcome);
    let before = scenario.probe("goblin").hp();
    breathe(&mut scenario, RED_CONE);
    before - scenario.probe("goblin").hp()
}

#[test]
fn a_failed_save_takes_1d10_and_a_successful_one_half() {
    for _ in 0..10 {
        let full = breath_damage(D20CheckOutcome::Failure);
        assert!((1..=10).contains(&full), "{full}");
        let half = breath_damage(D20CheckOutcome::Success);
        assert!((0..=5).contains(&half), "{half}");
    }
}

#[test]
fn two_uses_spend_the_action_and_return_on_a_long_rest() {
    let mut scenario = breath_scenario(D20CheckOutcome::Success);
    breathe(&mut scenario, RED_CONE);
    scenario
        .probe("wizard")
        .assert_resource("resource.action", Operator::Equal(0))
        .assert_resource(USES, Operator::Equal(1))
        .end_turn();
    breathe(&mut scenario, RED_LINE);
    scenario
        .probe("wizard")
        .assert_resource(USES, Operator::Equal(0))
        .end_turn()
        .assert_action_unavailable(RED_CONE)
        .assert_action_unavailable(RED_LINE);
    scenario
        .act("wizard", RED_CONE)
        .target_point([1.0, 0.0, 0.0])
        .assert_perform_fails();

    scenario
        .probe("wizard")
        .rest(RestKind::Short)
        .assert_resource(USES, Operator::Equal(0))
        .rest(RestKind::Long)
        .assert_resource(USES, Operator::Equal(2))
        .assert_action_available(RED_CONE);
}
