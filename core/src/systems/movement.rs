use hecs::{Entity, World};
use parry3d::{
    math::Isometry,
    na::Point3,
    query::{PointQuery, Ray, RayCast},
    shape::Ball,
};
use tracing::{debug, error, trace};
use uom::si::{
    f32::Length,
    length::{foot, meter},
};

use crate::{
    components::{
        actions::{
            action::{ActionConditionResolution, ActionResultComponent, DamageResult},
            targeting::{
                LineOfSight, LineOfSightTrajectory, TargetInstance, TargetingCheck, TargetingError,
                TargetingKind,
            },
        },
        activity::ActivityState,
        damage::{DamageRoll, DamageType},
        dice::{DiceSet, DieSize},
        health::life_state::LifeState,
        id::EntityIdentifier,
        modifier::{ModifierKind, ModifierMap, ModifierSource},
        speed::{EffectiveSpeed, Speed},
    },
    engine::{
        action_prompt::{ActionData, ActionError},
        engine_state::EngineState,
        event::{Event, EventKind},
        geometry::WorldPath,
    },
    systems::{
        self,
        actions::{ActionUsabilityCheck, ActionUsabilityError},
        geometry::{EPSILON, LineOfSightResult, RaycastFilter},
    },
};

pub const MOVEMENT_SPEED: f32 = 5.0; // [m/s]

pub const FALL_DAMAGE_DIE: DieSize = DieSize::D6;
pub const FALL_DAMAGE_MAX_DICE: u32 = 20;
pub const FALL_DAMAGE_THRESHOLD_FT: f32 = 10.0;

