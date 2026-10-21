use std::{fmt::Display, str::FromStr, sync::Arc};

use hecs::{Entity, World};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    components::{
        actions::action::{
            ActionContext, AttackRollFunction, AttackRollProvider, SavingThrowFunction,
            SavingThrowProvider,
        },
        ability::{Ability, AbilityScoreMap},
        d20::D20CheckDC,
        id::ClassId,
        modifier::{ModifierMap, ModifierSource},
        proficiency::ProficiencyLevel,
        saving_throw::SavingThrowKind,
        skill::Skill,
        spells::spellbook::Spellbook,
    },
    registry::serialize::schema::impl_string_schema,
    systems,
};

impl_string_schema!(
    AttackRollDefinition,
    "AttackRollDefinition",
    "description": "How the attack roll is made.",
    "enum": [
        "attack_context",
        "spell_attack_roll"
    ]
);

impl_string_schema!(
    SavingThrowDefinition,
    "SavingThrowDefinition",
    "description": "`<dc provider>;<ability>` where the DC provider is `spell_save_dc`, \
         `weapon_save_dc`, `spell_save_dc(<class id>)` for a class feature that uses that \
         class's spell save DC outside a spell, or `ability_save_dc(<ability>)` for 8 plus that \
         ability's modifier and the Proficiency Bonus (e.g. a Dragonborn's Breath Weapon), \
         e.g. `spell_save_dc;dexterity`.",
    "examples": [
        "spell_save_dc;dexterity",
        "weapon_save_dc;constitution",
        "spell_save_dc(nat20_core::class.cleric);constitution",
        "ability_save_dc(constitution);dexterity"
    ]
);

#[derive(Clone, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct AttackRollDefinition {
    pub raw: String,
    pub function: Arc<AttackRollFunction>,
}

impl Display for AttackRollDefinition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.raw)
    }
}

impl FromStr for AttackRollDefinition {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let function = match s {
            "attack_context" => Arc::new({
                |world: &World, entity: Entity, target: Entity, action_context: &ActionContext| {
                    systems::loadout::loadout(world, entity).attack_roll(
                        world,
                        entity,
                        target,
                        action_context,
                    )
                }
            }) as Arc<AttackRollFunction>,
            "spell_attack_roll" => Arc::new({
                |world: &World, entity: Entity, target: Entity, action_context: &ActionContext| {
                    systems::helpers::get_component::<Spellbook>(world, entity).attack_roll(
                        world,
                        entity,
                        target,
                        action_context,
                    )
                }
            }) as Arc<AttackRollFunction>,
            _ => {
                return Err(format!("Unknown AttackRollProvider: {}", s));
            }
        };

        Ok(Self {
            raw: s.to_string(),
            function,
        })
    }
}

impl TryFrom<String> for AttackRollDefinition {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}

impl From<AttackRollDefinition> for String {
    fn from(equation: AttackRollDefinition) -> Self {
        equation.raw
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct SavingThrowDefinition {
    pub raw: String,
    pub function: Arc<SavingThrowFunction>,
}

impl Display for SavingThrowDefinition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.raw)
    }
}

impl FromStr for SavingThrowDefinition {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Example format: "spell_save_dc;dexterity"

        let parts: Vec<&str> = s.split(';').collect();
        if parts.len() != 2 {
            return Err(format!("Invalid SavingThrowDefinition format: {}", s));
        }

        let kind: SavingThrowKind = serde_plain::from_str(parts[1]).unwrap();

        let function =
            match parts[0] {
                "weapon_save_dc" => Arc::new({
                    move |world: &World, entity: Entity, action_context: &ActionContext| {
                        systems::loadout::loadout(world, entity).saving_throw(
                            world,
                            entity,
                            action_context,
                            kind,
                        )
                    }
                }) as Arc<SavingThrowFunction>,
                "spell_save_dc" => {
                    Arc::new({
                        move |world: &World, entity: Entity, action_context: &ActionContext| {
                            systems::helpers::get_component::<Spellbook>(world, entity)
                                .saving_throw(world, entity, action_context, kind)
                        }
                    }) as Arc<SavingThrowFunction>
                }
                provider if provider.starts_with("ability_save_dc(") => {
                    let ability: Ability = provider
                        .strip_prefix("ability_save_dc(")
                        .and_then(|rest| rest.strip_suffix(')'))
                        .and_then(|ability| serde_plain::from_str(ability).ok())
                        .ok_or_else(|| format!("Invalid ability in SavingThrowDefinition: {}", s))?;
                    Arc::new({
                        move |world: &World, entity: Entity, _action_context: &ActionContext| {
                            ability_saving_throw(world, entity, ability, kind)
                        }
                    }) as Arc<SavingThrowFunction>
                }
                provider => {
                    let class: ClassId = provider
                        .strip_prefix("spell_save_dc(")
                        .and_then(|rest| rest.strip_suffix(')'))
                        .ok_or_else(|| format!("Unknown SavingThrowDefinition: {}", s))?
                        .parse()
                        .map_err(|_| format!("Invalid class id in SavingThrowDefinition: {}", s))?;
                    Arc::new({
                        move |world: &World, entity: Entity, _action_context: &ActionContext| {
                            systems::helpers::get_component::<Spellbook>(world, entity)
                                .class_saving_throw(world, entity, &class, kind)
                        }
                    }) as Arc<SavingThrowFunction>
                }
            };

        Ok(Self {
            raw: s.to_string(),
            function,
        })
    }
}

/// A DC of 8 plus `ability`'s modifier and the Proficiency Bonus, for a feature that is not a
/// spell, e.g. a Dragonborn's Breath Weapon (8 + Constitution modifier + Proficiency Bonus).
fn ability_saving_throw(
    world: &World,
    entity: Entity,
    ability: Ability,
    kind: SavingThrowKind,
) -> D20CheckDC {
    let modifier = systems::helpers::get_component::<AbilityScoreMap>(world, entity)
        .ability_modifier(&ability)
        .total();
    let proficiency_bonus =
        systems::helpers::level(world, entity).map_or(0, |level| level.proficiency_bonus());

    let mut dc = ModifierMap::from(ModifierSource::Base, 8);
    dc.add_modifier(ModifierSource::Ability(ability), modifier);
    dc.add_modifier(
        ModifierSource::Proficiency(ProficiencyLevel::Proficient),
        proficiency_bonus as i32,
    );
    D20CheckDC::SavingThrow {
        saving_throw: kind,
        dc: dc.evaluate(),
    }
}

impl TryFrom<String> for SavingThrowDefinition {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}

impl From<SavingThrowDefinition> for String {
    fn from(equation: SavingThrowDefinition) -> Self {
        equation.raw
    }
}

/// A skill check the actor makes against a fixed DC, e.g. "a successful DC 10
/// Wisdom (Medicine) check" to stabilize a creature
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SkillCheckDefinition {
    /// The skill the actor makes the check with
    pub skill: Skill,
    /// The DC the check has to meet or beat to succeed
    pub dc: u8,
}

impl From<SkillCheckDefinition> for D20CheckDC {
    fn from(value: SkillCheckDefinition) -> Self {
        D20CheckDC::Skill {
            skill: value.skill,
            dc: ModifierMap::from(ModifierSource::Base, value.dc as i32).evaluate(),
        }
    }
}
