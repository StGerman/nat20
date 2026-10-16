//! Headless JSON API around `nat20_core`.
//!
//! A [`session::Session`] holds one engine and one scene. Callers send
//! [`protocol::Request`]s (`load_scene`, `get_state`, `legal_actions`,
//! `submit`, `react`, `end_turn`, `events`) and get back the result, the log
//! entries the request produced, and who the engine is waiting on. Scripted
//! creatures act inside the session, so a caller only ever sees its own
//! decisions. `main.rs` exposes the same thing as JSON lines over stdio.

pub mod audit;
pub mod duel;
pub mod log;
pub mod monster;
pub mod pregen;
pub mod protocol;
pub mod session;
