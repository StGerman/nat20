//! The SRD 5.2.1 monster traits the stat blocks carry: Pack Tactics, the
//! goblins' extra 1d4 on an attack roll with Advantage, the Wolf's Bite
//! knocking its target Prone, Nimble Escape and the Goblin Boss's Multiattack.

extern crate nat20_core;

use nat20_core::{
    components::{
        d20::{AdvantageType, D20CheckKind, D20CheckOutcome, RollMode},
        damage::{AttackSource, DamageComponent, DamageType},
        dice::{DiceSet, DieSize},
        id::EffectId,
        items::equipment::{slots::EquipmentSlot, weapon::WeaponKind},
        modifier::{ModifierMap, ModifierSource},
        species::CreatureSize,
        time::TimeMode,
    },
    test_utils::scenario::{Operator, Scenario},
};

const PACK_TACTICS: &str = "effect.monster.pack_tactics";
const ADVANTAGE_RIDER: &str = "effect.monster.advantage_rider";
const PRONE: &str = "effect.condition.prone";
const MELEE_ATTACK_ROLL: D20CheckKind =
    D20CheckKind::AttackRoll(AttackSource::Weapon(WeaponKind::Melee));

/// A level 5 Fighter, so a few hits don't drop it
fn scenario() -> Scenario {
    let mut scenario = Scenario::new();
    scenario
        .spawn("fighter", "hero.fighter")
        .level(5)
        .time_mode(TimeMode::TurnBased { encounter_id: None })
        .spawn();
    scenario
}

fn spawn_at(scenario: &mut Scenario, handle: &str, template: &str, position: [f32; 3]) {
    scenario
        .spawn(handle, template)
        .position(position, false)
        .time_mode(TimeMode::TurnBased { encounter_id: None })
        .spawn();
}

fn pack_tactics_advantage(scenario: &mut Scenario, attacker: &str) -> Option<AdvantageType> {
    scenario
        .probe(attacker)
        .preview_attack_roll(
            "fighter",
            "action.melee_attack",
            EquipmentSlot::MeleeMainHand,
        )
        .advantage_tracker()
        .summary()
        .get(&ModifierSource::Effect(EffectId::from(PACK_TACTICS)))
        .cloned()
}

fn hit(scenario: &mut Scenario, attacker: &str, target: &str) {
    scenario
        .probe(attacker)
        .d20_force_outcome(MELEE_ATTACK_ROLL, D20CheckOutcome::Success)
        .act("action.melee_attack")
        .target_entity(target)
        .perform();
}

fn rider_count(scenario: &Scenario, attacker: &str) -> usize {
    scenario
        .event_filter()
        .actor(attacker)
        .damage_roll(DamageComponent::new(
            ModifierMap::from(
                ModifierSource::Effect(EffectId::from(ADVANTAGE_RIDER)),
                DiceSet::new(1, DieSize::D4),
            ),
            DamageType::Slashing,
        ))
        .filter()
        .len()
}

#[test]
fn pack_tactics_needs_an_ally_beside_the_target() {
    let mut scenario = scenario();
    spawn_at(&mut scenario, "wolf", "stat_block.wolf", [1.0, 0.0, 0.0]);
    spawn_at(
        &mut scenario,
        "packmate",
        "stat_block.wolf",
        [-1.0, 0.0, 0.0],
    );

    assert_eq!(
        pack_tactics_advantage(&mut scenario, "wolf"),
        Some(AdvantageType::Advantage)
    );
    assert_eq!(
        scenario
            .probe("wolf")
            .preview_attack_roll(
                "fighter",
                "action.melee_attack",
                EquipmentSlot::MeleeMainHand
            )
            .advantage_tracker()
            .roll_mode(),
        RollMode::Advantage
    );
}

#[test]
fn pack_tactics_ignores_an_ally_far_from_the_target() {
    let mut scenario = scenario();
    spawn_at(&mut scenario, "wolf", "stat_block.wolf", [1.0, 0.0, 0.0]);
    spawn_at(
        &mut scenario,
        "packmate",
        "stat_block.wolf",
        [10.0, 0.0, 0.0],
    );

    assert_eq!(pack_tactics_advantage(&mut scenario, "wolf"), None);
}

#[test]
fn an_attacker_alone_beside_the_target_is_not_its_own_ally() {
    let mut scenario = scenario();
    spawn_at(
        &mut scenario,
        "rat",
        "stat_block.giant_rat",
        [1.0, 0.0, 0.0],
    );

    assert_eq!(pack_tactics_advantage(&mut scenario, "rat"), None);
}

#[test]
fn pack_tactics_ignores_an_incapacitated_ally() {
    let mut scenario = scenario();
    spawn_at(&mut scenario, "wolf", "stat_block.wolf", [1.0, 0.0, 0.0]);
    spawn_at(
        &mut scenario,
        "packmate",
        "stat_block.wolf",
        [-1.0, 0.0, 0.0],
    );
    scenario
        .probe("packmate")
        .apply_effect("effect.condition.incapacitated");

    assert_eq!(pack_tactics_advantage(&mut scenario, "wolf"), None);
}

#[test]
fn pack_tactics_ignores_an_ally_at_0_hit_points() {
    let mut scenario = scenario();
    spawn_at(&mut scenario, "wolf", "stat_block.wolf", [1.0, 0.0, 0.0]);
    spawn_at(
        &mut scenario,
        "packmate",
        "stat_block.wolf",
        [-1.0, 0.0, 0.0],
    );
    scenario.probe("packmate").kill();

    assert_eq!(pack_tactics_advantage(&mut scenario, "wolf"), None);
}

