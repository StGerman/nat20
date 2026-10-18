extern crate nat20_core;

use nat20_core::components::{
    ability::Ability,
    d20::{D20Check, D20CheckKind, D20CheckOutcome, D20CheckResult},
    damage::AttackSource,
    modifier::{Modifiable, ModifierSource},
    proficiency::{Proficiency, ProficiencyLevel},
    saving_throw::SavingThrowKind,
    skill::Skill,
};

/// A check with a flat +5, e.g. Athletics for a character with 16 Strength and proficiency
fn check_plus_five(kind: D20CheckKind) -> D20Check {
    let mut check = D20Check::new(
        kind,
        Proficiency::new(ProficiencyLevel::None, ModifierSource::None),
    );
    check
        .modifiers_mut()
        .add_modifier(ModifierSource::Custom("Test".to_string()), 5);
    check
}

fn roll_until(check: &D20Check, natural: u8) -> D20CheckResult {
    let mut result = check.roll();
    while result.selected_roll != natural {
        result = check.roll();
    }
    result
}

#[test]
fn natural_20_does_not_pass_a_skill_check_above_the_total() {
    let check = check_plus_five(D20CheckKind::Skill(Skill::Athletics));
    let result = roll_until(&check, 20);
    assert_eq!(result.total(), 25);
    assert_eq!(result.outcome, None);
    assert!(!result.is_success_vs(30));
}

#[test]
fn natural_1_does_not_fail_a_skill_check_below_the_total() {
    let check = check_plus_five(D20CheckKind::Skill(Skill::Athletics));
    let result = roll_until(&check, 1);
    assert_eq!(result.total(), 6);
    assert_eq!(result.outcome, None);
    assert!(result.is_success_vs(5));
}

#[test]
fn natural_20_and_1_do_not_decide_a_saving_throw() {
    for kind in [
        SavingThrowKind::Ability(Ability::Strength),
        SavingThrowKind::Concentration,
    ] {
        let check = check_plus_five(D20CheckKind::SavingThrow(kind));
        assert!(!roll_until(&check, 20).is_success_vs(30));
        assert!(roll_until(&check, 1).is_success_vs(5));
    }
}

#[test]
fn natural_20_and_1_still_decide_an_attack_roll() {
    let check = check_plus_five(D20CheckKind::AttackRoll(AttackSource::Spell));

    let hit = roll_until(&check, 20);
    assert_eq!(hit.outcome, Some(D20CheckOutcome::CriticalSuccess));
    assert!(hit.is_success_vs(30));

    let miss = roll_until(&check, 1);
    assert_eq!(miss.outcome, Some(D20CheckOutcome::CriticalFailure));
    assert!(!miss.is_success_vs(5));
}

#[test]
fn natural_20_and_1_still_count_for_death_saving_throws() {
    let check = check_plus_five(D20CheckKind::SavingThrow(SavingThrowKind::Death));
    assert_eq!(
        roll_until(&check, 20).outcome,
        Some(D20CheckOutcome::CriticalSuccess)
    );
    assert_eq!(
        roll_until(&check, 1).outcome,
        Some(D20CheckOutcome::CriticalFailure)
    );
}

#[test]
fn forced_outcome_still_decides_a_skill_check() {
    let mut check = check_plus_five(D20CheckKind::Skill(Skill::Athletics));
    check.set_forced_outcome(
        ModifierSource::Custom("Test".to_string()),
        D20CheckOutcome::Success,
    );
    assert!(check.roll().is_success_vs(30));
}

#[test]
fn success_probability_of_a_skill_check_reaches_zero_and_one() {
    let check = check_plus_five(D20CheckKind::Skill(Skill::Athletics));
    assert_eq!(check.success_probability(30).max, 0.0);
    assert_eq!(check.success_probability(5).min, 1.0);

    let attack = check_plus_five(D20CheckKind::AttackRoll(AttackSource::Spell));
    assert_eq!(attack.success_probability(30).max, 0.05);
    assert_eq!(attack.success_probability(5).min, 0.95);
}
