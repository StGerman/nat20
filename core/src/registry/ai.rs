use std::{
    collections::{HashMap, HashSet},
    sync::LazyLock,
};

use hecs::Entity;
use rand::{
    RngExt,
    seq::{IndexedRandom, IteratorRandom},
};
use tracing::{debug, error};

use crate::{
    components::{
        actions::{
            action_builder::{ActionBuilder, ActionBuilderState, ReactionBuilder},
            targeting::TargetingKind,
        },
        activity::Activity,
        ai::AIController,
        id::AIControllerId,
        rng,
    },
    engine::{
        action_prompt::{ActionPrompt, ActionPromptKind},
        engine_state::EngineState,
    },
    systems::{self},
};

pub static AI_CONTROLLER_REGISTRY: LazyLock<HashMap<AIControllerId, Box<dyn AIController>>> =
    LazyLock::new(|| {
        HashMap::from([(
            RANDOM_CONTROLLER_ID.clone(),
            Box::new(RandomController) as Box<dyn AIController>,
        )])
    });

pub static RANDOM_CONTROLLER_ID: LazyLock<AIControllerId> =
    LazyLock::new(|| AIControllerId::new("nat20_core", "ai_controller.random"));

pub struct RandomController;

impl AIController for RandomController {
    fn decide(
        &self,
        engine_state: &mut EngineState,
        prompt: &ActionPrompt,
        actor: Entity,
    ) -> Option<Activity> {
        let rng = &mut rng::rng();

        match &prompt.kind {
            ActionPromptKind::Action { actor } => {
                let mut action_builder = ActionBuilder::available(engine_state, *actor);
                // Actions that turned out to have nothing the actor can do with them:
                // no possible targets, e.g. Help's Stabilize when nobody is dying, or no
                // variant it can use
                let mut unusable = HashSet::new();

                loop {
                    match action_builder.state() {
                        Ok(state) => match state {
                            ActionBuilderState::Action { actions } => {
                                let action = actions
                                    .keys()
                                    .filter(|action| !unusable.contains(*action))
                                    .choose(rng)
                                    .cloned()?;
                                action_builder.action(engine_state, &action);
                            }

                            ActionBuilderState::Variant {
                                action,
                                contexts_and_costs,
                                variants,
                            } => {
                                // Only the variants the actor can use, e.g. Help with a
                                // skill it is proficient in
                                let variant = variants
                                    .iter()
                                    .filter(|variant| {
                                        contexts_and_costs.iter().any(|(context, cost)| {
                                            systems::actions::action_usable(
                                                engine_state,
                                                *actor,
                                                action,
                                                Some(variant),
                                                context,
                                                cost,
                                                &[],
                                            )
                                            .is_ok()
                                        })
                                    })
                                    .choose(rng)
                                    .cloned();
                                let Some(variant) = variant else {
                                    debug!(
                                        "AI actor {:?} has no usable variant for action: {:?}",
                                        actor, action
                                    );
                                    // Pick again among the other actions
                                    unusable.insert(action.clone());
                                    action_builder = ActionBuilder::available(engine_state, *actor);
                                    continue;
                                };
                                action_builder.variant(&engine_state.world, &variant);
                            }

                            ActionBuilderState::Context {
                                contexts_and_costs, ..
                            } => {
                                action_builder.context_index(
                                    &engine_state.world,
                                    rng.random_range(0..contexts_and_costs.len()),
                                );
                            }

                            ActionBuilderState::Targets { action, .. } => {
                                // This means it was populated on the previous iteration
                                if !action.targets.is_empty() {
                                    match action_builder.build(engine_state) {
                                        Ok(activity) => return Some(activity),
                                        Err(error) => {
                                            error!(
                                                "AI actor {:?} failed to build action: {:?}",
                                                actor, error
                                            );
                                            return None;
                                        }
                                    }
                                }

                                let possible_targets =
                                    systems::ai::possible_targets(engine_state, action);

                                if possible_targets.is_empty() {
                                    debug!(
                                        "AI actor {:?} has no possible targets for action: {:?}",
                                        actor, action.action_id
                                    );
                                    // Pick again among the other actions
                                    unusable.insert(action.action_id.clone());
                                    action_builder = ActionBuilder::available(engine_state, *actor);
                                    continue;
                                }

                                let targeting = systems::actions::targeting_context_data(
                                    &engine_state.world,
                                    action,
                                );

                                match targeting.kind {
                                    TargetingKind::SelfTarget => {
                                        action_builder.target_entity(engine_state, *actor);
                                    }

                                    TargetingKind::Single => {
                                        action_builder.target_entity(
                                            engine_state,
                                            *possible_targets.choose(rng)?,
                                        );
                                    }

                                    TargetingKind::Multiple {
                                        max_targets,
                                        allow_duplicates,
                                    } => {
                                        let chosen_targets = if allow_duplicates {
                                            possible_targets
                                                .iter()
                                                .sample(rng, max_targets)
                                        } else {
                                            let max_unique_targets =
                                                max_targets.min(possible_targets.len());
                                            possible_targets[0..max_unique_targets].iter().collect()
                                        };

                                        for target in chosen_targets {
                                            action_builder.target_entity(engine_state, *target);
                                        }
                                    }

                                    TargetingKind::Area {
                                        shape,
                                        fixed_on_actor,
                                        filters,
                                    } => todo!(),
                                }
                            }
                        },

                        Err(error) => {
                            error!("Failed to build action: {:?}", error);
                            return None;
                        }
                    }
                }
            }

            ActionPromptKind::Reactions { options, .. } => {
                let mut reaction_builder = ReactionBuilder::new(engine_state, actor);

                reaction_builder.option_index(rng.random_range(0..options.get(&actor)?.len()));

                match reaction_builder.build() {
                    Ok(reaction) => Some(reaction),
                    Err(error) => {
                        error!("AI actor {:?} failed to build reaction: {:?}", actor, error);
                        None
                    }
                }
            }
        }
    }
}
