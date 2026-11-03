extern crate nat20_core;

use nat20_core::{
    components::{
        ability::Ability,
        d20::{D20CheckKind, D20CheckOutcome},
        damage::DamageType,
        id::EffectId,
        saving_throw::SavingThrowKind,
    },
    systems,
    test_utils::scenario::{Operator, Scenario},
};

const CHANNEL_DIVINITY: &str = "resource.cleric.channel_divinity";
const PRESERVE_LIFE: &str = "action.cleric.life_domain.preserve_life";
/// The fixture Cleric's Wisdom is 17 (15 plus the default +2)
const WISDOM_MODIFIER: u32 = 3;

/// A level 3 Life Domain Cleric with a Fighter ally next to her.
fn chapel() -> Scenario {
    let mut scenario = Scenario::new();
    scenario.spawn("cleric", "hero.cleric").level(3).spawn();
    scenario
        .spawn("fighter", "hero.fighter")
        .level(2)
        .position([1.0, 0.0, 0.0], false)
        .spawn();
    scenario
}

/// Leaves the creature with `hp` Hit Points.
fn wound_to(scenario: &mut Scenario, handle: &str, hp: u32) {
    let damage = scenario.probe(handle).hp() - hp;
    scenario.probe(handle).damage(damage, DamageType::Slashing);
    assert_eq!(scenario.probe(handle).hp(), hp);
}

#[test]
fn disciple_of_life_adds_2_plus_the_slot_level_to_healing_spells() {
    for slot in [1u32, 2] {
        let mut scenario = chapel();
        wound_to(&mut scenario, "fighter", 1);
        scenario
            .act("cleric", "action.cure_wounds")
            .context_spell_level(slot as u8)
            .target_entity("fighter")
            .perform();
        let healed = scenario.probe("fighter").hp() - 1;
        let bonus = 2 + slot;
        let dice = 2 * slot;
        assert!(
            (dice + WISDOM_MODIFIER + bonus..=8 * dice + WISDOM_MODIFIER + bonus).contains(&healed),
            "slot {slot}: {healed}"
        );
    }
}

#[test]
fn disciple_of_life_leaves_channel_divinity_healing_alone() {
    let mut scenario = chapel();
    wound_to(&mut scenario, "fighter", 1);
    scenario
        .act("cleric", "action.cleric.divine_spark")
        .variant("variant.divine_spark.heal")
        .target_entity("fighter")
        .perform();
    let healed = scenario.probe("fighter").hp() - 1;
    assert!(
        (1 + WISDOM_MODIFIER..=8 + WISDOM_MODIFIER).contains(&healed),
        "{healed}"
    );
}

#[test]
fn preserve_life_heals_up_to_half_the_hit_point_maximum() {
    let mut scenario = chapel();
    let half = scenario.probe("fighter").max_hp() / 2;
    wound_to(&mut scenario, "fighter", 1);
    scenario
        .act("cleric", PRESERVE_LIFE)
        .target_entity("fighter")
        .perform();
    // The pool is 15 (five times Cleric level 3), more than the Fighter can take.
    assert!(half - 1 < 15);
    assert_eq!(scenario.probe("fighter").hp(), half);
    scenario
        .probe("cleric")
        .assert_resource(CHANNEL_DIVINITY, Operator::Equal(1));
}

#[test]
fn preserve_life_divides_its_pool_evenly() {
    let mut scenario = chapel();
    scenario
        .spawn("ally", "hero.barbarian")
        .level(4)
        .position([-1.0, 0.0, 0.0], false)
        .spawn();
    wound_to(&mut scenario, "fighter", 1);
    wound_to(&mut scenario, "ally", 1);
    scenario
        .act("cleric", PRESERVE_LIFE)
        .target_entities(vec!["fighter", "ally"])
        .perform();
    let fighter_half = scenario.probe("fighter").max_hp() / 2;
    // 15 split two ways is 7 each, rounded down; the Fighter's share is capped at half.
    assert_eq!(scenario.probe("fighter").hp(), (1 + 7).min(fighter_half));
    assert_eq!(scenario.probe("ally").hp(), 1 + 7);
}

#[test]
fn preserve_life_only_heals_bloodied_creatures() {
    let mut scenario = chapel();
    scenario
        .act("cleric", PRESERVE_LIFE)
        .target_entity("fighter")
        .assert_perform_fails();
    scenario
        .probe("cleric")
        .assert_resource(CHANNEL_DIVINITY, Operator::Equal(2));
}

#[test]
fn a_level_2_cleric_has_no_life_domain_yet() {
    let mut scenario = Scenario::new();
    scenario.spawn("cleric", "hero.cleric").level(2).spawn();
    scenario.act("cleric", PRESERVE_LIFE).assert_perform_fails();
}

