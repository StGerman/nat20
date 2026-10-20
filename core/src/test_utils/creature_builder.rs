use std::{collections::BTreeMap, sync::LazyLock};

use hecs::Entity;
use parry3d::na::Point3;

use crate::{
    components::{
        id::{EntityIdentifier, MonsterId},
        resource::RechargeRule,
        time::{EntityClock, TimeMode},
    },
    engine::engine_state::EngineState,
    systems,
    test_utils::fixtures::creatures::{heroes, monsters},
};

/// `(engine_state, level/challenge_rating, optional_entity_to_spawn_at) -> spawned`
pub type CreatureTemplateFunction = fn(&mut EngineState, u8, Option<Entity>) -> EntityIdentifier;

pub static CREATURE_TEMPLATES: LazyLock<BTreeMap<&'static str, CreatureTemplateFunction>> =
    LazyLock::new(|| {
        BTreeMap::from([
            (
                "hero.barbarian",
                heroes::barbarian as CreatureTemplateFunction,
            ),
            ("hero.cleric", heroes::cleric as CreatureTemplateFunction),
            ("hero.fighter", heroes::fighter as CreatureTemplateFunction),
            ("hero.wizard", heroes::wizard as CreatureTemplateFunction),
            ("hero.warlock", heroes::warlock as CreatureTemplateFunction),
            (
                "monster.goblin_warrior",
                monsters::goblin_warrior as CreatureTemplateFunction,
            ),
            (
                "stat_block.goblin_warrior",
                stat_block_goblin_warrior as CreatureTemplateFunction,
            ),
            (
                "stat_block.goblin_boss",
                stat_block_goblin_boss as CreatureTemplateFunction,
            ),
            (
                "stat_block.wolf",
                stat_block_wolf as CreatureTemplateFunction,
            ),
            (
                "stat_block.giant_rat",
                stat_block_giant_rat as CreatureTemplateFunction,
            ),
        ])
    });

/// Spawns a monster from its registry stat block, so the level is ignored
fn stat_block(
    engine_state: &mut EngineState,
    id: &str,
    entity: Option<Entity>,
) -> EntityIdentifier {
    assert!(
        entity.is_none(),
        "A stat block monster can't be spawned at a given entity"
    );
    let monster_id = MonsterId::new("nat20_core", id);
    let entity = systems::monsters::spawn(engine_state, &monster_id)
        .unwrap_or_else(|error| panic!("Failed to spawn {monster_id}: {error}"));
    EntityIdentifier::from_world(&engine_state.world, entity)
}

fn stat_block_goblin_warrior(
    engine_state: &mut EngineState,
    _: u8,
    entity: Option<Entity>,
) -> EntityIdentifier {
    stat_block(engine_state, "monster.goblin_warrior", entity)
}

fn stat_block_goblin_boss(
    engine_state: &mut EngineState,
    _: u8,
    entity: Option<Entity>,
) -> EntityIdentifier {
    stat_block(engine_state, "monster.goblin_boss", entity)
}

fn stat_block_wolf(
    engine_state: &mut EngineState,
    _: u8,
    entity: Option<Entity>,
) -> EntityIdentifier {
    stat_block(engine_state, "monster.wolf", entity)
}

fn stat_block_giant_rat(
    engine_state: &mut EngineState,
    _: u8,
    entity: Option<Entity>,
) -> EntityIdentifier {
    stat_block(engine_state, "monster.giant_rat", entity)
}

// TODO: Not sure where this should live
pub struct CreatureBuilder {
    template: CreatureTemplateFunction,
    level: u8,
    position: Option<(Point3<f32>, bool)>,
    time_mode: TimeMode,
    entity_id: Option<Entity>,
}

impl CreatureBuilder {
    pub fn new(template: &str) -> Self {
        Self {
            template: *CREATURE_TEMPLATES
                .get(template)
                .unwrap_or_else(|| panic!("No creature template with name {template}")),
            level: 1,
            time_mode: TimeMode::RealTime,
            position: None,
            entity_id: None,
        }
    }

    pub fn level(&mut self, level: u8) -> &mut Self {
        self.level = level;
        self
    }

    pub fn position(&mut self, position: impl Into<Point3<f32>>, on_ground: bool) -> &mut Self {
        self.position = Some((position.into(), on_ground));
        self
    }

    pub fn time_mode(&mut self, time_mode: TimeMode) -> &mut Self {
        self.time_mode = time_mode;
        self
    }

    pub fn with_id(&mut self, entity: Entity) -> &mut Self {
        self.entity_id = Some(entity);
        self
    }

    pub fn spawn(&mut self, engine_state: &mut EngineState) -> EntityIdentifier {
        let entity_id = (self.template)(engine_state, self.level, self.entity_id);
        let entity = entity_id.id();

        if let Some((position, on_ground)) = self.position {
            if on_ground {
                systems::geometry::teleport_to_ground(
                    &mut engine_state.world,
                    &engine_state.geometry,
                    entity,
                    &position,
                );
            } else {
                systems::geometry::teleport_to(&mut engine_state.world, entity, &position);
            }
        }

        systems::helpers::get_component_mut::<EntityClock>(&mut engine_state.world, entity)
            .set_mode(self.time_mode);

        let _ = systems::health::heal_full(&mut engine_state.world, entity);

        systems::resources::recharge(&mut engine_state.world, entity, &RechargeRule::Daily);

        entity_id
    }
}
