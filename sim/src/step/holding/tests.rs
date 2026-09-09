use core::f64::consts::TAU;

use super::*;
use crate::TICKS_PER_SECOND;
use crate::belt::Belt;
use crate::fixture::World;
use crate::ids::{AsteroidId, RowId, TeamId};
use crate::orbit::body::Gravity;
use crate::roster::{CONSTRUCTOR, FRIGATE, Kind, LANCER, METALS_EXTRACTOR, RAIDER};
use crate::state::Rolls;
use crate::time::{Tick, Time};

const FAST: Gravity = Gravity::new(4.4e17);

const HOME: AsteroidId = AsteroidId(0);

const NEIGHBOUR_METERS: core::ops::Range<f64> = 1_800.0..2_200.0;

const HOP_MARGIN_SECONDS: f64 = 0.5;

const CROSSING_MARGIN_SECONDS: f64 = 6.0;

const PASSING_METERS: f64 = 1.0;

const LONGEST_CROSSING: u64 = 600 * TICKS_PER_SECOND as u64;

const CIRCLING_METERS: f64 = 1.0;

const WALKING_METERS: f64 = 1.5;

const SETTLING_SECONDS: u64 = 40;

fn seconds(count: u64) -> u64 {
    count * u64::from(TICKS_PER_SECOND)
}

fn circle_radius(world: &World, asteroid: AsteroidId) -> f64 {
    world.state[asteroid].radius() + Belt::SPACING_METERS + Belt::STATION_SPACING_METERS
}

fn unit_rows() -> Vec<(RowId, &'static str)> {
    World::ring(FAST, 1, &[TeamId(0)])
        .state
        .roster()
        .iter()
        .filter(|(_, row)| row.kind() == Kind::Unit)
        .map(|(id, row)| (id, row.name))
        .collect()
}

fn walked(world: &mut World, unit: EntityId, asteroid: AsteroidId, samples: u32) -> Vec<Vec3> {
    (0..samples)
        .map(|_| {
            world.steers(seconds(1) / 2);
            world.body(unit).pos - world.state.asteroid_body(asteroid).pos
        })
        .collect()
}

fn turned(offsets: &[Vec3]) -> f64 {
    offsets
        .windows(2)
        .filter_map(|pair| {
            let from = pair[0].normalized()?;
            let to = pair[1].normalized()?;
            Some(libm::acos(from.dot(to).clamp(-1.0, 1.0)))
        })
        .sum()
}

fn plane(world: &mut World, unit: EntityId) -> Vec3 {
    let first = world.body(unit).pos - world.state.asteroid_body(HOME).pos;
    world.steers(seconds(5));
    let then = world.body(unit).pos - world.state.asteroid_body(HOME).pos;
    first.cross(then).normalized().expect("a plane it turns in")
}

fn reload_ticks(world: &World, row: RowId) -> u64 {
    let rate = world.state[row]
        .damage_places()
        .next()
        .expect("a row that does damage")
        .hitscan
        .rate
        .0;
    (f64::from(TICKS_PER_SECOND) / rate) as u64
}

fn world() -> World {
    World::ring(FAST, 2, &[TeamId(0), TeamId(1)])
}

fn apart(state: &State, from: AsteroidId, to: AsteroidId) -> f64 {
    state
        .asteroid_body(from)
        .pos
        .distance(state.asteroid_body(to).pos)
}

fn least_seconds(state: &State, from: AsteroidId, to: AsteroidId) -> f64 {
    2.0 * (apart(state, from, to) / state.roster().movement_limit().0).sqrt()
}

fn pairs(state: &State) -> Vec<(AsteroidId, AsteroidId)> {
    let belt: Vec<AsteroidId> = state.asteroids().map(|(id, _)| id).collect();
    belt.iter()
        .flat_map(|from| belt.iter().map(move |to| (*from, *to)))
        .filter(|(from, to)| from != to)
        .collect()
}

