//! The Exhaustion condition's level and the hunger that can cause it (SRD 5.2.1, Rules
//! Glossary > Exhaustion; Gameplay Toolbox > Food and Water).

use serde::{Deserialize, Serialize};

/// The highest Exhaustion level; a creature at this level dies.
pub const MAX_EXHAUSTION_LEVEL: u8 = 6;

/// A creature's Exhaustion level, and how many of those levels malnutrition caused.
/// Levels from malnutrition can't be removed until the creature eats the full amount of
/// food it needs for a day.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Exhaustion {
    pub level: u8,
    /// Levels malnutrition caused that a Long Rest can't remove yet; never above `level`
    pub from_malnutrition: u8,
}

/// Days in a row a creature has eaten nothing.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hunger {
    pub days_without_food: u32,
}
