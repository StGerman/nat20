//! One game session: an `EngineState`, the loaded scene, and the server log.
//!
//! The session owns the clock. Nat20 resolves actions over simulated time
//! (`EngineState::update`), so after every decision the session ticks the
//! engine until nothing is moving or acting, copies new engine events into
//! the log, and runs scripted creatures until a client decision is needed.

use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};

use hecs::Entity;
use nat20_core::{
    components::{
        actions::{
            action_builder::{ActionBuilder, ActionBuilderState, ReactionBuilder},
            targeting::TargetingKind,
        },
        activity::ActivityState,
        faction::Attitude,
        health::{hit_points::HitPoints, life_state::LifeState},
        id::{ActionId, EntityIdentifier},
        items::equipment::slots::EquipmentSlot,
        modifier::FlatModifiable,
    },
    engine::{
        action_prompt::{ActionData, ActionPromptKind},
        encounter::EncounterId,
        engine_state::EngineState,
        geometry::WorldGeometry,
        prompt::PromptScopeId,
    },
    systems::{self, movement::TargetPathFindingResult},
};
use serde_json::{Value, json};

use crate::{
    log::{LogEntry, LogWriter, action_name},
    monster::MonsterDef,
    pregen,
    protocol::{ApiError, Control, Request, Response, SceneDef},
};

/// Simulated seconds per engine tick while settling.
const TICK_SECONDS: f32 = 0.1;
/// Upper bound on ticks per settle; a single action or move finishes far
/// sooner, so hitting this means the engine is stuck.
const MAX_TICKS: usize = 2_000;
/// Upper bound on scripted decisions in one `advance`.
const MAX_SCRIPTED_STEPS: usize = 200;
/// Scripted creatures take at most this many actions per turn before ending it.
const MAX_SCRIPTED_ACTIONS_PER_TURN: usize = 4;

const METERS_PER_FOOT: f32 = 0.3048;

struct SceneCreature {
    handle: String,
    entity: Entity,
    side: String,
    control: Control,
}

struct Scene {
    id: String,
    creatures: Vec<SceneCreature>,
    handles: HashMap<Entity, String>,
    encounter: EncounterId,
}

/// What the engine needs next.
#[derive(Debug, Clone)]
enum Awaiting {
    NoScene,
    CombatOver {
        winner: String,
    },
    Action {
        actor: Entity,
    },
    Reaction {
        reactor: Entity,
        options: Vec<ActionData>,
    },
    Nothing,
}

pub struct Session {
    data_dir: PathBuf,
    engine: Option<EngineState>,
    scene: Option<Scene>,
    log: Vec<LogEntry>,
    global_seen: usize,
    encounter_seen: usize,
    winner: Option<String>,
    scripted_turn: Option<(usize, Entity, usize)>,
}

impl Default for Session {
    fn default() -> Self {
        Self::new(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("data"))
    }
}

impl Session {
    pub fn new(data_dir: PathBuf) -> Self {
        Self {
            data_dir,
            engine: None,
            scene: None,
            log: Vec::new(),
            global_seen: 0,
            encounter_seen: 0,
            winner: None,
            scripted_turn: None,
        }
    }

    /// Handles one JSON request line and returns one JSON response line.
    pub fn handle_json(&mut self, line: &str) -> String {
        let response = match serde_json::from_str::<Request>(line) {
            Ok(request) => self.handle(request),
            Err(error) => Response {
                ok: false,
                result: None,
                error: Some(ApiError::new("bad_request", error.to_string())),
                new_events: Vec::new(),
                awaiting: self.awaiting_json(),
            },
        };
        serde_json::to_string(&response).expect("responses always serialize")
    }

    pub fn handle(&mut self, request: Request) -> Response {
        let before = self.log.len();
        let result = match request {
            Request::LoadScene { scene, scene_id } => self.load_scene(scene, scene_id),
            Request::GetState => self.state(),
            Request::LegalActions { actor } => self.legal_actions(&actor),
            Request::Submit {
                actor,
                action,
                context,
                targets,
            } => self.submit(&actor, &action, context, &targets),
            Request::React { actor, choice } => self.react(&actor, choice),
            Request::EndTurn { actor } => self.end_turn(&actor),
            Request::Events { since } => Ok(json!({
                "events": self.log.get(since..).unwrap_or(&[]),
                "next": self.log.len(),
            })),
        };
        let (ok, result, error) = match result {
            Ok(value) => (true, Some(value), None),
            Err(error) => (false, None, Some(error)),
        };
        Response {
            ok,
            result,
            error,
            new_events: self.log[before..].to_vec(),
            awaiting: self.awaiting_json(),
        }
    }