/// How far apart, along a path, the points are that get tried as a place to stop.
const STOP_SEARCH_STEP: f32 = 0.25; // [m]

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MovementError {
    InsufficientSpeed,
    NoPathFound,
    // Strictly speaking this isn't a movement error, but it makes it easier to
    // handle in the engine state if we put it here ;)
    NotYourTurn,
    NotAlive,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PathResult {
    pub full_path: WorldPath,
    pub taken_path: WorldPath,
}

impl PathResult {
    pub fn empty() -> Self {
        Self {
            full_path: WorldPath::new(vec![]),
            taken_path: WorldPath::new(vec![]),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.full_path.points.is_empty() && self.taken_path.points.is_empty()
    }

    pub fn reaches_goal(&self) -> bool {
        !self.is_empty() && self.full_path.points.last() == self.taken_path.points.last()
    }
}

/// The entity's speed with every effect hook applied. Always read derived movement
/// values from here rather than straight off the `Speed` component, otherwise
/// conditional modifiers (e.g. the Barbarian's Fast Movement) are missed.
pub fn speed(engine_state: &EngineState, entity: Entity) -> EffectiveSpeed {
    let mut speed = systems::helpers::get_component_clone::<Speed>(&engine_state.world, entity);
    systems::effects::effects(&engine_state.world, entity).speed(engine_state, entity, &mut speed);
    EffectiveSpeed::new(speed)
}

pub fn path(
    engine_state: &EngineState,
    entity: Entity,
    goal: &Point3<f32>,
    allow_partial: bool,
    trim_to_movement: bool,
) -> Result<PathResult, MovementError> {
    let full_path =
        systems::geometry::path(&engine_state.world, &engine_state.geometry, entity, *goal)
            .ok_or(MovementError::NoPathFound)?;

    let remaining_movement = speed(engine_state, entity).remaining_movement();

    let taken_path = if full_path.length > remaining_movement && trim_to_movement {
        if !allow_partial {
            return Err(MovementError::InsufficientSpeed);
        }
        full_path.trim_to_length(remaining_movement)
    } else {
        full_path.clone()
    };

    Ok(PathResult {
        full_path,
        taken_path,
    })
}

#[derive(Debug, Clone)]
pub struct PathInRangeOfTargetResult {
    pub path_result: PathResult,
    pub line_of_sight_result: LineOfSightResult,
}

pub fn path_in_range_of_target(
    engine_state: &mut EngineState,
    entity: Entity,
    target: &TargetInstance,
    range: Length,
    allow_partial: bool,
    line_of_sight: LineOfSight,
    trim_to_movement: bool,
) -> Result<PathInRangeOfTargetResult, MovementError> {
    trace!(
        "Attempting to path entity {:?} to within {:?} of target {:?}",
        entity, range, target
    );

    let target_point = target
        .position(&engine_state.world)
        .unwrap_or_else(|| panic!("Failed to get position of {target:?}"));

    let (shape, pose) = systems::geometry::get_shape(&engine_state.world, entity)
        .expect("Failed to get shape of entity for path_in_range_of_target");

    let distance_to_target =
        Length::new::<meter>(shape.distance_to_point(&pose, &target_point, true));

    let line_of_sight_result = systems::geometry::line_of_sight_entity_target(
        &engine_state.world,
        &engine_state.geometry,
        entity,
        target,
        &line_of_sight,
    );

    if distance_to_target <= range && line_of_sight_result.has_line_of_sight {
        // Already in range
        trace!("Entity is already in range of target point.");
        return Ok(PathInRangeOfTargetResult {
            path_result: PathResult::empty(),
            line_of_sight_result,
        });
    }

    trace!("Distance to target: {:?}", distance_to_target);

    let path_to_target = path(engine_state, entity, &target_point, true, trim_to_movement)?;

    if let Some(result) = determine_path_sphere_intersections(
        engine_state,
        entity,
        &line_of_sight.trajectory,
        range,
        &path_to_target.full_path,
        target,
    ) {
        let path_result = path(
            engine_state,
            entity,
            &result.intersection_point,
            allow_partial,
            trim_to_movement,
        )?;
        return Ok(PathInRangeOfTargetResult {
            path_result: stop_on_free_spot(engine_state, entity, path_result, trim_to_movement)?,
            line_of_sight_result: result.line_of_sight_result,
        });
    }

    if let Some((spot, line_of_sight_result)) =
        free_spot_around_target(engine_state, entity, target, range, &line_of_sight)
    {
        let path_result = path(engine_state, entity, &spot, allow_partial, trim_to_movement)?;
        return Ok(PathInRangeOfTargetResult {
            path_result: stop_on_free_spot(engine_state, entity, path_result, trim_to_movement)?,
            line_of_sight_result,
        });
    }

    if allow_partial {
        // Return the partial path even if we couldn't get in range. That path
        // leads to the target's own position, so it stops short of the target
        // (or anyone else) on the last free spot.
        trace!("No intersection found, but allowing partial path.");
        return Ok(PathInRangeOfTargetResult {
            path_result: stop_on_free_spot(engine_state, entity, path_to_target, trim_to_movement)?,
            // TODO: Not sure what to do about line of sight in this case
            line_of_sight_result,
        });
    }

    Err(MovementError::NoPathFound)
}

pub struct PathSphereIntersectionResult {
    pub intersection_point: Point3<f32>,
    pub line_of_sight_result: LineOfSightResult,
}

fn determine_path_sphere_intersections(
    engine_state: &mut EngineState,
    entity: Entity,
    trajectory: &LineOfSightTrajectory,
    range: Length,
    path_to_target: &WorldPath,
    target: &TargetInstance,
) -> Option<PathSphereIntersectionResult> {
    let target_point = target.position(&engine_state.world)?;
    let sphere = Ball::new(range.get::<meter>());

    for (start, end) in path_to_target
        .points
        .windows(2)
        .map(|window| (window[0], window[1]))
    {
        let segment_length = (end - start).norm();
        if segment_length <= EPSILON {
            continue;
        }
        let ray = Ray::new(start, (end - start) / segment_length);

        // Only where the segment itself enters the sphere: past its end the ray
        // leaves the path, and the point could be anywhere.
        let Some(entry) = sphere.cast_ray(
            &Isometry::translation(target_point.x, target_point.y, target_point.z),
            &ray,
            segment_length,
            true,
        ) else {
            continue;
        };

        // From where the segment enters the sphere, walk on towards the target
        // until a spot is free and has line of sight: a creature can move
        // through another's space but can't end its move there.
        let mut candidates = Vec::new();
        let mut toi = entry;
        while toi < segment_length {
            candidates.push(toi);
            toi += STOP_SEARCH_STEP;
        }
        candidates.push(segment_length);

        for toi in candidates {
            let intersection_point = ray.point_at(toi);
            if (intersection_point - target_point).norm() > range.get::<meter>() + EPSILON {
                continue;
            }
            if !is_free_spot(engine_state, entity, &intersection_point) {
                continue;
            }
            let line_of_sight_result = line_of_sight_from_point(
                engine_state,
                entity,
                trajectory,
                &intersection_point,
                target,
            )?;

            if !line_of_sight_result.has_line_of_sight {
                // No line of sight from this point; try the next one
                continue;
            } else {
                return Some(PathSphereIntersectionResult {
                    intersection_point,
                    line_of_sight_result,
                });
            }
        }
    }

    None
}

/// Line of sight to `target` for `entity` standing at `point`.
fn line_of_sight_from_point(
    engine_state: &EngineState,
    entity: Entity,
    trajectory: &LineOfSightTrajectory,
    point: &Point3<f32>,
    target: &TargetInstance,
) -> Option<LineOfSightResult> {
    // Entity shouldn't block its own line of sight
    let raycast_filter = RaycastFilter::ExcludeCreatures(vec![entity]);
    let target_point = target.position(&engine_state.world)?;
    let ground = systems::geometry::ground_position(&engine_state.geometry, point)?;
    let eye_position =
        systems::geometry::get_eye_position_at_point(&engine_state.world, entity, &ground)?;
    let mut line_of_sight_result = systems::geometry::line_of_sight_point_point(
        &engine_state.world,
        &engine_state.geometry,
        &eye_position,
        &target_point,
        trajectory,
        &raycast_filter,
    );
    if let TargetInstance::Entity {
        entity: target_entity,
        ..
    } = target
    {
        line_of_sight_result.check_entity_hit(target_entity.id());
    }
    Some(line_of_sight_result)
}

/// How many points around the target get tried when the straight way in has no
/// free spot in range.
const SPOTS_AROUND_TARGET: usize = 16;

/// A free spot within `range` of `target` with line of sight to it, for when the
/// path straight at the target has none (someone stands in the way): the points
/// on a circle around the target, the one with the shortest path from `entity`.
fn free_spot_around_target(
    engine_state: &EngineState,
    entity: Entity,
    target: &TargetInstance,
    range: Length,
    line_of_sight: &LineOfSight,
) -> Option<(Point3<f32>, LineOfSightResult)> {
    let target_point = target.position(&engine_state.world)?;
    let ground = systems::geometry::ground_position(&engine_state.geometry, &target_point)?;
    // The circle on the ground whose points are `range` from the target point,
    // pulled in a little so rounding can't put them out of range.
    let height = target_point.y - ground.y;
    let range = range.get::<meter>();
    if range <= height {
        return None;
    }
    let radius = (range * range - height * height).sqrt() * 0.9;

    let mut best: Option<(f32, Point3<f32>, LineOfSightResult)> = None;
    for i in 0..SPOTS_AROUND_TARGET {
        let angle = std::f32::consts::TAU * i as f32 / SPOTS_AROUND_TARGET as f32;
        let point = ground + parry3d::na::Vector3::new(angle.cos(), 0.0, angle.sin()) * radius;
        if !is_free_spot(engine_state, entity, &point) {
            continue;
        }
        let Some(line_of_sight_result) = line_of_sight_from_point(
            engine_state,
            entity,
            &line_of_sight.trajectory,
            &point,
            target,
        ) else {
            continue;
        };
        if !line_of_sight_result.has_line_of_sight {
            continue;
        }
        let Some(route) =
            systems::geometry::path(&engine_state.world, &engine_state.geometry, entity, point)
        else {
            continue;
        };
        let length = route.length.get::<meter>();
        if best
            .as_ref()
            .is_none_or(|(shortest, ..)| length < *shortest)
        {
            best = Some((length, point, line_of_sight_result));
        }
    }
    best.map(|(_, point, line_of_sight_result)| (point, line_of_sight_result))
}

/// Whether `entity` could end its move at `point`: standing there, it would
/// share no space with another creature that is not dead. A creature can move
/// through another creature's space, but it can't end its move there.
pub fn is_free_spot(engine_state: &EngineState, entity: Entity, point: &Point3<f32>) -> bool {
    let Some((shape, pose)) = systems::geometry::get_shape_at_point(
        &engine_state.world,
        &engine_state.geometry,
        entity,
        point,
    ) else {
        return false;
    };
    systems::geometry::entities_in_shape(&engine_state.world, &shape, &pose)
        .into_iter()
        .all(|other| {
            other == entity
                || matches!(
                    engine_state.world.get::<&LifeState>(other).as_deref(),
                    Ok(LifeState::Dead)
                )
        })
}

/// The furthest point along `path` where `entity` could end its move, if any.
fn last_free_point(
    engine_state: &EngineState,
    entity: Entity,
    path: &WorldPath,
) -> Option<Point3<f32>> {
    let mut distance = path.length.get::<meter>();
    loop {
        let point = path.point_at_distance(Length::new::<meter>(distance))?;
        if is_free_spot(engine_state, entity, &point) {
            return Some(point);
        }
        if distance <= 0.0 {
            return None;
        }
        distance = (distance - STOP_SEARCH_STEP).max(0.0);
    }
}

/// `path_result` itself when the move it takes ends on a free spot, otherwise
/// the same move cut back to the last free spot along it.
fn stop_on_free_spot(
    engine_state: &EngineState,
    entity: Entity,
    path_result: PathResult,
    trim_to_movement: bool,
) -> Result<PathResult, MovementError> {
    let Some(end) = path_result.taken_path.end() else {
        return Ok(path_result);
    };
    if is_free_spot(engine_state, entity, end) {
        return Ok(path_result);
    }
    let Some(start) = path_result.taken_path.start().copied() else {
        return Ok(path_result);
    };
    match last_free_point(engine_state, entity, &path_result.taken_path) {
        Some(stop) if (stop - start).norm() > EPSILON => {
            path(engine_state, entity, &stop, false, trim_to_movement)
        }
        // No free spot along the way but where it stands, or not even that (a
        // creature came to stand in its space): it stays where it is, short of
        // the goal.
        _ => Ok(PathResult {
            full_path: path_result.full_path,
            taken_path: WorldPath::new(vec![start]),
        }),
    }
}

pub fn recharge_movement(world: &mut World, entity: Entity) {
    systems::helpers::get_component_mut::<Speed>(world, entity).reset();
}

#[derive(Debug, Clone)]
pub enum TargetPathFindingResult {
    AlreadyInRange(LineOfSightResult),
    PathFound(PathInRangeOfTargetResult),
}

#[derive(Debug, Clone)]
pub enum TargetPathFindingError {
    NoPathFound,
    MultiplePotentialTargets,
    ShapeFixedToActor,
    ActionError(ActionError),
}

pub fn path_to_target(
    engine_state: &mut EngineState,
    action: &ActionData,
) -> Result<TargetPathFindingResult, TargetPathFindingError> {
    let validation_result = engine_state.validate_action(action, true);

    if let Err(action_error) = &validation_result {
        // Check if it's an error that can be resolved with pathfinding
        if let ActionError::Usability(usability_error) = action_error
            && let ActionUsabilityError::TargetingError(targeting_error) = usability_error
            && let TargetingError::OutOfRange { target, .. }
            | TargetingError::NoLineOfSight { target } = targeting_error
        {
            let targeting_context =
                systems::actions::targeting_context_data(&engine_state.world, action);

            if matches!(targeting_context.kind, TargetingKind::Multiple { .. }) {
                // For simplicity, don't attempt to find a path if there are multiple
                // potential targets since we don't know which one to path to
                return Err(TargetPathFindingError::MultiplePotentialTargets);
            }

            let targeting_context = systems::actions::targeting_context(
                &engine_state.world,
                action.actor.id(),
                &action.action_id,
                &action.context,
            );

            let in_combat = systems::combat::is_in_combat(&engine_state, action.actor.id());

            if let Ok(path_result) = systems::movement::path_in_range_of_target(
                engine_state,
                action.actor.id(),
                target,
                targeting_context.range.max(),
                true,
                targeting_context.line_of_sight,
                in_combat,
            ) {
                return Ok(TargetPathFindingResult::PathFound(path_result));
            } else {
                return Err(TargetPathFindingError::NoPathFound);
            }
        } else {
            return Err(TargetPathFindingError::ActionError(action_error.clone()));
        }
    }

    // TODO: Not sure if this is the best solution
    let line_of_sight_result = systems::geometry::line_of_sight_entity_first_target(
        &engine_state.world,
        &engine_state.geometry,
        action,
    );
    Ok(TargetPathFindingResult::AlreadyInRange(
        line_of_sight_result,
    ))
}

pub enum MoveMode {
    Voluntary,
    Displace,
}

pub fn move_entity(
    engine_state: &mut EngineState,
    entity: Entity,
    new_position: &Point3<f32>,
    mode: MoveMode,
) {
    match mode {
        MoveMode::Voluntary => {
            // Voluntary movement triggers opportunity attacks
            let Some(from_position) =
                systems::geometry::get_foot_position(&engine_state.world, entity)
            else {
                error!(
                    "Failed to get position of entity {:?} for voluntary movement",
                    entity
                );
                return;
            };

            let mut potential_attackers = engine_state.get_potential_reactors(entity);

            potential_attackers.sort_by(|a, b| {
                let distance_a =
                    systems::geometry::distance_between_entities(&engine_state.world, entity, *a)
                        .unwrap_or(Length::new::<meter>(f32::MAX));
                let distance_b =
                    systems::geometry::distance_between_entities(&engine_state.world, entity, *b)
                        .unwrap_or(Length::new::<meter>(f32::MAX));
                distance_a
                    .partial_cmp(&distance_b)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });

            let already_provoked = match &*systems::helpers::get_component::<ActivityState>(
                &engine_state.world,
                entity,
            ) {
                ActivityState::Moving { provoked, .. } => provoked.clone(),
                _ => Vec::new(),
            };

            for attacker in potential_attackers {
                if already_provoked.contains(&attacker) {
                    continue;
                }
                let Some((event, intersection)) = calculate_opportunity_attack(
                    engine_state,
                    entity,
                    &from_position,
                    new_position,
                    attacker,
                ) else {
                    continue;
                };

                // The mover waits where it provoked while the reaction is open, then
                // walks on along the rest of its route and takes the action it was
                // walking to take (SRD 5.2.1, Opportunity Attacks: the attack
                // interrupts the movement, it doesn't end it; nat20-table TASKS.md
                // ENG-22). A walk that provokes inside another creature's space
                // walks on the same way, to its goal (ENG-18). One that the attack
                // drops walks no further (`update_moving`).
                let rest = rest_of_walk(engine_state, entity, &intersection);

                move_new_position(engine_state, entity, from_position, &intersection);

                if let ActivityState::Moving {
                    path,
                    current_target,
                    provoked,
                    ..
                } = &mut *systems::helpers::get_component_mut::<ActivityState>(
                    &mut engine_state.world,
                    entity,
                ) {
                    *path = rest;
                    *current_target = 0;
                    provoked.push(attacker);
                }

                engine_state.process_event(event);
                return;
            }

            // No opportunity attacks, so just move the entity
            move_new_position(engine_state, entity, from_position, new_position);
        }

        MoveMode::Displace => {
            systems::geometry::teleport_to(&mut engine_state.world, entity, new_position)
        }
    }
}

fn move_new_position(
    engine_state: &mut EngineState,
    entity: Entity,
    from_position: Point3<f32>,
    new_position: &Point3<f32>,
) {
    systems::geometry::teleport_to(&mut engine_state.world, entity, new_position);

    if systems::combat::is_in_combat(&engine_state, entity) {
        let distance_moved = Length::new::<meter>((new_position - from_position).norm());
        systems::helpers::get_component_mut::<Speed>(&mut engine_state.world, entity)
            .record_movement(distance_moved);
    }
}

/// The rest of `entity`'s walk from `provoked`: the points of its moving path
/// still ahead.
fn rest_of_walk(engine_state: &EngineState, entity: Entity, provoked: &Point3<f32>) -> WorldPath {
    let mut points = vec![*provoked];
    if let Ok(activity) = engine_state.world.get::<&ActivityState>(entity)
        && let ActivityState::Moving {
            path,
            current_target,
            ..
        } = &*activity
    {
        for point in path.points.iter().skip(*current_target) {
            // A segment of no length adds nothing to walk.
            if points
                .last()
                .is_none_or(|last| (point - last).norm() > EPSILON)
            {
                points.push(*point);
            }
        }
    }
    WorldPath::new(points)
}

pub fn calculate_opportunity_attack(
    engine_state: &EngineState,
    entity: Entity,
    from_position: &Point3<f32>,
    new_position: &Point3<f32>,
    attacker: Entity,
) -> Option<(Event, Point3<f32>)> {
    if attacker == entity {
        // An entity can't attack itself
        return None;
    }

    let free_movement = speed(engine_state, entity).free_movement_remaining();
    if (from_position - new_position).norm() <= free_movement.get::<meter>() {
        trace!(
            "Entity {:?} has enough free movement to move from {:?} to {:?} without provoking opportunity attacks.",
            entity, from_position, new_position
        );
        return None;
    }

    let (event, intersection) =
        get_opportunity_attack_point(engine_state, entity, attacker, from_position, new_position)?;

    let reactions = systems::actions::available_reactions_to_event(
        engine_state,
        attacker,
        &event,
        &[ActionUsabilityCheck::Targeting(vec![
            TargetingCheck::Range,
            TargetingCheck::LineOfSight,
        ])],
    );

    if reactions.is_empty() {
        // No reactions so no opportunity attack
        trace!(
            "Potential attacker {:?} has no reactions available to event {:?}, so no opportunity attack.",
            attacker, event
        );
        return None;
    }

    Some((event, intersection))
}

fn get_opportunity_attack_point(
    engine_state: &EngineState,
    entity: Entity,
    attacker: Entity,
    from_position: &Point3<f32>,
    new_position: &Point3<f32>,
) -> Option<(Event, Point3<f32>)> {
    let Some(attacker_position) =
        systems::geometry::get_foot_position(&engine_state.world, attacker)
    else {
        error!(
            "Failed to get position of potential attacker {:?} for opportunity attack",
            attacker
        );
        return None;
    };

    let attacker_reach = {
        let Some((attacker_shape, _)) = systems::geometry::get_shape(&engine_state.world, attacker)
        else {
            error!(
                "Failed to get shape of potential attacker {:?} for opportunity attack",
                attacker
            );
            return None;
        };
        let attacker_loadout = systems::loadout::loadout(&engine_state.world, attacker);
        attacker_loadout.melee_range().max().get::<meter>() + attacker_shape.radius
    };
    let attacker_reach_squared = attacker_reach.powi(2);

    let mut intersections = systems::geometry::line_sphere_intersections(
        from_position,
        new_position,
        &attacker_position,
        attacker_reach,
    );

    // Scenario 1: No intersections
    // Entity either doesn't come within reach or doesn't leave reach, so no opportunity attack.
    if intersections.is_empty() {
        return None;
    }

    let event = Event::new(EventKind::MovingOutOfReach {
        mover: EntityIdentifier::from_world(&engine_state.world, entity),
        entity: EntityIdentifier::from_world(&engine_state.world, attacker),
    });

    // Scenario 2: One intersection
    // 2a: Entity starts outside of reach and enters reach: no opportunity attack
    // 2b: Entity starts inside of reach and remains inside reach: no opportunity attack
    // 2c: Entity starts inside of reach and leaves reach: opportunity attack
    if intersections.len() == 1 {
        // Scenario 2a
        if (from_position - attacker_position).norm_squared() > attacker_reach_squared - EPSILON {
            trace!(
                "Entity {:?} starts outside of reach of potential attacker {:?} and enters reach, so no opportunity attack.",
                entity, attacker
            );
            return None;
        }

        // Scenario 2b
        if (new_position - attacker_position).norm_squared() <= attacker_reach_squared + EPSILON {
            trace!(
                "Entity {:?} starts within reach of potential attacker {:?} and stays within reach, so no opportunity attack.",
                entity, attacker
            );
            return None;
        }

        // Scenario 2c
        trace!(
            "Entity {:?} starts within reach of potential attacker {:?} and leaves reach, so potential opportunity attack.",
            entity, attacker
        );
        return Some((event, intersections[0]));
    }

    // Scenario 3: More than one intersection
    // Entity enters and leaves reach at least once, so opportunity attack.
    trace!(
        "Entity {:?} enters and leaves reach of potential attacker {:?} multiple times, so potential opportunity attack.",
        entity, attacker
    );

    intersections.sort_by(|a, b| {
        // Negative distance so we get the furthest intersection first
        let distance_a = -(from_position - *a).norm_squared();
        let distance_b = -(from_position - *b).norm_squared();
        distance_a
            .partial_cmp(&distance_b)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    Some((event, intersections[0]))
}

// TODO: Not sure where this should live
pub fn apply_fall_damage(engine_state: &mut EngineState, entity: Entity, fall_distance: Length) {
    let fall_distance_ft = fall_distance.get::<foot>();
    let damage_dice =
        FALL_DAMAGE_MAX_DICE.min((fall_distance_ft / FALL_DAMAGE_THRESHOLD_FT).floor() as u32);
    if damage_dice < 1 {
        debug!(
            "Entity {:?} fell {:?} ({} ft), which is below the threshold for fall damage, so no damage applied.",
            entity, fall_distance, fall_distance_ft
        );
        return;
    }

    let mut damage_roll = DamageRoll::new(
        ModifierMap::from(
            ModifierSource::Base,
            ModifierKind::Dice(DiceSet::new(damage_dice, FALL_DAMAGE_DIE)),
        ),
        DamageType::Bludgeoning,
    )
    .roll(false);

    let (damage_taken, new_life_state) =
        systems::health::damage(engine_state, entity, &mut damage_roll, None, None);

    // TODO: Some kind of damage source so we can see it was from fall damage in the logs?

    engine_state.process_event(Event::action_result_event(
        EntityIdentifier::from_world(&engine_state.world, entity),
        ActionResultComponent::Damage(DamageResult {
            resolution: ActionConditionResolution::Unconditional,
            damage_roll: Some(damage_roll),
            damage_taken,
            new_life_state,
        }),
    ));
}
