use hecs::{Entity, World};

use crate::{
    components::{
        id::ItemId,
        items::{
            equipment::{loadout::TryEquipError, slots::EquipmentSlot},
            inventory::Inventory,
            money::{MonetaryValue, MonetaryValueError},
        },
    },
    engine::engine_state::EngineState,
    systems,
};

pub fn equip(
    engine_state: &mut EngineState,
    entity: Entity,
    item_id: &ItemId,
) -> Result<Vec<ItemId>, TryEquipError> {
    systems::loadout::equip(engine_state, entity, item_id)
}

pub fn unequip(
    engine_state: &mut EngineState,
    entity: Entity,
    slot: &EquipmentSlot,
) -> Option<ItemId> {
    systems::loadout::unequip(engine_state, entity, slot)
}

pub fn add_item(world: &mut World, entity: Entity, item: ItemId) {
    systems::helpers::get_component_mut::<Inventory>(world, entity).add_item(item);
}

pub fn remove_item(world: &mut World, entity: Entity, index: usize) -> Option<ItemId> {
    systems::helpers::get_component_mut::<Inventory>(world, entity).remove_item(index)
}

/// How many of this item the entity carries; none without an inventory
pub fn item_count(world: &World, entity: Entity, item: &ItemId) -> usize {
    world
        .get::<&Inventory>(entity)
        .map_or(0, |inventory| inventory.count(item))
}

pub fn use_up_item(world: &mut World, entity: Entity, item: &ItemId) -> bool {
    systems::helpers::get_component_mut::<Inventory>(world, entity).use_up(item)
}

pub fn add_money(world: &mut World, entity: Entity, amount: MonetaryValue) {
    systems::helpers::get_component_mut::<Inventory>(world, entity).add_money(amount);
}

pub fn remove_money(
    world: &mut World,
    entity: Entity,
    amount: MonetaryValue,
) -> Result<(), MonetaryValueError> {
    systems::helpers::get_component_mut::<Inventory>(world, entity).remove_money(amount)
}

/// Moves one of this item from one creature's inventory to another's. Returns
/// false, and moves nothing, if `from` carries none. Items worn or wielded in
/// the loadout are not in the inventory and cannot be given this way.
pub fn give_item(world: &mut World, from: Entity, to: Entity, item: &ItemId) -> bool {
    systems::helpers::get_component::<Inventory>(world, to);
    if !use_up_item(world, from, item) {
        return false;
    }
    add_item(world, to, item.clone());
    true
}

/// Moves coin from one creature to another. `from` pays with change (see
/// [`MonetaryValue::spend`]) and `to` receives the coins named in `amount`.
/// All or nothing: if `from` is short, or `to`'s purse could not hold the
/// coins ([`MonetaryValueError::Overflow`]), nothing moves.
pub fn give_money(
    world: &mut World,
    from: Entity,
    to: Entity,
    amount: MonetaryValue,
) -> Result<(), MonetaryValueError> {
    let mut received = systems::helpers::get_component::<Inventory>(world, to)
        .money()
        .clone();
    received.checked_add_value(&amount)?;
    remove_money(world, from, amount.clone())?;
    systems::helpers::get_component_mut::<Inventory>(world, to).checked_add_money(&amount)
}