#[test]
fn pack_tactics_doesnt_count_the_targets_friends() {
    let mut scenario = scenario();
    spawn_at(&mut scenario, "wolf", "stat_block.wolf", [1.0, 0.0, 0.0]);
    scenario
        .spawn("cleric", "hero.cleric")
        .position([-1.0, 0.0, 0.0], false)
        .spawn();

    assert_eq!(pack_tactics_advantage(&mut scenario, "wolf"), None);
}

#[test]
fn the_goblin_rider_needs_advantage() {
    let mut scenario = scenario();
    spawn_at(
        &mut scenario,
        "goblin",
        "stat_block.goblin_warrior",
        [1.0, 0.0, 0.0],
    );

    hit(&mut scenario, "goblin", "fighter");
    assert_eq!(rider_count(&scenario, "goblin"), 0);

    // A Prone target beside the goblin gives it Advantage
    scenario.probe("goblin").end_turn();
    scenario.probe("fighter").apply_effect(PRONE);
    hit(&mut scenario, "goblin", "fighter");
    assert_eq!(rider_count(&scenario, "goblin"), 1);
}

#[test]
fn the_goblin_rider_doesnt_fire_when_advantage_and_disadvantage_cancel() {
    let mut scenario = scenario();
    spawn_at(
        &mut scenario,
        "goblin",
        "stat_block.goblin_warrior",
        [1.0, 0.0, 0.0],
    );
    scenario.probe("fighter").apply_effect(PRONE);
    scenario.probe("goblin").apply_effect(PRONE);

    hit(&mut scenario, "goblin", "fighter");

    scenario
        .event_filter()
        .actor("goblin")
        .d20_roll_mode(MELEE_ATTACK_ROLL, RollMode::Normal)
        .assert_event_count(1);
    assert_eq!(rider_count(&scenario, "goblin"), 0);
}

#[test]
fn a_bite_knocks_a_medium_target_prone() {
    let mut scenario = scenario();
    spawn_at(&mut scenario, "wolf", "stat_block.wolf", [1.0, 0.0, 0.0]);

    scenario.probe("fighter").assert_no_effect(PRONE);
    hit(&mut scenario, "wolf", "fighter");
    scenario.probe("fighter").assert_effect(PRONE);
}

#[test]
fn a_bite_doesnt_knock_a_large_target_prone() {
    let mut scenario = scenario();
    spawn_at(&mut scenario, "wolf", "stat_block.wolf", [1.0, 0.0, 0.0]);
    let fighter = scenario.entity("fighter");
    *scenario
        .engine_state
        .world
        .get::<&mut CreatureSize>(fighter)
        .unwrap() = CreatureSize::Large;

    hit(&mut scenario, "wolf", "fighter");
    scenario.probe("fighter").assert_no_effect(PRONE);
}

#[test]
fn a_missed_bite_doesnt_knock_prone() {
    let mut scenario = scenario();
    spawn_at(&mut scenario, "wolf", "stat_block.wolf", [1.0, 0.0, 0.0]);

    scenario
        .probe("wolf")
        .d20_force_outcome(MELEE_ATTACK_ROLL, D20CheckOutcome::Failure)
        .act("action.melee_attack")
        .target_entity("fighter")
        .perform();
    scenario.probe("fighter").assert_no_effect(PRONE);
}

#[test]
fn nimble_escape_disengages_for_a_bonus_action() {
    let mut scenario = scenario();
    spawn_at(
        &mut scenario,
        "goblin",
        "stat_block.goblin_warrior",
        [1.0, 0.0, 0.0],
    );

    scenario
        .probe("goblin")
        .assert_action_available("action.monster.nimble_escape")
        .act("action.monster.nimble_escape")
        .perform();

    scenario
        .probe("goblin")
        .assert_effect("effect.disengage")
        .assert_resource("resource.bonus_action", Operator::Equal(0))
        .assert_resource("resource.action", Operator::Equal(1));
}

#[test]
fn the_goblin_boss_attacks_twice() {
    let mut scenario = scenario();
    spawn_at(
        &mut scenario,
        "boss",
        "stat_block.goblin_boss",
        [1.0, 0.0, 0.0],
    );

    hit(&mut scenario, "boss", "fighter");
    scenario
        .probe("boss")
        .assert_resource("resource.action", Operator::Equal(0))
        .assert_resource("resource.extra_attack", Operator::Equal(1));

    hit(&mut scenario, "boss", "fighter");
    scenario
        .probe("boss")
        .assert_resource("resource.extra_attack", Operator::Equal(0));
    scenario
        .event_filter()
        .actor("boss")
        .action_performed("action.melee_attack")
        .assert_event_count(2);
}

#[test]
fn the_goblin_warrior_attacks_once() {
    let mut scenario = scenario();
    spawn_at(
        &mut scenario,
        "goblin",
        "stat_block.goblin_warrior",
        [1.0, 0.0, 0.0],
    );

    hit(&mut scenario, "goblin", "fighter");
    scenario
        .probe("goblin")
        .assert_resource("resource.action", Operator::Equal(0))
        .assert_no_resource("resource.extra_attack")
        .assert_action_unavailable("action.melee_attack");
}
