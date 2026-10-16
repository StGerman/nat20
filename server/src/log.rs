//! The server's event log: every engine event the API exposes, flattened into
//! JSON-friendly entries. Every die the engine rolls lands here as a `Roll`
//! with a stable id (`r<seq>`) that narration can cite.

use std::collections::HashMap;

use hecs::Entity;
use nat20_core::{
    components::{
        actions::action::{ActionResultComponent, DamageResult, HealingResult},
        d20::{D20CheckDC, D20CheckOutcome, D20CheckResult},
        damage::DamageRollResult,
        health::life_state::LifeState,
        id::{ActionId, EntityIdentifier},
        modifier::{FlatModifiable, ModifierKindResult, ModifierResult},
        time::TurnBoundary,
    },
    engine::event::{EncounterEvent, Event, EventKind},
};
use schemars::JsonSchema;
use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq, JsonSchema)]
pub struct LogEntry {
    pub seq: usize,
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub round: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub roll: Option<Roll>,
}

#[derive(Debug, Clone, Serialize, PartialEq, JsonSchema)]
pub struct Roll {
    pub id: String,
    /// attack | saving_throw | skill_check | initiative | damage | healing
    pub kind: String,
    pub label: String,
    pub dice: Vec<DiceRoll>,
    pub modifiers: Vec<Modifier>,
    pub total: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub against: Option<Against>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outcome: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, JsonSchema)]
pub struct DiceRoll {
    pub source: String,
    pub dice: String,
    pub results: Vec<u32>,
    /// For a d20 rolled with advantage or disadvantage, the die that counted.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kept: Option<u32>,
}

#[derive(Debug, Clone, Serialize, PartialEq, JsonSchema)]
pub struct Modifier {
    pub source: String,
    pub value: i32,
}

#[derive(Debug, Clone, Serialize, PartialEq, JsonSchema)]
pub struct Against {
    /// "AC" or "DC"
    pub kind: String,
    pub value: i32,
}

/// Turns engine events into log entries. Knows the scene's handles so entries
/// use the same names the API takes as input.
pub struct LogWriter<'a> {
    pub handles: &'a HashMap<Entity, String>,
    pub round: Option<usize>,
}

