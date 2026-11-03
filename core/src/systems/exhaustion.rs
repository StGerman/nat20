//! The Exhaustion condition and the malnutrition that causes it (SRD 5.2.1).
//!
//! Exhaustion is cumulative: each level reduces every D20 Test by 2 and Speed by 5 feet,
//! and a creature dies at level 6. One effect per level carries the penalties
//! (`effect.condition.exhaustion_<n>`); this module swaps them as the level changes.
//! Finishing a Long Rest removes one level (see `systems::time::on_rest_end`).
//!
//! Food (Gameplay Toolbox, Food and Water): the caller reports how much a creature ate
//! during a day with [`end_of_day`]. A creature that eats but less than half of what its
//! size needs makes a DC 10 Constitution saving throw or gains a level; one that eats
//! nothing for 5 days gains a level at the end of the fifth day and of each day after.
//! Levels from malnutrition can't be removed until the creature eats the full amount for
//! a day.

use hecs::{Entity, World};

use crate::{
    components::{
        ability::Ability,
        d20::D20CheckDC,
        exhaustion::{Exhaustion, Hunger, MAX_EXHAUSTION_LEVEL},
        health::{hit_points::HitPoints, life_state::LifeState},
        id::{EffectId, EntityIdentifier},
        modifier::{ModifierMap, ModifierSource},
        saving_throw::SavingThrowKind,
        species::CreatureSize,
    },
    engine::{
        engine_state::EngineState,
        event::{CallbackResult, Event, EventCallback, EventKind},
    },
    systems,
};

/// The DC of the Constitution saving throw after a day on less than half rations.
pub const MALNUTRITION_SAVE_DC: i32 = 10;

/// Days without any food before the first automatic Exhaustion level.
pub const DAYS_WITHOUT_FOOD: u32 = 5;

/// How a creature ate during one day, compared with what its size needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Meal {
    /// Nothing at all
    Nothing,
    /// Something, but less than half
    LessThanHalf,
    /// At least half, but not all of it
    AtLeastHalf,
    /// The full amount or more
    Full,
}

/// The food a creature of `size` needs per day, in quarter pounds (SRD 5.2.1, Food Needs
/// per Day: Tiny 1/4 lb., Small and Medium 1 lb., Large 4 lb., Huge 16 lb., Gargantuan
/// 64 lb.).
pub fn food_needed_quarter_lb(size: &CreatureSize) -> u32 {
    match size {
        CreatureSize::Tiny => 1,
        CreatureSize::Small | CreatureSize::Medium => 4,
        CreatureSize::Large => 16,
        CreatureSize::Huge => 64,
        CreatureSize::Gargantuan => 256,
    }
}

/// How a creature of `size` that ate `eaten_quarter_lb` quarter pounds of food ate.
pub fn meal(size: &CreatureSize, eaten_quarter_lb: u32) -> Meal {
    let needed = food_needed_quarter_lb(size);
    if eaten_quarter_lb == 0 {
        Meal::Nothing
    } else if eaten_quarter_lb >= needed {
        Meal::Full
    } else if eaten_quarter_lb * 2 >= needed {
        Meal::AtLeastHalf
    } else {
        Meal::LessThanHalf
    }
}

/// The creature's Exhaustion level, 0 to 6.
pub fn level(world: &World, entity: Entity) -> u8 {
    exhaustion(world, entity).level
}

/// The creature's Exhaustion, including the levels malnutrition caused.
pub fn exhaustion(world: &World, entity: Entity) -> Exhaustion {
    world
        .get::<&Exhaustion>(entity)
        .map(|e| *e)
        .unwrap_or_default()
}

/// Days in a row the creature has eaten nothing.
pub fn days_without_food(world: &World, entity: Entity) -> u32 {
    world
        .get::<&Hunger>(entity)
        .map(|h| h.days_without_food)
        .unwrap_or(0)
}

fn effect_id(level: u8) -> EffectId {
    EffectId::new("nat20_core", format!("effect.condition.exhaustion_{level}"))
}

fn source() -> ModifierSource {
    ModifierSource::Custom("Exhaustion".to_string())
}

