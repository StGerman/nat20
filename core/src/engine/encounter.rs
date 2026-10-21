use std::collections::HashSet;

use hecs::{Entity, World};
use uuid::Uuid;

use crate::{
    components::{
        actions::targeting::EntityFilter,
        d20::{AdvantageAware, AdvantageType, D20CheckDC, D20CheckOutcome, D20CheckResult},
        effects::effect_manager::EffectManager,
        health::life_state::{DEATH_SAVING_THROW_DC, LifeState},
        modifier::{ModifierMap, ModifierSource},
        saving_throw::SavingThrowKind,
        skill::{Skill, SkillSet},
        time::{EntityClock, TimeMode, TimeStep, TurnBoundary},
    },
    engine::{
        action_prompt::{ActionPrompt, ActionPromptKind},
        engine_state::EngineState,
        event::{CallbackResult, EncounterEvent, Event, EventCallback, EventKind, EventLog},
        prompt::PromptScopeId,
    },
    systems::{self},
};

pub type EncounterId = Uuid;

#[derive(Debug)]
pub struct Encounter {
    id: EncounterId,
    participants: HashSet<Entity>,
    round: usize,
    turn_index: usize,
    initiative_order: Vec<(Entity, D20CheckResult)>,
    event_log: EventLog,
}

impl Encounter {
    /// `surprised` participants roll Initiative with Disadvantage (SRD 5.2.1 Surprise)
    pub fn new(
        engine_state: &mut EngineState,
        participants: HashSet<Entity>,
        surprised: &HashSet<Entity>,
        id: EncounterId,
    ) -> Self {
        let mut encounter = Self {
            id,
            participants,
            round: 1,
            turn_index: 0,
            initiative_order: Vec::new(),
            event_log: EventLog::new(),
        };
        encounter.roll_initiative(engine_state, surprised);
        encounter.start_turn(engine_state);
        encounter
            .event_log
            .push(Event::encounter_event(EncounterEvent::NewRound(
                encounter.id,
                encounter.round(),
            )));
        encounter
    }

    fn roll_initiative(&mut self, engine_state: &EngineState, surprised: &HashSet<Entity>) {
        // Roll in a fixed order rather than HashSet order, so a seeded run gets
        // the same initiative every time (ties also keep this order).
        let mut participants: Vec<&Entity> = self.participants.iter().collect();
        participants.sort();

        let mut indexed_rolls: Vec<(Entity, D20CheckResult)> = participants
            .into_iter()
            .map(|entity| {
                let mut check =
                    systems::helpers::get_component::<SkillSet>(&engine_state.world, *entity)
                        .get(&Skill::Initiative)
                        .clone();
                if surprised.contains(entity) {
                    check.add_advantage(
                        AdvantageType::Disadvantage,
                        ModifierSource::Custom("Surprised".to_string()),
                    );
                }
                let roll = check.roll_hooks(engine_state, *entity);
                (*entity, roll)
            })
            .collect();

        indexed_rolls.sort_by_key(|(_, roll)| -roll.total());
        self.initiative_order = indexed_rolls.into_iter().collect();
    }

    pub fn id(&self) -> &EncounterId {
        &self.id
    }

    pub fn initiative_order(&self) -> &Vec<(Entity, D20CheckResult)> {
        &self.initiative_order
    }

    pub fn current_entity(&self) -> Entity {
        let (idx, _) = self.initiative_order[self.turn_index];
        idx
    }

    pub fn participants(&self, world: &World, filters: &[EntityFilter]) -> Vec<Entity> {
        self.participants
            .iter()
            .filter(|entity| {
                filters
                    .iter()
                    .all(|filter| filter.matches(world, **entity, None))
            })
            .cloned()
            .collect()
    }

    fn start_turn(&mut self, engine_state: &mut EngineState) {
        self.advance_time(engine_state, TurnBoundary::Start);

        if self.should_skip_turn(engine_state) {
            self.end_turn(engine_state, self.current_entity());
            return;
        }

        let scope = engine_state
            .prompts
            .scope_mut(PromptScopeId::Encounter(self.id));

        scope.queue_prompt(
            ActionPrompt::new(ActionPromptKind::Action {
                actor: self.current_entity(),
            }),
            false,
        );
    }

    pub fn end_turn(&mut self, engine_state: &mut EngineState, entity: Entity) {
        if entity != self.current_entity() {
            panic!("Cannot end turn for entity that is not the current entity");
        }

        self.advance_time(engine_state, TurnBoundary::End);

        let scope = engine_state
            .prompts
            .scope_mut(PromptScopeId::Encounter(self.id));

        for prompt in scope.pending_prompts().iter() {
            for respondent in prompt.actors() {
                if respondent != entity {
                    panic!(
                        "Attempted to end turn for {:?} but there is a pending prompt for {:?}",
                        entity, respondent
                    );
                }
            }
        }

        scope.clear_prompts();

        self.turn_index = (self.turn_index + 1) % self.participants.len();
        if self.turn_index == 0 {
            self.round += 1;
            self.event_log
                .push(Event::encounter_event(EncounterEvent::NewRound(
                    self.id,
                    self.round(),
                )));
        }

        self.start_turn(engine_state);
    }