fn neighbours(state: &State) -> Vec<(AsteroidId, AsteroidId)> {
    pairs(state)
        .into_iter()
        .filter(|(from, to)| NEIGHBOUR_METERS.contains(&apart(state, *from, *to)))
        .collect()
}

fn farthest(state: &State) -> (AsteroidId, AsteroidId) {
    pairs(state)
        .into_iter()
        .max_by(|one, other| apart(state, one.0, one.1).total_cmp(&apart(state, other.0, other.1)))
        .expect("the belt holds two asteroids")
}

fn flying(world: &mut World, row: RowId, from: AsteroidId, to: AsteroidId) -> EntityId {
    let unit = world.hold(0, row, from, 0.0);
    world.send(unit, to);
    unit
}

fn arrival(world: &mut World, unit: EntityId) -> Time {
    for _ in 0..LONGEST_CROSSING {
        if !world.state.entity(unit).is_flying() {
            return world.state.time();
        }
        world.steers(1);
    }
    panic!("the flier never arrived");
}

fn crossing_seconds(world: &mut World, from: AsteroidId, to: AsteroidId) -> f64 {
    let started = world.state.time();
    let unit = flying(world, FRIGATE, from, to);
    let arrived = arrival(world, unit);
    Time(arrived.0 - started.0).seconds()
}

fn middle(world: &World, force: &[EntityId]) -> Vec3 {
    force
        .iter()
        .fold(Vec3::ZERO, |sum, one| sum + world.body(*one).pos)
        * (1.0 / force.len() as f64)
}

#[test]
fn a_lone_unit_stays_inside_the_zone_for_a_whole_asteroid_period() {
    let mut world = world();
    let unit = world.hold(0, FRIGATE, HOME, 2.0);
    let period = world.period(HOME);

    for _ in 0..100 {
        world.steers(period / 100);
        let off = world.off_asteroid(unit, HOME);
        assert!(off < Belt::ZONE_RADIUS_METERS, "drifted {off} meters off");
    }
}

#[test]
fn a_force_held_for_a_asteroid_period_never_has_a_ship_inside_the_asteroid() {
    for teams in 1..=4u8 {
        let mut world = World::ring(FAST, 2, &(0..teams).map(TeamId).collect::<Vec<TeamId>>());
        let force: Vec<EntityId> = (0..teams)
            .flat_map(|seat| {
                (0..3)
                    .map(|_| {
                        let body = world.state.spawn_body(HOME, world.state.time());
                        world.free(seat, FRIGATE, HOME, body)
                    })
                    .collect::<Vec<EntityId>>()
            })
            .collect();
        let radius = world.state[HOME].radius();
        let period = world.period(HOME);

        for _ in 0..200 {
            world.steers(period / 200);
            for one in &force {
                let off = world.off_asteroid(*one, HOME);
                assert!(
                    off > radius,
                    "{teams} teams: {one:?} is {off} meters from an asteroid of {radius}"
                );
            }
        }
    }
}

#[test]
fn a_lone_unit_is_never_still_for_a_whole_second() {
    let mut world = world();
    let unit = world.hold(0, FRIGATE, HOME, 0.0);
    world.steers(seconds(10));

    for _ in 0..60 {
        let was = world.body(unit).pos;
        world.steers(seconds(1));
        let moved = world.body(unit).pos.distance(was);
        assert!(moved > 0.01, "it sat still for a second, moving {moved}");
    }
}

#[test]
fn a_force_released_together_settles_inside_the_zone_at_the_spacing() {
    let mut world = world();
    let force: Vec<EntityId> = (0..20)
        .map(|at| world.hold(0, FRIGATE, HOME, f64::from(at) * 0.01))
        .collect();

    world.steers(seconds(60));

    let mut closest = f64::MAX;
    for (at, one) in force.iter().enumerate() {
        let off = world.off_asteroid(*one, HOME);
        assert!(off < Belt::ZONE_RADIUS_METERS, "{one:?} settled {off} off");
        for other in &force[at + 1..] {
            closest = closest.min(world.body(*one).pos.distance(world.body(*other).pos));
        }
    }
    assert!(
        closest > 0.9 * Belt::SPACING_METERS,
        "the closest pair settled {closest} meters apart"
    );
}

