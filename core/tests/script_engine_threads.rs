//! The script engine is process-wide, so engines on several threads share it (one engine per
//! test thread in a test binary, for one). The Wolf's Bite result hook runs inside Lua and knocks
//! its target Prone, which collects reactions and so reads usability modules from the cache while
//! holding the Lua lock. Reading a cached module from outside Lua must not hold the cache lock
//! while it waits for the Lua lock, or the two threads deadlock.

extern crate nat20_core;

use std::{sync::mpsc, thread, time::Duration};

use nat20_core::{
    components::{
        d20::{D20CheckKind, D20CheckOutcome},
        damage::AttackSource,
        items::equipment::weapon::WeaponKind,
        time::TimeMode,
    },
    systems,
    test_utils::scenario::Scenario,
};

const MELEE_ATTACK_ROLL: D20CheckKind =
    D20CheckKind::AttackRoll(AttackSource::Weapon(WeaponKind::Melee));

#[test]
fn engines_on_several_threads_run_scripts_without_deadlock() {
    const THREADS: usize = 8;
    const ROUNDS: usize = 40;
    let (done, finished) = mpsc::channel();
    for _ in 0..THREADS {
        let done = done.clone();
        thread::spawn(move || {
            for _ in 0..ROUNDS {
                let mut scenario = Scenario::new();
                scenario
                    .spawn("fighter", "hero.fighter")
                    .level(5)
                    .time_mode(TimeMode::TurnBased { encounter_id: None })
                    .spawn();
                scenario
                    .spawn("cleric", "hero.cleric")
                    .level(2)
                    .position([-1.0, 0.0, 0.0], false)
                    .time_mode(TimeMode::TurnBased { encounter_id: None })
                    .spawn();
                scenario
                    .spawn("wolf", "stat_block.wolf")
                    .position([1.0, 0.0, 0.0], false)
                    .time_mode(TimeMode::TurnBased { encounter_id: None })
                    .spawn();
                // Inside Lua: the bite's hook knocks the fighter Prone.
                scenario
                    .probe("wolf")
                    .d20_force_outcome(MELEE_ATTACK_ROLL, D20CheckOutcome::Success)
                    .act("action.melee_attack")
                    .target_entity("fighter")
                    .perform();
                // Outside Lua: listing actions reads cached usability modules.
                for handle in ["fighter", "cleric", "wolf"] {
                    let entity = scenario.entity(handle);
                    let actions =
                        systems::actions::available_actions(&scenario.engine_state, entity);
                    assert!(!actions.is_empty(), "{handle} has no actions");
                }
            }
            done.send(()).unwrap();
        });
    }
    for n in 0..THREADS {
        finished
            .recv_timeout(Duration::from_secs(120))
            .unwrap_or_else(|_| {
                panic!(
                    "{} of {THREADS} threads still running scripts after 120 s: deadlock",
                    THREADS - n
                )
            });
    }
}