    pub fn log(&self) -> &[LogEntry] {
        &self.log
    }

    // ---- commands -------------------------------------------------------

    fn load_scene(
        &mut self,
        scene: Option<SceneDef>,
        scene_id: Option<String>,
    ) -> Result<Value, ApiError> {
        let scene = match (scene, scene_id) {
            (Some(scene), _) => scene,
            (None, Some(id)) => {
                if !id
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
                {
                    return Err(ApiError::new(
                        "bad_request",
                        "scene_id must be a plain name",
                    ));
                }
                let path = self.data_dir.join("scenes").join(format!("{id}.json"));
                let text = std::fs::read_to_string(&path)
                    .map_err(|e| ApiError::new("not_found", format!("scene {id}: {e}")))?;
                serde_json::from_str(&text)
                    .map_err(|e| ApiError::new("bad_scene", format!("scene {id}: {e}")))?
            }
            (None, None) => {
                return Err(ApiError::new(
                    "bad_request",
                    "give either scene or scene_id",
                ));
            }
        };

        let mut engine = new_engine_state();
        let mut creatures = Vec::new();
        let mut seen = HashSet::new();
        for def in &scene.creatures {
            if !seen.insert(def.handle.clone()) {
                return Err(ApiError::new(
                    "bad_scene",
                    format!("duplicate handle {}", def.handle),
                ));
            }
            let entity = match (&def.pregen, &def.monster) {
                (Some(pregen), None) => pregen::spawn(&mut engine, pregen).ok_or_else(|| {
                    ApiError::new(
                        "bad_scene",
                        format!("unknown pregen {pregen}; known: {:?}", pregen::PREGENS),
                    )
                })?,
                (None, Some(monster)) => {
                    let path = monster_path(&self.data_dir, monster)?;
                    let def = MonsterDef::load(&path)
                        .map_err(|e| ApiError::new("bad_monster", format!("{monster}: {e}")))?;
                    def.spawn(&mut engine)
                        .map_err(|e| ApiError::new("bad_monster", format!("{monster}: {e}")))?
                }
                _ => {
                    return Err(ApiError::new(
                        "bad_scene",
                        format!("{}: give exactly one of pregen or monster", def.handle),
                    ));
                }
            };
            // Client-controlled characters are player controlled already;
            // scripted creatures are tagged at spawn so the engine's random AI
            // never acts for them either way.
            let [x, y, z] = def.position_m;
            systems::geometry::teleport_to_ground(
                &mut engine.world,
                &engine.geometry,
                entity,
                &[x, y, z].into(),
            );
            creatures.push(SceneCreature {
                handle: def.handle.clone(),
                entity,
                side: def.side.clone(),
                control: def.control,
            });
        }

        let handles = creatures
            .iter()
            .map(|c| (c.entity, c.handle.clone()))
            .collect();
        let participants = creatures.iter().map(|c| c.entity).collect();
        let encounter = engine.start_encounter(participants);

        self.engine = Some(engine);
        self.scene = Some(Scene {
            id: scene.id.clone(),
            creatures,
            handles,
            encounter,
        });
        self.log.clear();
        self.global_seen = 0;
        self.encounter_seen = 0;
        self.winner = None;
        self.scripted_turn = None;

        self.drain();
        self.log_initiative();
        self.advance();
        self.state()
    }