#[test]
fn life_domain_spells_are_always_prepared() {
    let mut scenario = chapel();
    // The fixture prepared none of Aid and Lesser Restoration, yet both can be cast.
    for spell in ["action.aid", "action.lesser_restoration"] {
        scenario.probe("cleric").assert_action_available(spell);
    }
}

#[test]
fn aid_raises_the_maximum_and_current_hit_points_until_it_ends() {
    let mut scenario = chapel();
    wound_to(&mut scenario, "fighter", 4);
    let max_hp = scenario.probe("fighter").max_hp();
    let cleric_max_hp = scenario.probe("cleric").max_hp();
    scenario
        .act("cleric", "action.aid")
        .context_spell_level(2)
        .target_entities(vec!["fighter", "cleric"])
        .perform();
    assert_eq!(scenario.probe("fighter").max_hp(), max_hp + 5);
    assert_eq!(scenario.probe("fighter").hp(), 4 + 5);
    assert_eq!(scenario.probe("cleric").max_hp(), cleric_max_hp + 5);

    let fighter = scenario.entity("fighter");
    systems::effects::remove_effects_by_id(
        &mut scenario.engine_state,
        fighter,
        &EffectId::new("nat20_core", "effect.spell.aid"),
    );
    assert_eq!(scenario.probe("fighter").max_hp(), max_hp);
    assert_eq!(scenario.probe("fighter").hp(), 4 + 5);
}

#[test]
fn a_second_aid_does_not_add_to_the_first() {
    let mut scenario = chapel();
    let max_hp = scenario.probe("fighter").max_hp();
    for _ in 0..2 {
        scenario
            .act("cleric", "action.aid")
            .context_spell_level(2)
            .target_entity("fighter")
            .perform();
        scenario.probe("cleric").end_turn();
    }
    assert_eq!(scenario.probe("fighter").max_hp(), max_hp + 5);
    let fighter = scenario.entity("fighter");
    let aid = EffectId::new("nat20_core", "effect.spell.aid");
    let active = systems::effects::effects(&scenario.engine_state.world, fighter)
        .values()
        .filter(|instance| instance.effect_id == aid)
        .count();
    assert_eq!(active, 1);
    systems::effects::remove_effects_by_id(
        &mut scenario.engine_state,
        fighter,
        &EffectId::new("nat20_core", "effect.spell.aid"),
    );
    assert_eq!(scenario.probe("fighter").max_hp(), max_hp);
}

#[test]
fn losing_aid_takes_away_hit_points_above_the_maximum() {
    let mut scenario = chapel();
    let max_hp = scenario.probe("fighter").max_hp();
    scenario
        .act("cleric", "action.aid")
        .context_spell_level(2)
        .target_entity("fighter")
        .perform();
    assert_eq!(scenario.probe("fighter").hp(), max_hp + 5);
    let fighter = scenario.entity("fighter");
    systems::effects::remove_effects_by_id(
        &mut scenario.engine_state,
        fighter,
        &EffectId::new("nat20_core", "effect.spell.aid"),
    );
    assert_eq!(scenario.probe("fighter").hp(), max_hp);
}

#[test]
fn lesser_restoration_ends_paralysis_and_the_hold_person_behind_it() {
    let mut scenario = chapel();
    scenario.probe("fighter").d20_force_outcome(
        D20CheckKind::SavingThrow(SavingThrowKind::Ability(Ability::Wisdom)),
        D20CheckOutcome::CriticalFailure,
    );
    scenario
        .act("cleric", "action.hold_person")
        .context_spell_level(2)
        .target_entity("fighter")
        .perform();
    scenario
        .probe("fighter")
        .assert_effect("effect.condition.paralyzed");

    scenario
        .act("cleric", "action.lesser_restoration")
        .variant("variant.lesser_restoration.paralyzed")
        .target_entity("fighter")
        .perform();
    scenario
        .probe("fighter")
        .assert_no_effect("effect.spell.hold_person")
        .assert_no_effect("effect.condition.paralyzed")
        .assert_no_effect("effect.condition.incapacitated");
}

#[test]
fn lesser_restoration_leaves_a_permanent_condition_alone() {
    let mut scenario = chapel();
    scenario
        .probe("fighter")
        .apply_effect("effect.condition.poisoned");
    scenario
        .act("cleric", "action.lesser_restoration")
        .variant("variant.lesser_restoration.poisoned")
        .target_entity("fighter")
        .perform();
    scenario
        .probe("fighter")
        .assert_effect("effect.condition.poisoned");
}
