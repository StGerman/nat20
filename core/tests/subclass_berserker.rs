extern crate nat20_core;

use hecs::Entity;
use nat20_core::{
    components::{
        d20::{D20CheckKind, D20CheckOutcome},
        damage::{AttackSource, DamageRollResult},
        items::equipment::weapon::WeaponKind,
        modifier::ModifierSource,
        time::TimeMode,
    },
    engine::event::EventKind,
    test_utils::scenario::Scenario,
};

const FRENZY: &str = "effect.barbarian.berserker.frenzy";

/// A level 3 Berserker, raging or not, hits a level 10 Fighter with a Reckless Attack;
/// returns every damage roll the Barbarian made.
fn reckless_hit(rage: bool) -> Vec<DamageRollResult> {
    let mut scenario = Scenario::new();
    scenario
        .spawn("barbarian", "hero.barbarian")
        .level(3)
        .time_mode(TimeMode::TurnBased { encounter_id: None })
        .spawn();
    spawn_target(&mut scenario);
    reckless_hits(&mut scenario, 1, rage)
}

fn spawn_target(scenario: &mut Scenario) {
    scenario
        .spawn("target", "hero.fighter")
        .level(10)
        .position([1.0, 0.0, 0.0], false)
        .spawn();
}

/// A level 5 Berserker and a level 10 Fighter in an encounter, on the Berserker's turn.
fn berserker_turn() -> Scenario {
    let mut scenario = Scenario::new();
    scenario
        .spawn("barbarian", "hero.barbarian")
        .level(5)
        .spawn();
    spawn_target(&mut scenario);
    scenario
        .encounter()
        .initiative_order(vec!["barbarian", "target"])
        .build();
    let barbarian = scenario.entity("barbarian");
    // Initiative is rolled, so the Fighter may still go first
    while current(&scenario) != barbarian {
        let entity = current(&scenario);
        scenario.engine_state.end_turn(entity);
        scenario.engine_state.update(0.0);
    }
    scenario
}

fn current(scenario: &Scenario) -> Entity {
    scenario
        .engine_state
        .encounter_for_entity(scenario.entity("barbarian"))
        .unwrap()
        .current_entity()
}

/// The Berserker, raging or not, hits the target `hits` times in one turn, the first
/// with a Reckless Attack; returns every damage roll the Barbarian made.
fn reckless_hits(scenario: &mut Scenario, hits: usize, rage: bool) -> Vec<DamageRollResult> {
    if rage {
        scenario
            .probe("barbarian")
            .act("action.barbarian.rage")
            .perform();
    }
    scenario.probe("barbarian").d20_force_outcome(
        D20CheckKind::AttackRoll(AttackSource::Weapon(WeaponKind::Melee)),
        D20CheckOutcome::Success,
    );
    for hit in 0..hits {
        // Reckless Attack is the first attack of the turn; Extra Attack makes the rest
        let action = if hit == 0 {
            "action.barbarian.reckless_attack"
        } else {
            "action.melee_attack"
        };
        scenario
            .probe("barbarian")
            .act(action)
            .context_filter(|context, _cost| {
                context.is_weapon_attack() && context.is_melee_attack()
            })
            .target_entity("target")
            .perform();
    }

    let barbarian = scenario.entity("barbarian");
    let log = match scenario.engine_state.encounter_for_entity(barbarian) {
        Some(encounter) => encounter.event_log(),
        None => &scenario.engine_state.event_log,
    };
    log.events
        .iter()
        .filter_map(|event| match &event.kind {
            EventKind::DamageRollPerformed { actor, result } if actor.id() == barbarian => {
                Some(result.clone())
            }
            _ => None,
        })
        .collect()
}

fn frenzy_dice(rolls: &[DamageRollResult]) -> Vec<i32> {
    let frenzy = ModifierSource::Effect(FRENZY.into());
    rolls
        .iter()
        .flat_map(|roll| &roll.components)
        .filter(|component| component.result.get(&frenzy).is_some())
        .map(|component| component.result.total())
        .collect()
}

#[test]
fn frenzy_adds_rage_damage_d6s_to_a_raging_reckless_hit() {
    let rolls = reckless_hit(true);
    assert_eq!(rolls.len(), 1, "{rolls:?}");
    let frenzy = frenzy_dice(&rolls);
    // Rage Damage is +2 at Barbarian level 3: 2d6
    assert_eq!(frenzy.len(), 1, "{rolls:?}");
    assert!((2..=12).contains(&frenzy[0]), "{rolls:?}");
}

#[test]
fn frenzy_needs_rage() {
    let rolls = reckless_hit(false);
    assert_eq!(rolls.len(), 1, "{rolls:?}");
    assert!(frenzy_dice(&rolls).is_empty(), "{rolls:?}");
}

#[test]
fn frenzy_adds_its_dice_to_the_first_hit_of_the_turn_only() {
    // Extra Attack at level 5 gives a second hit in the same turn
    let mut scenario = berserker_turn();
    let rolls = reckless_hits(&mut scenario, 2, true);
    assert_eq!(rolls.len(), 2, "{rolls:?}");
    assert_eq!(frenzy_dice(&rolls).len(), 1, "{rolls:?}");
    assert!(frenzy_dice(&rolls[1..]).is_empty(), "{rolls:?}");
}