    fn state(&self) -> Result<Value, ApiError> {
        let (engine, scene) = self.loaded()?;
        let encounter = engine.encounter(&scene.encounter);
        let mut creatures = Vec::new();
        for c in &scene.creatures {
            let mut distances = serde_json::Map::new();
            for other in &scene.creatures {
                if other.entity != c.entity
                    && let Some(d) = distance_m(engine, c.entity, other.entity)
                {
                    distances.insert(other.handle.clone(), json!((d / METERS_PER_FOOT).round()));
                }
            }
            let exists = engine.world.contains(c.entity);
            creatures.push(json!({
                "handle": c.handle,
                "name": exists.then(|| EntityIdentifier::from_world(&engine.world, c.entity).name().as_str().to_string()),
                "side": c.side,
                "control": c.control,
                "hp": exists.then(|| systems::helpers::get_component::<HitPoints>(&engine.world, c.entity).current()),
                "max_hp": exists.then(|| systems::helpers::get_component::<HitPoints>(&engine.world, c.entity).max()),
                "ac": exists.then(|| systems::loadout::armor_class(engine, c.entity).total()),
                "life_state": life_state_name(engine, c.entity),
                "position_m": systems::geometry::get_foot_position(&engine.world, c.entity).map(|p| [p.x, p.y, p.z]),
                "distance_ft": distances,
            }));
        }
        Ok(json!({
            "scene": scene.id,
            "round": encounter.map(|e| e.round()),
            "turn": encounter.map(|e| self.handle_of(e.current_entity())),
            "initiative": encounter.map(|e| e.initiative_order().iter().map(|(entity, roll)| json!({
                "handle": self.handle_of(*entity),
                "total": roll.total(),
            })).collect::<Vec<_>>()),
            "creatures": creatures,
            "winner": self.winner,
        }))
    }

    fn legal_actions(&mut self, actor: &str) -> Result<Value, ApiError> {
        let entity = self.entity(actor)?;
        let scene_entities: Vec<(String, Entity)> = self
            .scene
            .as_ref()
            .unwrap()
            .creatures
            .iter()
            .map(|c| (c.handle.clone(), c.entity))
            .collect();
        let awaiting = self.awaiting();
        let engine = self.engine.as_mut().unwrap();

        if !matches!(awaiting, Awaiting::Action { actor } if actor == entity) {
            return Ok(
                json!({ "actor": actor, "actions": [], "note": "not this creature's turn" }),
            );
        }

        let actions = match ActionBuilder::available(engine, entity).state() {
            Ok(ActionBuilderState::Action { actions }) => actions.clone(),
            _ => return Ok(json!({ "actor": actor, "actions": [] })),
        };

        let mut out = Vec::new();
        for (action_id, contexts) in &actions {
            let mut options = Vec::new();
            for (index, (context, cost)) in contexts.iter().enumerate() {
                let mut builder = ActionBuilder::available(engine, entity);
                builder.action(engine, action_id);
                if let Ok(ActionBuilderState::Variant { variants, .. }) = builder.state() {
                    options.push(json!({
                        "context": index,
                        "variants": variants.iter().map(|v| v.to_string()).collect::<Vec<_>>(),
                        "note": "variant actions are not exposed by this API yet",
                    }));
                    continue;
                }
                if !pick_context(&mut builder, &engine.world, index) {
                    continue;
                }
                let Ok(ActionBuilderState::Targets { action, .. }) = builder.state() else {
                    continue;
                };
                let targeting =
                    systems::actions::targeting_context_data(&engine.world, action).kind;
                let weapon = context
                    .attack
                    .as_ref()
                    .and_then(|a| a.slot.as_ref())
                    .and_then(|slot| weapon_name(engine, entity, slot));
                let cost: serde_json::Map<String, Value> = cost
                    .map
                    .iter()
                    .map(|(id, amount)| (id.to_string(), json!(amount)))
                    .collect();

                let (targeting_name, targets) = match targeting {
                    TargetingKind::SelfTarget => {
                        ("self", vec![json!({ "handle": actor, "reach": "self" })])
                    }
                    TargetingKind::Single | TargetingKind::Multiple { .. } => {
                        let mut targets = Vec::new();
                        for (handle, other) in &scene_entities {
                            if *other == entity || !systems::health::is_alive(&engine.world, *other)
                            {
                                continue;
                            }
                            let mut probe = ActionBuilder::available(engine, entity);
                            probe.action(engine, action_id);
                            if !pick_context(&mut probe, &engine.world, index) {
                                continue;
                            }
                            probe.target_entity(engine, *other);
                            let reach = match probe.state() {
                                Ok(ActionBuilderState::Targets {
                                    path_to_target: Some(TargetPathFindingResult::AlreadyInRange(_)),
                                    ..
                                }) => "in_range",
                                Ok(ActionBuilderState::Targets {
                                    path_to_target: Some(TargetPathFindingResult::PathFound(path)),
                                    ..
                                }) if path.path_result.reaches_goal() => "move_then_act",
                                _ => continue,
                            };
                            let attitude = match systems::factions::mutual_attitude(
                                &engine.world,
                                entity,
                                *other,
                            ) {
                                Attitude::Hostile => "hostile",
                                Attitude::Neutral => "neutral",
                                Attitude::Friendly => "friendly",
                            };
                            targets.push(
                                json!({ "handle": handle, "reach": reach, "attitude": attitude }),
                            );
                        }
                        let name = if matches!(targeting, TargetingKind::Single) {
                            "single"
                        } else {
                            "multiple"
                        };
                        (name, targets)
                    }
                    TargetingKind::Area { .. } => ("area", Vec::new()),
                };
                if targeting_name != "area" && targets.is_empty() {
                    continue;
                }
                options.push(json!({
                    "context": index,
                    "weapon": weapon,
                    "cost": cost,
                    "targeting": targeting_name,
                    "targets": targets,
                }));
            }
            if !options.is_empty() {
                out.push(json!({
                    "action": action_id.to_string(),
                    "name": action_name(action_id),
                    "options": options,
                }));
            }
        }
        // Actions the creature has but cannot use right now, with the engine's
        // reason, so a caller can explain "why not".
        let mut unavailable = Vec::new();
        for (action_id, contexts) in systems::actions::all_actions(engine, entity) {
            if actions.contains_key(&action_id)
                || systems::actions::get_action(&action_id).is_none_or(|a| a.is_reaction())
            {
                continue;
            }
            let reason = contexts
                .iter()
                .find_map(|(context, cost)| {
                    systems::actions::action_usable(
                        engine,
                        entity,
                        &action_id,
                        None,
                        context,
                        cost,
                        &[
                            systems::actions::ActionUsabilityCheck::ActionKnown,
                            systems::actions::ActionUsabilityCheck::Variant,
                        ],
                    )
                    .err()
                })
                .map(|e| short_debug(&e));
            unavailable.push(json!({ "action": action_id.to_string(), "reason": reason }));
        }

        Ok(
            json!({ "actor": actor, "actions": out, "unavailable": unavailable, "can_end_turn": true }),
        )
    }

