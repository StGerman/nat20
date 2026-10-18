extern crate nat20_core;

use nat20_core::{
    components::{
        ability::Ability,
        d20::{AdvantageType, D20CheckDC, D20CheckKind, D20CheckOutcome},
        damage::{AttackSource, DamageType},
        dice::{DiceSet, DieSize},
        health::life_state::LifeState,
        items::equipment::{armor::ArmorTrainingSet, armor::ArmorType, weapon::WeaponCategory},
        modifier::{FlatModifiable, ModifierMap, ModifierSource},
        proficiency::ProficiencyLevel,
        saving_throw::SavingThrowKind,
        skill::Skill,
    },
    systems::{self, time::RestKind},
    test_utils::scenario::{Operator, Scenario},
};

const CHANNEL_DIVINITY: &str = "resource.cleric.channel_divinity";
/// The fixture Cleric's Wisdom is 17 (15 plus the default +2)
const WISDOM_MODIFIER: u32 = 3;

/// A level 2 Cleric with a Fighter ally and a Goblin Warrior next to her.
fn chapel() -> Scenario {
    let mut scenario = Scenario::new();
    scenario.spawn("cleric", "hero.cleric").level(2).spawn();
    scenario
        .spawn("fighter", "hero.fighter")
        .level(2)
        .position([1.0, 0.0, 0.0], false)
        .spawn();
    scenario
        .spawn("goblin", "monster.goblin_warrior")
        .level(1)
        .position([-1.0, 0.0, 0.0], false)
        .spawn();
    scenario
}

fn life_state(scenario: &Scenario, handle: &str) -> LifeState {
    *systems::helpers::get_component::<LifeState>(
        &scenario.engine_state.world,
        scenario.entity(handle),
    )
}

#[test]
fn a_level_2_cleric_has_the_srd_numbers() {
    let mut scenario = chapel();
    // d8 + 1, then 5 + 1 (Constitution 13 plus nothing from the default +1 to Charisma)
    assert_eq!(scenario.probe("cleric").max_hp(), 15);
    scenario
        .probe("cleric")
        .assert_resource(CHANNEL_DIVINITY, Operator::Equal(2))
        .assert_resource_tiered("resource.spell_slot", 1, Operator::Equal(3));
    for action in [
        "action.guidance",
        "action.sacred_flame",
        "action.spare_the_dying",
        "action.bless",
        "action.cure_wounds",
        "action.guiding_bolt",
        "action.healing_word",
        "action.shield_of_faith",
        "action.cleric.divine_spark",
    ] {
        scenario.probe("cleric").assert_action_available(action);
    }
}

#[test]
fn a_level_1_cleric_has_no_channel_divinity_yet() {
    let mut scenario = Scenario::new();
    scenario.spawn("cleric", "hero.cleric").level(1).spawn();
    scenario
        .probe("cleric")
        .assert_no_resource(CHANNEL_DIVINITY)
        .assert_no_action("action.cleric.divine_spark");
}

#[test]
fn protector_trains_heavy_armor_and_martial_weapons() {
    let scenario = chapel();
    let world = &scenario.engine_state.world;
    let cleric = scenario.entity("cleric");
    let armor = systems::helpers::get_component::<ArmorTrainingSet>(world, cleric);
    assert!(armor.contains(&ArmorType::Heavy), "{armor:?}");
    let weapons = systems::helpers::get_component::<
        nat20_core::components::items::equipment::weapon::WeaponProficiencyMap,
    >(world, cleric);
    assert_eq!(
        weapons.proficiency(&WeaponCategory::Martial).level(),
        &ProficiencyLevel::Proficient
    );
}

#[test]
fn channel_divinity_regains_one_use_on_a_short_rest() {
    let mut scenario = chapel();
    for _ in 0..2 {
        scenario
            .act("cleric", "action.cleric.divine_spark")
            .variant("variant.divine_spark.heal")
            .target_entity("fighter")
            .perform();
        scenario.probe("cleric").rest(RestKind::Short);
        scenario.probe("cleric").rest(RestKind::Short);
    }
    scenario
        .probe("cleric")
        .assert_resource(CHANNEL_DIVINITY, Operator::Equal(2));
}

