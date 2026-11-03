extern crate nat20_core;

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

/// A level 3 Berserker, raging or not, hits a Goblin Warrior with a Reckless Attack;
/// returns every damage roll the Barbarian made.
fn reckless_hit(rage: bool) -> Vec<DamageRollResult> {
    let mut scenario = Scenario::new();
    scenario
        .spawn("barbarian", "hero.barbarian")
        .level(3)
        .time_mode(TimeMode::TurnBased { encounter_id: None })
        .spawn();
    scenario
        .spawn("goblin", "monster.goblin_warrior")
        .level(1)
        .position([1.0, 0.0, 0.0], false)
        .spawn();
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
    scenario
        .probe("barbarian")
        .act("action.barbarian.reckless_attack")
        .context_filter(|context, _cost| context.is_weapon_attack() && context.is_melee_attack())
        .target_entity("goblin")
        .perform();

    let barbarian = scenario.entity("barbarian");
    scenario
        .engine_state
        .event_log
        .events
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
