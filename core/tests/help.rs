extern crate nat20_core;

use nat20_core::{
    components::{
        actions::{
            action::ActionContext,
            targeting::{TargetInstance, TargetingError},
        },
        activity::ActivityError,
        d20::{AdvantageType, D20CheckDC, D20CheckKind, D20CheckOutcome},
        damage::{AttackSource, DamageType},
        health::life_state::LifeState,
        items::equipment::{slots::EquipmentSlot, weapon::WeaponKind},
        modifier::{ModifierMap, ModifierSource},
        resource::{ResourceAmount, ResourceAmountMap},
        saving_throw::SavingThrowKind,
        skill::Skill,
    },
    engine::{
        action_prompt::{ActionData, ActionDecisionKind, ActionError},
        event::{EventFilter, EventKind},
    },
    systems::{self, actions::ActionUsabilityError},
    test_utils::scenario::Scenario,
};

const HELP_ATHLETICS: &str = "effect.help.ability_check.athletics";
const HELP_ATTACK_ROLL: &str = "effect.help.attack_roll";

fn skill_check(skill: Skill) -> D20CheckDC {
    D20CheckDC::Skill {
        skill,
        dc: ModifierMap::from(ModifierSource::Custom("Test skill check".to_string()), 10)
            .evaluate(),
    }
}

fn life_state(scenario: &Scenario, handle: &str) -> LifeState {
    *systems::helpers::get_component::<LifeState>(
        &scenario.engine_state.world,
        scenario.entity(handle),
    )
}

/// The fighter (proficient in Athletics, Acrobatics, Intimidation and
/// Perception) next to the wizard (Arcana, History, Insight and Investigation).
/// Level 1, so the fighter has no Tactical Mind to react to a failed check
fn party() -> Scenario {
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
    scenario
}

#[test]
fn help_ability_check_advantage_is_spent_by_the_chosen_skill() {
    let mut scenario = party();

    scenario
        .act("fighter", "action.help.assist_ability_check")
        .variant("variant.help.athletics")
        .target_entity("wizard")
        .perform();

    scenario
        .probe("wizard")
        .assert_effect(HELP_ATHLETICS)
        .assert_d20_advantage(
            &D20CheckKind::Skill(Skill::Athletics),
            &ModifierSource::Effect(HELP_ATHLETICS.into()),
            AdvantageType::Advantage,
        )
        // A check with another skill leaves the benefit alone
        .d20_check(&skill_check(Skill::Acrobatics))
        .assert_effect(HELP_ATHLETICS)
        // The next Athletics check has Advantage and spends it
        .d20_check(&skill_check(Skill::Athletics))
        .assert_no_effect(HELP_ATHLETICS)
        .d20_check(&skill_check(Skill::Athletics));

    scenario
        .event_filter()
        .actor("wizard")
        .d20_advantage(
            D20CheckKind::Skill(Skill::Athletics),
            ModifierSource::Effect(HELP_ATHLETICS.into()),
            AdvantageType::Advantage,
        )
        .assert_event_count(1);
}

#[test]
fn help_ability_check_expires_at_the_start_of_the_helpers_next_turn() {
    let mut scenario = party();
    scenario
        .encounter()
        .initiative_order(vec!["fighter", "wizard"])
        .build();

    scenario
        .act("fighter", "action.help.assist_ability_check")
        .variant("variant.help.athletics")
        .target_entity("wizard")
        .perform();

    scenario.probe("fighter").end_turn();
    scenario
        .probe("wizard")
        .assert_effect(HELP_ATHLETICS)
        .end_turn()
        .assert_no_effect(HELP_ATHLETICS);
}

#[test]
fn help_ability_check_from_two_helpers_lasts_until_the_later_helpers_turn() {
    let mut scenario = party();
    scenario
        .spawn("fighter_2", "hero.fighter")
        .level(1)
        .position([0.0, 0.0, 1.0], false)
        .spawn();
    scenario
        .encounter()
        .initiative_order(vec!["fighter", "fighter_2", "wizard"])
        .build();

    for helper in ["fighter", "fighter_2"] {
        scenario
            .act(helper, "action.help.assist_ability_check")
            .variant("variant.help.athletics")
            .target_entity("wizard")
            .perform();
        scenario.probe(helper).end_turn();
    }
    scenario.probe("wizard").end_turn();

    // The first helper's turn starts, the second Help still stands
    scenario
        .probe("wizard")
        .assert_effect(HELP_ATHLETICS)
        .assert_d20_advantage(
            &D20CheckKind::Skill(Skill::Athletics),
            &ModifierSource::Effect(HELP_ATHLETICS.into()),
            AdvantageType::Advantage,
        );
    scenario.probe("fighter").end_turn();
    scenario.probe("wizard").assert_no_effect(HELP_ATHLETICS);
}

