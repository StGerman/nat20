use std::{collections::HashSet, sync::Arc};

use hecs::{Entity, World};
use serde::Deserialize;

use crate::{
    components::{
        id::{EffectId, FeatId, IdProvider},
        items::equipment::{armor::ArmorType, weapon::WeaponCategory},
        level_up::LevelUpPrompt,
    },
    registry::serialize::feat::FeatDefinition,
};

pub type FeatPrerequisite = dyn Fn(&World, Entity) -> bool + Send + Sync;

#[derive(Clone, Deserialize)]
#[serde(from = "FeatDefinition")]
pub struct Feat {
    id: FeatId,
    description: String,
    prerequisite: Option<Arc<FeatPrerequisite>>,
    effects: Vec<EffectId>,
    /// Some feats might require a choice to be made when selected.
    /// In most cases this will be some kind of ability score increase, but could
    /// also be a choice between learning a new spell etc.
    // TODO: Is it ever more than one?
    prompts: Vec<LevelUpPrompt>,
    /// Most feats are single-use, but some can be taken multiple times.
    /// This mostly applies to Ability Score Improvement.
    repeatable: bool,
    /// Armor training the feat grants, e.g. Heavy armor for the Cleric's Protector.
    armor_training: HashSet<ArmorType>,
    /// Weapon proficiencies the feat grants, e.g. Martial weapons for the Protector.
    weapon_proficiencies: HashSet<WeaponCategory>,
}

impl Feat {
    pub fn new(
        id: FeatId,
        description: String,
        prerequisite: Option<Arc<FeatPrerequisite>>,
        effects: Vec<EffectId>,
        prompts: Vec<LevelUpPrompt>,
        repeatable: bool,
    ) -> Self {
        Self {
            id,
            description,
            prerequisite,
            effects,
            prompts,
            repeatable,
            armor_training: HashSet::new(),
            weapon_proficiencies: HashSet::new(),
        }
    }

    /// Adds armor training and weapon proficiencies the feat grants.
    pub fn with_training(
        mut self,
        armor_training: HashSet<ArmorType>,
        weapon_proficiencies: HashSet<WeaponCategory>,
    ) -> Self {
        self.armor_training = armor_training;
        self.weapon_proficiencies = weapon_proficiencies;
        self
    }

    pub fn armor_training(&self) -> &HashSet<ArmorType> {
        &self.armor_training
    }

    pub fn weapon_proficiencies(&self) -> &HashSet<WeaponCategory> {
        &self.weapon_proficiencies
    }

    pub fn id(&self) -> &FeatId {
        &self.id
    }

    pub fn meets_prerequisite(&self, world: &World, entity: Entity) -> bool {
        if let Some(prerequisite) = &self.prerequisite {
            prerequisite(world, entity)
        } else {
            true
        }
    }

    pub fn effects(&self) -> &Vec<EffectId> {
        &self.effects
    }

    pub fn prompts(&self) -> &[LevelUpPrompt] {
        &self.prompts
    }

    pub fn is_repeatable(&self) -> bool {
        self.repeatable
    }

    pub fn description(&self) -> &str {
        &self.description
    }
}

impl IdProvider for Feat {
    type Id = FeatId;

    fn id(&self) -> &Self::Id {
        &self.id
    }
}