    fn submit(
        &mut self,
        actor: &str,
        action: &str,
        context: usize,
        targets: &[String],
    ) -> Result<Value, ApiError> {
        let entity = self.entity(actor)?;
        self.require_action_prompt(entity)?;
        let action_id: ActionId = action
            .parse()
            .map_err(|e| ApiError::new("bad_request", format!("action {action}: {e:?}")))?;
        let target_entities = targets
            .iter()
            .map(|t| self.entity(t))
            .collect::<Result<Vec<_>, _>>()?;

        self.submit_entity(entity, &action_id, context, &target_entities)?;
        self.settle();
        self.drain();
        self.advance();
        Ok(json!({ "accepted": true }))
    }

    fn react(&mut self, actor: &str, choice: Option<usize>) -> Result<Value, ApiError> {
        let entity = self.entity(actor)?;
        match self.awaiting() {
            Awaiting::Reaction { reactor, .. } if reactor == entity => {}
            other => {
                return Err(ApiError::new(
                    "not_awaiting_you",
                    format!(
                        "engine is not waiting on a reaction from {actor} ({})",
                        self.describe(&other)
                    ),
                ));
            }
        }
        self.react_entity(entity, choice)?;
        self.settle();
        self.drain();
        self.advance();
        Ok(json!({ "accepted": true }))
    }

    fn end_turn(&mut self, actor: &str) -> Result<Value, ApiError> {
        let entity = self.entity(actor)?;
        self.require_action_prompt(entity)?;
        self.engine.as_mut().unwrap().end_turn(entity);
        self.settle();
        self.drain();
        self.advance();
        Ok(json!({ "accepted": true }))
    }

    // ---- engine driving -------------------------------------------------

