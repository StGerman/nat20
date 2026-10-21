use std::fmt;

use hecs::Entity;

use crate::{
    components::{
        faction::FactionSet,
        health::hit_points::HitPoints,
        id::{ItemId, MonsterId, Name},
        items::{
            equipment::{
                armor::ArmorTrainingSet, loadout::TryEquipError, weapon::WeaponProficiencyMap,
            },
            inventory::ItemInstance,
        },
        modifier::ModifierSource,
        proficiency::{Proficiency, ProficiencyLevel},
        saving_throw::{SavingThrowKind, SavingThrowSet},
        skill::SkillSet,
        speed::Speed,
    },
    engine::engine_state::EngineState,
    entities::creature::Monster,
    registry::{
        self,
        registry::{ItemsRegistry, MonstersRegistry},
        serialize::monster::MonsterDefinition,
    },
    systems,
};

#[derive(Debug, Clone, PartialEq)]
pub enum SpawnMonsterError {
    UnknownMonster(MonsterId),
    InvalidSpeed {
        monster: MonsterId,
        message: String,
    },
    Equip {
        item: ItemId,
        error: TryEquipError,
    },
    /// Equipping `item` took `displaced` off again, so the stat block's gear
    /// doesn't fit together.
    GearConflict {
        item: ItemId,
        displaced: Vec<ItemId>,
    },
}

impl fmt::Display for SpawnMonsterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SpawnMonsterError::UnknownMonster(id) => write!(f, "Unknown monster {}", id),
            SpawnMonsterError::InvalidSpeed { monster, message } => {
                write!(f, "Invalid speed for {}: {}", monster, message)
            }
            SpawnMonsterError::Equip { item, error } => {
                write!(f, "Failed to equip {}: {:?}", item, error)
            }
            SpawnMonsterError::GearConflict { item, displaced } => {
                write!(f, "Equipping {} unequipped {:?}", item, displaced)
            }
        }
    }
}

impl std::error::Error for SpawnMonsterError {}

/// Spawns the monster with the stat block `monster_id` from the registry: its
/// numbers, its gear equipped (with training and proficiency in all of it), its
/// skill and saving throw proficiencies and its permanent effects, at full Hit
/// Points. It is controlled by the random AI until the caller says otherwise.
/// Nothing is left in the world if spawning fails.
pub fn spawn(
    engine_state: &mut EngineState,
    monster_id: &MonsterId,
) -> Result<Entity, SpawnMonsterError> {
    let definition = MonstersRegistry::get(monster_id)
        .ok_or_else(|| SpawnMonsterError::UnknownMonster(monster_id.clone()))?;

    let speed = definition
        .speed
        .evaluate_without_variables()
        .map_err(|error| SpawnMonsterError::InvalidSpeed {
            monster: monster_id.clone(),
            message: format!("{:?}", error),
        })?;

    let monster = Monster::new(
        Name::new(definition.name.clone()),
        registry::ai::RANDOM_CONTROLLER_ID.clone(),
        definition.challenge_rating.0,
        HitPoints::new(definition.hit_points.average),
        definition.size.clone(),
        definition.creature_type.clone(),
        Speed::new(speed),
        (&definition.abilities).into(),
        definition.factions.iter().cloned().collect::<FactionSet>(),
    );
    let entity = engine_state.world.spawn(monster);

    if let Err(error) = fill_in(engine_state, entity, definition) {
        let _ = engine_state.despawn(entity);
        return Err(error);
    }

    Ok(entity)
}

fn fill_in(
    engine_state: &mut EngineState,
    entity: Entity,
    definition: &MonsterDefinition,
) -> Result<(), SpawnMonsterError> {
    let source = ModifierSource::Custom(format!("{} stat block", definition.name));

    {
        let skills =
            systems::helpers::get_component_mut::<SkillSet>(&mut engine_state.world, entity);
        for (skill, level) in &definition.skills {
            skills.set_proficiency(skill, Proficiency::new(*level, source.clone()));
        }
    }
    {
        let saving_throws =
            systems::helpers::get_component_mut::<SavingThrowSet>(&mut engine_state.world, entity);
        for (ability, level) in &definition.saving_throws {
            saving_throws.set_proficiency(
                &SavingThrowKind::Ability(*ability),
                Proficiency::new(*level, source.clone()),
            );
        }
    }

    for item_id in &definition.gear {
        // Monsters are trained in their armour and proficient with their weapons.
        match ItemsRegistry::get(item_id) {
            Some(ItemInstance::Armor(armor)) => {
                systems::helpers::get_component_mut::<ArmorTrainingSet>(
                    &mut engine_state.world,
                    entity,
                )
                .insert(armor.armor_type.clone());
            }
            Some(ItemInstance::Weapon(weapon)) => {
                systems::helpers::get_component_mut::<WeaponProficiencyMap>(
                    &mut engine_state.world,
                    entity,
                )
                .set_proficiency(
                    weapon.category().clone(),
                    Proficiency::new(ProficiencyLevel::Proficient, source.clone()),
                );
            }
            _ => {}
        }

        let displaced =
            systems::loadout::equip(engine_state, entity, item_id).map_err(|error| {
                SpawnMonsterError::Equip {
                    item: item_id.clone(),
                    error,
                }
            })?;
        if !displaced.is_empty() {
            return Err(SpawnMonsterError::GearConflict {
                item: item_id.clone(),
                displaced,
            });
        }
    }

    systems::effects::add_permanent_effects(
        engine_state,
        entity,
        &definition.effects,
        &source,
        None,
    );

    let _ = systems::health::heal_full(&mut engine_state.world, entity);

    Ok(())
}
