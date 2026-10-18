extern crate nat20_core;

use hecs::Entity;
use nat20_core::{
    components::{
        modifier::{Modifiable, ModifierSource},
        skill::{Skill, SkillSet},
    },
    engine::encounter::EncounterId,
    systems,
    test_utils::scenario::Scenario,
};

fn set_initiative_bonus(scenario: &mut Scenario, entity: Entity, bonus: i32) {
    systems::helpers::get_component_mut::<SkillSet>(&mut scenario.engine_state.world, entity)
        .get_mut(&Skill::Initiative)
        .modifiers_mut()
        .add_modifier(
            ModifierSource::Custom("Test initiative bonus".to_string()),
            bonus,
        );
}

/// A fighter (acting first) and a goblin in a fight, plus a second goblin that
/// arrives after it started with the given Initiative bonus
fn fight_with_reinforcement(bonus: i32) -> (Scenario, EncounterId, [Entity; 3]) {
    let mut scenario = Scenario::new();
    scenario.spawn("fighter", "hero.fighter").level(1).spawn();
    scenario
        .spawn("goblin", "monster.goblin_warrior")
        .level(1)
        .position([1.0, 0.0, 0.0], false)
        .spawn();
    let (fighter, goblin) = (scenario.entity("fighter"), scenario.entity("goblin"));
    set_initiative_bonus(&mut scenario, fighter, 100);
    let encounter = scenario
        .engine_state
        .start_encounter([fighter, goblin].into_iter().collect());

    scenario
        .spawn("goblin_2", "monster.goblin_warrior")
        .level(1)
        .position([2.0, 0.0, 0.0], false)
        .spawn();
    let goblin_2 = scenario.entity("goblin_2");
    set_initiative_bonus(&mut scenario, goblin_2, bonus);
    scenario.engine_state.join_encounter(&encounter, goblin_2);
    (scenario, encounter, [fighter, goblin, goblin_2])
}

fn order(scenario: &Scenario, encounter: EncounterId) -> Vec<Entity> {
    scenario
        .engine_state
        .encounter(&encounter)
        .unwrap()
        .initiative_order()
        .iter()
        .map(|(entity, _)| *entity)
        .collect()
}

fn current(scenario: &Scenario, encounter: EncounterId) -> Entity {
    scenario
        .engine_state
        .encounter(&encounter)
        .unwrap()
        .current_entity()
}

fn round(scenario: &Scenario, encounter: EncounterId) -> usize {
    scenario.engine_state.encounter(&encounter).unwrap().round()
}

#[test]
fn a_creature_joining_later_in_the_order_acts_this_round() {
    let (mut scenario, encounter, [fighter, goblin, goblin_2]) = fight_with_reinforcement(50);

    assert_eq!(order(&scenario, encounter), vec![fighter, goblin_2, goblin]);
    assert_eq!(current(&scenario, encounter), fighter);
    assert!(
        scenario
            .engine_state
            .encounter_for_entity(goblin_2)
            .is_some_and(|e| *e.id() == encounter)
    );

    scenario.engine_state.end_turn(fighter);
    assert_eq!(current(&scenario, encounter), goblin_2);
    assert_eq!(round(&scenario, encounter), 1);
}

#[test]
fn a_creature_joining_earlier_in_the_order_acts_next_round() {
    let (mut scenario, encounter, [fighter, goblin, goblin_2]) = fight_with_reinforcement(200);

    assert_eq!(order(&scenario, encounter), vec![goblin_2, fighter, goblin]);
    // The fighter's turn carries on
    assert_eq!(current(&scenario, encounter), fighter);

    scenario.engine_state.end_turn(fighter);
    assert_eq!(current(&scenario, encounter), goblin);
    scenario.engine_state.end_turn(goblin);
    assert_eq!(round(&scenario, encounter), 2);
    assert_eq!(current(&scenario, encounter), goblin_2);
}

#[test]
fn joining_twice_changes_nothing() {
    let (mut scenario, encounter, [_, _, goblin_2]) = fight_with_reinforcement(50);
    let before = order(&scenario, encounter);

    scenario.engine_state.join_encounter(&encounter, goblin_2);

    assert_eq!(order(&scenario, encounter), before);
}

#[test]
fn a_creature_in_another_fight_cannot_join() {
    let (mut scenario, first, [fighter, goblin, goblin_2]) = fight_with_reinforcement(50);
    scenario
        .spawn("fighter_2", "hero.fighter")
        .level(1)
        .position([10.0, 0.0, 0.0], false)
        .spawn();
    scenario
        .spawn("goblin_3", "monster.goblin_warrior")
        .level(1)
        .position([11.0, 0.0, 0.0], false)
        .spawn();
    let (fighter_2, goblin_3) = (scenario.entity("fighter_2"), scenario.entity("goblin_3"));
    let second = scenario
        .engine_state
        .start_encounter([fighter_2, goblin_3].into_iter().collect());
    let before = order(&scenario, second);

    scenario.engine_state.join_encounter(&second, goblin);

    assert_eq!(order(&scenario, second), before);
    assert_eq!(order(&scenario, first), vec![fighter, goblin_2, goblin]);
    assert!(
        scenario
            .engine_state
            .encounter_for_entity(goblin)
            .is_some_and(|e| *e.id() == first)
    );
}
