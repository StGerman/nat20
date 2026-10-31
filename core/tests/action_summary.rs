//! Short summaries on actions and spells: optional, carried from the registry JSON onto the
//! loaded rule, and kept to plain words a player reads while choosing (no DCs, no odds).

extern crate nat20_core;

use nat20_core::{
    components::id::{ActionId, SpellId},
    registry::registry::{ActionsRegistry, SpellsRegistry},
};

#[test]
fn an_action_carries_its_summary() {
    let dodge = ActionsRegistry::get(&ActionId::new("nat20_core", "action.dodge")).unwrap();
    let summary = dodge.summary.as_deref().unwrap();
    assert!(
        summary.starts_with("Until the start of your next turn"),
        "{summary}"
    );
    assert_ne!(Some(dodge.description.as_str()), dodge.summary.as_deref());
}

#[test]
fn a_spell_carries_its_summary_on_its_action() {
    let fireball = SpellsRegistry::get(&SpellId::new("nat20_core", "spell.fireball")).unwrap();
    let summary = fireball.action().summary.as_deref().unwrap();
    assert!(summary.contains("20-foot radius"), "{summary}");
}

#[test]
fn the_summary_is_optional() {
    let escape =
        ActionsRegistry::get(&ActionId::new("nat20_core", "action.monster.nimble_escape")).unwrap();
    assert_eq!(escape.summary, None);
}

/// Every action and spell the four Silent Mill pregens can take has one.
const PREGEN_ACTIONS: &[&str] = &[
    "action.dash",
    "action.disengage",
    "action.dodge",
    "action.help.assist_attack_roll",
    "action.help.assist_ability_check",
    "action.help.stabilize",
    "action.unarmed_attack",
    "action.melee_attack",
    "action.ranged_attack",
    "action.opportunity_attack",
    "action.potion_of_healing",
    "action.fighter.second_wind",
    "action.fighter.action_surge",
    "action.fighter.tactical_mind",
    "action.barbarian.rage",
    "action.barbarian.extend_rage",
    "action.barbarian.reckless_attack",
    "action.barbarian.reckless_attack_reaction",
    "action.cleric.divine_spark",
    "action.dragonborn.breath_weapon.red.cone",
    "action.dragonborn.breath_weapon.red.line",
    "action.dragonborn.breath_weapon.black.cone",
    "action.dragonborn.breath_weapon.black.line",
    "action.dragonborn.breath_weapon.white.cone",
    "action.dragonborn.breath_weapon.white.line",
];
const PREGEN_SPELLS: &[&str] = &[
    "spell.sacred_flame",
    "spell.guidance",
    "spell.spare_the_dying",
    "spell.bless",
    "spell.cure_wounds",
    "spell.healing_word",
    "spell.guiding_bolt",
    "spell.shield_of_faith",
    "spell.fire_bolt",
    "spell.ray_of_frost",
    "spell.acid_splash",
    "spell.magic_missile",
    "spell.burning_hands",
    "spell.thunderwave",
    "spell.ice_knife",
    "spell.shield",
    "spell.fireball",
];

#[test]
fn every_pregen_action_and_spell_has_a_summary() {
    for id in PREGEN_ACTIONS {
        let action = ActionsRegistry::get(&ActionId::new("nat20_core", *id))
            .unwrap_or_else(|| panic!("{id}"));
        assert!(action.summary.is_some(), "{id} has no summary");
    }
    for id in PREGEN_SPELLS {
        let spell =
            SpellsRegistry::get(&SpellId::new("nat20_core", *id)).unwrap_or_else(|| panic!("{id}"));
        assert!(spell.action().summary.is_some(), "{id} has no summary");
    }
}

#[test]
fn summaries_stay_short_and_carry_no_dc_or_odds() {
    let actions =
        ActionsRegistry::keys().filter_map(|id| ActionsRegistry::get(id)?.summary.clone());
    let spells =
        SpellsRegistry::keys().filter_map(|id| SpellsRegistry::get(id)?.action().summary.clone());
    for summary in actions.chain(spells) {
        assert!(summary.len() <= 240, "{summary}");
        for banned in ["DC", "%", "average", "chance"] {
            assert!(!summary.contains(banned), "{banned} in {summary}");
        }
    }
}
