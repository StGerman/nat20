extern crate nat20_core;

use nat20_core::{
    components::{
        actions::action::ActionConditionResolution,
        effects::effect::{EffectEntiyReference, EffectInstanceTemplate, EffectLifetimeTemplate},
        modifier::ModifierSource,
        time::{TimeDuration, TimeMode, TurnBoundary},
    },
    systems,
    test_utils::scenario::Scenario,
};

const RAY_OF_FROST: &str = "effect.spell.ray_of_frost";

#[test]
fn an_effect_ends_at_its_appliers_turn_on_a_creature_outside_the_encounter() {
    let mut scenario = Scenario::new();
    scenario
        .spawn("wizard", "hero.wizard")
        .level(1)
        .position([0.0, 0.0, 0.0], false)
        .spawn();
    scenario
        .spawn("fighter", "hero.fighter")
        .level(1)
        .position([1.0, 0.0, 0.0], false)
        .spawn();
    scenario
        .encounter()
        .initiative_order(vec!["wizard", "fighter"])
        .build();
    // Spawned after the encounter started, so it isn't a participant, and its
    // time passes in turns rather than in real time
    scenario
        .spawn("bystander", "hero.fighter")
        .level(1)
        .position([0.0, 1.0, 0.0], false)
        .time_mode(TimeMode::TurnBased { encounter_id: None })
        .spawn();

    // Until the start of the wizard's next turn, on a creature that isn't fighting
    let (wizard, bystander) = (scenario.entity("wizard"), scenario.entity("bystander"));
    systems::effects::add_effect_template(
        &mut scenario.engine_state,
        wizard,
        bystander,
        ModifierSource::Custom("Test effect".to_string()),
        &EffectInstanceTemplate {
            effect_id: RAY_OF_FROST.into(),
            lifetime: EffectLifetimeTemplate::TurnBoundary {
                entity: EffectEntiyReference::Applier,
                boundary: TurnBoundary::Start,
                duration: TimeDuration::from_turns(1),
            },
            end_condition: None,
        },
        None,
        ActionConditionResolution::Unconditional,
    );
    scenario.probe("bystander").assert_effect(RAY_OF_FROST);

    scenario.probe("wizard").end_turn();
    scenario.probe("bystander").assert_effect(RAY_OF_FROST);
    scenario.probe("fighter").end_turn();
    scenario.probe("bystander").assert_no_effect(RAY_OF_FROST);
}
