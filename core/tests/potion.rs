extern crate nat20_core;

use nat20_core::{
    components::{
        actions::{
            action::ActionContext,
            targeting::{TargetInstance, TargetingError},
        },
        damage::DamageType,
        health::life_state::LifeState,
        id::ItemId,
        resource::{ResourceAmount, ResourceAmountMap},
    },
    systems::{
        self,
        actions::{ActionUsabilityCheck, ActionUsabilityError},
    },
    test_utils::scenario::{Operator, Scenario},
};

const POTION: &str = "item.potion_of_healing";
const DRINK: &str = "action.potion_of_healing";

/// Two Fighters side by side and a third out of reach, each 19 HP short.
fn camp(potions: usize) -> Scenario {
    let mut scenario = Scenario::new();
    scenario.spawn("bren", "hero.fighter").level(2).spawn();
    scenario
        .spawn("ally", "hero.fighter")
        .level(2)
        .position([1.0, 0.0, 0.0], false)
        .spawn();
    scenario
        .spawn("far", "hero.fighter")
        .level(2)
        .position([4.0, 0.0, 0.0], false)
        .spawn();
    for handle in ["bren", "ally", "far"] {
        scenario.probe(handle).damage(19, DamageType::Slashing);
    }
    for _ in 0..potions {
        give_potion(&mut scenario, "bren");
    }
    scenario
}

fn give_potion(scenario: &mut Scenario, handle: &str) {
    let entity = scenario.entity(handle);
    systems::inventory::add_item(
        &mut scenario.engine_state.world,
        entity,
        ItemId::from(POTION),
    );
}

fn potions(scenario: &Scenario, handle: &str) -> usize {
    systems::inventory::item_count(
        &scenario.engine_state.world,
        scenario.entity(handle),
        &ItemId::from(POTION),
    )
}

#[test]
fn drinking_a_potion_heals_2d4_plus_2_as_a_bonus_action_and_uses_it_up() {
    let mut scenario = camp(1);
    let before = scenario.probe("bren").hp();
    scenario.act("bren", DRINK).target_entity("bren").perform();
    let healed = scenario.probe("bren").hp() - before;
    assert!((4..=10).contains(&healed), "healed {healed}");
    assert_eq!(potions(&scenario, "bren"), 0);
    scenario
        .probe("bren")
        .assert_resource("resource.bonus_action", Operator::Equal(0))
        .assert_resource("resource.action", Operator::Equal(1))
        .assert_no_action(DRINK);
}

#[test]
fn a_potion_can_be_given_to_a_creature_within_5_feet_and_revives_it() {
    let mut scenario = camp(1);
    let max_hp = scenario.probe("ally").max_hp();
    scenario.probe("ally").damage(max_hp, DamageType::Slashing);
    assert_ne!(life_state(&scenario, "ally"), LifeState::Normal);
    scenario.act("bren", DRINK).target_entity("ally").perform();
    assert!((4..=10).contains(&scenario.probe("ally").hp()));
    assert_eq!(life_state(&scenario, "ally"), LifeState::Normal);
    assert_eq!(potions(&scenario, "bren"), 0);
}

#[test]
fn a_potion_reaches_5_feet_and_only_while_one_is_carried() {
    let scenario = camp(1);
    let usable_on = |scenario: &Scenario, actor: &str, target: &str| {
        systems::actions::action_usable_on_targets(
            &scenario.engine_state,
            scenario.entity(actor),
            &DRINK.into(),
            None,
            &ActionContext::item(POTION.into()),
            &ResourceAmountMap::from_iter([(
                "resource.bonus_action".into(),
                ResourceAmount::Flat(1),
            )]),
            &[TargetInstance::entity(scenario.entity_identifier(target))],
            &[ActionUsabilityCheck::ActionKnown],
        )
    };
    assert_eq!(usable_on(&scenario, "bren", "ally"), Ok(()));
    assert!(
        matches!(
            usable_on(&scenario, "bren", "far"),
            Err(ActionUsabilityError::TargetingError(
                TargetingError::OutOfRange { .. }
            ))
        ),
        "{:?}",
        usable_on(&scenario, "bren", "far")
    );
    // Even when the known-action check is skipped, the item must be carried
    assert!(matches!(
        usable_on(&scenario, "ally", "bren"),
        Err(ActionUsabilityError::InvalidContext(_))
    ));
}

#[test]
fn several_potions_are_one_option_and_each_use_spends_one() {
    let mut scenario = camp(2);
    let all = systems::actions::all_actions(&scenario.engine_state, scenario.entity("bren"));
    assert_eq!(all.get(&DRINK.into()).map(Vec::len), Some(1), "{all:?}");
    scenario.act("bren", DRINK).target_entity("bren").perform();
    assert_eq!(potions(&scenario, "bren"), 1);
    scenario.probe("bren").assert_has_action(DRINK);
}

#[test]
fn a_creature_without_a_potion_has_no_potion_action() {
    let mut scenario = camp(0);
    scenario.probe("bren").assert_no_action(DRINK);
}

fn life_state(scenario: &Scenario, handle: &str) -> LifeState {
    *systems::helpers::get_component::<LifeState>(
        &scenario.engine_state.world,
        scenario.entity(handle),
    )
}
