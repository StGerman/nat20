extern crate nat20_core;

use nat20_core::{
    components::{
        ability::{Ability, AbilityScoreMap},
        damage::DamageType,
        dice::DieSize,
        health::hit_point_dice::{HitPointDice, HitPointDiceError},
    },
    systems::{self, health::SpendHitPointDieError, time::RestKind},
    test_utils::scenario::Scenario,
};

fn fighter_scenario(level: u8) -> Scenario {
    let mut scenario = Scenario::new();
    scenario
        .spawn("fighter", "hero.fighter")
        .level(level)
        .spawn();
    scenario
}

fn hit_point_dice(scenario: &mut Scenario) -> HitPointDice {
    let mut dice = HitPointDice::new();
    scenario
        .probe("fighter")
        .component::<HitPointDice>(|d| dice = d.clone());
    dice
}

fn start_short_rest(scenario: &mut Scenario) {
    let fighter = scenario.entity("fighter");
    systems::time::start_rest(&mut scenario.engine_state, vec![fighter], &RestKind::Short).unwrap();
}

fn finish_rest(scenario: &mut Scenario) {
    let fighter = scenario.entity("fighter");
    systems::time::finish_rest(&mut scenario.engine_state, vec![fighter]).unwrap();
}

fn spend(scenario: &mut Scenario, die: DieSize) -> Result<i32, SpendHitPointDieError> {
    let fighter = scenario.entity("fighter");
    systems::health::spend_hit_point_die(&mut scenario.engine_state, fighter, die)
        .map(|result| result.total())
}

#[test]
fn one_hit_point_die_per_class_level() {
    let mut scenario = fighter_scenario(3);
    let dice = hit_point_dice(&mut scenario);
    assert_eq!(dice.max(DieSize::D10), 3);
    assert_eq!(dice.remaining(DieSize::D10), 3);
}

#[test]
fn spending_a_die_heals_the_roll_plus_constitution() {
    let mut scenario = fighter_scenario(3);
    scenario.probe("fighter").damage(20, DamageType::Slashing);
    let hp_before = scenario.probe("fighter").hp();

    let mut constitution = 0;
    scenario
        .probe("fighter")
        .component::<AbilityScoreMap>(|scores| {
            constitution = scores
                .get(&Ability::Constitution)
                .ability_modifier()
                .total()
        });

    start_short_rest(&mut scenario);
    let total = spend(&mut scenario, DieSize::D10).unwrap();
    assert!((1 + constitution..=10 + constitution).contains(&total));

    let healed = scenario.probe("fighter").hp() - hp_before;
    assert_eq!(healed, total.max(1) as u32);
    assert_eq!(hit_point_dice(&mut scenario).remaining(DieSize::D10), 2);
}

#[test]
fn a_short_rest_heals_only_through_hit_point_dice() {
    let mut scenario = fighter_scenario(3);
    scenario.probe("fighter").damage(20, DamageType::Slashing);
    let hp_before = scenario.probe("fighter").hp();

    scenario.probe("fighter").rest(RestKind::Short);

    assert_eq!(scenario.probe("fighter").hp(), hp_before);
    assert_eq!(hit_point_dice(&mut scenario).remaining(DieSize::D10), 3);
}

#[test]
fn hit_point_dice_can_only_be_spent_during_a_short_rest() {
    let mut scenario = fighter_scenario(1);
    assert_eq!(
        spend(&mut scenario, DieSize::D10),
        Err(SpendHitPointDieError::NotShortResting)
    );

    start_short_rest(&mut scenario);
    finish_rest(&mut scenario);
    assert_eq!(
        spend(&mut scenario, DieSize::D10),
        Err(SpendHitPointDieError::NotShortResting)
    );
}

#[test]
fn spending_fails_once_the_pool_is_empty() {
    let mut scenario = fighter_scenario(1);
    start_short_rest(&mut scenario);
    assert!(spend(&mut scenario, DieSize::D10).is_ok());
    assert_eq!(
        spend(&mut scenario, DieSize::D10),
        Err(SpendHitPointDieError::HitPointDice(
            HitPointDiceError::NoneRemaining(DieSize::D10)
        ))
    );
    assert_eq!(
        spend(&mut scenario, DieSize::D6),
        Err(SpendHitPointDieError::HitPointDice(
            HitPointDiceError::NoSuchDie(DieSize::D6)
        ))
    );
}

#[test]
fn a_long_rest_restores_all_hit_point_dice() {
    let mut scenario = fighter_scenario(2);
    start_short_rest(&mut scenario);
    spend(&mut scenario, DieSize::D10).unwrap();
    spend(&mut scenario, DieSize::D10).unwrap();
    finish_rest(&mut scenario);
    assert_eq!(hit_point_dice(&mut scenario).remaining(DieSize::D10), 0);

    scenario.probe("fighter").rest(RestKind::Long);
    assert_eq!(hit_point_dice(&mut scenario).remaining(DieSize::D10), 2);
}

#[test]
fn spending_is_refused_once_combat_starts() {
    let mut scenario = fighter_scenario(1);
    scenario
        .spawn("goblin", "monster.goblin_warrior")
        .level(1)
        .position([1.0, 0.0, 0.0], false)
        .spawn();
    start_short_rest(&mut scenario);
    scenario
        .encounter()
        .initiative_order(vec!["fighter", "goblin"])
        .build();

    assert_eq!(
        spend(&mut scenario, DieSize::D10),
        Err(SpendHitPointDieError::InCombat)
    );
    assert_eq!(hit_point_dice(&mut scenario).remaining(DieSize::D10), 1);
}

#[test]
fn a_resting_monster_has_no_hit_point_dice_to_spend() {
    let mut scenario = Scenario::new();
    scenario
        .spawn("goblin", "monster.goblin_warrior")
        .level(1)
        .spawn();
    let goblin = scenario.entity("goblin");
    systems::time::start_rest(&mut scenario.engine_state, vec![goblin], &RestKind::Short).unwrap();

    assert_eq!(
        systems::health::spend_hit_point_die(&mut scenario.engine_state, goblin, DieSize::D6),
        Err(SpendHitPointDieError::HitPointDice(
            HitPointDiceError::NoSuchDie(DieSize::D6)
        ))
    );
}