/// Sets the level, swapping the level's effect, and kills the creature at level 6.
fn set(engine_state: &mut EngineState, entity: Entity, new: Exhaustion) {
    let old = exhaustion(&engine_state.world, entity);
    if old.level == new.level {
        let _ = engine_state.world.insert_one(entity, new);
        return;
    }
    if old.level > 0 {
        systems::effects::remove_effects_by_id(engine_state, entity, &effect_id(old.level));
    }
    let _ = engine_state.world.insert_one(entity, new);
    if new.level > 0 {
        systems::effects::add_permanent_effect(
            engine_state,
            entity,
            effect_id(new.level),
            &source(),
            None,
        );
    }
    if new.level >= MAX_EXHAUSTION_LEVEL {
        die(engine_state, entity);
    }
}

fn die(engine_state: &mut EngineState, entity: Entity) {
    let max = engine_state
        .world
        .get::<&HitPoints>(entity)
        .map(|hp| hp.max())
        .unwrap_or(0);
    let _ = engine_state
        .world
        .insert(entity, (HitPoints::with_current(0, max), LifeState::Dead));
    let event = Event::new(EventKind::LifeStateChanged {
        entity: EntityIdentifier::from_world(&engine_state.world, entity),
        new_state: LifeState::Dead,
        actor: None,
    });
    engine_state.process_event(event);
}

/// Gives the creature `levels` more Exhaustion levels, up to 6; returns the new level.
pub fn gain(engine_state: &mut EngineState, entity: Entity, levels: u8) -> u8 {
    gain_from(engine_state, entity, levels, false)
}

fn gain_from(engine_state: &mut EngineState, entity: Entity, levels: u8, malnutrition: bool) -> u8 {
    let mut new = exhaustion(&engine_state.world, entity);
    let before = new.level;
    new.level = new.level.saturating_add(levels).min(MAX_EXHAUSTION_LEVEL);
    if malnutrition {
        new.from_malnutrition += new.level - before;
    }
    set(engine_state, entity, new);
    new.level
}

/// Removes one Exhaustion level a Long Rest can remove: none while every level is from
/// malnutrition the creature hasn't eaten its fill since. Returns the new level.
pub fn reduce_after_long_rest(engine_state: &mut EngineState, entity: Entity) -> u8 {
    let mut new = exhaustion(&engine_state.world, entity);
    if new.level > new.from_malnutrition {
        new.level -= 1;
        set(engine_state, entity, new);
    }
    new.level
}

/// Ends the creature's day: `eaten_quarter_lb` is how much food it ate today, in quarter
/// pounds. Runs the Malnutrition rules and returns how it ate. The DC 10 Constitution
/// saving throw, when one is due, is an ordinary saving throw event; on a failure the
/// creature gains a level.
pub fn end_of_day(engine_state: &mut EngineState, entity: Entity, eaten_quarter_lb: u32) -> Meal {
    let size = engine_state
        .world
        .get::<&CreatureSize>(entity)
        .map(|s| (*s).clone())
        .unwrap_or(CreatureSize::Medium);
    let meal = meal(&size, eaten_quarter_lb);
    let days = if meal == Meal::Nothing {
        days_without_food(&engine_state.world, entity) + 1
    } else {
        0
    };
    let _ = engine_state.world.insert_one(
        entity,
        Hunger {
            days_without_food: days,
        },
    );
    match meal {
        Meal::Full => {
            // Eating the full amount lets a Long Rest remove malnutrition levels again.
            let mut new = exhaustion(&engine_state.world, entity);
            new.from_malnutrition = 0;
            set(engine_state, entity, new);
        }
        Meal::AtLeastHalf => {}
        Meal::LessThanHalf => {
            let save = systems::d20::check(
                engine_state,
                entity,
                &D20CheckDC::SavingThrow {
                    saving_throw: SavingThrowKind::Ability(Ability::Constitution),
                    dc: ModifierMap::from(ModifierSource::Base, MALNUTRITION_SAVE_DC).evaluate(),
                },
            );
            let callback = EventCallback::new(move |engine_state, event, _| {
                if let EventKind::D20CheckResolved { result, dc, .. } = &event.kind
                    && !result.is_success(dc)
                {
                    gain_from(engine_state, entity, 1, true);
                }
                CallbackResult::None
            });
            engine_state.process_event_with_response_callback(save, callback);
        }
        Meal::Nothing => {
            if days >= DAYS_WITHOUT_FOOD {
                gain_from(engine_state, entity, 1, true);
            }
        }
    }
    meal
}
