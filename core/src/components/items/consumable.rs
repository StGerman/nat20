use schemars::JsonSchema;
use serde::Deserialize;

use crate::components::{id::ActionId, items::item::Item};

/// An item used up when its action is performed, e.g. a Potion of Healing.
/// Carrying one grants its action, with the item in the action's context.
#[derive(Debug, Clone, PartialEq, Deserialize, JsonSchema)]
pub struct ConsumableItem {
    pub item: Item,
    pub action: ActionId,
}
