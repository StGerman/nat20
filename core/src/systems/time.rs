use std::collections::HashMap;

use hecs::{Entity, World};
use serde::{Deserialize, Serialize};
use tracing::{debug, error, info};

use crate::{
    components::{
        health::hit_point_dice::HitPointDice,
        id::EntityIdentifier,
        resource::RechargeRule,
        time::{EntityClock, TimeMode, TimeStep, TurnBoundary},
    },
    engine::{
        event::{Event, EventKind},
        engine_state::EngineState,
    },
    systems,
};

pub fn set_time_mode(world: &mut World, entity: Entity, mode: TimeMode) {
    let clock = systems::helpers::get_component_mut::<EntityClock>(world, entity);
    clock.set_mode(mode);
}

pub fn advance_time(engine_state: &mut EngineState, entity: Entity, time_step: TimeStep) {
    // TODO: Recharge resources on time advance?
    {
        let clock =
            systems::helpers::get_component_mut::<EntityClock>(&mut engine_state.world, entity);

        match (clock.mode(), time_step) {
            (TimeMode::Paused, _) | (TimeMode::TurnBased { .. }, TimeStep::RealTime { .. }) => {
                return;
            }
            _ => { /* valid combination, continue */ }
        }

        clock.update(time_step);
    }

    match time_step {
        TimeStep::TurnBoundary {
            entity: turn_entity,
            boundary,
        } => {
            debug!(
                "Advancing turn-based time for entity {:?} at turn boundary {:?} of entity {:?}",
                entity, boundary, turn_entity
            );

            if turn_entity == entity {
                engine_state.process_event(Event::new(EventKind::TurnBoundary {
                    entity: EntityIdentifier::from_world(&engine_state.world, entity),
                    boundary,
                }));

                match boundary {
                    TurnBoundary::Start => systems::time::on_turn_start(engine_state, entity),
                    TurnBoundary::End => systems::time::on_turn_end(&mut engine_state.world, entity),
                }
            }
        }
        _ => { /* no special logging for other time steps */ }
    }

    systems::effects::effects_mut(&mut engine_state.world, entity).advance_time(time_step);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RestKind {
    Short,
    Long,
}

impl RestKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            RestKind::Short => "short",
            RestKind::Long => "long",
        }
    }
}

#[derive(Debug, Clone)]
pub enum RestError {
    InCombat { entities: Vec<Entity> },
    NotResting { entities: Vec<Entity> },
    DifferentRestKinds { entities: HashMap<Entity, RestKind> },
    /// A Long Rest needs at least 1 Hit Point to start
    NoHitPoints { entities: Vec<Entity> },
    /// Less than 16 hours have passed since these entities finished a Long Rest
    TooSoon { entities: Vec<Entity> },
}
/// After a Long Rest ends, a creature must wait at least 16 hours before starting
/// another one (SRD 5.2.1, Long Rest).
pub const LONG_REST_WAIT_SECONDS: f32 = 16.0 * 60.0 * 60.0;

/// When the entity last finished a Long Rest, on its own clock.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LongRestFinished {
    pub at_seconds: f32,
}

fn local_time(world: &World, entity: Entity) -> f32 {
    world
        .get::<&EntityClock>(entity)
        .map(|clock| clock.local_time_seconds())
        .unwrap_or(0.0)
}

/// How long the entity must still wait before it can start a Long Rest, in seconds;
/// 0 when it can start one now.
pub fn long_rest_wait_left(world: &World, entity: Entity) -> f32 {
    match world.get::<&LongRestFinished>(entity) {
        Ok(finished) => {
            let since = local_time(world, entity) - finished.at_seconds;
            (LONG_REST_WAIT_SECONDS - since).max(0.0)
        }
        Err(_) => 0.0,
    }
}

pub fn on_turn_start(engine_state: &mut EngineState, entity: Entity) {
    debug!("Starting turn for entity {:?}", entity);
    systems::resources::recharge(&mut engine_state.world, entity, &RechargeRule::Turn);
    systems::movement::recharge_movement(&mut engine_state.world, entity);

    let hooks = systems::effects::effects(&engine_state.world, entity)
        .collect_hooks(|effect| effect.on_turn_start.as_ref());
    for hook in hooks {
        hook(engine_state, entity);
    }
}

pub fn on_turn_end(_world: &mut World, _entity: Entity) {
    debug!("Ending turn for entity {:?}", _entity);
    // Currently nothing happens at turn end
}

