extern crate nat20_core;

use std::collections::HashSet;

use nat20_core::{
    components::{
        d20::{D20CheckKind, D20CheckOutcome},
        damage::{AttackSource, DamageComponentResult, DamageRollResult, DamageType},
        dice::{DiceSet, DiceSetResult, DieSize},
        id::{BackgroundId, ClassId, FeatId, SpeciesId},
        items::equipment::weapon::WeaponKind,
        level_up::{ChoiceItem, LevelUpPrompt},
        modifier::{ModifierKindResult, ModifierResult, ModifierSource},
        proficiency::ProficiencyLevel,
        skill::{Skill, SkillSet},
        species::CreatureSize,
    },
    entities::creature::Character,
    registry::registry::{BackgroundsRegistry, ClassesRegistry},
    systems::{self, level_up::LevelUpDecision},
    test_utils::{fixtures, scenario::Scenario},
};
use uom::si::{f32::Length, length::foot};

const SAVAGE_ATTACKER: &str = "nat20_core::feat.savage_attacker";
const SKILLED: &str = "nat20_core::feat.skilled";

fn id<T: std::str::FromStr>(id: &str) -> T
where
    T::Err: std::fmt::Debug,
{
    id.parse().unwrap()
}

/// A level 1 Human Fighter who takes Skilled as the Versatile origin feat.
fn human_fighter_decisions() -> Vec<LevelUpDecision> {
    vec![
        LevelUpDecision::single_choice(ChoiceItem::Species(id::<SpeciesId>(
            "nat20_core::species.human",
        ))),
        // Skillful
        LevelUpDecision::SkillProficiency(HashSet::from([Skill::Insight])),
        // Versatile
        LevelUpDecision::single_choice_with_id(
            "choice.human.versatile",
            ChoiceItem::Feat(id::<FeatId>(SKILLED)),
        ),
        // Skilled
        LevelUpDecision::SkillProficiency(HashSet::from([
            Skill::Medicine,
            Skill::Persuasion,
            Skill::Stealth,
        ])),
        LevelUpDecision::single_choice(ChoiceItem::Background(id::<BackgroundId>(
            "nat20_core::background.soldier",
        ))),
        LevelUpDecision::single_choice(ChoiceItem::Class(id::<ClassId>(
            "nat20_core::class.fighter",
        ))),
        LevelUpDecision::AbilityScores(
            ClassesRegistry::get(&id::<ClassId>("nat20_core::class.fighter"))
                .unwrap()
                .default_abilities
                .clone(),
        ),
        LevelUpDecision::single_choice_with_id(
            "choice.fighting_style",
            ChoiceItem::Feat(id::<FeatId>("nat20_core::feat.fighting_style.defense")),
        ),
        LevelUpDecision::SkillProficiency(HashSet::from([Skill::Athletics, Skill::Perception])),
        LevelUpDecision::single_choice_with_id(
            "choice.starting_equipment.fighter",
            ChoiceItem::Equipment {
                items: Vec::new(),
                money: "155 GP".to_string(),
            },
        ),
        LevelUpDecision::single_choice_with_id(
            "choice.starting_equipment.soldier",
            ChoiceItem::Equipment {
                items: Vec::new(),
                money: "50 GP".to_string(),
            },
        ),
    ]
}

#[test]
fn human_asks_for_a_skill_and_an_origin_feat() {
    let mut engine_state = fixtures::engine::engine_state();
    let character = engine_state.world.spawn(Character::default());

    let prompts = systems::species::set_species(
        &mut engine_state,
        character,
        &id("nat20_core::species.human"),
    );

    assert_eq!(prompts.len(), 2, "{prompts:#?}");
    let LevelUpPrompt::SkillProficiency {
        skills, choices, ..
    } = &prompts[0]
    else {
        panic!("Expected Skillful first: {prompts:#?}");
    };
    assert_eq!(*choices, 1);
    assert_eq!(skills.len(), 18);
    let LevelUpPrompt::Choice(versatile) = &prompts[1] else {
        panic!("Expected Versatile second: {prompts:#?}");
    };
    assert_eq!(versatile.id, "choice.human.versatile");
    assert_eq!(
        versatile.options,
        vec![
            ChoiceItem::Feat(id(SAVAGE_ATTACKER)),
            ChoiceItem::Feat(id(SKILLED)),
        ]
    );
}

#[test]
fn human_fighter_with_skilled() {
    let mut engine_state = fixtures::engine::engine_state();
    let character = engine_state.world.spawn(Character::default());

    systems::level_up::apply_level_up_decision(
        &mut engine_state,
        character,
        1,
        human_fighter_decisions(),
    );

    let world = &engine_state.world;
    assert_eq!(
        *systems::helpers::get_component::<CreatureSize>(world, character),
        CreatureSize::Medium
    );
    assert_eq!(
        systems::movement::speed(&engine_state, character).total_speed(),
        Length::new::<foot>(30.0)
    );

    let feats = systems::feats::feats(world, character).clone();
    assert!(feats.contains(&id(SKILLED)), "{feats:?}");
    // Soldier's origin feat
    assert!(feats.contains(&id(SAVAGE_ATTACKER)), "{feats:?}");

    let skills = systems::helpers::get_component::<SkillSet>(world, character);
    for skill in [
        Skill::Insight,
        Skill::Medicine,
        Skill::Persuasion,
        Skill::Stealth,
        Skill::Athletics,
        Skill::Intimidation,
        Skill::Perception,
    ] {
        assert_eq!(
            skills.get(&skill).proficiency().level(),
            &ProficiencyLevel::Proficient,
            "{skill:?}"
        );
    }
    assert_eq!(
        skills.get(&Skill::Arcana).proficiency().level(),
        &ProficiencyLevel::None
    );
}

