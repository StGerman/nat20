extern crate nat20_core;

use std::str::FromStr;

use nat20_core::{
    components::{
        ability::{Ability, AbilityScore, AbilityScoreMap},
        d20::{AdvantageAware, AdvantageType, D20CheckKind},
        dice::DiceSet,
        modifier::{Modifiable, ModifierSource},
        skill::Skill,
    },
    systems,
    test_utils::fixtures,
};

#[test]
fn passive_perception_is_ten_plus_the_check_bonus() {
    let mut engine_state = fixtures::engine::engine_state();
    let entity = fixtures::creatures::heroes::fighter(&mut engine_state, 2, None).id();
    systems::helpers::get_component_mut::<AbilityScoreMap>(&mut engine_state.world, entity)
        .set(Ability::Wisdom, AbilityScore::new(Ability::Wisdom, 14));

    let bonus = systems::d20::get(
        &engine_state.world,
        entity,
        D20CheckKind::Skill(Skill::Perception),
    )
    .clone()
    .roll_hooks(&engine_state, entity)
    .total_modifier();
    assert_eq!(
        systems::d20::passive_score(&engine_state, entity, Skill::Perception),
        10 + bonus
    );
}

#[test]
fn advantage_adds_five_and_disadvantage_takes_five() {
    let mut engine_state = fixtures::engine::engine_state();
    let entity = fixtures::creatures::heroes::fighter(&mut engine_state, 2, None).id();
    let plain = systems::d20::passive_score(&engine_state, entity, Skill::Insight);

    let source = ModifierSource::Custom("test".to_string());
    systems::d20::get_mut(
        &mut engine_state.world,
        entity,
        D20CheckKind::Skill(Skill::Insight),
    )
    .add_advantage(AdvantageType::Advantage, source.clone());
    assert_eq!(
        systems::d20::passive_score(&engine_state, entity, Skill::Insight),
        plain + 5
    );

    let check = systems::d20::get_mut(
        &mut engine_state.world,
        entity,
        D20CheckKind::Skill(Skill::Insight),
    );
    check.remove_advantage(&source);
    check.add_advantage(AdvantageType::Disadvantage, source);
    assert_eq!(
        systems::d20::passive_score(&engine_state, entity, Skill::Insight),
        plain - 5
    );
}

#[test]
fn dice_bonuses_do_not_count_toward_a_passive_score() {
    let mut engine_state = fixtures::engine::engine_state();
    let entity = fixtures::creatures::heroes::fighter(&mut engine_state, 2, None).id();
    let plain = systems::d20::passive_score(&engine_state, entity, Skill::Perception);

    systems::d20::get_mut(
        &mut engine_state.world,
        entity,
        D20CheckKind::Skill(Skill::Perception),
    )
    .add_modifier(
        ModifierSource::Custom("guidance".to_string()),
        DiceSet::from_str("1d4").unwrap(),
    );
    assert_eq!(
        systems::d20::passive_score(&engine_state, entity, Skill::Perception),
        plain
    );
}
