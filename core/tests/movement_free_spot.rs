//! A creature walking into range of its target ends its move on a free spot: it
//! can move through another creature's space, but it can't end its move there
//! (nat20-table TASKS.md ENG-12).

use nat20_core::{
    components::actions::targeting::{LineOfSight, TargetInstance},
    systems,
    test_utils::scenario::Scenario,
};
use parry3d::na::Point3;
use uom::si::{f32::Length, length::meter};

/// Melee reach, measured from the walker to the target's centre.
const RANGE: f32 = 1.5;
const GOBLIN_X: f32 = 8.0;

fn walk_scenario(blocker_x: Option<f32>) -> Scenario {
    let mut scenario = Scenario::new();
    scenario
        .spawn("fighter", "hero.fighter")
        .position([0.0, 0.0, 0.0], true)
        .spawn();
    scenario
        .spawn("goblin", "monster.goblin_warrior")
        .position([GOBLIN_X, 0.0, 0.0], true)
        .spawn();
    if let Some(x) = blocker_x {
        scenario
            .spawn("cleric", "hero.cleric")
            .position([x, 0.0, 0.0], true)
            .spawn();
    }
    scenario
}

/// Where the fighter's walk into range of the goblin ends, and the goblin's centre.
fn walk_to_goblin(scenario: &mut Scenario) -> (Point3<f32>, Point3<f32>) {
    let fighter = scenario.entity("fighter");
    let target = TargetInstance::entity(scenario.entity_identifier("goblin"));
    let result = systems::movement::path_in_range_of_target(
        &mut scenario.engine_state,
        fighter,
        &target,
        Length::new::<meter>(RANGE),
        true,
        LineOfSight::ignore(),
        false,
    )
    .expect("a path into range");
    let end = *result
        .path_result
        .taken_path
        .end()
        .expect("the fighter moves");
    let centre = target
        .position(&scenario.engine_state.world)
        .expect("the goblin has a position");
    (end, centre)
}

#[test]
fn the_space_a_creature_stands_in_is_not_free() {
    let scenario = walk_scenario(None);
    let fighter = scenario.entity("fighter");
    let engine_state = &scenario.engine_state;

    assert!(systems::movement::is_free_spot(
        engine_state,
        fighter,
        &Point3::new(0.0, 0.0, 0.0)
    ));
    assert!(systems::movement::is_free_spot(
        engine_state,
        fighter,
        &Point3::new(4.0, 0.0, 0.0)
    ));
    assert!(!systems::movement::is_free_spot(
        engine_state,
        fighter,
        &Point3::new(GOBLIN_X, 0.0, 0.0)
    ));
}

#[test]
fn a_walk_into_reach_with_nobody_in_the_way_stops_in_reach() {
    let mut scenario = walk_scenario(None);
    let fighter = scenario.entity("fighter");

    let (end, centre) = walk_to_goblin(&mut scenario);

    assert!(
        (end - centre).norm() <= RANGE + 0.01,
        "{end} is out of reach"
    );
    assert!(systems::movement::is_free_spot(
        &scenario.engine_state,
        fighter,
        &end
    ));
}

#[test]
fn a_walk_into_reach_goes_around_a_creature_standing_where_it_would_stop() {
    // The cleric stands where the straight walk would enter the goblin's reach,
    // so the only free spots in reach are off the straight line.
    let mut scenario = walk_scenario(Some(GOBLIN_X - RANGE));
    let fighter = scenario.entity("fighter");

    let (end, centre) = walk_to_goblin(&mut scenario);

    assert!(
        (end - centre).norm() <= RANGE + 0.01,
        "{end} is out of reach"
    );
    assert!(
        systems::movement::is_free_spot(&scenario.engine_state, fighter, &end),
        "{end} is in another creature's space"
    );
}