#[test]
fn divine_spark_heals_1d8_plus_wisdom() {
    let mut scenario = chapel();
    scenario.probe("fighter").damage(15, DamageType::Slashing);
    let before = scenario.probe("fighter").hp();
    scenario
        .act("cleric", "action.cleric.divine_spark")
        .variant("variant.divine_spark.heal")
        .target_entity("fighter")
        .perform();
    let healed = scenario.probe("fighter").hp() - before;
    assert!(
        (1 + WISDOM_MODIFIER..=8 + WISDOM_MODIFIER).contains(&healed),
        "{healed}"
    );
    scenario
        .probe("cleric")
        .assert_resource(CHANNEL_DIVINITY, Operator::Equal(1));
}

fn divine_spark_damage(outcome: D20CheckOutcome) -> u32 {
    let mut scenario = chapel();
    scenario.probe("goblin").d20_force_outcome(
        D20CheckKind::SavingThrow(SavingThrowKind::Ability(Ability::Constitution)),
        outcome,
    );
    let before = scenario.probe("goblin").hp();
    scenario
        .act("cleric", "action.cleric.divine_spark")
        .variant("variant.divine_spark.radiant")
        .target_entity("goblin")
        .perform();
    // Spell save DC 13: 8 + Wisdom 3 + proficiency 2
    scenario
        .event_filter()
        .actor("goblin")
        .d20_dc(
            D20CheckKind::SavingThrow(SavingThrowKind::Ability(Ability::Constitution)),
            13,
        )
        .assert_event_count(1);
    before - scenario.probe("goblin").hp()
}

#[test]
fn divine_spark_damage_is_halved_on_a_successful_save() {
    for _ in 0..10 {
        let full = divine_spark_damage(D20CheckOutcome::Failure);
        assert!((4..=10).contains(&full), "{full}");
        let half = divine_spark_damage(D20CheckOutcome::Success);
        assert!((2..=5).contains(&half), "{half}");
    }
}

#[test]
fn cure_wounds_and_healing_word_heal_2_dice_plus_wisdom() {
    for (spell, die, cost) in [
        ("action.cure_wounds", 8, "resource.action"),
        ("action.healing_word", 4, "resource.bonus_action"),
    ] {
        let mut scenario = chapel();
        scenario.probe("fighter").damage(19, DamageType::Slashing);
        let before = scenario.probe("fighter").hp();
        scenario
            .act("cleric", spell)
            .context_spell_level(1)
            .target_entity("fighter")
            .perform();
        let healed = scenario.probe("fighter").hp() - before;
        assert!(
            (2 + WISDOM_MODIFIER..=2 * die + WISDOM_MODIFIER).contains(&healed),
            "{spell}: {healed}"
        );
        scenario
            .probe("cleric")
            .assert_resource(cost, Operator::Equal(0))
            .assert_resource_tiered("resource.spell_slot", 1, Operator::Equal(2));
    }
}

#[test]
fn sacred_flame_deals_no_damage_on_a_successful_save() {
    for (outcome, damage) in [
        (D20CheckOutcome::Failure, 1..=8),
        (D20CheckOutcome::Success, 0..=0),
    ] {
        let mut scenario = chapel();
        scenario.probe("goblin").d20_force_outcome(
            D20CheckKind::SavingThrow(SavingThrowKind::Ability(Ability::Dexterity)),
            outcome,
        );
        let before = scenario.probe("goblin").hp();
        scenario
            .act("cleric", "action.sacred_flame")
            .target_entity("goblin")
            .perform();
        let dealt = before - scenario.probe("goblin").hp();
        assert!(damage.contains(&dealt), "{outcome:?}: {dealt}");
    }
}

#[test]
fn spare_the_dying_stabilizes_a_creature_at_0_hit_points() {
    let mut scenario = chapel();
    let max_hp = scenario.probe("fighter").max_hp();
    scenario
        .probe("fighter")
        .damage(max_hp, DamageType::Slashing);
    assert!(matches!(
        life_state(&scenario, "fighter"),
        LifeState::Unconscious(_)
    ));
    scenario
        .act("cleric", "action.spare_the_dying")
        .target_entity("fighter")
        .perform();
    assert_eq!(life_state(&scenario, "fighter"), LifeState::Stable);
}

