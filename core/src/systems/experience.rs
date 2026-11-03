//! Experience points (SRD 5.2.1): what defeated monsters are worth, sharing an award evenly,
//! and whether a character has the XP for its next level.
//!
//! The caller decides when XP is earned (a fight won, a goal met); core counts it. Levelling
//! up stays the separate level-up choice (`systems::level_up`).

use hecs::{Entity, World};

use crate::{
    components::level::{ChallengeRating, CharacterLevels},
    engine::engine_state::EngineState,
};

/// The XP a creature is worth when defeated: its Challenge Rating's XP, or 0 for a creature
/// without one (a character).
pub fn worth(world: &World, entity: Entity) -> u32 {
    world
        .get::<&ChallengeRating>(entity)
        .map(|cr| cr.experience())
        .unwrap_or(0)
}

/// The XP a character has; 0 for a creature without character levels.
pub fn experience(world: &World, entity: Entity) -> u32 {
    world
        .get::<&CharacterLevels>(entity)
        .map(|levels| levels.experience())
        .unwrap_or(0)
}

/// Whether the character has the XP for its next level.
pub fn can_level_up(world: &World, entity: Entity) -> bool {
    world
        .get::<&CharacterLevels>(entity)
        .map(|levels| levels.can_level_up())
        .unwrap_or(false)
}

/// Splits `total` XP evenly among `characters`, rounding each share down, and adds a share to
/// each that has character levels. Returns the share.
pub fn award(engine_state: &mut EngineState, characters: &[Entity], total: u32) -> u32 {
    let Ok(count) = u32::try_from(characters.len()) else {
        return 0;
    };
    if count == 0 {
        return 0;
    }
    let share = total / count;
    for entity in characters {
        if let Ok(mut levels) = engine_state.world.get::<&mut CharacterLevels>(*entity) {
            levels.add_experience(share);
        }
    }
    share
}