    fn submit_entity(
        &mut self,
        entity: Entity,
        action_id: &ActionId,
        context: usize,
        targets: &[Entity],
    ) -> Result<(), ApiError> {
        let engine = self.engine.as_mut().unwrap();
        let mut builder = ActionBuilder::available(engine, entity);
        builder.action(engine, action_id);
        if let Ok(ActionBuilderState::Variant { .. }) = builder.state() {
            return Err(ApiError::new(
                "unsupported",
                "variant actions are not exposed by this API yet",
            ));
        }
        if !pick_context(&mut builder, &engine.world, context) {
            return Err(ApiError::new(
                "rejected",
                format!("no option {context} for this action"),
            ));
        }
        if let Ok(ActionBuilderState::Targets { action, .. }) = builder.state() {
            let kind = systems::actions::targeting_context_data(&engine.world, action).kind;
            if matches!(kind, TargetingKind::SelfTarget) {
                // The builder fills in the actor itself for most self-targeted
                // actions; only add it when it did not.
                if action.targets.is_empty() {
                    builder.target_entity(engine, entity);
                }
            } else {
                if targets.is_empty() {
                    return Err(ApiError::new("rejected", "this action needs a target"));
                }
                for target in targets {
                    builder.target_entity(engine, *target);
                }
            }
        }
        let activity = builder
            .build(engine)
            .map_err(|e| ApiError::new("rejected", short_debug(&e)))?;
        engine
            .submit_activity(activity)
            .map_err(|e| ApiError::new("rejected", short_debug(&e)))
    }

    fn react_entity(&mut self, entity: Entity, choice: Option<usize>) -> Result<(), ApiError> {
        let engine = self.engine.as_mut().unwrap();
        let mut builder = ReactionBuilder::new(engine, entity);
        match choice {
            Some(index) => builder.option_index(index),
            None => builder.option_none(),
        };
        let activity = builder
            .build()
            .map_err(|e| ApiError::new("rejected", short_debug(&e)))?;
        engine
            .submit_activity(activity)
            .map_err(|e| ApiError::new("rejected", short_debug(&e)))
    }

    /// Ticks the engine until no creature is moving or acting and no event is
    /// waiting on anything but a reaction decision.
    fn settle(&mut self) {
        for tick in 0..MAX_TICKS {
            if tick >= 2 && self.quiescent() {
                return;
            }
            self.engine.as_mut().unwrap().update(TICK_SECONDS);
        }
        tracing::warn!("engine did not settle within {MAX_TICKS} ticks");
    }

    fn quiescent(&self) -> bool {
        let (engine, scene) = self.loaded().expect("scene loaded");
        let all_idle = scene.creatures.iter().all(|c| {
            engine
                .world
                .get::<&ActivityState>(c.entity)
                .map(|s| s.is_idle())
                .unwrap_or(true)
        });
        let scope = engine
            .prompts
            .scope(&PromptScopeId::Encounter(scene.encounter));
        let waiting_on_reaction = engine
            .next_prompt(PromptScopeId::Encounter(scene.encounter))
            .is_some_and(|p| matches!(p.kind, ActionPromptKind::Reactions { .. }));
        let no_pending = scope.is_none_or(|s| s.pending_events().is_empty());
        all_idle && (no_pending || waiting_on_reaction)
    }

    /// Runs scripted creatures until the engine waits on a client, or combat
    /// is over.
    fn advance(&mut self) {
        for _ in 0..MAX_SCRIPTED_STEPS {
            self.settle();
            self.drain();
            if self.check_combat_over() {
                return;
            }
            match self.awaiting() {
                Awaiting::Action { actor } if self.control_of(actor) == Some(Control::Scripted) => {
                    self.scripted_action(actor);
                }
                Awaiting::Reaction { reactor, .. }
                    if self.control_of(reactor) == Some(Control::Scripted) =>
                {
                    // Scripted creatures always take the first reaction offered
                    // (for a goblin, the opportunity attack).
                    if self.react_entity(reactor, Some(0)).is_err() {
                        let _ = self.react_entity(reactor, None);
                    }
                }
                _ => return,
            }
        }
        tracing::warn!("scripted creatures did not yield within {MAX_SCRIPTED_STEPS} steps");
    }