impl LogWriter<'_> {
    fn who(&self, id: &EntityIdentifier) -> String {
        self.handles
            .get(&id.id())
            .cloned()
            .unwrap_or_else(|| id.name().as_str().to_string())
    }

    fn entry(&self, kind: &str, text: String) -> LogEntry {
        LogEntry {
            seq: 0,
            kind: kind.to_string(),
            round: self.round,
            actor: None,
            target: None,
            text,
            roll: None,
        }
    }

    /// Returns zero or more entries for one engine event. `seq` and roll ids
    /// are assigned when the entries are appended to the session log.
    pub fn entries(&self, event: &Event) -> Vec<LogEntry> {
        match &event.kind {
            EventKind::Encounter(EncounterEvent::EncounterStarted(_)) => {
                vec![self.entry("encounter_started", "Combat begins.".into())]
            }
            EventKind::Encounter(EncounterEvent::EncounterEnded(..)) => {
                vec![self.entry("encounter_ended", "Combat ends.".into())]
            }
            EventKind::Encounter(EncounterEvent::NewRound(_, round)) => {
                let mut entry = self.entry("round", format!("Round {round}."));
                entry.round = Some(*round);
                vec![entry]
            }
            EventKind::TurnBoundary {
                entity,
                boundary: TurnBoundary::Start,
            } => {
                let mut entry = self.entry("turn_start", format!("{}'s turn.", self.who(entity)));
                entry.actor = Some(self.who(entity));
                vec![entry]
            }
            EventKind::ActionRequested { action } => {
                let actor = self.who(&action.actor);
                let targets: Vec<String> = action
                    .targets
                    .iter()
                    .filter_map(|t| match t {
                        nat20_core::components::actions::targeting::TargetInstance::Entity {
                            entity,
                            ..
                        } => Some(self.who(entity)),
                        _ => None,
                    })
                    .collect();
                let name = action_name(&action.action_id);
                let text = if targets.is_empty() || targets == [actor.clone()] {
                    format!("{actor} uses {name}.")
                } else {
                    format!("{actor} uses {name} on {}.", targets.join(", "))
                };
                let mut entry = self.entry("action", text);
                entry.actor = Some(actor);
                entry.target = targets.first().cloned();
                vec![entry]
            }
            EventKind::D20CheckResolved { actor, result, dc } => {
                let mut entry = self.entry("roll", String::new());
                entry.actor = Some(self.who(actor));
                let (kind, label, against, target) = match dc {
                    D20CheckDC::AttackRoll {
                        target,
                        armor_class,
                        ..
                    } => (
                        "attack",
                        format!("attack roll vs {}", self.who(target)),
                        Against {
                            kind: "AC".into(),
                            value: armor_class.total(),
                        },
                        Some(self.who(target)),
                    ),
                    D20CheckDC::SavingThrow { saving_throw, dc } => (
                        "saving_throw",
                        format!("{} saving throw", snake(&format!("{saving_throw:?}"))),
                        Against {
                            kind: "DC".into(),
                            value: dc.total(),
                        },
                        None,
                    ),
                    D20CheckDC::Skill { skill, dc } => (
                        "skill_check",
                        format!("{} check", snake(&format!("{skill:?}"))),
                        Against {
                            kind: "DC".into(),
                            value: dc.total(),
                        },
                        None,
                    ),
                };
                let success = result.is_success(dc);
                let outcome = match (kind, result.outcome, success) {
                    ("attack", Some(D20CheckOutcome::CriticalSuccess), _) => "critical hit",
                    ("attack", Some(D20CheckOutcome::CriticalFailure), _) => "critical miss",
                    ("attack", _, true) => "hit",
                    ("attack", _, false) => "miss",
                    (_, _, true) => "success",
                    (_, _, false) => "failure",
                };
                let roll = d20_roll(kind, label, result, Some(against), Some(outcome));
                entry.text = format!(
                    "{} {}: {}.",
                    self.who(actor),
                    roll.label,
                    roll_summary(&roll)
                );
                entry.target = target;
                entry.roll = Some(roll);
                vec![entry]
            }
            EventKind::DamageRollResolved { actor, result } => {
                let mut entry = self.entry("roll", String::new());
                entry.actor = Some(self.who(actor));
                let roll = damage_roll(result);
                entry.text = format!(
                    "{} rolls {}: {}.",
                    self.who(actor),
                    roll.label,
                    roll_summary(&roll)
                );
                entry.roll = Some(roll);
                vec![entry]
            }
            EventKind::ActionResult { result, .. } => {
                let target = self.who(&result.target);
                let mut entries = Vec::new();
                for component in &result.components {
                    let new_life_state = match component {
                        ActionResultComponent::Damage(DamageResult {
                            damage_taken,
                            new_life_state,
                            ..
                        }) => {
                            if let Some(taken) = damage_taken {
                                let mut entry = self.entry(
                                    "damage_taken",
                                    format!("{target} takes {} damage.", taken.total),
                                );
                                entry.target = Some(target.clone());
                                entries.push(entry);
                            }
                            new_life_state
                        }
                        ActionResultComponent::Healing(HealingResult {
                            healing,
                            new_life_state,
                        }) => {
                            let mut entry = self.entry("roll", String::new());
                            entry.actor = Some(target.clone());
                            entry.target = Some(target.clone());
                            let roll = modifier_roll("healing", "healing".into(), healing);
                            entry.text =
                                format!("{target} regains hit points: {}.", roll_summary(&roll));
                            entry.roll = Some(roll);
                            entries.push(entry);
                            new_life_state
                        }
                        _ => &None,
                    };
                    if let Some(state) = new_life_state {
                        let mut entry = self.entry(
                            "life_state",
                            format!("{target} is {}.", life_state_word(state)),
                        );
                        entry.target = Some(target.clone());
                        entries.push(entry);
                    }
                }
                entries
            }
            EventKind::LifeStateChanged {
                entity, new_state, ..
            } => {
                let mut entry = self.entry(
                    "life_state",
                    format!("{} is {}.", self.who(entity), life_state_word(new_state)),
                );
                entry.target = Some(self.who(entity));
                vec![entry]
            }
            // Performed events are followed by Resolved events carrying the
            // final result after any reactions; logging both would show every
            // die twice.
            _ => Vec::new(),
        }
    }

    pub fn initiative(&self, entity: &EntityIdentifier, result: &D20CheckResult) -> LogEntry {
        let mut entry = self.entry("roll", String::new());
        entry.actor = Some(self.who(entity));
        let roll = d20_roll("initiative", "initiative".into(), result, None, None);
        entry.text = format!(
            "{} rolls initiative: {}.",
            self.who(entity),
            roll_summary(&roll)
        );
        entry.roll = Some(roll);
        entry
    }
}

