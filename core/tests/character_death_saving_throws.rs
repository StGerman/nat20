extern crate nat20_core;

use nat20_core::{
    components::{
        damage::{DamageComponentResult, DamageRollResult, DamageType},
        health::life_state::LifeState,
        modifier::{ModifierKindResult, ModifierResult, ModifierSource},
        time::TimeMode,
    },
    systems,
    test_utils::scenario::Scenario,
};

/// A fighter at 0 Hit Points who is Stable
fn stable_fighter() -> Scenario {
    let mut scenario = Scenario::new();
    scenario
        .spawn("fighter", "hero.fighter")
        .level(1)
        .time_mode(TimeMode::TurnBased { encounter_id: None })
        .spawn();
    let hp = scenario.probe("fighter").hp();
    scenario
        .probe("fighter")
        .damage(hp, DamageType::Bludgeoning);
    let fighter = scenario.entity("fighter");
    *scenario
        .engine_state
        .world
        .get::<&mut LifeState>(fighter)
        .unwrap() = LifeState::Stable;
    scenario
}

fn hit(scenario: &mut Scenario, crit: bool) -> LifeState {
    let mut damage = DamageRollResult {
        components: vec![DamageComponentResult {
            result: ModifierResult::from_iter([(
                ModifierSource::Custom("Test damage".to_string()),
                ModifierKindResult::Flat(3),
            )]),
            damage_type: DamageType::Slashing,
        }],
        total: 3,
        crit,
    };
    let fighter = scenario.entity("fighter");
    systems::health::damage(&mut scenario.engine_state, fighter, &mut damage, None, None);
    *scenario
        .engine_state
        .world
        .get::<&LifeState>(fighter)
        .unwrap()
}

#[test]
fn damage_to_a_stable_creature_is_a_death_saving_throw_failure() {
    let mut scenario = stable_fighter();

    let LifeState::Unconscious(death_saving_throws) = hit(&mut scenario, false) else {
        panic!("a Stable creature that takes damage makes Death Saving Throws again");
    };
    assert_eq!(death_saving_throws.failures(), 1);
    assert_eq!(death_saving_throws.successes(), 0);
}

#[test]
fn a_critical_hit_on_a_stable_creature_is_two_death_saving_throw_failures() {
    let mut scenario = stable_fighter();

    let LifeState::Unconscious(death_saving_throws) = hit(&mut scenario, true) else {
        panic!("a Stable creature that takes damage makes Death Saving Throws again");
    };
    assert_eq!(death_saving_throws.failures(), 2);
}