    /// The monster policy: attack the nearest hostile creature, preferring a
    /// melee weapon already in reach, then any weapon already in range, then
    /// moving into melee. End the turn when no attack is possible.
    fn scripted_action(&mut self, actor: Entity) {
        let round = self
            .loaded()
            .ok()
            .and_then(|(e, s)| e.encounter(&s.encounter).map(|enc| enc.round()))
            .unwrap_or(0);
        let taken = match self.scripted_turn {
            Some((r, e, n)) if r == round && e == actor => n,
            _ => 0,
        };

        let choice = if taken < MAX_SCRIPTED_ACTIONS_PER_TURN {
            self.choose_attack(actor)
        } else {
            None
        };
        if let Some((action_id, context, target)) = choice
            && self
                .submit_entity(actor, &action_id, context, &[target])
                .is_ok()
        {
            self.scripted_turn = Some((round, actor, taken + 1));
            return;
        }
        self.engine.as_mut().unwrap().end_turn(actor);
    }

    fn choose_attack(&mut self, actor: Entity) -> Option<(ActionId, usize, Entity)> {
        let (engine, scene) = self.loaded().ok()?;
        let mut hostiles: Vec<(f32, Entity)> = scene
            .creatures
            .iter()
            .filter(|c| c.entity != actor)
            .filter(|c| life_state_name(engine, c.entity) == "normal")
            .filter(|c| {
                systems::factions::mutual_attitude(&engine.world, actor, c.entity)
                    == Attitude::Hostile
            })
            .map(|c| {
                (
                    distance_m(engine, actor, c.entity).unwrap_or(f32::MAX),
                    c.entity,
                )
            })
            .collect();
        hostiles.sort_by(|a, b| a.0.total_cmp(&b.0));

        let melee: ActionId = "nat20_core::action.melee_attack".parse().ok()?;
        let ranged: ActionId = "nat20_core::action.ranged_attack".parse().ok()?;
        let engine = self.engine.as_mut().unwrap();
        let available = match ActionBuilder::available(engine, actor).state() {
            Ok(ActionBuilderState::Action { actions }) => actions.clone(),
            _ => return None,
        };

        // (action, allow moving first)
        for (action_id, allow_move) in [(&melee, false), (&ranged, false), (&melee, true)] {
            let Some(contexts) = available.get(action_id) else {
                continue;
            };
            for context in 0..contexts.len() {
                for (_, target) in &hostiles {
                    let mut probe = ActionBuilder::available(engine, actor);
                    probe.action(engine, action_id);
                    if !pick_context(&mut probe, &engine.world, context) {
                        continue;
                    }
                    probe.target_entity(engine, *target);
                    let usable = match probe.state() {
                        Ok(ActionBuilderState::Targets {
                            path_to_target: Some(TargetPathFindingResult::AlreadyInRange(_)),
                            ..
                        }) => true,
                        Ok(ActionBuilderState::Targets {
                            path_to_target: Some(TargetPathFindingResult::PathFound(path)),
                            ..
                        }) => allow_move && path.path_result.reaches_goal(),
                        _ => false,
                    };
                    if usable {
                        return Some((action_id.clone(), context, *target));
                    }
                }
            }
        }
        None
    }

    /// Combat ends when one side has nobody left standing.
    fn check_combat_over(&mut self) -> bool {
        if self.winner.is_some() {
            return true;
        }
        let Ok((engine, scene)) = self.loaded() else {
            return false;
        };
        let mut sides: Vec<&str> = scene.creatures.iter().map(|c| c.side.as_str()).collect();
        sides.sort();
        sides.dedup();
        let standing: Vec<&str> = sides
            .iter()
            .copied()
            .filter(|side| {
                scene
                    .creatures
                    .iter()
                    .any(|c| c.side == *side && life_state_name(engine, c.entity) == "normal")
            })
            .collect();
        if standing.len() > 1 {
            return false;
        }
        let winner = standing
            .first()
            .map(|s| s.to_string())
            .unwrap_or_else(|| "nobody".into());
        let encounter = scene.encounter;
        self.drain();
        self.engine.as_mut().unwrap().end_encounter(&encounter);
        self.drain();
        let seq = self.log.len();
        self.log.push(LogEntry {
            seq,
            kind: "combat_over".into(),
            round: None,
            actor: None,
            target: None,
            text: format!("Combat is over. Winner: {winner}."),
            roll: None,
        });
        self.winner = Some(winner);
        true
    }

