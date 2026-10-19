//! Dependents name core's types through the re-exported crates. These signatures only compile if
//! the re-exports are the same crates (and versions) core's public API is built from.

use nat20_core::{
    engine::geometry::WorldGeometry,
    hecs::{Entity, World},
    parry3d::na::Point3,
    rerecast::{Config, ConfigBuilder},
    systems::geometry::{distance_entity_point, get_foot_position},
    uom::si::{f32::Length, length::foot},
};

#[test]
fn reexported_crates_are_the_ones_core_uses() {
    let _: fn(&World, Entity) -> Option<Point3<f32>> = get_foot_position;
    let _: fn(&World, Entity, Point3<f32>) -> Option<Length> = distance_entity_point;
    let _: fn(&str, &Config) -> WorldGeometry =
        |path, config| WorldGeometry::from_obj_path(path, config);
    let _: Config = ConfigBuilder::default().build();

    let mut world = World::new();
    let entity = world.spawn(());
    assert_eq!(get_foot_position(&world, entity), None);
    assert_eq!(Length::new::<foot>(5.0).get::<foot>(), 5.0);
}
