extern crate nat20_core;

use std::collections::HashSet;

use hecs::Entity;
use nat20_core::{
    components::{
        d20::{D20CheckResult, RollMode},
        modifier::ModifierSource,
    },
    engine::encounter::EncounterId,
    test_utils::scenario::Scenario,
};

fn fighter_and_goblin() -> Scenario {
    let mut scenario = Scenario::new();
    scenario.spawn("fighter", "hero.fighter").level(1).spawn();
    scenario
        .spawn("goblin", "monster.goblin_warrior")
        .level(1)
        .position([1.0, 0.0, 0.0], false)
        .spawn();
    scenario
}

fn initiative(scenario: &Scenario, encounter: EncounterId, entity: Entity) -> D20CheckResult {
    scenario
        .engine_state
        .encounter(&encounter)
        .unwrap()
        .initiative_order()
        .iter()
        .find(|(e, _)| *e == entity)
        .map(|(_, roll)| roll.clone())
        .unwrap()
}

#[test]
fn surprised_creatures_roll_initiative_with_disadvantage() {
    let mut scenario = fighter_and_goblin();
    let (fighter, goblin) = (scenario.entity("fighter"), scenario.entity("goblin"));
    let encounter = scenario
        .engine_state
        .start_surprised_encounter(HashSet::from([fighter, goblin]), HashSet::from([goblin]));

    let goblin_roll = initiative(&scenario, encounter, goblin);
    assert_eq!(
        goblin_roll.advantage_tracker().roll_mode(),
        RollMode::Disadvantage
    );
    assert_eq!(goblin_roll.rolls.len(), 2);
    assert_eq!(
        goblin_roll.selected_roll,
        *goblin_roll.rolls.iter().min().unwrap()
    );
    assert!(
        goblin_roll
            .advantage_tracker()
            .summary()
            .contains_key(&ModifierSource::Custom("Surprised".to_string()))
    );

    let fighter_roll = initiative(&scenario, encounter, fighter);
    assert_eq!(
        fighter_roll.advantage_tracker().roll_mode(),
        RollMode::Normal
    );
    assert_eq!(fighter_roll.rolls.len(), 1);
}

#[test]
fn an_encounter_without_surprise_rolls_initiative_normally() {
    let mut scenario = fighter_and_goblin();
    let (fighter, goblin) = (scenario.entity("fighter"), scenario.entity("goblin"));
    let encounter = scenario
        .engine_state
        .start_encounter(HashSet::from([fighter, goblin]));

    for entity in [fighter, goblin] {
        assert_eq!(
            initiative(&scenario, encounter, entity)
                .advantage_tracker()
                .roll_mode(),
            RollMode::Normal
        );
    }
}
