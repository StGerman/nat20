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

#[test]
fn summaries_stay_short_and_carry_no_dc_or_odds() {
    let actions =
        ActionsRegistry::keys().filter_map(|id| ActionsRegistry::get(id)?.summary.clone());
    let spells =
        SpellsRegistry::keys().filter_map(|id| SpellsRegistry::get(id)?.action().summary.clone());
    let all: Vec<String> = actions.chain(spells).collect();
    assert!(all.len() >= 40, "summaries found: {}", all.len());
    for summary in all {
        assert!(summary.len() <= 240, "{summary}");
        for banned in ["DC", "%", "average", "chance"] {
            assert!(!summary.contains(banned), "{banned} in {summary}");
        }
    }
}