#[test]
fn backgrounds_match_the_srd() {
    use nat20_core::components::ability::Ability::*;

    for (background, abilities, feat) in [
        (
            "nat20_core::background.soldier",
            [Strength, Dexterity, Constitution],
            Some(SAVAGE_ATTACKER),
        ),
        (
            "nat20_core::background.sage",
            [Constitution, Intelligence, Wisdom],
            None,
        ),
        (
            "nat20_core::background.criminal",
            [Dexterity, Constitution, Intelligence],
            None,
        ),
        (
            "nat20_core::background.acolyte",
            [Intelligence, Wisdom, Charisma],
            None,
        ),
    ] {
        let definition = BackgroundsRegistry::get(&id(background)).unwrap();
        let actual: HashSet<_> = definition.ability_scores.iter().collect();
        assert_eq!(actual, abilities.iter().collect(), "{background}");
        if let Some(feat) = feat {
            assert_eq!(definition.feat, id(feat), "{background}");
        }
    }
}

fn weapon_damage(rolls: Vec<u32>) -> DamageRollResult {
    let mut result = DamageRollResult {
        components: vec![DamageComponentResult {
            result: ModifierResult::from_iter([
                (
                    ModifierSource::Base,
                    ModifierKindResult::Dice(DiceSetResult::new(
                        DiceSet::new(rolls.len() as u32, DieSize::D8),
                        rolls,
                    )),
                ),
                (
                    ModifierSource::Ability(nat20_core::components::ability::Ability::Strength),
                    ModifierKindResult::Flat(3),
                ),
            ]),
            damage_type: DamageType::Slashing,
        }],
        total: 0,
        crit: false,
    };
    result.recalculate_total();
    result
}

#[test]
fn reroll_never_lowers_the_weapon_dice() {
    // A maximum roll can't be beaten, so it's always kept
    let mut max = weapon_damage(vec![8]);
    assert!(!max.reroll_base_dice_keep_higher());
    assert_eq!(max.total, 11);

    // From a minimum roll the total only ever goes up, and the flat modifier stays
    let mut kept = 0;
    for _ in 0..50 {
        let mut min = weapon_damage(vec![1, 1]);
        if min.reroll_base_dice_keep_higher() {
            kept += 1;
        }
        assert!(min.total >= 5, "{min:?}");
        assert_eq!(
            min.components[0]
                .result
                .clone()
                .iter_mut()
                .find(|(source, _)| matches!(source, ModifierSource::Ability(_)))
                .map(|(_, result)| result.total()),
            Some(3)
        );
    }
    assert!(kept > 0);
}

fn duel() -> Scenario {
    let mut scenario = Scenario::new();
    scenario
        .spawn("fighter", "hero.fighter")
        .level(5)
        .position([0.0, 0.0, 0.0], false)
        .spawn();
    scenario
        .spawn("goblin", "monster.goblin_warrior")
        .level(5)
        .position([1.0, 0.0, 0.0], false)
        .spawn();
    scenario
}

fn force_attack(scenario: &mut Scenario, outcome: D20CheckOutcome) {
    scenario.probe("fighter").d20_force_outcome(
        D20CheckKind::AttackRoll(AttackSource::Weapon(WeaponKind::Melee)),
        outcome,
    );
}

fn attack(scenario: &mut Scenario) {
    scenario
        .act("fighter", "action.melee_attack")
        .target_entity("goblin")
        .perform();
}

/// Whether Savage Attacker is still unused this turn. Asking marks it as used.
fn savage_attacker_unused(scenario: &Scenario) -> bool {
    scenario
        .engine_state
        .use_once_per_turn(scenario.entity("fighter"), SAVAGE_ATTACKER)
}

#[test]
fn a_miss_does_not_use_savage_attacker() {
    let mut scenario = duel();
    scenario
        .probe("fighter")
        .assert_effect("effect.feat.savage_attacker");
    scenario
        .encounter()
        .initiative_order(vec!["fighter", "goblin"])
        .build();

    // A miss doesn't use it
    force_attack(&mut scenario, D20CheckOutcome::CriticalFailure);
    attack(&mut scenario);
    assert!(savage_attacker_unused(&scenario));
}

#[test]
fn savage_attacker_once_per_turn() {
    let mut scenario = duel();
    scenario
        .encounter()
        .initiative_order(vec!["fighter", "goblin"])
        .build();

    force_attack(&mut scenario, D20CheckOutcome::Success);
    attack(&mut scenario);
    assert!(!savage_attacker_unused(&scenario));

    // A new turn (the goblin's) makes it available again, e.g. for an opportunity attack
    scenario.probe("fighter").end_turn();
    assert!(savage_attacker_unused(&scenario));
    assert!(!savage_attacker_unused(&scenario));
}

#[test]
fn once_per_turn_is_always_available_out_of_combat() {
    let scenario = duel();
    assert!(savage_attacker_unused(&scenario));
    assert!(savage_attacker_unused(&scenario));
}
