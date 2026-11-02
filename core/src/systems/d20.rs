use hecs::{Entity, Ref, World};

use crate::{
    components::{
        d20::{D20Check, D20CheckDC, D20CheckKind, D20CheckResult, RollMode},
        damage::AttackSource,
        id::{ActionId, EntityIdentifier},
        items::equipment::loadout::Loadout,
        modifier::{Modifiable, ModifierKind},
        saving_throw::SavingThrowSet,
        skill::{Skill, SkillSet},
        spells::spellbook::Spellbook,
    },
    engine::{
        event::{Event, EventKind},
        engine_state::EngineState,
    },
    systems,
};

pub fn get(world: &World, entity: Entity, kind: D20CheckKind) -> Ref<'_, D20Check> {
    match kind {
        D20CheckKind::SavingThrow(saving_throw) => Ref::map(
            systems::helpers::get_component::<SavingThrowSet>(world, entity),
            |set| set.get(&saving_throw),
        ),

        D20CheckKind::Skill(skill) => Ref::map(
            systems::helpers::get_component::<SkillSet>(world, entity),
            |set| set.get(&skill),
        ),

        D20CheckKind::AttackRoll(source) => match source {
            AttackSource::Weapon(weapon_kind) => Ref::map(
                systems::helpers::get_component::<Loadout>(world, entity),
                |loadout| loadout.attack_roll_template(&weapon_kind),
            ),

            AttackSource::Spell => Ref::map(
                systems::helpers::get_component::<Spellbook>(world, entity),
                |spellbook| spellbook.attack_roll_template(),
            ),
        },
    }
}

pub fn get_mut(world: &mut World, entity: Entity, kind: D20CheckKind) -> &mut D20Check {
    match kind {
        D20CheckKind::SavingThrow(saving_throw) => {
            systems::helpers::get_component_mut::<SavingThrowSet>(world, entity)
                .get_mut(&saving_throw)
        }

        D20CheckKind::Skill(skill) => {
            systems::helpers::get_component_mut::<SkillSet>(world, entity).get_mut(&skill)
        }

        D20CheckKind::AttackRoll(source) => match source {
            AttackSource::Weapon(weapon_kind) => {
                systems::helpers::get_component_mut::<Loadout>(world, entity)
                    .attack_roll_template_mut(&weapon_kind)
            }

            AttackSource::Spell => systems::helpers::get_component_mut::<Spellbook>(world, entity)
                .attack_roll_template_mut(),
        },
    }
}

/// The passive score of a skill (SRD 5.2.1, Rules Glossary > Passive
/// Perception): 10 plus every flat modifier the skill's check gets, plus 5 with
/// Advantage or minus 5 with Disadvantage. The formula is the SRD's for
/// Perception and works the same for any skill. A passive score rolls nothing,
/// so dice modifiers (such as Guidance) do not count.
pub fn passive_score(engine_state: &EngineState, entity: Entity, skill: Skill) -> i32 {
    let mut check = (*get(&engine_state.world, entity, D20CheckKind::Skill(skill))).clone();
    check.apply_pre_roll_hooks(engine_state, entity);
    let modifiers: i32 = check.modifiers().values().map(flat_part).sum();
    let advantage = match check.advantage_tracker().roll_mode() {
        RollMode::Normal => 0,
        RollMode::Advantage => 5,
        RollMode::Disadvantage => -5,
    };
    10 + modifiers + advantage
}

fn flat_part(kind: &ModifierKind) -> i32 {
    match kind {
        ModifierKind::Flat(value) => *value,
        ModifierKind::Dice(_) => 0,
        ModifierKind::Composite(kinds) => kinds.iter().map(flat_part).sum(),
    }
}

pub fn check_no_event(
    engine_state: &EngineState,
    entity: Entity,
    dc: &D20CheckDC,
    action: Option<&ActionId>,
) -> D20CheckResult {
    // TODO: Figure out a way around this
    if let D20CheckDC::AttackRoll { .. } = dc {
        panic!("check_no_event cannot be used for attack rolls; use check_attack instead");
    }

    let mut check = (*get(&engine_state.world, entity, dc.kind())).clone();
    if let Some(action) = action {
        check.set_action(action.clone());
    }

    check.roll_dc(engine_state, entity, dc).unwrap()
}

#[must_use]
pub fn check(engine_state: &mut EngineState, entity: Entity, dc: &D20CheckDC) -> Event {
    check_with_action(engine_state, entity, dc, None)
}

#[must_use]
pub fn check_with_action(
    engine_state: &mut EngineState,
    entity: Entity,
    dc: &D20CheckDC,
    action: Option<&ActionId>,
) -> Event {
    Event::new(EventKind::D20CheckPerformed {
        actor: EntityIdentifier::from_world(&engine_state.world, entity),
        result: check_no_event(engine_state, entity, dc, action),
        dc: dc.clone(),
    })
}

pub fn preview_attack_roll(
    engine_state: &EngineState,
    attacker: Entity,
    target: Entity,
    check: &mut D20Check,
) {
    check.set_target(target);
    systems::effects::effects(&engine_state.world, target)
        .on_attacked(engine_state, target, attacker, check);

    check.apply_pre_roll_hooks(engine_state, attacker);
}

#[must_use]
pub fn check_attack(
    engine_state: &mut EngineState,
    attacker: Entity,
    target: Entity,
    source: AttackSource,
    mut check: D20Check,
) -> Event {
    preview_attack_roll(engine_state, attacker, target, &mut check);

    let mut result = check.roll();
    systems::effects::effects(&engine_state.world, attacker).post_d20_check(
        engine_state,
        attacker,
        &mut result,
    );

    let armor_class = systems::loadout::armor_class(engine_state, target);

    Event::new(EventKind::D20CheckPerformed {
        actor: EntityIdentifier::from_world(&engine_state.world, attacker),
        result,
        dc: D20CheckDC::AttackRoll {
            target: EntityIdentifier::from_world(&engine_state.world, target),
            source,
            armor_class,
        },
    })
}
