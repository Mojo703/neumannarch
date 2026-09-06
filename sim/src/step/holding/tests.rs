use super::*;
use crate::TICKS_PER_SECOND;
use crate::fixture::World;
use crate::ids::{SeatId, TeamId};
use crate::orbit::body::Gravity;
use crate::roster::{CONSTRUCTOR, FRIGATE, LANCER, METALS_EXTRACTOR, RAIDER};
use crate::state::{Flight, Send};
use crate::time::Tick;

const FAST: Gravity = Gravity::new(4.4e17);

const SLOW: Gravity = Gravity::new(4.0e13);

const HOME: RockId = RockId(0);

const AWAY: RockId = RockId(1);

fn seconds(count: u64) -> u64 {
    count * u64::from(TICKS_PER_SECOND)
}

fn world() -> World {
    World::ring(FAST, 2, &[TeamId(0), TeamId(1)])
}

fn spread(world: &World, force: &[EntityId]) -> f64 {
    force
        .iter()
        .flat_map(|one| force.iter().map(move |other| (one, other)))
        .map(|(one, other)| world.body(*one).pos.distance(world.body(*other).pos))
        .fold(0.0_f64, f64::max)
}

fn centre(world: &World, force: &[EntityId]) -> Vec3 {
    force
        .iter()
        .fold(Vec3::ZERO, |sum, one| sum + world.body(*one).pos)
        * (1.0 / force.len() as f64)
}

#[test]
fn a_lone_unit_stays_inside_the_zone_for_a_whole_rock_period() {
    let mut world = world();
    let unit = world.hold(0, FRIGATE, HOME, 2.0);
    let period = world.period(HOME);

    for _ in 0..100 {
        world.steers(period / 100);
        let off = world.off_rock(unit, HOME);
        assert!(off < Belt::ZONE_RADIUS_METERS, "drifted {off} meters off");
    }
}

