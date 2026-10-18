extern crate nat20_core;

mod tests {
    use nat20_core::{
        components::{
            id::ItemId,
            items::equipment::{loadout::TryEquipError, slots::EquipmentSlot, weapon::WeaponKind},
            modifier::FlatModifiable,
        },
        engine::engine_state::EngineState,
        entities::creature::Character,
        systems,
        test_utils::fixtures,
    };

    fn item(id: &str) -> ItemId {
        ItemId::new("nat20_core", id)
    }

    fn equip(engine_state: &mut EngineState, entity: hecs::Entity, slot: EquipmentSlot, id: &str) {
        systems::loadout::equip_in_slot(engine_state, entity, &slot, &item(id))
            .unwrap_or_else(|e| panic!("{id} in {slot}: {e:?}"));
    }

    /// A level 1 Fighter: Chain Mail and a Shield, with Shield training from the class.
    fn fighter(engine_state: &mut EngineState) -> hecs::Entity {
        let fighter = fixtures::creatures::heroes::fighter(engine_state, 1, None).id();
        equip(
            engine_state,
            fighter,
            EquipmentSlot::Armor,
            "item.chainmail",
        );
        equip(
            engine_state,
            fighter,
            EquipmentSlot::MeleeMainHand,
            "item.longsword",
        );
        fighter
    }

    #[test]
    fn shield_adds_two_to_armor_class() {
        let mut engine_state = fixtures::engine::engine_state();
        let fighter = fighter(&mut engine_state);
        let before = systems::loadout::armor_class(&engine_state, fighter).total();

        equip(
            &mut engine_state,
            fighter,
            EquipmentSlot::MeleeOffHand,
            "item.shield",
        );
        assert_eq!(
            systems::loadout::armor_class(&engine_state, fighter).total(),
            before + 2
        );

        systems::loadout::unequip(&mut engine_state, fighter, &EquipmentSlot::MeleeOffHand);
        assert_eq!(
            systems::loadout::armor_class(&engine_state, fighter).total(),
            before
        );
    }

    #[test]
    fn shield_goes_in_the_off_hand_only() {
        let mut engine_state = fixtures::engine::engine_state();
        let character = engine_state.world.spawn(Character::default());
        for slot in [EquipmentSlot::Armor, EquipmentSlot::MeleeMainHand] {
            assert!(matches!(
                systems::loadout::equip_in_slot(
                    &mut engine_state,
                    character,
                    &slot,
                    &item("item.shield")
                ),
                Err(TryEquipError::InvalidSlot { .. })
            ));
        }
        // Equipped without a slot, it finds the off hand.
        systems::loadout::equip(&mut engine_state, character, &item("item.shield")).unwrap();
        assert_eq!(
            systems::loadout::loadout(&engine_state.world, character)
                .item_in_slot(&EquipmentSlot::MeleeOffHand),
            Some(&item("item.shield"))
        );
    }

    #[test]
    fn shield_without_training_adds_nothing() {
        let mut engine_state = fixtures::engine::engine_state();
        // No class, so no armour training at all.
        let character = engine_state.world.spawn(Character::default());
        equip(
            &mut engine_state,
            character,
            EquipmentSlot::MeleeOffHand,
            "item.shield",
        );
        assert_eq!(
            systems::loadout::armor_class(&engine_state, character).total(),
            10
        );
    }

    #[test]
    fn versatile_weapon_is_one_handed_beside_a_shield() {
        let mut engine_state = fixtures::engine::engine_state();
        let fighter = fighter(&mut engine_state);
        let two_handed = |engine_state: &EngineState| {
            systems::loadout::loadout(&engine_state.world, fighter)
                .is_wielding_weapon_with_both_hands(&WeaponKind::Melee)
        };
        assert!(
            two_handed(&engine_state),
            "a lone longsword is held in both hands"
        );
        equip(
            &mut engine_state,
            fighter,
            EquipmentSlot::MeleeOffHand,
            "item.shield",
        );
        assert!(!two_handed(&engine_state), "a shield takes the off hand");
    }

    #[test]
    fn mace_and_leather_armor() {
        let mut engine_state = fixtures::engine::engine_state();
        let character = engine_state.world.spawn(Character::default());
        equip(
            &mut engine_state,
            character,
            EquipmentSlot::Armor,
            "item.leather_armor",
        );
        // Leather Armor: 11 + Dexterity modifier (10 by default, so +0).
        assert_eq!(
            systems::loadout::armor_class(&engine_state, character).total(),
            11
        );
        equip(
            &mut engine_state,
            character,
            EquipmentSlot::MeleeMainHand,
            "item.mace",
        );
        let loadout = systems::loadout::loadout(&engine_state.world, character);
        let mace = loadout
            .weapon_in_hand(&EquipmentSlot::MeleeMainHand)
            .expect("the mace is a weapon");
        assert_eq!(mace.kind(), &WeaponKind::Melee);
        assert!(mace.properties().is_empty());
    }
}