pub fn start_rest(
    engine_state: &mut EngineState,
    participants: Vec<Entity>,
    kind: &RestKind,
) -> Result<(), RestError> {
    info!("Starting {:?} rest for entities {:?}", kind, participants);

    let entities_in_combat = entities_in_combat(engine_state, &participants);
    if !entities_in_combat.is_empty() {
        error!("Entities in combat cannot rest: {:?}", entities_in_combat);
        return Err(RestError::InCombat {
            entities: entities_in_combat,
        });
    }

    if *kind == RestKind::Long {
        let no_hit_points: Vec<Entity> = participants
            .iter()
            .copied()
            .filter(|&entity| !systems::health::is_alive(&engine_state.world, entity))
            .collect();
        if !no_hit_points.is_empty() {
            error!(
                "Entities with 0 Hit Points cannot start a Long Rest: {:?}",
                no_hit_points
            );
            return Err(RestError::NoHitPoints {
                entities: no_hit_points,
            });
        }
        let too_soon: Vec<Entity> = participants
            .iter()
            .copied()
            .filter(|&entity| long_rest_wait_left(&engine_state.world, entity) > 0.0)
            .collect();
        if !too_soon.is_empty() {
            error!(
                "Entities finished a Long Rest less than 16 hours ago: {:?}",
                too_soon
            );
            return Err(RestError::TooSoon { entities: too_soon });
        }
    }

    let event = Event::new(EventKind::RestStarted {
        kind: *kind,
        participants: participants
            .iter()
            .map(|&entity| EntityIdentifier::from_world(&engine_state.world, entity))
            .collect(),
    });
    engine_state.process_event(event);

    participants.iter().for_each(|&entity| {
        engine_state.resting.insert(entity, *kind);
    });

    Ok(())
}

pub fn finish_rest(engine_state: &mut EngineState, participants: Vec<Entity>) -> Result<(), RestError> {
    info!("Finishing rest for entities {:?}", participants);

    // Check that all participants are actually resting and of the same kind
    let mut not_resting_entities: Vec<Entity> = Vec::new();
    let mut rest_kinds = HashMap::new();
    for &entity in &participants {
        if let Some(kind) = engine_state.resting.remove(&entity) {
            rest_kinds.insert(entity, kind);
        } else {
            not_resting_entities.push(entity);
        }
    }

    if !not_resting_entities.is_empty() {
        error!("Entities not resting: {:?}", not_resting_entities);
        return Err(RestError::NotResting {
            entities: not_resting_entities,
        });
    }

    let first_kind = rest_kinds
        .values()
        .next()
        .expect("At least one participant should be present");

    if rest_kinds.values().any(|kind| kind != first_kind) {
        error!("Entities with different rest kinds: {:#?}", rest_kinds);
        return Err(RestError::DifferentRestKinds {
            entities: rest_kinds,
        });
    }

    let event = Event::new(EventKind::RestFinished {
        kind: *first_kind,
        participants: participants
            .iter()
            .map(|&entity| EntityIdentifier::from_world(&engine_state.world, entity))
            .collect(),
    });
    engine_state.process_event(event);

    on_rest_end(engine_state, &participants, first_kind);

    Ok(())
}

fn entities_in_combat(engine_state: &EngineState, participants: &[Entity]) -> Vec<Entity> {
    // Can only rest if no one is in combat
    participants
        .iter()
        .cloned()
        .filter(|entity| systems::combat::is_in_combat(&engine_state, *entity))
        .collect()
}

pub fn on_rest_end(engine_state: &mut EngineState, participants: &[Entity], kind: &RestKind) {
    for &entity in participants {
        let world = &mut engine_state.world;
        match kind {
            RestKind::Short => {
                systems::resources::recharge(world, entity, &RechargeRule::Rest(RestKind::Short));
                // Healing comes from the Hit Point Dice spent during the rest,
                // see `systems::health::spend_hit_point_die`
            }

            RestKind::Long => {
                systems::resources::recharge(world, entity, &RechargeRule::Rest(RestKind::Long));
                systems::health::heal_full(world, entity);
                if let Ok(mut hit_point_dice) = world.get::<&mut HitPointDice>(entity) {
                    hit_point_dice.restore_all();
                }
                // The 16-hour wait before the next Long Rest counts from here. Effects
                // with a duration end as the rest's hours pass, which the caller advances;
                // the SRD ends nothing else at the end of a Long Rest.
                let at_seconds = local_time(world, entity);
                let _ = world.insert_one(entity, LongRestFinished { at_seconds });
            }
        }

        // Hooks run last so they see the recharged, healed-up creature
        let rest_hooks = systems::effects::effects(&engine_state.world, entity)
            .collect_hooks(|effect| effect.on_rest.as_ref());
        for hook in rest_hooks {
            hook(engine_state, entity, kind);
        }
    }
}