#[test]
fn a_unit_never_leaves_its_zone_for_an_enemy_outside_it() {
    let mut world = world();
    let hunter = world.hold(0, FRIGATE, HOME, 0.0);
    let strayed = world.hold(1, RAIDER, HOME, Belt::ZONE_RADIUS_METERS + 5.0);

    for _ in 0..seconds(30) {
        let asteroid = world.state.asteroid_body(HOME);
        let radial = asteroid.pos.normalized().expect("a radius");
        let out = asteroid.pos + radial * (Belt::ZONE_RADIUS_METERS + 5.0);
        world.state.steer(strayed, Body::new(out, asteroid.vel));
        world.steers(1);
        let off = world.off_asteroid(hunter, HOME);
        assert!(
            off < Belt::ZONE_RADIUS_METERS,
            "the hunter followed its enemy {off} meters out"
        );
    }

    let out = world.off_asteroid(strayed, HOME);
    assert!(
        out > Belt::ZONE_RADIUS_METERS,
        "the enemy the hunter would not follow stood {out} out, inside the zone"
    );
}

#[test]
fn a_unit_that_chases_a_faster_enemy_across_the_zone_strikes_the_structures_it_passes() {
    let mut world = world();
    let chaser = world.hold(0, FRIGATE, HOME, 0.0);
    let quarry = world.hold(1, RAIDER, HOME, 1.0);
    let struck = world.fix(1, METALS_EXTRACTOR, HOME);
    let whole = world.state.entity(struck).hp();
    let station = {
        let rolls = Rolls::called(&world.state);
        rolls[HOME]
            .station(chaser)
            .expect("a unit that does damage is stationed")
    };
    let asteroid = world.state.asteroid_body(HOME);
    world.state.steer(chaser, Body::new(station, asteroid.vel));
    assert!(
        station.distance(world.body(struck).pos) > world.state[FRIGATE].max_damage_range(),
        "the chaser reaches the structure from its own station, without running anywhere"
    );
    let across = (asteroid.pos - station)
        .normalized()
        .expect("a way across the stage")
        * (Belt::ZONE_RADIUS_METERS - 5.0);

    for _ in 0..seconds(30) {
        let at = world.state.asteroid_body(HOME);
        world
            .state
            .steer(quarry, Body::new(at.pos + across, at.vel));
        world.tick(&[]);
        if world.state.entity(struck).hp() < whole {
            break;
        }
    }

    assert!(
        world.state.entity(struck).hp() < whole,
        "the chase never brought the chaser within reach of the structures it ran past"
    );
}

#[test]
fn a_unit_that_does_no_damage_is_steered_by_no_enemy_in_its_zone() {
    let mut world = world();
    let builder = world.hold(0, CONSTRUCTOR, HOME, Belt::ZONE_RADIUS_METERS - 2.0);
    let mut alone = World {
        state: world.state.clone(),
    };
    world.fix(1, METALS_EXTRACTOR, HOME);

    for _ in 0..60 {
        world.steers(seconds(1));
        alone.steers(seconds(1));
        assert_eq!(
            world.body(builder).pos,
            alone.body(builder).pos,
            "an enemy in the zone steered a unit that does no damage"
        );
    }
}

