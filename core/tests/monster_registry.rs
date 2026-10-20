extern crate nat20_core;

mod tests {
    use std::{fs, path::Path, str::FromStr};

    use nat20_core::{
        components::{
            health::hit_points::HitPoints,
            id::{ItemId, MonsterId},
            items::equipment::slots::EquipmentSlot,
            level::{ChallengeRating, Level},
            modifier::FlatModifiable,
            proficiency::ProficiencyLevel,
            skill::{Skill, SkillSet},
        },
        registry::registry::{MonstersRegistry, RegistryError, RegistrySet, registry_root},
        systems::{self, monsters::SpawnMonsterError},
        test_utils::fixtures,
    };

    fn copy_dir(from: &Path, to: &Path) {
        fs::create_dir_all(to).unwrap();
        for entry in fs::read_dir(from).unwrap() {
            let entry = entry.unwrap();
            let target = to.join(entry.file_name());
            if entry.file_type().unwrap().is_dir() {
                copy_dir(&entry.path(), &target);
            } else {
                fs::copy(entry.path(), &target).unwrap();
            }
        }
    }

    fn monster(id: &str) -> MonsterId {
        MonsterId::new("nat20_core", id)
    }

    #[test]
    fn every_stat_block_spawns() {
        let mut engine_state = fixtures::engine::engine_state();
        let ids: Vec<MonsterId> = MonstersRegistry::keys().cloned().collect();
        assert!(!ids.is_empty());
        for id in ids {
            systems::monsters::spawn(&mut engine_state, &id)
                .unwrap_or_else(|error| panic!("{id}: {error}"));
        }
    }

    #[test]
    fn goblin_warrior_matches_its_stat_block() {
        let mut engine_state = fixtures::engine::engine_state();
        let goblin =
            systems::monsters::spawn(&mut engine_state, &monster("monster.goblin_warrior"))
                .unwrap();
        let world = &engine_state.world;

        // Leather Armor 11 + Dex 2 + Shield 2
        assert_eq!(
            systems::loadout::armor_class(&engine_state, goblin).total(),
            15
        );
        let hit_points = systems::helpers::get_component::<HitPoints>(world, goblin);
        assert_eq!((hit_points.current(), hit_points.max()), (10, 10));

        let challenge_rating = systems::helpers::get_component::<ChallengeRating>(world, goblin);
        assert_eq!(*challenge_rating, ChallengeRating::QUARTER);
        assert_eq!(challenge_rating.experience(), 50);
        assert_eq!(challenge_rating.proficiency_bonus(), 2);

        let skills = systems::helpers::get_component::<SkillSet>(world, goblin);
        assert_eq!(
            skills.get(&Skill::Stealth).proficiency().level(),
            &ProficiencyLevel::Expertise
        );

        let loadout = systems::loadout::loadout(world, goblin);
        assert_eq!(
            loadout.item_in_slot(&EquipmentSlot::MeleeOffHand),
            Some(&ItemId::new("nat20_core", "item.shield"))
        );
        assert_eq!(
            loadout.item_in_slot(&EquipmentSlot::MeleeMainHand),
            Some(&ItemId::new("nat20_core", "item.scimitar"))
        );
        assert_eq!(
            loadout.item_in_slot(&EquipmentSlot::RangedMainHand),
            Some(&ItemId::new("nat20_core", "item.shortbow"))
        );
    }

    #[test]
    fn unknown_monster_is_an_error() {
        let mut engine_state = fixtures::engine::engine_state();
        let id = monster("monster.does_not_exist");
        assert_eq!(
            systems::monsters::spawn(&mut engine_state, &id),
            Err(SpawnMonsterError::UnknownMonster(id))
        );
    }

    #[test]
    fn challenge_rating_must_parse() {
        assert!(ChallengeRating::from_str("1/4").is_ok());
        let root = std::env::temp_dir()
            .join(format!("nat20_monster_cr_{}", std::process::id()))
            .join("registries");
        let _ = fs::remove_dir_all(&root);
        copy_dir(&registry_root(None), &root);

        let goblin = fs::read_to_string(root.join("monsters/goblin_warrior.json")).unwrap();
        fs::write(
            root.join("monsters/goblin_warrior.json"),
            goblin.replace("\"1/4\"", "\"1/3\""),
        )
        .unwrap();

        let result = RegistrySet::load_from_root_directory(&root);
        fs::remove_dir_all(root.parent().unwrap()).unwrap();
        let Err(RegistryError::Many(errors)) = result else {
            panic!("a CR of 1/3 should not load");
        };
        assert!(
            errors
                .iter()
                .any(|error| matches!(error, RegistryError::DeserializeJson { .. })),
            "{errors:?}"
        );
    }

    #[test]
    fn unknown_effect_in_a_stat_block_fails_validation() {
        let root = std::env::temp_dir()
            .join(format!("nat20_monster_refs_{}", std::process::id()))
            .join("registries");
        let _ = fs::remove_dir_all(&root);
        copy_dir(&registry_root(None), &root);

        let path = root.join("monsters/goblin_warrior.json");
        let mut goblin: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        goblin["effects"] = serde_json::json!(["nat20_core::effect.monster.does_not_exist"]);
        fs::write(&path, serde_json::to_string_pretty(&goblin).unwrap()).unwrap();

        let result = RegistrySet::load_from_root_directory(&root);
        fs::remove_dir_all(root.parent().unwrap()).unwrap();
        let Err(RegistryError::Many(errors)) = result else {
            panic!("an unknown effect id should fail validation");
        };
        assert!(
            errors.iter().any(|error| matches!(
                error,
                RegistryError::MissingRegistryEntry { reference, .. }
                    if reference.to_string().contains("effect.monster.does_not_exist")
            )),
            "{errors:?}"
        );
    }
}