#[test]
fn help_ability_check_requires_proficiency_in_the_skill() {
    let mut scenario = party();

    // The fighter isn't proficient in Medicine
    let action = ActionData::new(
        scenario.entity_identifier("fighter"),
        "action.help.assist_ability_check".into(),
        ActionContext::default(),
        ResourceAmountMap::from_iter(vec![("resource.action".into(), ResourceAmount::Flat(1))]),
        vec![TargetInstance::entity(scenario.entity_identifier("wizard"))],
    )
    .with_variant("variant.help.medicine");
    let result = scenario.submit_action_decision(ActionDecisionKind::Action { action });

    assert_eq!(
        result,
        Err(ActivityError::ActionError(ActionError::Usability(
            ActionUsabilityError::UsabilityFunctionError(
                "Requires proficiency in Medicine".to_string()
            )
        )))
    );
    scenario
        .probe("wizard")
        .assert_no_effect("effect.help.ability_check.medicine");
}

#[test]
fn help_attack_roll_gives_the_next_attack_by_an_ally_advantage() {
    let mut scenario = Scenario::new();
    scenario
        .spawn("fighter", "hero.fighter")
        .level(2)
        .position([0.0, 0.0, 0.0], false)
        .spawn();
    scenario
        .spawn("ally", "hero.fighter")
        .level(1)
        .position([0.0, 0.0, 1.0], false)
        .spawn();
    scenario
        .spawn("goblin", "monster.goblin_warrior")
        .level(5)
        .position([1.0, 0.0, 0.0], false)
        .spawn();
    scenario
        .spawn("goblin_2", "monster.goblin_warrior")
        .level(5)
        .position([1.0, 0.0, 1.0], false)
        .spawn();
    scenario
        .encounter()
        .initiative_order(vec!["fighter", "ally", "goblin", "goblin_2"])
        .build();

    scenario
        .act("fighter", "action.help.assist_attack_roll")
        .target_entity("goblin")
        .perform();
    scenario.probe("goblin").assert_effect(HELP_ATTACK_ROLL);

    let advantage = |scenario: &mut Scenario, attacker: &str| {
        scenario
            .probe(attacker)
            .preview_attack_roll(
                "goblin",
                "action.melee_attack",
                EquipmentSlot::MeleeMainHand,
            )
            .advantage_tracker()
            .summary()
            .get(&ModifierSource::Effect(HELP_ATTACK_ROLL.into()))
            .copied()
    };
    // Neither the helper nor the goblin's own allies benefit
    assert_eq!(advantage(&mut scenario, "fighter"), None);
    assert_eq!(advantage(&mut scenario, "goblin_2"), None);
    assert_eq!(
        advantage(&mut scenario, "ally"),
        Some(AdvantageType::Advantage)
    );

    // The helper's own attack doesn't spend it. Miss, so the goblin survives
    let melee = D20CheckKind::AttackRoll(AttackSource::Weapon(WeaponKind::Melee));
    scenario
        .probe("fighter")
        .d20_force_outcome(melee.clone(), D20CheckOutcome::CriticalFailure)
        .act("action.fighter.action_surge")
        .perform();
    scenario
        .act("fighter", "action.melee_attack")
        .target_entity("goblin")
        .perform();
    scenario.probe("goblin").assert_effect(HELP_ATTACK_ROLL);

    scenario.probe("fighter").end_turn();
    scenario
        .probe("ally")
        .d20_force_outcome(melee.clone(), D20CheckOutcome::CriticalFailure)
        .act("action.melee_attack")
        .target_entity("goblin")
        .perform();

    scenario
        .event_filter()
        .actor("ally")
        .d20_advantage(
            melee,
            ModifierSource::Effect(HELP_ATTACK_ROLL.into()),
            AdvantageType::Advantage,
        )
        .assert_event_count(1);
    scenario.probe("goblin").assert_no_effect(HELP_ATTACK_ROLL);
    assert_eq!(advantage(&mut scenario, "ally"), None);
}

#[test]
fn help_attack_roll_spent_by_an_ally_leaves_that_allys_own_help() {
    let mut scenario = Scenario::new();
    scenario
        .spawn("fighter", "hero.fighter")
        .level(1)
        .position([0.0, 0.0, 0.0], false)
        .spawn();
    scenario
        .spawn("ally", "hero.fighter")
        .level(2)
        .position([0.0, 0.0, 1.0], false)
        .spawn();
    scenario
        .spawn("goblin", "monster.goblin_warrior")
        .level(5)
        .position([1.0, 0.0, 0.0], false)
        .spawn();
    scenario
        .encounter()
        .initiative_order(vec!["fighter", "ally", "goblin"])
        .build();

    scenario
        .act("fighter", "action.help.assist_attack_roll")
        .target_entity("goblin")
        .perform();
    scenario.probe("fighter").end_turn();
    scenario
        .act("ally", "action.help.assist_attack_roll")
        .target_entity("goblin")
        .perform();

    // The ally's attack draws on the fighter's Help only. Miss, so the goblin survives
    let melee = D20CheckKind::AttackRoll(AttackSource::Weapon(WeaponKind::Melee));
    scenario
        .probe("ally")
        .d20_force_outcome(melee.clone(), D20CheckOutcome::CriticalFailure)
        .act("action.fighter.action_surge")
        .perform();
    scenario
        .act("ally", "action.melee_attack")
        .target_entity("goblin")
        .perform();
    scenario
        .event_filter()
        .actor("ally")
        .d20_advantage(
            melee,
            ModifierSource::Effect(HELP_ATTACK_ROLL.into()),
            AdvantageType::Advantage,
        )
        .assert_event_count(1);

    // The ally's own Help is left for the fighter
    scenario.probe("goblin").assert_effect(HELP_ATTACK_ROLL);
    let advantage = scenario
        .probe("fighter")
        .preview_attack_roll(
            "goblin",
            "action.melee_attack",
            EquipmentSlot::MeleeMainHand,
        )
        .advantage_tracker()
        .summary()
        .get(&ModifierSource::Effect(HELP_ATTACK_ROLL.into()))
        .copied();
    assert_eq!(advantage, Some(AdvantageType::Advantage));
}