#[test]
fn an_arrival_that_does_no_damage_comes_in_from_the_rim_to_its_own_circle() {
    let mut world = World::started(&[TeamId(0)]);
    let (from, to) = neighbours(&world.state)
        .first()
        .copied()
        .expect("the belt holds two kilometer hops");
    let unit = flying(&mut world, CONSTRUCTOR, from, to);

    arrival(&mut world, unit);
    let landed = world.off_asteroid(unit, to);
    assert!(
        landed > Belt::ZONE_RADIUS_METERS - PASSING_METERS,
        "it arrived {landed} meters off, inside the rim"
    );

    world.steers(seconds(SETTLING_SECONDS));

    let radius = circle_radius(&world, to);
    let off = world.off_asteroid(unit, to);
    assert!(
        (off - radius).abs() < CIRCLING_METERS,
        "it holds {off} meters off the body, not on a circle of {radius}"
    );
    let rolls = Rolls::called(&world.state);
    let station = rolls[to]
        .station(unit)
        .expect("a unit that does no damage is stationed");
    let behind = world.body(unit).pos.distance(station);
    assert!(
        behind < WALKING_METERS,
        "it walks {behind} meters behind its station"
    );
}

#[test]
fn no_unit_of_the_shipped_roster_ever_enters_the_asteroid() {
    for (row, name) in unit_rows() {
        let mut world = world();
        let units: Vec<EntityId> = (0..3)
            .map(|at| world.hold(0, row, HOME, Belt::ZONE_RADIUS_METERS - f64::from(at)))
            .collect();
        let radius = world.state[HOME].radius();

        for _ in 0..120 {
            world.steers(seconds(1) / 2);
            for one in &units {
                let off = world.off_asteroid(*one, HOME);
                assert!(off > radius, "a {name} holds {off} off a rock of {radius}");
            }
        }
    }
}

#[test]
fn a_unit_that_does_no_damage_circles_its_asteroid_within_a_minute_on_its_own_radius() {
    let mut world = world();
    let unit = world.hold(0, CONSTRUCTOR, HOME, Belt::ZONE_RADIUS_METERS - 1.0);
    world.steers(seconds(SETTLING_SECONDS));
    let radius = circle_radius(&world, HOME);

    let offsets = walked(&mut world, unit, HOME, 120);

    for offset in &offsets {
        let off = offset.length();
        assert!(
            (off - radius).abs() < CIRCLING_METERS,
            "it left a circle of {radius} for {off}"
        );
    }
    let turned = turned(&offsets);
    assert!(
        turned > TAU,
        "it turned {turned} radians about its asteroid in a minute"
    );
}

#[test]
fn two_units_of_a_row_that_does_no_damage_circle_on_two_planes_and_never_share_a_point() {
    let mut world = world();
    let one = world.hold(0, CONSTRUCTOR, HOME, Belt::ZONE_RADIUS_METERS - 1.0);
    let other = world.hold(0, CONSTRUCTOR, HOME, Belt::ZONE_RADIUS_METERS - 2.0);
    world.steers(seconds(SETTLING_SECONDS));

    let mut closest = f64::MAX;
    for _ in 0..120 {
        world.steers(seconds(1) / 2);
        closest = closest.min(world.body(one).pos.distance(world.body(other).pos));
    }

    assert!(
        closest > 0.9 * Belt::SPACING_METERS,
        "the pair closed to {closest} meters"
    );
    let aligned = plane(&mut world, one).dot(plane(&mut world, other));
    assert!(
        aligned.abs() < 0.99,
        "the pair circles in one plane, its normals {aligned} aligned"
    );
}

#[test]
fn the_circling_of_a_unit_that_does_no_damage_is_reproduced_by_a_rewind_to_the_same_tick() {
    let mut world = world();
    let unit = world.hold(0, CONSTRUCTOR, HOME, Belt::ZONE_RADIUS_METERS - 1.0);
    let start = world.state.clone();
    world.steers(seconds(30));
    let once = world.body(unit).pos;

    let mut again = World { state: start };
    again.steers(seconds(30));

    assert_eq!(again.body(unit).pos, once);
    assert_eq!(again.state.hash(), world.state.hash());
    assert_ne!(again.state.tick(), Tick::ZERO);
}