#[test]
fn a_force_held_for_a_rock_period_never_has_a_ship_inside_the_rock() {
    let mut world = world();
    let force: Vec<EntityId> = (0..12)
        .map(|_| {
            let body = crate::step::spawn_body(&world.state, HOME, world.state.time());
            world.free(0, FRIGATE, HOME, body)
        })
        .collect();
    let radius = world.state[HOME].radius();
    let period = world.period(HOME);

    for _ in 0..200 {
        world.steers(period / 200);
        for one in &force {
            let off = world.off_rock(*one, HOME);
            assert!(
                off > radius,
                "{one:?} is {off} meters from a rock of {radius}"
            );
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
        let off = world.off_rock(*one, HOME);
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
fn a_unit_chases_an_enemy_inside_the_zone_to_half_its_range() {
    let mut world = world();
    let hunter = world.hold(0, FRIGATE, HOME, Belt::ZONE_RADIUS_METERS - 1.0);
    let prey = world.fix(1, METALS_EXTRACTOR, HOME);

    world.steers(seconds(60));

    let standoff = world.state[FRIGATE].standoff().expect("an armed row");
    let apart = world.body(hunter).pos.distance(world.body(prey).pos);
    assert!(
        (apart - standoff).abs() < 1.0,
        "the hunter holds {apart} meters off, not {standoff}"
    );
}

#[test]
fn a_unit_leaves_an_enemy_outside_the_zone_alone() {
    let mut world = world();
    let hunter = world.hold(0, FRIGATE, HOME, 0.0);
    let prey = world.hold(1, CONSTRUCTOR, HOME, Belt::ZONE_RADIUS_METERS + 5.0);
    let before = world.body(hunter).pos.distance(world.body(prey).pos);

    world.steers(seconds(30));

    let after = world.body(hunter).pos.distance(world.body(prey).pos);
    assert!(
        after > before - Belt::ZONE_RADIUS_METERS,
        "the hunter closed from {before} to {after}"
    );
}

#[test]
fn an_unarmed_unit_never_closes_on_an_enemy_in_its_zone() {
    let mut world = world();
    let builder = world.hold(0, CONSTRUCTOR, HOME, Belt::ZONE_RADIUS_METERS - 2.0);
    let prey = world.fix(1, METALS_EXTRACTOR, HOME);
    let before = world.body(builder).pos.distance(world.body(prey).pos);
    assert!(before > 20.0, "the builder starts on top of its enemy");

    world.steers(seconds(60));

    let after = world.body(builder).pos.distance(world.body(prey).pos);
    assert!(
        after >= before,
        "an unarmed unit closed from {before} to {after}"
    );
}

#[test]
fn a_lone_unit_outnumbered_where_it_stands_falls_back_toward_its_allies() {
    let mut world = world();
    let scout = world.hold(0, FRIGATE, HOME, 10.0);
    let allies: Vec<EntityId> = (0..6)
        .map(|at| world.hold(0, FRIGATE, HOME, f64::from(at) * 0.6))
        .collect();
    let enemies: Vec<EntityId> = (0..6)
        .map(|at| world.hold(1, FRIGATE, HOME, 14.0 + f64::from(at) * 0.6))
        .collect();
    let toward_enemy = centre(&world, &enemies).distance(world.body(scout).pos);
    let toward_allies = centre(&world, &allies).distance(world.body(scout).pos);

    world.steers(seconds(6));

    assert!(
        centre(&world, &allies).distance(world.body(scout).pos) < toward_allies,
        "it did not fall back toward its allies"
    );
    assert!(
        centre(&world, &enemies).distance(world.body(scout).pos) > toward_enemy,
        "it closed on the force that outnumbers it"
    );
}

#[test]
fn a_group_closes_on_its_fire_target_as_one_body() {
    let mut world = world();
    let force: Vec<EntityId> = (0..8)
        .map(|at| world.hold(0, FRIGATE, HOME, 24.0 + f64::from(at) * 0.6))
        .collect();
    let prey = world.fix(1, METALS_EXTRACTOR, HOME);
    let range = world.state[FRIGATE].max_damage_range();
    let opening = spread(&world, &force);

    let mut widest = 0.0_f64;
    for _ in 0..seconds(20) {
        world.steers(1);
        widest = widest.max(spread(&world, &force));
    }

    assert!(
        widest < opening + Belt::SPACING_METERS,
        "the force spread from {opening} to {widest} while closing"
    );
    for one in &force {
        let apart = world.body(*one).pos.distance(world.body(prey).pos);
        assert!(apart <= range, "{one:?} is {apart} meters from the target");
    }
}

#[test]
fn a_flight_ends_at_the_tick_its_schedule_arrives() {
    let mut world = World::ring(SLOW, 2, &[TeamId(0), TeamId(1)]);
    let flier = world.hold(0, FRIGATE, AWAY, 0.0);
    let send = Send::joining(&world.state, HOME, AWAY, SeatId(0), &[flier])
        .expect("a send across the ring");
    world.launch(flier, HOME, 0.0, Flight::new(HOME, send.schedule));
    let arrive = send.schedule.arrive();

    world.steers(arrive.0 - world.state.tick().0 - 1);
    assert!(world.state[flier].flight().is_some(), "it arrived early");

    world.steers(1);
    assert!(world.state[flier].flight().is_none(), "it is still flying");
    assert_eq!(world.state.time(), arrive);
}

#[test]
fn an_arrived_send_holds_inside_its_destinations_zone() {
    let mut world = World::ring(SLOW, 2, &[TeamId(0), TeamId(1)]);
    let force: Vec<EntityId> = [FRIGATE, CONSTRUCTOR, LANCER]
        .into_iter()
        .map(|row| world.hold(0, row, AWAY, 0.0))
        .collect();
    let send =
        Send::joining(&world.state, HOME, AWAY, SeatId(0), &force).expect("a send across the ring");
    for (at, one) in force.iter().enumerate() {
        world.launch(
            *one,
            HOME,
            at as f64 * 3.0,
            Flight::new(HOME, send.schedule),
        );
    }

    world.steers(send.schedule.arrive().0 - world.state.tick().0);
    world.steers(seconds(60));

    for one in &force {
        let off = world.off_rock(*one, AWAY);
        assert!(off < Belt::ZONE_RADIUS_METERS, "{one:?} holds {off} off");
    }
}

#[test]
fn a_flying_unit_thrusts_by_its_schedule_and_by_separation_alone() {
    let mut world = World::ring(SLOW, 2, &[TeamId(0), TeamId(1)]);
    let flier = world.hold(0, FRIGATE, AWAY, 0.0);
    let send = Send::joining(&world.state, HOME, AWAY, SeatId(0), &[flier])
        .expect("a send across the ring");
    world.launch(flier, HOME, 0.0, Flight::new(HOME, send.schedule));
    while !world.state[flier].is_flying(world.state.time()) {
        world.state.advance();
    }

    let sweep = world.state.sweep();
    let alone = Holding::of(&world.state, &sweep).run();
    assert_eq!(alone.of(flier), Vec3::ZERO);

    let body = world.body(flier);
    world.free(
        0,
        FRIGATE,
        HOME,
        Body::new(
            body.pos + Vec3::new(0.1 * Belt::SPACING_METERS, 0.0, 0.0),
            body.vel,
        ),
    );
    let crowded = world.state.sweep();
    let pushed = Holding::of(&world.state, &crowded).run().of(flier);

    assert!(
        pushed.x < 0.0,
        "a flier in company is not pushed: {pushed:?}"
    );
}

#[test]
fn a_thrust_never_exceeds_the_rows_manoeuvring_limit() {
    let mut world = world();
    for at in 0..12 {
        world.hold(0, RAIDER, HOME, f64::from(at) * 0.05);
        world.hold(1, LANCER, HOME, 20.0 + f64::from(at) * 0.05);
    }

    for _ in 0..seconds(5) {
        let sweep = world.state.sweep();
        let thrusts = Holding::of(&world.state, &sweep).run();
        for entity in world.state.entities() {
            let limit = world.state[entity.row()].manoeuvring.0;
            let asked = thrusts.of(entity.id()).length();
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

    let sweep = world.state.sweep();
    let once = Holding::of(&world.state, &sweep).run();
    let twice = Holding::of(&world.state, &sweep).run();

    assert_eq!(once, twice);
    assert_ne!(once, Thrusts::default());
}

#[test]
fn a_structure_is_never_given_a_thrust() {
    let mut world = world();
    let fixed = world.fix(0, FRIGATE, HOME);
    world.hold(0, FRIGATE, HOME, 0.2);

    let sweep = world.state.sweep();

    assert_eq!(
        Holding::of(&world.state, &sweep).run().of(fixed),
        Vec3::ZERO
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
