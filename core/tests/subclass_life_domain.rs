extern crate nat20_core;

use nat20_core::{
    components::damage::DamageType,
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
