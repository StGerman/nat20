use std::collections::HashSet;

use hecs::{Entity, World};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::components::{actions::targeting::EntityFilter, d20::D20CheckResult};

pub type EncounterId = Uuid;

/// Represents a turn-based combat encounter. Primarily responsible for tracking
/// the turn order and round number
#[derive(Debug, Serialize, Deserialize)]
pub struct Encounter {
    id: EncounterId,
    participants: HashSet<Entity>,
    round: usize,
    turn_index: usize,
    initiative_order: Vec<(Entity, D20CheckResult)>,
}

impl Encounter {
    pub fn new(
        id: EncounterId,
        participants: HashSet<Entity>,
        initiative_order: Vec<(Entity, D20CheckResult)>,
    ) -> Self {
        Self {
            id,
            participants,
            round: 1,
            turn_index: 0,
            initiative_order,
        }
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

    pub fn is_empty(&self) -> bool {
        self.participants.is_empty()
    }

    pub fn round(&self) -> usize {
        self.round
    }

    /// Returns true if this was the last turn of the round
    pub(crate) fn advance_turn(&mut self) -> bool {
        self.turn_index = (self.turn_index + 1) % self.participants.len();
        let new_round = self.turn_index == 0;
        if new_round {
            self.round += 1;
        }
        new_round
    }

    /// Adds a creature that joins the fight after it started, with its Initiative `roll`. It
    /// takes its turn at its place in the order: later this round if its count hasn't come up
    /// yet, otherwise from the next round. On a tie it goes after the creatures already in the
    /// order.
    pub(crate) fn add_participant(&mut self, entity: Entity, roll: D20CheckResult) {
        if !self.participants.insert(entity) {
            return;
        }
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

    /// The turn stays with whoever has it, so if that's the entity being
    /// removed, move the turn on first
    pub(crate) fn remove_participant(&mut self, entity: Entity) {
        let current_entity = self.current_entity();
        self.participants.remove(&entity);
        self.initiative_order.retain(|(e, _)| *e != entity);
        self.turn_index = self
            .initiative_order
            .iter()
            .position(|(e, _)| *e == current_entity)
            .unwrap_or(0);
    }
}
