use std::collections::{HashMap, HashSet};

use schemars::JsonSchema;
use serde::Deserialize;

use crate::{
    components::{
        id::{ActionId, EffectId, ResourceId, SpeciesId, SubspeciesId},
        level_up::LevelUpPrompt,
        modifier::ModifierSource,
        resource::{ResourceBudgetKind, ResourceMergePolicy},
        species::{CreatureSize, CreatureType, Species, SpeciesBase, Subspecies},
        speed::Speed,
    },
    registry::serialize::{class::ClassResourceDefinition, quantity::LengthExpressionDefinition},
};

#[derive(Clone, Deserialize, JsonSchema)]
pub struct SpeciesDefinition {
    pub id: SpeciesId,
    pub creature_type: CreatureType,
    pub size: CreatureSize,
    #[serde(default)]
    pub effects_by_level: HashMap<u8, Vec<EffectId>>,
    #[serde(default)]
    pub actions_by_level: HashMap<u8, Vec<ActionId>>,
    /// Resources granted at each level, e.g. uses of a species feature.
    #[serde(default)]
    pub resources_by_level: HashMap<u8, Vec<ClassResourceDefinition>>,
    pub subspecies: HashSet<SubspeciesId>,
    pub speed: LengthExpressionDefinition,
    /// Choices the species asks for when it is picked, e.g. a skill or an origin feat.
    #[serde(default)]
    pub prompts: Vec<LevelUpPrompt>,
}

impl From<SpeciesDefinition> for Species {
    fn from(value: SpeciesDefinition) -> Self {
        let id = value.id;
        Species {
            id: id.clone(),
            base: SpeciesBase {
                effects_by_level: value.effects_by_level,
                actions_by_level: value.actions_by_level,
                resources_by_level: resource_grants(
                    value.resources_by_level,
                    ModifierSource::Species(id.clone()),
                ),
            },
            subspecies: value.subspecies,
            creature_type: value.creature_type,
            size: value.size,
            speed: Speed::new(value.speed.evaluate_without_variables().unwrap()),
            prompts: value.prompts,
        }
    }
}

#[derive(Clone, Deserialize, JsonSchema)]
pub struct SubspeciesDefinition {
    pub id: SubspeciesId,
    #[serde(default)]
    pub effects_by_level: HashMap<u8, Vec<EffectId>>,
    #[serde(default)]
    pub actions_by_level: HashMap<u8, Vec<ActionId>>,
    /// Resources granted at each level, e.g. uses of a species feature.
    #[serde(default)]
    pub resources_by_level: HashMap<u8, Vec<ClassResourceDefinition>>,
}

impl From<SubspeciesDefinition> for Subspecies {
    fn from(value: SubspeciesDefinition) -> Self {
        let id = value.id;
        Subspecies {
            id: id.clone(),
            base: SpeciesBase {
                effects_by_level: value.effects_by_level,
                actions_by_level: value.actions_by_level,
                resources_by_level: resource_grants(
                    value.resources_by_level,
                    ModifierSource::Subspecies(id.clone()),
                ),
            },
        }
    }
}

fn resource_grants(
    resources_by_level: HashMap<u8, Vec<ClassResourceDefinition>>,
    source: ModifierSource,
) -> HashMap<u8, Vec<(ResourceId, ResourceBudgetKind, ResourceMergePolicy)>> {
    resources_by_level
        .into_iter()
        .map(|(level, resources)| {
            (
                level,
                resources
                    .into_iter()
                    .map(|resource| resource.into_grant(source.clone()))
                    .collect(),
            )
        })
        .collect()
}
