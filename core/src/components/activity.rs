use hecs::Entity;
use parry3d::na::Point3;
use strum::EnumDiscriminants;
use tracing::{debug, warn};

use crate::{
    components::actions::action::{Action, ActionTimeline},
    engine::{
        action_prompt::{ActionDecision, ActionError},
        event::EventId,
        geometry::WorldPath,
    },
    systems::{geometry::Parabola, movement::MovementError},
};

// TODO: Should these two enums live here?
#[derive(Debug, Clone)]
pub enum Activity {
    Move {
        entity: Entity,
        goal: Point3<f32>,
    },
    Act {
        action: ActionDecision,
    },
    MoveAndAct {
        goal: Point3<f32>,
        action: ActionDecision,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum ActivityError {
    MovementError(MovementError),
    ActionError(ActionError),
}

impl From<MovementError> for ActivityError {
    fn from(error: MovementError) -> Self {
        Self::MovementError(error)
    }
}

impl From<ActionError> for ActivityError {
    fn from(error: ActionError) -> Self {
        Self::ActionError(error)
    }
}

#[derive(Debug, Clone, EnumDiscriminants)]
#[strum_discriminants(name(ActivityStateTag))]
#[derive(Default)]
pub enum ActivityState {
    #[default]
    Idle,
    Moving {
        path: WorldPath,
        /// Current index in the path which the entity is moving towards
        current_target: usize,
        /// Potential action to be performed after reaching the destination
        action: Option<ActionDecision>,
        /// Creatures this walk has already given an opportunity attack: each one
        /// gets a single chance per walk.
        provoked: Vec<Entity>,
    },
    Acting {
        timeline: ActionTimeline,
        elapsed_time: f32,
        phase_cooldown: f32,
        blocking_event: Option<EventId>,
        /// The walk this action interrupted, when the creature takes it in the
        /// middle of one (a reaction such as Shield against the opportunity
        /// attack its walk provoked). The walk goes on when the action ends.
        interrupted_walk: Option<Box<ActivityState>>,
    },
    Displaced {
        trajectory: Parabola,
        elapsed_time: f32,
    },
}

impl ActivityState {
    pub fn tag(&self) -> ActivityStateTag {
        ActivityStateTag::from(self)
    }

    pub fn set_idle(&mut self) {
        debug!("Setting entity to idle");
        *self = ActivityState::Idle;
    }

    pub fn is_idle(&self) -> bool {
        matches!(self, ActivityState::Idle)
    }

    pub fn set_moving(&mut self, path: WorldPath, action: Option<ActionDecision>) {
        debug!("Setting entity to move to goal {:?}", path.points.last());

        *self = ActivityState::Moving {
            path,
            current_target: 0,
            action,
            provoked: Vec::new(),
        };
    }

    pub fn set_acting(&mut self, action: &Action, blocking_event: Option<EventId>) {
        if matches!(self, ActivityState::Acting { .. }) {
            warn!(
                "Overriding activity state for entity which is already acting, with action {:?}",
                action
            );
        }
        debug!("Setting entity to perform action {:?}", action);

        // A walk with steps still ahead is only interrupted. One at its goal is
        // over: its own follow-up action is what starts here.
        let walking = matches!(
            self,
            ActivityState::Moving { path, current_target, .. }
                if *current_target < path.points.len()
        );
        let interrupted_walk = walking.then(|| Box::new(std::mem::take(self)));

        *self = ActivityState::Acting {
            timeline: action.timeline.clone(),
            elapsed_time: 0.0,
            phase_cooldown: action.timeline.step_spacing,
            blocking_event,
            interrupted_walk,
        };
    }

    pub fn is_acting(&self) -> bool {
        matches!(self, ActivityState::Acting { .. })
    }

    pub fn set_displaced(&mut self, trajectory: Parabola) {
        debug!(
            "Setting entity to be displaced with trajectory {:?}",
            trajectory
        );
        *self = ActivityState::Displaced {
            trajectory,
            elapsed_time: 0.0,
        };
    }
}
