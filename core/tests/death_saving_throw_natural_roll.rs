extern crate nat20_core;

use nat20_core::{
    components::{
        d20::{D20CheckKind, D20CheckOutcome},
        damage::DamageType,
        health::life_state::LifeState,
        saving_throw::SavingThrowKind,
    },
    engine::event::EventKind,
    systems,
    test_utils::scenario::Scenario,
};

const DEATH_SAVE: D20CheckKind = D20CheckKind::SavingThrow(SavingThrowKind::Death);

fn life_state(scenario: &Scenario, handle: &str) -> LifeState {
    *systems::helpers::get_component::<LifeState>(
        &scenario.engine_state.world,
        scenario.entity(handle),
    )
}

/// The fighter's turn, then the wizard's, with the wizard at 0 HP and making Death Saving
/// Throws; the wizard's next save comes out as `outcome`
fn dying_wizard_rolls(outcome: D20CheckOutcome) -> Scenario {
    let mut scenario = Scenario::new();
    scenario
        .spawn("fighter", "hero.fighter")
        .level(1)
        .position([0.0, 0.0, 0.0], false)
        .spawn();
    scenario
        .spawn("wizard", "hero.wizard")
        .level(2)
        .position([1.0, 0.0, 0.0], false)
        .spawn();
    let hp = scenario.probe("wizard").hp();
    scenario.probe("wizard").damage(hp, DamageType::default());
    scenario
        .encounter()
        .initiative_order(vec!["fighter", "wizard"])
        .build();
    scenario
        .probe("wizard")
        .d20_force_outcome(DEATH_SAVE, outcome);
    scenario.probe("fighter").end_turn();
    scenario
}

/// SRD 5.2.1, Death Saving Throws: "If you roll a 20 on the d20, you regain 1 Hit Point."
#[test]
fn a_natural_20_on_a_death_saving_throw_restores_1_hit_point() {
    let mut scenario = dying_wizard_rolls(D20CheckOutcome::CriticalSuccess);

    assert_eq!(scenario.probe("wizard").hp(), 1);
    assert_eq!(life_state(&scenario, "wizard"), LifeState::Normal);

    // Logged globally: Death Saving Throws are made while the encounter changes turns
    let wizard = scenario.entity("wizard");
    let conscious = scenario
        .engine_state
        .event_log
        .events
        .iter()
        .filter(|event| {
            matches!(
                &event.kind,
                EventKind::LifeStateChanged {
                    entity,
                    new_state: LifeState::Normal,
                    actor: None,
                } if entity.id() == wizard
            )
        })
        .count();
    assert_eq!(conscious, 1);

    // Awake at the start of its turn, the wizard takes that turn
    let encounter = *scenario.engine_state.encounters.keys().next().unwrap();
    assert_eq!(
        scenario
            .engine_state
            .encounter(&encounter)
            .unwrap()
            .current_entity(),
        wizard
    );
}

/// SRD 5.2.1, Death Saving Throws: "If you roll a 1 on the d20, that counts as two failures."
#[test]
fn a_natural_1_on_a_death_saving_throw_is_two_failures() {
    let scenario = dying_wizard_rolls(D20CheckOutcome::CriticalFailure);

    let LifeState::Unconscious(death_saving_throws) = life_state(&scenario, "wizard") else {
        panic!("two failures leave the wizard dying");
    };
    assert_eq!(death_saving_throws.failures(), 2);
    assert_eq!(death_saving_throws.successes(), 0);
}
