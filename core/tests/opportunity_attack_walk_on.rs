//! An opportunity attack interrupts a walk, it doesn't end it: once the
//! reaction is settled, the mover walks on to its goal and takes the action it
//! was walking to take (SRD 5.2.1, Opportunity Attacks; nat20-table TASKS.md
//! ENG-22). Each creature gets one chance per walk.

use nat20_core::{
    components::{
        activity::{ActivityState, ActivityStateTag},
        d20::{D20CheckKind, D20CheckOutcome},
        damage::AttackSource,
        items::equipment::weapon::WeaponKind,
    },
    systems,
    test_utils::scenario::Scenario,
};
use uom::si::{f32::Length, length::meter};

/// The reach of the fighter in meters: radius + 5 ft melee attack range.
const FIGHTER_REACH: f32 = 1.974;

fn scenario_with(creatures: &[(&str, &str, f32)]) -> Scenario {
    let mut scenario = Scenario::new();
    for (handle, id, x) in creatures {
        scenario
            .spawn(*handle, *id)
            .position([*x, 0.0, 0.0], true)
            .spawn();
    }
    scenario
}

fn activity(scenario: &Scenario, handle: &str) -> ActivityStateTag {
    systems::helpers::get_component::<ActivityState>(
        &scenario.engine_state.world,
        scenario.entity(handle),
    )
    .tag()
}

#[test]
fn after_a_declined_opportunity_attack_the_mover_walks_on_to_its_goal() {
    let mut scenario = scenario_with(&[
        ("fighter", "hero.fighter", 0.0),
        ("goblin", "monster.goblin_warrior", 1.0),
    ]);

    scenario.movement("goblin", [3.0, 0.0, 0.0]);
    scenario
        .probe("goblin")
        .assert_position([FIGHTER_REACH, 0.0, 0.0], Length::new::<meter>(0.01));
    assert_eq!(activity(&scenario, "goblin"), ActivityStateTag::Moving);

    scenario.react("fighter").option_none().perform();

    scenario
        .probe("goblin")
        .assert_position([3.0, 0.0, 0.0], Length::new::<meter>(0.01));
    assert_eq!(activity(&scenario, "goblin"), ActivityStateTag::Idle);
}

#[test]
fn after_a_missed_opportunity_attack_the_mover_walks_on_and_is_not_attacked_again() {
    let mut scenario = scenario_with(&[
        ("fighter", "hero.fighter", 0.0),
        ("goblin", "monster.goblin_warrior", 1.0),
    ]);

    scenario.movement("goblin", [3.0, 0.0, 0.0]);
    scenario.probe("fighter").d20_force_outcome(
        D20CheckKind::AttackRoll(AttackSource::Weapon(WeaponKind::Melee)),
        D20CheckOutcome::Failure,
    );
    scenario.react("fighter").option_index(0).perform();

    scenario
        .probe("goblin")
        .assert_position([3.0, 0.0, 0.0], Length::new::<meter>(0.01));
    // No second window for the same walk.
    scenario
        .react("fighter")
        .option_index(0)
        .assert_perform_fails();
}

#[test]
fn a_walk_first_attack_goes_on_after_an_allys_opportunity_attack_is_declined() {
    // The fighter stands in the cleric's reach and attacks a goblin out of its
    // own reach, so it walks first and leaves the cleric's reach on the way.
    let mut scenario = scenario_with(&[
        ("cleric", "hero.cleric", 0.0),
        ("fighter", "hero.fighter", 1.0),
        ("goblin", "monster.goblin_warrior", 6.0),
    ]);

    scenario
        .probe("fighter")
        .act("action.melee_attack")
        .context_filter(|context, _cost| context.is_weapon_attack() && context.is_melee_attack())
        .target_entity("goblin")
        .perform();
    assert_eq!(activity(&scenario, "fighter"), ActivityStateTag::Moving);
    scenario
        .event_filter()
        .actor("fighter")
        .action_requested("action.melee_attack")
        .assert_event_count(0);

    scenario.react("cleric").option_none().perform();

    // The fighter walked on into reach and took the attack it carried.
    scenario
        .event_filter()
        .actor("fighter")
        .action_requested("action.melee_attack")
        .assert_event();
}

#[test]
fn a_walk_goes_on_past_a_creature_that_cant_see_it_leave() {
    // The cleric stands between the fighter and the goblin, so the fighter can't
    // see the goblin where it leaves the fighter's reach: no window opens there,
    // and the walk doesn't stop for it. It stops next where it leaves the
    // cleric's reach, since the cleric can see it.
    let mut scenario = scenario_with(&[
        ("fighter", "hero.fighter", 0.0),
        ("cleric", "hero.cleric", 0.95),
        ("goblin", "monster.goblin_warrior", 1.75),
    ]);

    scenario.movement("goblin", [4.0, 0.0, 0.0]);
    let at = scenario.probe("goblin").position();
    assert!(at.x > FIGHTER_REACH + 0.5, "the walk stopped at {at}");
    assert_eq!(activity(&scenario, "goblin"), ActivityStateTag::Moving);
    scenario
        .react("fighter")
        .option_index(0)
        .assert_perform_fails();

    scenario.react("cleric").option_none().perform();
    scenario
        .probe("goblin")
        .assert_position([4.0, 0.0, 0.0], Length::new::<meter>(0.01));
}