    // TODO: Some of this feels like it should belong somewhere else?
    fn should_skip_turn(&mut self, engine_state: &mut EngineState) -> bool {
        let current_entity = self.current_entity();

        let is_unconscious = matches!(
            *systems::helpers::get_component::<LifeState>(&engine_state.world, current_entity),
            LifeState::Unconscious(_)
        );

        if is_unconscious {
            let death_saving_throw_event = systems::d20::check(
                engine_state,
                current_entity,
                &D20CheckDC::SavingThrow {
                    saving_throw: SavingThrowKind::Death,
                    dc: ModifierMap::from(
                        ModifierSource::Custom("Death Saving Throw".to_string()),
                        DEATH_SAVING_THROW_DC as i32,
                    )
                    .evaluate(),
                },
            );

            engine_state.process_event_with_response_callback(
                death_saving_throw_event,
                EventCallback::new({
                    move |engine_state, event, _source| match &event.kind {
                        EventKind::D20CheckResolved {
                            actor,
                            result,
                            dc: _,
                        } => {
                            // A natural 20: the creature regains 1 Hit Point instead of
                            // counting a success (SRD 5.2.1, Death Saving Throws)
                            if matches!(result.outcome, Some(D20CheckOutcome::CriticalSuccess))
                                && let Some(new_state) =
                                    systems::health::heal(&mut engine_state.world, actor.id(), 1)
                            {
                                return CallbackResult::Event(Event::new(
                                    EventKind::LifeStateChanged {
                                        entity: actor.clone(),
                                        new_state,
                                        actor: None,
                                    },
                                ));
                            }

                            let life_state = systems::helpers::get_component_mut::<LifeState>(
                                &mut engine_state.world,
                                actor.id(),
                            );

                            if let LifeState::Unconscious(ref mut death_saving_throws) = *life_state
                            {
                                death_saving_throws.update(result);

                                let next_state = death_saving_throws.next_state();

                                if next_state != *life_state {
                                    *life_state = next_state;

                                    CallbackResult::Event(Event::new(EventKind::LifeStateChanged {
                                        entity: actor.clone(),
                                        new_state: next_state,
                                        actor: None,
                                    }))
                                } else {
                                    CallbackResult::None
                                }
                            } else {
                                CallbackResult::None
                            }
                        }
                        _ => panic!("Expected D20CheckResolved event"),
                    }
                }),
            );

            return true;
        }

        if matches!(
            *systems::helpers::get_component::<LifeState>(&engine_state.world, current_entity),
            LifeState::Normal
        ) {
            return systems::resources::can_act(&engine_state.world, current_entity).is_err();
        }

        // Normal / other states => decide if they can act
        true
    }

    pub fn round(&self) -> usize {
        self.round
    }

    pub(crate) fn log_event(&mut self, event: Event) {
        self.event_log.push(event);
    }

    pub(crate) fn event_log_mut(&mut self) -> &mut EventLog {
        &mut self.event_log
    }

    fn advance_time(&mut self, engine_state: &mut EngineState, boundary: TurnBoundary) {
        let time_step = TimeStep::TurnBoundary {
            entity: self.current_entity(),
            boundary,
        };
        for entity in self.participants.iter() {
            systems::time::advance_time(engine_state, *entity, time_step);
        }

        // An effect that lasts until a participant's turn can be on a creature
        // outside the encounter, e.g. an ally it helped before the fight, so their
        // effects see the boundary too. A creature in real time already counts the
        // time down as it passes, so only the ones whose time passes in turns
        let outsiders: Vec<Entity> = engine_state
            .world
            .query::<(Entity, (&EffectManager, &EntityClock))>()
            .iter()
            .filter(|(entity, (_, clock))| {
                !self.participants.contains(entity) && clock.mode() != TimeMode::RealTime
            })
            .map(|(entity, _)| entity)
            .collect();
        for entity in outsiders {
            systems::effects::effects_mut(&mut engine_state.world, entity).advance_time(time_step);
        }
    }

    pub fn event_log(&self) -> &EventLog {
        &self.event_log
    }

    pub fn event_log_move(&mut self) -> EventLog {
        std::mem::take(&mut self.event_log)
    }

    /// Adds a creature that joins the fight after it started. It rolls Initiative
    /// and takes its turn at its place in the order: later this round if its count
    /// hasn't come up yet, otherwise from the next round. On a tie it goes after
    /// the creatures already in the order.
    pub fn add_participant(&mut self, engine_state: &EngineState, entity: Entity) {
        if !self.participants.insert(entity) {
            return;
        }
        let roll = systems::helpers::get_component::<SkillSet>(&engine_state.world, entity)
            .check(&Skill::Initiative, engine_state, entity);
        let index = self
            .initiative_order
            .iter()
            .position(|(_, other)| other.total() < roll.total())
            .unwrap_or(self.initiative_order.len());
        if index <= self.turn_index {
            self.turn_index += 1;
        }
        self.initiative_order.insert(index, (entity, roll));
    }

    pub fn remove_participant(&mut self, engine_state: &mut EngineState, entity: Entity) {
        self.participants.remove(&entity);
        let current_entity = self.current_entity();
        self.initiative_order.retain(|(e, _)| *e != entity);
        self.turn_index = self
            .initiative_order
            .iter()
            .position(|(e, _)| *e == current_entity)
            .unwrap_or(0);
        if current_entity == entity {
            // If the current participant was removed, end their turn to move to the next one
            self.end_turn(engine_state, entity);
        }
    }
}
