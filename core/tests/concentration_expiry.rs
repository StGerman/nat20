extern crate nat20_core;

use nat20_core::test_utils::scenario::Scenario;

const BLESS: &str = "effect.spell.bless";

/// SRD 5.2.1 Bless: Concentration, up to 1 minute. When the spell's effects
/// run out, the caster is no longer concentrating on it.
#[test]
fn concentration_ends_when_the_spells_effects_expire() {
    let mut scenario = Scenario::new();
    scenario.spawn("cleric", "hero.cleric").level(2).spawn();
    scenario
        .spawn("fighter", "hero.fighter")
        .level(2)
        .position([1.0, 0.0, 0.0], false)
        .spawn();
    scenario
        .encounter()
        .initiative_order(vec!["cleric", "fighter"])
        .build();

    scenario
        .act("cleric", "action.bless")
        .context_spell_level(1)
        .target_entities(vec!["cleric", "fighter"])
        .perform();
    scenario.probe("cleric").assert_concentration(BLESS);

    // One minute is ten rounds; the effects end at the start of the
    // cleric's eleventh turn
    for _ in 0..9 {
        scenario.probe("cleric").end_turn();
        scenario.probe("fighter").end_turn();
    }
    scenario
        .probe("cleric")
        .assert_effect(BLESS)
        .assert_concentration(BLESS);
    scenario.probe("fighter").assert_effect(BLESS);

    scenario.probe("cleric").end_turn();
    scenario.probe("fighter").end_turn();
    scenario
        .probe("cleric")
        .assert_no_effect(BLESS)
        .assert_no_concentration();
    scenario.probe("fighter").assert_no_effect(BLESS);
}