#[test]
fn two_sides_form_two_lines_facing_across_the_stage() {
    let mut world = world();
    let sides: Vec<Vec<EntityId>> = (0..2)
        .map(|seat| {
            (0..4)
                .map(|at| world.hold(seat, LANCER, HOME, f64::from(at) * 0.3))
                .collect()
        })
        .collect();

    world.steers(seconds(60));

    let rolls = Rolls::called(&world.state);
    let standoff = world.state[LANCER]
        .standoff()
        .expect("a row that does damage");
    for one in sides.iter().flatten() {
        let station = rolls[HOME]
            .station(*one)
            .expect("a unit that does damage is stationed");
        let off = world.body(*one).pos.distance(station);
        assert!(off < 1.0, "{one:?} holds {off} meters off its station");
    }
    let asteroid = world.state.asteroid_body(HOME);
    let motion = asteroid.vel.normalized().expect("a moving asteroid");
    let along = |one: &EntityId| (world.body(*one).pos - asteroid.pos).dot(motion);
    for behind in sides[0].iter().map(along) {
        assert!(behind < 0.0, "the lowest team left the retrograde side");
    }
    for ahead in sides[1].iter().map(along) {
        assert!(ahead > 0.0, "the next team left the prograde side");
    }
    let apart = (middle(&world, &sides[1]) - middle(&world, &sides[0])).dot(motion);
    assert!(
        (apart - 2.0 * standoff).abs() < 1.0,
        "the lines stand {apart} apart across the stage, not {}",
        2.0 * standoff
    );
}

#[test]
fn a_long_range_row_stands_behind_a_short_range_one() {
    let mut world = world();
    let near: Vec<EntityId> = (0..3)
        .map(|at| world.hold(0, RAIDER, HOME, f64::from(at) * 0.3))
        .collect();
    let far: Vec<EntityId> = (0..3)
        .map(|at| world.hold(0, LANCER, HOME, 2.0 + f64::from(at) * 0.3))
        .collect();

    world.steers(seconds(60));

    let asteroid = world.state.asteroid_body(HOME);
    let motion = asteroid.vel.normalized().expect("a moving asteroid");
    let rear = |unit: &EntityId| (world.body(*unit).pos - asteroid.pos).dot(-motion);
    let front = near.iter().map(rear).fold(f64::MIN, f64::max);
    let back = far.iter().map(rear).fold(f64::MAX, f64::min);
    assert!(
        back > front,
        "a long range unit stands {back} back, before a short range one at {front}"
    );
}

#[test]
fn an_arrival_walks_from_the_rim_to_its_station() {
    let mut world = World::started(&[TeamId(0), TeamId(1)]);
    let (from, to) = neighbours(&world.state)
        .first()
        .copied()
        .expect("the belt holds two kilometer hops");
    world.hold(1, LANCER, to, 0.0);
    let unit = flying(&mut world, LANCER, from, to);

    arrival(&mut world, unit);
    let landed = world.off_asteroid(unit, to);
    assert!(
        landed > Belt::ZONE_RADIUS_METERS - PASSING_METERS,
        "it arrived {landed} meters off, inside the rim"
    );

    world.steers(seconds(60));

    let rolls = Rolls::called(&world.state);
    let station = rolls[to]
        .station(unit)
        .expect("a unit that does damage is stationed");
    let off = world.body(unit).pos.distance(station);
    assert!(off < 1.0, "it holds {off} meters off the station it joined");
}