    fn awaiting(&self) -> Awaiting {
        let Ok((engine, scene)) = self.loaded() else {
            return Awaiting::NoScene;
        };
        if let Some(winner) = &self.winner {
            return Awaiting::CombatOver {
                winner: winner.clone(),
            };
        }
        let scope_id = PromptScopeId::Encounter(scene.encounter);
        let Some(prompt) = engine.next_prompt(scope_id) else {
            return Awaiting::Nothing;
        };
        match &prompt.kind {
            ActionPromptKind::Action { actor } => Awaiting::Action { actor: *actor },
            ActionPromptKind::Reactions { options, .. } => {
                let decided = engine
                    .prompts
                    .scope(&scope_id)
                    .and_then(|s| s.decisions_for_prompt(&prompt.id));
                // Deterministic order: by scene order.
                for c in &scene.creatures {
                    if let Some(opts) = options.get(&c.entity)
                        && !decided.is_some_and(|d| d.contains_key(&c.entity))
                    {
                        return Awaiting::Reaction {
                            reactor: c.entity,
                            options: opts.clone(),
                        };
                    }
                }
                Awaiting::Nothing
            }
        }
    }

    fn awaiting_json(&self) -> Value {
        match self.awaiting() {
            Awaiting::NoScene => json!({ "kind": "no_scene" }),
            Awaiting::Nothing => json!({ "kind": "nothing" }),
            Awaiting::CombatOver { winner } => json!({ "kind": "combat_over", "winner": winner }),
            Awaiting::Action { actor } => {
                json!({ "kind": "action", "actor": self.handle_of(actor) })
            }
            Awaiting::Reaction { reactor, options } => json!({
                "kind": "reaction",
                "actor": self.handle_of(reactor),
                "options": options.iter().enumerate().map(|(i, o)| json!({
                    "choice": i,
                    "action": o.action_id.to_string(),
                    "name": action_name(&o.action_id),
                })).collect::<Vec<_>>(),
            }),
        }
    }

    fn describe(&self, awaiting: &Awaiting) -> String {
        match awaiting {
            Awaiting::NoScene => "no scene loaded".into(),
            Awaiting::Nothing => "waiting on nothing".into(),
            Awaiting::CombatOver { winner } => format!("combat is over, winner {winner}"),
            Awaiting::Action { actor } => format!("waiting on {}'s action", self.handle_of(*actor)),
            Awaiting::Reaction { reactor, .. } => {
                format!("waiting on {}'s reaction", self.handle_of(*reactor))
            }
        }
    }

    fn require_action_prompt(&self, entity: Entity) -> Result<(), ApiError> {
        match self.awaiting() {
            Awaiting::Action { actor } if actor == entity => Ok(()),
            other => Err(ApiError::new(
                "not_your_turn",
                format!(
                    "{} cannot act now: {}",
                    self.handle_of(entity),
                    self.describe(&other)
                ),
            )),
        }
    }

    /// Copies new engine events into the log.
    fn drain(&mut self) {
        let Some(scene) = self.scene.as_ref() else {
            return;
        };
        let engine = self.engine.as_ref().unwrap();
        let encounter = engine.encounter(&scene.encounter);
        let writer = LogWriter {
            handles: &scene.handles,
            round: encounter.map(|e| e.round()),
        };

        let mut entries = Vec::new();
        if let Some(encounter) = encounter {
            let events = &encounter.event_log().events;
            for event in events.iter().skip(self.encounter_seen) {
                entries.extend(writer.entries(event));
            }
            self.encounter_seen = events.len();
        }
        let events = &engine.event_log.events;
        for event in events.iter().skip(self.global_seen) {
            entries.extend(writer.entries(event));
        }
        self.global_seen = events.len();

        // The engine splits events between the encounter log and the global
        // log, so within one batch put the combat/round headers first.
        entries.sort_by_key(|e| match e.kind.as_str() {
            "encounter_started" => 0,
            "round" => 1,
            _ => 2,
        });
        for entry in entries {
            self.push(entry);
        }
    }

    fn log_initiative(&mut self) {
        let Some(scene) = self.scene.as_ref() else {
            return;
        };
        let engine = self.engine.as_ref().unwrap();
        let Some(encounter) = engine.encounter(&scene.encounter) else {
            return;
        };
        let writer = LogWriter {
            handles: &scene.handles,
            round: Some(encounter.round()),
        };
        let entries: Vec<LogEntry> = encounter
            .initiative_order()
            .iter()
            .map(|(entity, roll)| {
                writer.initiative(&EntityIdentifier::from_world(&engine.world, *entity), roll)
            })
            .collect();
        // Initiative goes right after "Combat begins" and before round 1.
        let insert_at = self
            .log
            .iter()
            .position(|e| e.kind == "encounter_started")
            .map(|i| i + 1)
            .unwrap_or(0);
        let tail = self.log.split_off(insert_at);
        for entry in entries.into_iter().chain(tail) {
            self.push(entry);
        }
    }

