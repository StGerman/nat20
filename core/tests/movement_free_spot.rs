//! A creature ends its move on a free spot: it can move through another
//! creature's space, but it can't end its move there. That holds for a walk
//! into range of its target (nat20-table TASKS.md ENG-12) and for a move an
//! opportunity attack stops (ENG-18).

use nat20_core::{
    components::{
        actions::targeting::{LineOfSight, TargetInstance},
        activity::Activity,
        speed::Speed,
    },
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

/// The fighter's reach in meters (radius + 5 ft), as in `opportunity_attack.rs`.
const FIGHTER_REACH: f32 = 1.974;
/// The cleric stands where the goblin leaves the fighter's reach.
const CLERIC_X: f32 = 2.2;
const GOBLIN_START_X: f32 = 1.0;

/// The goblin, in the fighter's reach, walks away through the cleric's space and
/// leaves the fighter's reach while inside it.
fn walk_away_scenario() -> Scenario {
    let mut scenario = Scenario::new();
    scenario
        .spawn("fighter", "hero.fighter")
        .position([0.0, 0.0, 0.0], true)
        .spawn();
    scenario
        .spawn("goblin", "monster.goblin_warrior")
        .position([GOBLIN_START_X, 0.0, 0.0], true)
        .spawn();
    scenario
        .spawn("cleric", "hero.cleric")
        .position([CLERIC_X, 0.0, 0.0], true)
        .spawn();
    scenario
}

#[test]
fn a_move_an_opportunity_attack_stops_ends_on_a_free_spot() {
    let mut scenario = walk_away_scenario();
    let goblin = scenario.entity("goblin");
    let provoked = [FIGHTER_REACH, 0.0, 0.0].into();
    assert!(
        !systems::movement::is_free_spot(&scenario.engine_state, goblin, &provoked),
        "the goblin must leave the fighter's reach inside the cleric's space"
    );

    scenario.movement("goblin", [3.0, 0.0, 0.0]);

    let stop = scenario.probe("goblin").position();
    assert!(
        stop.x >= GOBLIN_START_X - 0.01 && stop.x < FIGHTER_REACH - 0.01,
        "{stop} is not back along the way it walked"
    );
    assert!(
        systems::movement::is_free_spot(&scenario.engine_state, goblin, &stop),
        "{stop} is in another creature's space"
    );
    // Still in the fighter's reach, so the fighter gets the opportunity attack.
    scenario.react("fighter").option_index(0).perform();
}

#[test]
fn a_stop_behind_where_the_walk_had_got_to_gives_that_movement_back() {
    let mut scenario = walk_away_scenario();
    scenario
        .encounter()
        .initiative_order(vec!["goblin", "fighter", "cleric"])
        .build();
    let goblin = scenario.entity("goblin");
    scenario
        .engine_state
        .submit_activity(Activity::Move {
            entity: goblin,
            goal: [3.0, 0.0, 0.0].into(),
        })
        .expect("the goblin can move");
    // Short steps, so the step that leaves the fighter's reach starts inside
    // the cleric's space and the stop is behind where the walk had got to.
    const STEP_SECONDS: f32 = 0.1;
    for _ in 0..40 {
        scenario.update(STEP_SECONDS);
    }

    let stop = scenario.probe("goblin").position();
    let first_step = GOBLIN_START_X + systems::movement::MOVEMENT_SPEED * STEP_SECONDS;
    assert!(stop.x < first_step, "{stop} is not behind {first_step}");
    assert!(
        systems::movement::is_free_spot(&scenario.engine_state, goblin, &stop),
        "{stop} is in another creature's space"
    );
    let mut moved = 0.0;
    scenario
        .probe("goblin")
        .component::<Speed>(|speed| moved = speed.moved_this_turn().get::<meter>());
    assert!(
        (moved - (stop.x - GOBLIN_START_X)).abs() < 0.01,
        "moved {moved} m to stop at {stop}"
    );
}
