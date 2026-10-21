extern crate nat20_core;

use std::collections::HashSet;

use nat20_core::{
    engine::{
        encounter::EncounterId,
        event::{EncounterEvent, Event, EventLog},
    },
    test_utils::scenario::Scenario,
};
use uuid::Uuid;

fn round(encounter: EncounterId, round: usize) -> Event {
    Event::encounter_event(EncounterEvent::NewRound(encounter, round))
}

/// Events pushed to two logs in turn can be put back in the order they were pushed by `seq`
#[test]
fn seq_orders_events_across_logs() {
    let encounter = Uuid::new_v4();
    let (mut first, mut second) = (EventLog::new(), EventLog::new());
    first.push(round(encounter, 1));
    second.push(round(encounter, 2));
    first.push(round(encounter, 3));
    second.push(round(encounter, 4));

    let mut merged: Vec<&Event> = first.events.iter().chain(&second.events).collect();
    merged.sort_by_key(|event| event.seq);
    let rounds: Vec<usize> = merged
        .iter()
        .map(|event| match event.kind {
            nat20_core::engine::event::EventKind::Encounter(EncounterEvent::NewRound(_, n)) => n,
            _ => unreachable!(),
        })
        .collect();
    assert_eq!(rounds, [1, 2, 3, 4]);
}

/// An event has no `seq` until it is logged
#[test]
fn seq_is_set_when_an_event_is_logged() {
    let event = round(Uuid::new_v4(), 1);
    assert_eq!(event.seq, 0);
    let mut log = EventLog::new();
    log.push(event);
    assert!(log.events[0].seq > 0);
}

/// A real fight writes to the encounter's log and the global log; every event there has its own
/// `seq`, rising within each log
#[test]
fn a_fight_numbers_both_logs() {
    let mut scenario = Scenario::new();
    scenario.spawn("fighter", "hero.fighter").level(1).spawn();
    scenario
        .spawn("goblin", "monster.goblin_warrior")
        .level(1)
        .position([1.0, 0.0, 0.0], false)
        .spawn();
    let (fighter, goblin) = (scenario.entity("fighter"), scenario.entity("goblin"));
    let encounter = scenario
        .engine_state
        .start_encounter([fighter, goblin].into_iter().collect());

    let engine = &scenario.engine_state;
    let logs = [
        &engine.event_log.events,
        &engine.encounter(&encounter).unwrap().event_log().events,
    ];
    let mut seen = HashSet::new();
    for events in logs {
        assert!(events.windows(2).all(|pair| pair[0].seq < pair[1].seq));
        for event in events.iter() {
            assert!(event.seq > 0, "{event:?}");
            assert!(seen.insert(event.seq), "seq {} twice", event.seq);
        }
    }
    assert!(!seen.is_empty());
}
