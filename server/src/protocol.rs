//! Wire format: one JSON request per line in, one JSON response per line out.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::log::LogEntry;

#[derive(Debug, Clone, Deserialize)]
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
    EndTurn {
        actor: String,
    },
    /// The event log from entry `since` onwards (0 for all of it).
    Events {
        #[serde(default)]
        since: usize,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneDef {
    pub id: String,
    #[serde(default)]
    pub description: String,
    pub creatures: Vec<SceneCreatureDef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneCreatureDef {
    pub handle: String,
    #[serde(default)]
    pub pregen: Option<String>,
    #[serde(default)]
    pub monster: Option<String>,
    pub side: String,
    pub control: Control,
    pub position_m: [f32; 3],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Control {
    /// Decisions come from the API caller (human via the DM, or an AI player).
    Client,
    /// Decisions come from the server's scripted policy.
    Scripted,
}

#[derive(Debug, Clone, Serialize)]
pub struct Response {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ApiError>,
    /// Log entries appended while handling this request.
    pub new_events: Vec<LogEntry>,
    /// Who the engine is waiting on after this request.
    pub awaiting: Value,
}

#[derive(Debug, Clone, Serialize)]
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