#[test]
fn bless_adds_a_d4_to_attack_rolls_and_saves() {
    let mut scenario = chapel();
    scenario
        .act("cleric", "action.bless")
        .context_spell_level(1)
        .target_entities(vec!["cleric", "fighter"])
        .perform();
    scenario
        .probe("cleric")
        .assert_concentration("effect.spell.bless");

    let bless = ModifierSource::Effect("effect.spell.bless".into());
    let d4 = DiceSet::new(1, DieSize::D4);
    scenario
        .probe("fighter")
        .assert_effect("effect.spell.bless")
        .d20_check(&D20CheckDC::SavingThrow {
            saving_throw: SavingThrowKind::Ability(Ability::Wisdom),
            dc: ModifierMap::from(ModifierSource::Custom("Test save".to_string()), 10).evaluate(),
        });
    scenario
        .event_filter()
        .actor("fighter")
        .d20_modifier(
            D20CheckKind::SavingThrow(SavingThrowKind::Ability(Ability::Wisdom)),
            bless.clone(),
            d4,
        )
        .assert_event_count(1);

    scenario
        .act("fighter", "action.melee_attack")
        .target_entity("goblin")
        .perform();
    scenario
        .event_filter()
        .actor("fighter")
        .d20_modifier(
            D20CheckKind::AttackRoll(AttackSource::Weapon(
                nat20_core::components::items::equipment::weapon::WeaponKind::Melee,
            )),
            bless,
            d4,
        )
        .assert_event_count(1);
}

#[test]
fn guidance_adds_a_d4_to_checks_with_the_chosen_skill() {
    let mut scenario = chapel();
    scenario
        .act("cleric", "action.guidance")
        .variant("variant.guidance.athletics")
        .target_entity("fighter")
        .perform();

    let check = |skill| D20CheckDC::Skill {
        skill,
        dc: ModifierMap::from(ModifierSource::Custom("Test check".to_string()), 10).evaluate(),
    };
    scenario
        .probe("fighter")
        .d20_check(&check(Skill::Athletics))
        .d20_check(&check(Skill::Acrobatics));
    let guidance = ModifierSource::Effect("effect.spell.guidance.athletics".into());
    let d4 = DiceSet::new(1, DieSize::D4);
    for (skill, count) in [(Skill::Athletics, 1), (Skill::Acrobatics, 0)] {
        scenario
            .event_filter()
            .actor("fighter")
            .d20_modifier(D20CheckKind::Skill(skill), guidance.clone(), d4)
            .assert_event_count(count);
    }
}

#[test]
fn guiding_bolt_gives_the_next_attack_against_the_target_advantage() {
    // A sturdy target, so the bolt doesn't drop it
    let mut scenario = chapel();
    scenario
        .spawn("brute", "hero.barbarian")
        .level(10)
        // Off the line from the Cleric to the Fighter, within the Fighter's reach
        .position([1.0, 0.0, 1.2], false)
        .spawn();
    scenario.probe("cleric").d20_force_outcome(
        D20CheckKind::AttackRoll(AttackSource::Spell),
        D20CheckOutcome::Success,
    );
    let before = scenario.probe("brute").hp();
    scenario
        .act("cleric", "action.guiding_bolt")
        .context_spell_level(1)
        .target_entity("brute")
        .perform();
    let dealt = before - scenario.probe("brute").hp();
    assert!((4..=24).contains(&dealt), "{dealt}");
    scenario
        .probe("brute")
        .assert_effect("effect.spell.guiding_bolt");

    // The first attack against the target, by anyone, has Advantage and spends it
    let melee = D20CheckKind::AttackRoll(AttackSource::Weapon(
        nat20_core::components::items::equipment::weapon::WeaponKind::Melee,
    ));
    let guiding_bolt = ModifierSource::Effect("effect.spell.guiding_bolt".into());
    for _ in 0..2 {
        scenario
            .act("fighter", "action.melee_attack")
            .target_entity("brute")
            .perform();
        scenario.probe("fighter").rest(RestKind::Short);
    }
    scenario
        .event_filter()
        .actor("fighter")
        .d20_advantage(melee, guiding_bolt, AdvantageType::Advantage)
        .assert_event_count(1);
    scenario
        .probe("brute")
        .assert_no_effect("effect.spell.guiding_bolt");
}

#[test]
fn shield_of_faith_adds_2_to_armor_class() {
    let mut scenario = chapel();
    let ac = |scenario: &Scenario, handle: &str| {
        systems::loadout::armor_class(&scenario.engine_state, scenario.entity(handle)).total()
    };
    let before = ac(&scenario, "fighter");
    scenario
        .act("cleric", "action.shield_of_faith")
        .context_spell_level(1)
        .target_entity("fighter")
        .perform();
    assert_eq!(ac(&scenario, "fighter"), before + 2);
    scenario
        .probe("cleric")
        .assert_concentration("effect.spell.shield_of_faith");
}