/// The fighter next to the wizard, who is at 0 HP and making Death Saving Throws
fn dying_wizard() -> Scenario {
    let mut scenario = party();
    let hp = scenario.probe("wizard").hp();
    scenario.probe("wizard").damage(hp, DamageType::default());
    assert!(matches!(
        life_state(&scenario, "wizard"),
        LifeState::Unconscious(_)
    ));

    scenario
        .encounter()
        .initiative_order(vec!["fighter", "wizard"])
        .build();
    scenario
}

/// Death Saving Throws are made while the encounter is changing turns, which
/// logs them globally rather than in the encounter's log
fn death_saves(scenario: &Scenario) -> usize {
    let wizard = scenario.entity("wizard");
    scenario
        .engine_state
        .event_log
        .events
        .iter()
        .filter(|event| {
            matches!(
                &event.kind,
                EventKind::D20CheckResolved {
                    actor,
                    dc: D20CheckDC::SavingThrow {
                        saving_throw: SavingThrowKind::Death,
                        ..
                    },
                    ..
                } if actor.id() == wizard
            )
        })
        .count()
}

#[test]
fn help_stabilize_success() {
    let mut scenario = dying_wizard();

    scenario
        .probe("fighter")
        .d20_force_outcome(
            D20CheckKind::Skill(Skill::Medicine),
            D20CheckOutcome::Success,
        )
        .act("action.help.stabilize")
        .target_entity("wizard")
        .perform();

    scenario
        .event_filter()
        .actor("fighter")
        .d20_dc(D20CheckKind::Skill(Skill::Medicine), 10)
        .assert_event_count(1);
    assert_eq!(life_state(&scenario, "wizard"), LifeState::Stable);

    let (fighter, wizard) = (scenario.entity("fighter"), scenario.entity("wizard"));
    let stabilized = scenario.filter_events(EventFilter::new(move |event| {
        matches!(
            &event.kind,
            EventKind::LifeStateChanged {
                entity,
                new_state: LifeState::Stable,
                actor: Some(actor),
            } if entity.id() == wizard && actor.id() == fighter
        )
    }));
    assert_eq!(stabilized.len(), 1);

    // A Stable creature doesn't make Death Saving Throws
    scenario.probe("fighter").end_turn();
    assert_eq!(death_saves(&scenario), 0);
    assert_eq!(life_state(&scenario, "wizard"), LifeState::Stable);

    // Taking damage starts them again
    scenario.probe("wizard").damage(1, DamageType::default());
    assert_eq!(life_state(&scenario, "wizard"), LifeState::unconscious());
}

#[test]
fn help_stabilize_failure() {
    let mut scenario = dying_wizard();

    scenario
        .probe("fighter")
        .d20_force_outcome(
            D20CheckKind::Skill(Skill::Medicine),
            D20CheckOutcome::Failure,
        )
        .act("action.help.stabilize")
        .target_entity("wizard")
        .perform();

    assert!(matches!(
        life_state(&scenario, "wizard"),
        LifeState::Unconscious(_)
    ));

    scenario.probe("fighter").end_turn();
    assert_eq!(death_saves(&scenario), 1);
}

#[test]
fn help_stabilize_needs_a_dying_target() {
    let mut scenario = party();

    let action = ActionData::new(
        scenario.entity_identifier("fighter"),
        "action.help.stabilize".into(),
        ActionContext::default(),
        ResourceAmountMap::from_iter(vec![("resource.action".into(), ResourceAmount::Flat(1))]),
        vec![TargetInstance::entity(scenario.entity_identifier("wizard"))],
    );
    let result = scenario.submit_action_decision(ActionDecisionKind::Action { action });

    assert!(
        matches!(
            result,
            Err(ActivityError::ActionError(ActionError::Usability(
                ActionUsabilityError::TargetingError(TargetingError::InvalidTarget { .. })
            )))
        ),
        "expected an invalid target, got {:?}",
        result
    );
    scenario
        .event_filter()
        .actor("fighter")
        .d20_dc(D20CheckKind::Skill(Skill::Medicine), 10)
        .assert_event_count(0);
}
