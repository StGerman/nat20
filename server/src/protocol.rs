//! Wire format: one JSON request per line in, one JSON response per line out.
//!
//! These types are the API contract. `schema.rs` turns them into the JSON
//! Schemas checked in under `server/schema/`; a test fails when they drift.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::log::LogEntry;

#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum Request {
    /// Load a scene, spawn its creatures and start combat. Give either an
    /// inline `scene` or a `scene_id` from `data/scenes`.
    LoadScene {
        #[serde(default)]
        scene: Option<SceneDef>,
        #[serde(default)]
        scene_id: Option<String>,
    },
    /// Creatures, HP, AC, positions, initiative order and who the engine is
    /// waiting on.
    GetState,
    /// Everything `actor` may do right now, with valid targets.
    LegalActions {
        actor: String,
    },
    /// Take an action. `context` picks among the action's options (for an
    /// attack, which weapon); see `legal_actions`.
    Submit {
        actor: String,
        action: String,
        #[serde(default)]
        context: usize,
        #[serde(default)]
        targets: Vec<String>,
    },
    /// Answer a reaction prompt: an option index from `awaiting.options`, or
    /// null to decline.
    React {
        actor: String,
        #[serde(default)]
        choice: Option<usize>,
    },
    /// End `actor`'s turn.
    EndTurn {
        actor: String,
    },
    /// The event log from entry `since` onwards (0 for all of it).
    Events {
        #[serde(default)]
        since: usize,
    },
}

/// A scene: the creatures to spawn, which side each is on and who controls it.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SceneDef {
    pub id: String,
    #[serde(default)]
    pub description: String,
    pub creatures: Vec<SceneCreatureDef>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SceneCreatureDef {
    /// The name every request and log entry uses for this creature.
    pub handle: String,
    #[serde(default)]
    /// A pregenerated character id. Give exactly one of `pregen` or `monster`.
    pub pregen: Option<String>,
    #[serde(default)]
    /// A stat block in `data/monsters`.
    pub monster: Option<String>,
    /// Creatures on different sides fight; combat ends when one side is left.
    pub side: String,
    pub control: Control,
    /// Spawn point in metres; the creature is dropped to the ground.
    pub position_m: [f32; 3],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Control {
    /// Decisions come from the API caller (human via the DM, or an AI player).
    Client,
    /// Decisions come from the server's scripted policy.
    Scripted,
}

/// One response per request.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct Response {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ApiError>,
    /// Log entries appended while handling this request.
    pub new_events: Vec<LogEntry>,
    /// Who the engine is waiting on after this request.
    pub awaiting: WaitingOn,
}

/// What the engine needs next.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WaitingOn {
    /// No scene is loaded.
    NoScene,
    /// Nothing is pending.
    Nothing,
    /// One side has nobody left standing.
    CombatOver { winner: String },
    /// `actor` must submit an action or end its turn.
    Action { actor: String },
    /// `actor` must answer a reaction prompt with `react`.
    Reaction {
        actor: String,
        options: Vec<ReactionOption>,
    },
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ReactionOption {
    /// The index to send as `react.choice`.
    pub choice: usize,
    pub action: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ApiError {
    pub code: String,
    pub message: String,
}

impl ApiError {
    pub fn new(code: &str, message: impl Into<String>) -> Self {
        Self {
            code: code.to_string(),
            message: message.into(),
        }
    }
}