fn life_state_word(state: &LifeState) -> &'static str {
    match state {
        LifeState::Normal => "conscious",
        LifeState::Unconscious(_) => "unconscious",
        LifeState::Stable => "stable",
        LifeState::Dead => "dead",
    }
}

pub fn action_name(id: &ActionId) -> String {
    let id = id.id();
    let tail = id.strip_prefix("action.").unwrap_or(id);
    tail.replace(['.', '_'], " ")
}

fn snake(s: &str) -> String {
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if c.is_uppercase() && i > 0 {
            out.push(' ');
        }
        out.push(c.to_ascii_lowercase());
    }
    out
}

fn collect_modifiers(result: &ModifierResult, dice: &mut Vec<DiceRoll>, mods: &mut Vec<Modifier>) {
    fn walk(
        source: &str,
        kind: &ModifierKindResult,
        dice: &mut Vec<DiceRoll>,
        mods: &mut Vec<Modifier>,
    ) {
        match kind {
            ModifierKindResult::Flat(value) => mods.push(Modifier {
                source: source.to_string(),
                value: *value,
            }),
            ModifierKindResult::Dice(result) => dice.push(DiceRoll {
                source: source.to_string(),
                dice: result.dice().to_string(),
                results: result.rolls().to_vec(),
                kept: None,
            }),
            ModifierKindResult::Composite(parts) => {
                for part in parts {
                    walk(source, part, dice, mods);
                }
            }
        }
    }
    for (source, kind) in result.iter() {
        walk(&source.to_string(), kind, dice, mods);
    }
}

fn d20_roll(
    kind: &str,
    label: String,
    result: &D20CheckResult,
    against: Option<Against>,
    outcome: Option<&str>,
) -> Roll {
    let mut dice = vec![DiceRoll {
        source: "d20".into(),
        dice: format!("{}d20", result.rolls.len()),
        results: result.rolls.iter().map(|r| *r as u32).collect(),
        kept: (result.rolls.len() > 1).then_some(result.selected_roll as u32),
    }];
    let mut modifiers = Vec::new();
    collect_modifiers(result.modifiers(), &mut dice, &mut modifiers);
    Roll {
        id: String::new(),
        kind: kind.into(),
        label,
        dice,
        modifiers,
        total: result.total(),
        against,
        outcome: outcome.map(str::to_string),
    }
}

fn damage_roll(result: &DamageRollResult) -> Roll {
    let mut dice = Vec::new();
    let mut modifiers = Vec::new();
    let mut types = Vec::new();
    for component in &result.components {
        collect_modifiers(&component.result, &mut dice, &mut modifiers);
        types.push(component.damage_type.to_string().to_lowercase());
    }
    types.dedup();
    let label = format!(
        "{}{} damage",
        if result.crit { "critical " } else { "" },
        types.join(" and ")
    );
    Roll {
        id: String::new(),
        kind: "damage".into(),
        label,
        dice,
        modifiers,
        total: result.total,
        against: None,
        outcome: None,
    }
}

fn modifier_roll(kind: &str, label: String, result: &ModifierResult) -> Roll {
    let mut dice = Vec::new();
    let mut modifiers = Vec::new();
    collect_modifiers(result, &mut dice, &mut modifiers);
    Roll {
        id: String::new(),
        kind: kind.into(),
        label,
        dice,
        modifiers,
        total: result.total(),
        against: None,
        outcome: None,
    }
}

/// "d20 (14) +5 = 19 vs AC 15, hit"
pub fn roll_summary(roll: &Roll) -> String {
    let mut parts = Vec::new();
    for die in &roll.dice {
        let results = die
            .results
            .iter()
            .map(|r| r.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        match die.kept {
            Some(kept) => parts.push(format!("{} ({results}, kept {kept})", die.dice)),
            None => parts.push(format!("{} ({results})", die.dice)),
        }
    }
    let flat: i32 = roll.modifiers.iter().map(|m| m.value).sum();
    let mut text = parts.join(" + ");
    if flat != 0 || text.is_empty() {
        text.push_str(&format!(
            " {}{}",
            if flat < 0 { "-" } else { "+" },
            flat.abs()
        ));
    }
    text.push_str(&format!(" = {}", roll.total));
    if let Some(against) = &roll.against {
        text.push_str(&format!(" vs {} {}", against.kind, against.value));
    }
    if let Some(outcome) = &roll.outcome {
        text.push_str(&format!(", {outcome}"));
    }
    text
}
