//! The SRD 5.2.1 numbers of the monster stat blocks, as the engine derives
//! them from each block's ability scores, gear and proficiencies.

extern crate nat20_core;

mod tests {
    use nat20_core::{
        components::{
            ability::Ability,
            health::hit_points::HitPoints,
            id::MonsterId,
            level::{ChallengeRating, Level},
            modifier::FlatModifiable,
            proficiency::ProficiencyLevel,
            saving_throw::{SavingThrowKind, SavingThrowSet},
            skill::{Skill, SkillSet},
        },
        systems,
        test_utils::fixtures,
    };
    use uom::si::length::foot;

    struct Expected {
        id: &'static str,
        armor_class: i32,
        hit_points: u32,
        speed_ft: f32,
        challenge_rating: &'static str,
        experience: u32,
        skills: &'static [(Skill, ProficiencyLevel)],
        saving_throws: &'static [(Ability, ProficiencyLevel)],
    }

    const STAT_BLOCKS: &[Expected] = &[
        Expected {
            id: "monster.goblin_warrior",
            armor_class: 15,
            hit_points: 10,
            speed_ft: 30.0,
            challenge_rating: "1/4",
            experience: 50,
            skills: &[(Skill::Stealth, ProficiencyLevel::Expertise)],
            saving_throws: &[],
        },
        Expected {
            id: "monster.goblin_boss",
            armor_class: 17,
            hit_points: 21,
            speed_ft: 30.0,
            challenge_rating: "1",
            experience: 200,
            skills: &[(Skill::Stealth, ProficiencyLevel::Expertise)],
            saving_throws: &[],
        },
        Expected {
            id: "monster.wolf",
            armor_class: 12,
            hit_points: 11,
            speed_ft: 40.0,
            challenge_rating: "1/4",
            experience: 50,
            skills: &[
                (Skill::Perception, ProficiencyLevel::Expertise),
                (Skill::Stealth, ProficiencyLevel::Proficient),
            ],
            saving_throws: &[],
        },
        Expected {
            id: "monster.giant_rat",
            armor_class: 13,
            hit_points: 7,
            speed_ft: 30.0,
            challenge_rating: "1/8",
            experience: 25,
            skills: &[(Skill::Perception, ProficiencyLevel::Proficient)],
            saving_throws: &[(Ability::Dexterity, ProficiencyLevel::Proficient)],
        },
    ];

    #[test]
    fn stat_blocks_match_the_srd() {
        let mut engine_state = fixtures::engine::engine_state();
        for expected in STAT_BLOCKS {
            let id = expected.id;
            let monster =
                systems::monsters::spawn(&mut engine_state, &MonsterId::new("nat20_core", id))
                    .unwrap_or_else(|error| panic!("{id}: {error}"));

            assert_eq!(
                systems::loadout::armor_class(&engine_state, monster).total(),
                expected.armor_class,
                "{id} AC"
            );
            assert_eq!(
                systems::movement::speed(&engine_state, monster)
                    .total_speed()
                    .get::<foot>()
                    .round(),
                expected.speed_ft,
                "{id} speed"
            );

            let world = &engine_state.world;
            let hit_points = systems::helpers::get_component::<HitPoints>(world, monster);
            assert_eq!(
                (hit_points.current(), hit_points.max()),
                (expected.hit_points, expected.hit_points),
                "{id} HP"
            );

            let challenge_rating =
                systems::helpers::get_component::<ChallengeRating>(world, monster);
            assert_eq!(
                challenge_rating.to_string(),
                expected.challenge_rating,
                "{id} CR"
            );
            assert_eq!(
                challenge_rating.experience(),
                expected.experience,
                "{id} XP"
            );
            assert_eq!(challenge_rating.proficiency_bonus(), 2, "{id} PB");

            let skills = systems::helpers::get_component::<SkillSet>(world, monster);
            for (skill, level) in expected.skills {
                assert_eq!(
                    skills.get(skill).proficiency().level(),
                    level,
                    "{id} {skill:?}"
                );
            }
            let saving_throws = systems::helpers::get_component::<SavingThrowSet>(world, monster);
            for (ability, level) in expected.saving_throws {
                assert_eq!(
                    saving_throws
                        .get(&SavingThrowKind::Ability(*ability))
                        .proficiency()
                        .level(),
                    level,
                    "{id} {ability:?} save"
                );
            }
        }
    }
}
