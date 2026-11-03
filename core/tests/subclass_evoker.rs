extern crate nat20_core;

use nat20_core::{
    components::{
        d20::{D20CheckKind, D20CheckOutcome},
        damage::{AttackSource, DamageRollResult},
        modifier::ModifierSource,
    },
    engine::event::EventKind,
    test_utils::scenario::Scenario,
};

/// A Wizard of `level` misses a Goblin Warrior with Fire Bolt; returns the damage rolled, if any.
fn missed_fire_bolt(level: u8) -> Option<DamageRollResult> {
    let mut scenario = Scenario::new();
    scenario.spawn("wizard", "hero.wizard").level(level).spawn();
    scenario
        .spawn("goblin", "monster.goblin_warrior")
        .level(1)
        .position([1.0, 0.0, 0.0], false)
        .spawn();
    scenario.probe("wizard").d20_force_outcome(
        D20CheckKind::AttackRoll(AttackSource::Spell),
        D20CheckOutcome::CriticalFailure,
    );
    scenario
        .act("wizard", "action.fire_bolt")
        .target_entity("goblin")
        .perform();

    let wizard = scenario.entity("wizard");
    scenario
        .engine_state
        .event_log
        .events
        .iter()
        .find_map(|event| match &event.kind {
            EventKind::DamageRollPerformed { actor, result } if actor.id() == wizard => {
                Some(result.clone())
            }
            _ => None,
        })
}

#[test]
fn potent_cantrip_deals_half_damage_on_a_miss() {
    let result = missed_fire_bolt(3).expect("a missed cantrip still rolls damage");
    let miss = ModifierSource::Custom("Attack Miss".to_string());
    assert!(
        result
            .components
            .iter()
            .all(|component| component.result.get(&miss).is_some()),
        "{result:?}"
    );
    assert!((0..=5).contains(&result.total), "{result:?}");
}

#[test]
fn a_level_2_wizard_deals_nothing_on_a_miss() {
    assert!(missed_fire_bolt(2).is_none());
}
