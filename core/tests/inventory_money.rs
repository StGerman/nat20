extern crate nat20_core;

use std::str::FromStr;

use nat20_core::{
    components::{
        id::ItemId,
        items::{inventory::Inventory, money::MonetaryValue},
    },
    systems,
    test_utils::scenario::Scenario,
};

const POTION: &str = "item.potion_of_healing";

fn party() -> Scenario {
    let mut scenario = Scenario::new();
    scenario.spawn("bren", "hero.fighter").level(2).spawn();
    scenario
        .spawn("maren", "hero.fighter")
        .level(2)
        .position([1.0, 0.0, 0.0], false)
        .spawn();
    scenario
}

fn coins(s: &str) -> MonetaryValue {
    MonetaryValue::from_str(s).unwrap()
}

fn purse(scenario: &Scenario, handle: &str) -> u64 {
    scenario
        .engine_state
        .world
        .get::<&Inventory>(scenario.entity(handle))
        .unwrap()
        .money()
        .total_in_copper()
}

fn fill(scenario: &mut Scenario, handle: &str, amount: &str) {
    let entity = scenario.entity(handle);
    let world = &mut scenario.engine_state.world;
    let held = world.get::<&Inventory>(entity).unwrap().money().clone();
    systems::inventory::remove_money(world, entity, held).unwrap();
    systems::inventory::add_money(world, entity, coins(amount));
}

#[test]
fn paying_makes_change_from_a_gold_piece() {
    let mut scenario = party();
    fill(&mut scenario, "bren", "1 GP");
    let bren = scenario.entity("bren");
    systems::inventory::remove_money(&mut scenario.engine_state.world, bren, coins("5 SP"))
        .unwrap();
    assert_eq!(purse(&scenario, "bren"), 50);
}

#[test]
fn a_short_purse_pays_nothing() {
    let mut scenario = party();
    fill(&mut scenario, "bren", "1 GP, 5 SP");
    let bren = scenario.entity("bren");
    assert!(
        systems::inventory::remove_money(&mut scenario.engine_state.world, bren, coins("2 GP"))
            .is_err()
    );
    assert_eq!(purse(&scenario, "bren"), 150);
}

#[test]
fn coin_moves_between_creatures_and_the_total_is_kept() {
    let mut scenario = party();
    fill(&mut scenario, "bren", "3 GP");
    fill(&mut scenario, "maren", "0 GP");
    let (bren, maren) = (scenario.entity("bren"), scenario.entity("maren"));
    let world = &mut scenario.engine_state.world;
    systems::inventory::give_money(world, bren, maren, coins("2 GP, 5 SP")).unwrap();
    assert_eq!(purse(&scenario, "bren"), 50);
    assert_eq!(purse(&scenario, "maren"), 250);

    let world = &mut scenario.engine_state.world;
    assert!(systems::inventory::give_money(world, bren, maren, coins("1 GP")).is_err());
    assert_eq!(purse(&scenario, "bren") + purse(&scenario, "maren"), 300);
}

#[test]
fn an_item_moves_between_creatures_only_if_carried() {
    let mut scenario = party();
    let (bren, maren) = (scenario.entity("bren"), scenario.entity("maren"));
    let potion = ItemId::from(POTION);
    let world = &mut scenario.engine_state.world;
    let before = systems::inventory::item_count(world, maren, &potion);
    systems::inventory::add_item(world, bren, potion.clone());
    let carried = systems::inventory::item_count(world, bren, &potion);

    assert!(systems::inventory::give_item(world, bren, maren, &potion));
    assert_eq!(
        systems::inventory::item_count(world, bren, &potion),
        carried - 1
    );
    assert_eq!(
        systems::inventory::item_count(world, maren, &potion),
        before + 1
    );

    for _ in 1..carried {
        assert!(systems::inventory::give_item(world, bren, maren, &potion));
    }
    assert!(!systems::inventory::give_item(world, bren, maren, &potion));
    assert_eq!(
        systems::inventory::item_count(world, maren, &potion),
        before + carried
    );
}
