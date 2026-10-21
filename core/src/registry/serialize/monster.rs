//! Monster stat blocks (`assets/registries/monsters/`). A stat block holds the
//! numbers the [`Monster`](crate::entities::creature::Monster) bundle holds,
//! plus references to existing item, effect and faction ids; behaviour comes
//! only from those references. [`systems::monsters::spawn`](crate::systems::monsters::spawn)
//! builds the creature from it.

use std::{collections::HashMap, fmt, str::FromStr};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    components::{
        ability::{Ability, AbilityScoreMap},
        id::{EffectId, FactionId, IdProvider, ItemId, MonsterId},
        level::ChallengeRating,
        proficiency::ProficiencyLevel,
        skill::Skill,
        species::{CreatureSize, CreatureType},
    },
    registry::{
        registry_validation::{ReferenceCollector, RegistryReference, RegistryReferenceCollector},
        serialize::{quantity::LengthExpressionDefinition, schema::impl_string_schema},
    },
};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct MonsterDefinition {
    pub id: MonsterId,
    pub name: String,
    pub size: CreatureSize,
    pub creature_type: CreatureType,
    pub challenge_rating: ChallengeRatingDefinition,
    pub hit_points: MonsterHitPoints,
    pub speed: LengthExpressionDefinition,
    pub abilities: MonsterAbilityScores,
    /// Skill proficiencies, e.g. `"stealth": "expertise"`.
    #[serde(default)]
    pub skills: HashMap<Skill, ProficiencyLevel>,
    /// Saving throw proficiencies, e.g. `"dexterity": "proficient"`.
    #[serde(default)]
    pub saving_throws: HashMap<Ability, ProficiencyLevel>,
    /// Items equipped in this order, each in the slot core picks for it. The
    /// monster is trained in all of its armour and proficient with all of its
    /// weapons.
    #[serde(default)]
    pub gear: Vec<ItemId>,
    /// Permanent effects, e.g. traits.
    #[serde(default)]
    pub effects: Vec<EffectId>,
    pub factions: Vec<FactionId>,
}

impl IdProvider for MonsterDefinition {
    type Id = MonsterId;

    fn id(&self) -> &Self::Id {
        &self.id
    }
}

impl RegistryReferenceCollector for MonsterDefinition {
    fn collect_registry_references(&self, collector: &mut ReferenceCollector) {
        for item in &self.gear {
            collector.add(RegistryReference::Item(item.clone()));
        }
        for effect in &self.effects {
            collector.add(RegistryReference::Effect(effect.clone()));
        }
        for faction in &self.factions {
            collector.add(RegistryReference::Faction(faction.clone()));
        }
    }
}

/// The stat block's Hit Points: the fixed average the monster spawns with, and
/// the dice it comes from.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct MonsterHitPoints {
    pub average: u32,
    /// The Hit Point dice as the stat block writes them, e.g. `2d8 + 2`.
    pub dice: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct MonsterAbilityScores {
    pub strength: i32,
    pub dexterity: i32,
    pub constitution: i32,
    pub intelligence: i32,
    pub wisdom: i32,
    pub charisma: i32,
}

impl From<&MonsterAbilityScores> for AbilityScoreMap {
    fn from(value: &MonsterAbilityScores) -> Self {
        AbilityScoreMap::from([
            (Ability::Strength, value.strength),
            (Ability::Dexterity, value.dexterity),
            (Ability::Constitution, value.constitution),
            (Ability::Intelligence, value.intelligence),
            (Ability::Wisdom, value.wisdom),
            (Ability::Charisma, value.charisma),
        ])
    }
}

/// A [`ChallengeRating`] written as a string, e.g. `"1/4"` or `"3"`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ChallengeRatingDefinition(pub ChallengeRating);

impl_string_schema!(
    ChallengeRatingDefinition,
    "ChallengeRating",
    "description": "Challenge Rating: `0`, `1/8`, `1/4`, `1/2` or a whole number up to `30`.",
    "examples": ["1/4", "1", "5"]
);

impl TryFrom<String> for ChallengeRatingDefinition {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        ChallengeRating::from_str(&value).map(Self)
    }
}

impl From<ChallengeRatingDefinition> for String {
    fn from(value: ChallengeRatingDefinition) -> Self {
        value.to_string()
    }
}

impl fmt::Display for ChallengeRatingDefinition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}