    fn push(&mut self, mut entry: LogEntry) {
        entry.seq = self.log.len();
        if let Some(roll) = entry.roll.as_mut() {
            roll.id = format!("r{}", entry.seq);
        }
        self.log.push(entry);
    }

    // ---- lookups --------------------------------------------------------

    fn loaded(&self) -> Result<(&EngineState, &Scene), ApiError> {
        match (&self.engine, &self.scene) {
            (Some(engine), Some(scene)) => Ok((engine, scene)),
            _ => Err(ApiError::new("no_scene", "load a scene first")),
        }
    }

    fn entity(&self, handle: &str) -> Result<Entity, ApiError> {
        let (_, scene) = self.loaded()?;
        scene
            .creatures
            .iter()
            .find(|c| c.handle == handle)
            .map(|c| c.entity)
            .ok_or_else(|| ApiError::new("unknown_creature", format!("no creature {handle}")))
    }

    fn control_of(&self, entity: Entity) -> Option<Control> {
        let (_, scene) = self.loaded().ok()?;
        scene
            .creatures
            .iter()
            .find(|c| c.entity == entity)
            .map(|c| c.control)
    }

    fn handle_of(&self, entity: Entity) -> String {
        self.scene
            .as_ref()
            .and_then(|s| s.handles.get(&entity).cloned())
            .unwrap_or_else(|| format!("{entity:?}"))
    }
}

fn new_engine_state() -> EngineState {
    let terrain = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../assets/models/geometry/scenario_terrain.obj");
    EngineState::new(WorldGeometry::from_obj_path(
        terrain.to_str().expect("utf-8 path"),
        &rerecast::ConfigBuilder::default().build(),
    ))
}

fn monster_path(data_dir: &Path, monster: &str) -> Result<PathBuf, ApiError> {
    if !monster
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        return Err(ApiError::new(
            "bad_scene",
            format!("bad monster id {monster}"),
        ));
    }
    Ok(data_dir.join("monsters").join(format!("{monster}.json")))
}

fn life_state_name(engine: &EngineState, entity: Entity) -> &'static str {
    match engine.world.get::<&LifeState>(entity).map(|s| *s) {
        Ok(LifeState::Normal) => "normal",
        Ok(LifeState::Unconscious(_)) => "unconscious",
        Ok(LifeState::Stable) => "stable",
        Ok(LifeState::Dead) | Err(_) => "dead",
    }
}

fn distance_m(engine: &EngineState, a: Entity, b: Entity) -> Option<f32> {
    let pa = systems::geometry::get_foot_position(&engine.world, a)?;
    let pb = systems::geometry::get_foot_position(&engine.world, b)?;
    Some((pa - pb).norm())
}

fn weapon_name(engine: &EngineState, entity: Entity, slot: &EquipmentSlot) -> Option<String> {
    let loadout = systems::loadout::loadout(&engine.world, entity);
    let id = loadout.item_in_slot(slot)?;
    Some(
        id.id()
            .strip_prefix("item.")
            .unwrap_or(id.id())
            .replace('_', " "),
    )
}

/// Selects option `index` of the chosen action. The builder skips the choice
/// itself when an action has a single option, so index 0 is then a no-op.
fn pick_context(builder: &mut ActionBuilder, world: &hecs::World, index: usize) -> bool {
    match builder.state() {
        Ok(ActionBuilderState::Context { .. }) => {
            builder.context_index(world, index);
            builder.state().is_ok()
        }
        Ok(ActionBuilderState::Targets { .. }) => index == 0,
        _ => false,
    }
}

fn short_debug(value: &impl std::fmt::Debug) -> String {
    let text = format!("{value:?}");
    if text.len() > 400 {
        format!(
            "{}…",
            &text[..text
                .char_indices()
                .nth(400)
                .map(|(i, _)| i)
                .unwrap_or(text.len())]
        )
    } else {
        text
    }
}