#[test]
fn a_short_range_unit_turns_for_its_station_no_sooner_than_its_prey_reloads_and_stays_in_the_zone()
{
    let mut world = world();
    let runner = world.hold(0, FRIGATE, HOME, 0.0);
    world.hold(1, FRIGATE, HOME, 5.0);
    let reload = reload_ticks(&world, FRIGATE);

    let mut turns: Vec<u64> = Vec::new();
    let mut was = Pass::Running;
    for tick in 0..seconds(60) {
        world.run(1);
        if !world.still_holds(runner) {
            break;
        }
        let off = world.off_asteroid(runner, HOME);
        assert!(off < Belt::ZONE_RADIUS_METERS, "it passed {off} meters out");
        let pass = world.state.entity(runner).pass();
        if (was, pass) == (Pass::Running, Pass::Returning) {
            turns.push(tick);
        }
        was = pass;
    }

    assert!(
        turns.len() > 1,
        "it turned for its station {} times under fire",
        turns.len()
    );
    let soonest = turns
        .windows(2)
        .map(|pair| pair[1] - pair[0])
        .min()
        .expect("two turns");
    assert!(
        soonest >= reload,
        "it turned again {soonest} ticks on, inside its prey's {reload} tick reload"
    );
}

#[test]
fn a_short_range_unit_stays_on_a_prey_that_never_fires_back_until_it_kills_it() {
    let mut world = world();
    let runner = world.hold(0, RAIDER, HOME, 0.0);
    let prey = world.fix(1, METALS_EXTRACTOR, HOME);

    for _ in 0..seconds(60) {
        if !world.still_holds(prey) {
            break;
        }
        world.run(1);
        assert_eq!(
            world.state.entity(runner).pass(),
            Pass::Running,
            "it turned for its station with nothing shooting at it"
        );
    }

    assert!(!world.still_holds(prey), "it never killed what it ran at");
}

#[test]
fn a_thrust_never_exceeds_the_rows_manoeuvring_limit() {
    let mut world = world();
    for at in 0..12 {
        world.hold(0, RAIDER, HOME, f64::from(at) * 0.05);
        world.hold(1, LANCER, HOME, 20.0 + f64::from(at) * 0.05);
    }

    for _ in 0..seconds(5) {
        let steering = Holding::of(
            &world.state,
            &Rolls::called(&world.state),
            &Shots::default(),
        )
        .run();
        for entity in world.state.entities() {
            let limit = world.state[entity.row()].manoeuvring.0;
            let asked = steering.thrusts.of(entity.id()).length();
            assert!(asked <= limit + 1e-12, "{asked} against {limit}");
        }
        world.steers(1);
    }
}

#[test]
fn the_rule_reads_only_the_tick_it_is_given() {
    let mut world = world();
    for at in 0..6 {
        world.hold(0, FRIGATE, HOME, f64::from(at) * 0.7);
        world.hold(1, RAIDER, HOME, 9.0 + f64::from(at) * 0.7);
    }
    world.steers(seconds(3));

    let once = Holding::of(
        &world.state,
        &Rolls::called(&world.state),
        &Shots::default(),
    )
    .run();
    let twice = Holding::of(
        &world.state,
        &Rolls::called(&world.state),
        &Shots::default(),
    )
    .run();

    assert_eq!(once, twice);
    assert_ne!(once, Steering::default());
}

#[test]
fn a_structure_is_never_given_a_thrust() {
    let mut world = world();
    let fixed = world.fix(0, FRIGATE, HOME);
    let steered = world.hold(0, FRIGATE, HOME, 0.2);

    let steering = Holding::of(
        &world.state,
        &Rolls::called(&world.state),
        &Shots::default(),
    )
    .run();

    assert_eq!(steering.thrusts.of(fixed), Vec3::ZERO);
    assert_ne!(
        steering.thrusts.of(steered),
        Vec3::ZERO,
        "no unit at the asteroid was thrust either"
    );
}

#[test]
fn the_drift_is_reproduced_by_a_rewind_to_the_same_tick() {
    let mut world = world();
    let unit = world.hold(0, FRIGATE, HOME, 1.0);
    let start = world.state.clone();
    world.steers(seconds(4));
    let once = world.body(unit).pos;

    let mut again = World { state: start };
    again.steers(seconds(4));

    assert_eq!(again.body(unit).pos, once);
    assert_ne!(again.state.tick(), Tick::ZERO);
}

