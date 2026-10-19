use hecs::{Entity, World};

use crate::{
    components::{
        id::FeatId,
        items::equipment::{armor::ArmorTrainingSet, weapon::WeaponProficiencyMap},
        level_up::LevelUpPrompt,
        modifier::ModifierSource,
        proficiency::{Proficiency, ProficiencyLevel},
    },
    engine::engine_state::EngineState,
    registry::registry::FeatsRegistry,
    systems,
};

#[derive(Debug, Clone)]
pub enum FeatError {
    RegistryMissing(String),
    PrequisiteNotMet { feat_id: FeatId, entity: Entity },
    AlreadyHasUnrepeatableFeat { feat_id: FeatId, entity: Entity },
}

pub fn feats(world: &World, entity: Entity) -> hecs::Ref<'_, Vec<FeatId>> {
    systems::helpers::get_component::<Vec<FeatId>>(world, entity)
}

pub fn feats_mut(world: &mut World, entity: Entity) -> &mut Vec<FeatId> {
    systems::helpers::get_component_mut::<Vec<FeatId>>(world, entity)
}

pub fn can_acquire_feat(world: &World, entity: Entity, feat_id: &FeatId) -> Result<(), FeatError> {
    let feat = FeatsRegistry::get(feat_id);
    if feat.is_none() {
        return Err(FeatError::RegistryMissing(feat_id.to_string()));
    }

    let feat = feat.unwrap();

    if !feat.meets_prerequisite(world, entity) {
        return Err(FeatError::PrequisiteNotMet {
            feat_id: feat.id().clone(),
            entity,
        });
    }

    if !feat.is_repeatable() && feats(world, entity).contains(feat_id) {
        return Err(FeatError::AlreadyHasUnrepeatableFeat {
            feat_id: feat.id().clone(),
            entity,
        });
    }

    Ok(())
}

pub fn add_feat(
    engine_state: &mut EngineState,
    entity: Entity,
    feat_id: &FeatId,
) -> Result<Vec<LevelUpPrompt>, FeatError> {
    let mut prompts = Vec::new();

    can_acquire_feat(&engine_state.world, entity, feat_id)?;
    let feat = FeatsRegistry::get(feat_id).unwrap();

    systems::effects::add_permanent_effects(
        engine_state,
        entity,
        feat.effects(),
        &ModifierSource::Feat(feat.id().clone()),
        None,
    );

    let source = ModifierSource::Feat(feat.id().clone());
    systems::helpers::get_component_mut::<ArmorTrainingSet>(&mut engine_state.world, entity)
        .extend(feat.armor_training().iter().cloned());
    let weapon_proficiencies = systems::helpers::get_component_mut::<WeaponProficiencyMap>(
        &mut engine_state.world,
        entity,
    );
    for category in feat.weapon_proficiencies() {
        weapon_proficiencies.set_proficiency(
            category.clone(),
            Proficiency::new(ProficiencyLevel::Proficient, source.clone()),
        );
    }

    prompts.extend(feat.prompts().iter().cloned());

    feats_mut(&mut engine_state.world, entity).push(feat.id().clone());

    Ok(prompts)
}
