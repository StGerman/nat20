extern crate nat20_core;

use nat20_core::{
    components::{
        id::ClassId,
        level::{CharacterLevels, experience_for_level},
    },
    systems,
    test_utils::scenario::Scenario,
};

fn party() -> Scenario {
    let mut scenario = Scenario::new();
    scenario.spawn("fighter", "hero.fighter").level(2).spawn();
    scenario.spawn("wizard", "hero.wizard").level(2).spawn();
    scenario
        .spawn("goblin", "stat_block.goblin_warrior")
        .spawn();
    scenario
}

fn experience(scenario: &Scenario, name: &str) -> u32 {
    systems::experience::experience(&scenario.engine_state.world, scenario.entity(name))
}

#[test]
fn a_character_built_to_a_level_has_that_levels_experience() {
    let scenario = party();
    assert_eq!(experience_for_level(2), 300);
    assert_eq!(experience(&scenario, "fighter"), 300);
}

#[test]
fn a_monster_is_worth_its_challenge_ratings_experience() {
    let scenario = party();
    let world = &scenario.engine_state.world;
    // Goblin Warrior, CR 1/4: 50 XP.
    assert_eq!(
        systems::experience::worth(world, scenario.entity("goblin")),
        50
    );
    assert_eq!(
        systems::experience::worth(world, scenario.entity("fighter")),
        0
    );
}

#[test]
fn an_award_is_split_evenly_and_rounded_down() {
    let mut scenario = party();
    let heroes = vec![scenario.entity("fighter"), scenario.entity("wizard")];
    let share = systems::experience::award(&mut scenario.engine_state, &heroes, 125);
    assert_eq!(share, 62);
    assert_eq!(experience(&scenario, "fighter"), 362);
    assert_eq!(experience(&scenario, "wizard"), 362);
    assert_eq!(
        systems::experience::award(&mut scenario.engine_state, &[], 100),
        0
    );
}

#[test]
fn a_character_can_level_up_at_the_next_levels_threshold() {
    let mut scenario = party();
    let fighter = scenario.entity("fighter");
    assert!(!systems::experience::can_level_up(
        &scenario.engine_state.world,
        fighter
    ));
    systems::experience::award(&mut scenario.engine_state, &[fighter], 599);
    assert!(!systems::experience::can_level_up(
        &scenario.engine_state.world,
        fighter
    ));
    systems::experience::award(&mut scenario.engine_state, &[fighter], 1);
    assert!(systems::experience::can_level_up(
        &scenario.engine_state.world,
        fighter
    ));
    assert!(!systems::experience::can_level_up(
        &scenario.engine_state.world,
        scenario.entity("goblin")
    ));
}

fn level_20_fighter() -> CharacterLevels {
    let mut levels = CharacterLevels::new();
    for _ in 0..20 {
        levels.level_up(ClassId::new("nat20_core", "class.fighter"));
    }
    levels
}

#[test]
fn levels_never_exceed_20_however_much_experience() {
    let mut levels = level_20_fighter();
    assert_eq!(levels.total_level(), 20);
    levels.add_experience(u32::MAX);
    levels.add_experience(1);
    assert_eq!(levels.experience(), u32::MAX);
    assert!(!levels.can_level_up());
}

#[test]
#[should_panic(expected = "maximum level")]
fn a_level_20_character_takes_no_level_in_another_class() {
    let mut levels = level_20_fighter();
    levels.level_up(ClassId::new("nat20_core", "class.wizard"));
}