#[test]
fn a_hop_to_a_neighbour_takes_about_two_root_distance_over_the_limit() {
    let mut world = World::started(&[TeamId(0)]);
    let (from, to) = neighbours(&world.state)
        .first()
        .copied()
        .expect("the belt holds two kilometer hops");
    let least = least_seconds(&world.state, from, to);

    let flown = crossing_seconds(&mut world, from, to);

    assert!(
        (least..least + HOP_MARGIN_SECONDS).contains(&flown),
        "a {least:.1} second hop took {flown:.1} seconds"
    );
}

#[test]
fn the_widest_crossing_of_the_belt_takes_about_two_root_distance_over_the_limit() {
    let mut world = World::started(&[TeamId(0)]);
    let (from, to) = farthest(&world.state);
    let least = least_seconds(&world.state, from, to);

    let flown = crossing_seconds(&mut world, from, to);

    assert!(
        (least..least + CROSSING_MARGIN_SECONDS).contains(&flown),
        "a {least:.1} second crossing took {flown:.1} seconds"
    );
}

#[test]
fn a_flier_arrives_at_rest_and_never_passes_its_destination() {
    let mut world = World::started(&[TeamId(0)]);
    let (from, to) = neighbours(&world.state)
        .first()
        .copied()
        .expect("the belt holds two kilometer hops");
    let unit = flying(&mut world, FRIGATE, from, to);

    let mut near = false;
    for _ in 0..LONGEST_CROSSING {
        if !world.state.entity(unit).is_flying() {
            break;
        }
        world.steers(1);
        let off = world.off_asteroid(unit, to);
        near |= off <= PASSING_METERS;
        assert!(
            !near || off <= PASSING_METERS,
            "it passed its destination and came back, {off} meters off"
        );
    }

    let landed = Transfer::of(
        world.body(unit),
        world.state.asteroid_body(to),
        world.state.roster().movement_limit().0,
    );
    assert!(landed.arrived(), "it stopped flying before it arrived");
    assert_eq!(world.state.entity(unit).standing(), Some(to));
}

#[test]
fn no_thrust_of_a_flier_exceeds_the_movement_limit() {
    let mut world = World::started(&[TeamId(0)]);
    let (from, to) = neighbours(&world.state)
        .first()
        .copied()
        .expect("the belt holds two kilometer hops");
    let unit = flying(&mut world, FRIGATE, from, to);
    let limit = world.state.roster().movement_limit().0;

    for _ in 0..LONGEST_CROSSING {
        if !world.state.entity(unit).is_flying() {
            break;
        }
        let asked = Holding::of(
            &world.state,
            &Rolls::called(&world.state),
            &Shots::default(),
        )
        .run()
        .thrusts
        .of(unit)
        .length();
        assert!(asked <= limit + 1e-12, "{asked} against {limit}");
        world.steers(1);
    }
}

#[test]
fn an_arrived_force_holds_inside_its_destinations_zone() {
    let mut world = World::started(&[TeamId(0)]);
    let (from, to) = neighbours(&world.state)
        .first()
        .copied()
        .expect("the belt holds two kilometer hops");
    let force: Vec<EntityId> = [FRIGATE, CONSTRUCTOR, LANCER]
        .into_iter()
        .enumerate()
        .map(|(at, row)| {
            let unit = world.hold(0, row, from, at as f64 * 3.0);
            world.send(unit, to);
            unit
        })
        .collect();

    for one in &force {
        arrival(&mut world, *one);
    }
    world.steers(seconds(60));

    for one in &force {
        let off = world.off_asteroid(*one, to);
        assert!(off < Belt::ZONE_RADIUS_METERS, "{one:?} holds {off} off");
    }
}
