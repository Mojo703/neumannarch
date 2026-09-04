use crate::belt::Belt;
use crate::ids::{EntityId, RockId, RowId, SeatId};
use crate::materials::Materials;
use crate::orbit::body::Body;
use crate::post::Post;
use crate::roster::Kind;
use crate::state::{Batch, Flight, Frame, Issued, Motion, Rejected, State};
use crate::step::construction::{Construction, Progress};
use crate::step::extraction::{Extraction, Income};
use crate::step::fire::{Fire, Shots};
use crate::step::fulfilment::{Assigned, Fulfilment};
use crate::step::holding::Holding;
use crate::step::propagation::{Moved, Propagation};
use crate::time::Tick;
use crate::vec3::Vec3;

#[derive(Clone, Debug, PartialEq)]
pub struct Outcome {
    pub rejected: Vec<(Issued, Rejected)>,
    pub shots: Shots,
}

impl State {
    pub fn step(&self, issued: &Batch) -> (State, Outcome) {
        let mut applied = self.clone();
        let rejected = issued
            .iter()
            .filter_map(|issued| applied.apply(issued).err().map(|why| (issued, why)))
            .collect();
        let snap = &applied;
        let sweep = snap.sweep();
        let thrusts = Holding::of(snap, &sweep).run();
        let moved = Propagation::of(snap, &thrusts).run();
        let filled = Fulfilment::of(snap).run();
        let income = Extraction::of(snap).run();
        let work = Construction::of(snap).run();
        let shots = Fire::of(snap, &sweep).run();
        let next = State::next(snap, &moved, &filled, &income, &work, &shots);
        (next, Outcome { rejected, shots })
    }

    fn next(
        snap: &State,
        moved: &Moved,
        filled: &Assigned,
        income: &Income,
        work: &Progress,
        shots: &Shots,
    ) -> State {
        let mut next = snap.clone();
        let mut closing = Vec::new();
        move_bodies(&mut next, moved);
        fulfil(&mut next, snap, filled, &mut closing);
        earn(&mut next, income);
        build(&mut next, snap, work, &mut closing);
        resolve(&mut next, shots);
        next.close_frames(&closing);
        reap(&mut next);
        next.refresh_capacities();
        next.advance();
        next
    }
}

fn move_bodies(next: &mut State, moved: &Moved) {
    for step in moved.iter() {
        next.set_motion(
            step.entity,
            Motion::Steered {
                body: step.body,
                flight: step.flight,
            },
        );
    }
}

fn fulfil(next: &mut State, snap: &State, filled: &Assigned, closing: &mut Vec<usize>) {
    for placement in &filled.placements {
        let taken = next
            .seat_mut(placement.post.seat)
            .is_some_and(|seat| seat.take_reserved(placement.row));
        if taken {
            spawn(next, placement.post, placement.row);
        }
    }
    for send in &filled.sends {
        let flight = Flight::new(send.source, send.schedule);
        for member in &send.members {
            join(next, *member, send.destination, flight);
        }
    }
    for opening in &filled.openings {
        next.add_frame(Frame::new(opening.post, opening.row, 0.0, snap.tick()));
    }
    for cancellation in &filled.cancellations {
        let cost = snap[cancellation.row].cost;
        refund(next, cancellation.seat, share(cost, cancellation.progress));
        closing.push(cancellation.frame);
    }
}

fn earn(next: &mut State, income: &Income) {
    for (seat, materials) in income.iter() {
        refund(next, seat, materials);
    }
}

fn build(next: &mut State, snap: &State, work: &Progress, closing: &mut Vec<usize>) {
    for spend in &work.spends {
        let frame = &snap.frames()[spend.frame];
        if let Some(seat) = next.seat_mut(frame.post().seat) {
            seat.stockpile_mut().spend(spend.materials);
        }
        if let Some(open) = next.frame_mut(spend.frame) {
            match spend.materials.total() > 0.0 {
                true => open.build(spend.materials.total(), snap.tick()),
                false => open.short_of(spend.short),
            }
        }
        if spend.completed {
            spawn(next, frame.post(), frame.row());
            closing.push(spend.frame);
        }
    }
    for repair in &work.repairs {
        let Some(full) = next
            .entity(repair.entity)
            .map(|entity| snap[entity.row()].hp.0)
        else {
            continue;
        };
        if let Some(entity) = next.entity_mut(repair.entity) {
            entity.heal(repair.hp, full);
        }
    }
}

fn resolve(next: &mut State, shots: &Shots) {
    for (target, damage) in shots.damage() {
        if let Some(entity) = next.entity_mut(target) {
            entity.hurt(damage);
        }
    }
    for ready in &shots.ready {
        next.set_ready(ready.entity(), ready.weapon(), ready.at());
    }
}

fn reap(next: &mut State) {
    let dead: Vec<EntityId> = next
        .entities()
        .filter(|entity| entity.hp() <= 0.0)
        .map(|entity| entity.id())
        .collect();
    for id in dead {
        next.remove_entity(id);
    }
    let lost: Vec<SeatId> = next
        .seats()
        .iter()
        .enumerate()
        .map(|(at, seat)| (SeatId(at as u8), seat))
        .filter(|(id, seat)| {
            seat.alive()
                && seat.reserve_is_empty()
                && !next.entities().any(|entity| entity.seat() == *id)
        })
        .map(|(id, _)| id)
        .collect();
    for id in lost {
        if let Some(seat) = next.seat_mut(id) {
            seat.eliminate();
        }
        next.close_seat_frames(id);
        let posts: Vec<_> = next
            .posts()
            .map(|(post, _)| post)
            .filter(|post| post.seat == id)
            .collect();
        for post in posts {
            next.close_post(post);
        }
    }
}

fn spawn(next: &mut State, post: Post, row: RowId) {
    let motion = if next[row].kind() == Kind::Structure {
        Motion::Fixed
    } else {
        Motion::Steered {
            body: spawn_body(next, post.rock, next.tick().next()),
            flight: None,
        }
    };
    next.spawn(post.seat, row, post.rock, motion);
}

pub(crate) fn spawn_body(state: &State, rock: RockId, tick: Tick) -> Body {
    let home = state[rock].orbit().at(tick, state.gravity());
    let already = state
        .standing_at(rock)
        .filter(|entity| entity.motion() != Motion::Fixed)
        .count();
    let radial = home.pos.normalized().unwrap_or(Vec3::ZERO);
    let floor = state[rock].radius() + Belt::SPACING_METERS;
    Body::new(
        home.pos + radial * (floor + Belt::SPACING_METERS * already as f64),
        home.vel,
    )
}

fn join(next: &mut State, entity: EntityId, destination: RockId, flight: Flight) {
    let Some(target) = next.entity_mut(entity) else {
        return;
    };
    let Motion::Steered { body, .. } = target.motion() else {
        return;
    };
    target.set_home(destination);
    target.set_motion(Motion::Steered {
        body,
        flight: Some(flight),
    });
}

fn refund(next: &mut State, seat: SeatId, materials: Materials) {
    if let Some(seat) = next.seat_mut(seat) {
        seat.stockpile_mut().add(materials);
    }
}

fn share(cost: Materials, progress: f64) -> Materials {
    let total = cost.total();
    if total > 0.0 {
        cost * (progress / total)
    } else {
        Materials::ZERO
    }
}

pub mod construction;
pub mod extraction;
pub mod fire;
pub mod fulfilment;
pub mod holding;
pub mod propagation;

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::fixture::World;
    use crate::ids::TeamId;
    use crate::materials::Material;
    use crate::orbit::body::Gravity;
    use crate::real::Real;
    use crate::roster::Roster;
    use crate::roster::{CONSTRUCTOR, EXTRACTOR, FRIGATE, LANCER, RAIDER, SHIPYARD, STORAGE};
    use crate::state::{Entity, MAX_WANT, Schedule, Seat, Send};
    use crate::time::Tick;
    use crate::{Materials, TICKS_PER_SECOND};

    fn rock(at: u32) -> RockId {
        RockId(at)
    }

    fn stocked(roster: Roster, reserve: BTreeMap<RowId, u32>) -> World {
        World::crewed(
            roster,
            vec![Seat::new(TeamId(0), Materials::new(1e4, 1e4, 1e4), reserve)],
        )
    }

    #[test]
    fn a_first_want_places_the_reserve_shipyard_at_once() {
        let mut world = World::started(&[TeamId(0)]);
        let seconds = TICKS_PER_SECOND as u64;

        world.tick(&[Issued::want(0, rock(0), SHIPYARD, 1)]);

        assert_eq!(world.count(0, rock(0), SHIPYARD), 1);
        let state = &world.state;
        assert_eq!(state[SeatId(0)].reserved(SHIPYARD), 0);
        assert_eq!(state[SeatId(0)].reserved(CONSTRUCTOR), 1);
        assert_eq!(state.frames().len(), 0, "the reserve needs no frame");
        let shipyard = state.entities().next().expect("the shipyard");
        assert_eq!(shipyard.motion(), Motion::Fixed);
        assert_eq!(state.body_of(shipyard), state.rock_body(RockId(0)));

        world.run(seconds);
        let seat = &world.state[SeatId(0)];
        assert_eq!(
            seat.stockpile().capacity(),
            seat.base_capacity() + world.state[SHIPYARD].capacity
        );
    }

    #[test]
    fn a_frame_that_spends_nothing_for_a_second_names_the_material_it_wants() {
        let a_second_in = |stock| {
            let mut world = World::stocked(stock, BTreeMap::from([(SHIPYARD, 1)]));
            world.tick(&[Issued::want(0, rock(0), SHIPYARD, 1)]);
            world.tick(&[Issued::want(0, rock(0), LANCER, 1)]);
            world.run(TICKS_PER_SECOND as u64 + 1);
            world
                .view(0)
                .plan_of(rock(0), LANCER)
                .and_then(|plan| plan.building)
                .expect("the lancer's frame is open")
        };

        let short = a_second_in(Materials::new(300.0, 0.0, 300.0));
        let fed = a_second_in(Materials::new(300.0, 300.0, 300.0));

        assert_eq!(short.starved_of, Some(Material::Volatiles));
        assert_eq!(short.progress, 0.0, "a starved frame does no work");
        assert_eq!(fed.starved_of, None, "a frame that spends names nothing");
        assert!(fed.progress > 0.0);
    }

    #[test]
    fn a_shortfall_with_a_builder_opens_a_frame_and_completes_it() {
        let mut world = World::started(&[TeamId(0)]);
        world.tick(&[Issued::want(0, rock(0), SHIPYARD, 1)]);
        let stock = world.state[SeatId(0)].stockpile().stock();

        world.tick(&[Issued::want(0, rock(0), STORAGE, 1)]);
        assert_eq!(world.state.frames().len(), 1);
        world.run(10);
        assert_eq!(world.state.frames().len(), 1);
        assert!(world.state.frames()[0].progress() > 0.0);

        let cost = world.state[STORAGE].cost;
        let seconds = cost.total() / 15.0;
        world.run((seconds * f64::from(TICKS_PER_SECOND)).ceil() as u64 + 2);

        assert_eq!(world.count(0, rock(0), STORAGE), 1);
        assert_eq!(world.state.frames().len(), 0);
        let spent = stock - world.state[SeatId(0)].stockpile().stock();
        assert!(
            (spent.total() - cost.total()).abs() < 1e-6,
            "spent {spent:?} on a {cost:?} extractor"
        );
    }

    #[test]
    fn a_builder_reaches_the_frames_at_its_own_rock_and_no_others() {
        let mut world = World::started(&[TeamId(0)]);
        world.tick(&[Issued::want(0, rock(0), SHIPYARD, 1)]);

        world.tick(&[
            Issued::numbered(0, 0, rock(0), STORAGE, 1),
            Issued::numbered(0, 1, rock(5), STORAGE, 1),
        ]);
        world.run(u64::from(TICKS_PER_SECOND));

        assert!(
            world.progress(0, rock(0)) > 0.0,
            "the builder built nothing"
        );
        assert_eq!(
            world.progress(0, rock(5)),
            0.0,
            "a builder reached a frame a rock away"
        );
    }

    #[test]
    fn a_surplus_of_two_rows_fills_the_nearest_shortfall_by_one_send() {
        let mut world = stocked(
            Roster::shipped(),
            BTreeMap::from([(CONSTRUCTOR, 1), (RAIDER, 1)]),
        );
        world.tick(&[
            Issued::numbered(0, 0, rock(0), CONSTRUCTOR, 1),
            Issued::numbered(0, 1, rock(0), RAIDER, 1),
        ]);
        assert_eq!(world.count(0, rock(0), CONSTRUCTOR), 1);
        assert_eq!(world.count(0, rock(0), RAIDER), 1);

        world.tick(&[
            Issued::numbered(0, 0, rock(0), CONSTRUCTOR, 0),
            Issued::numbered(0, 1, rock(0), RAIDER, 0),
            Issued::numbered(0, 2, rock(1), CONSTRUCTOR, 1),
            Issued::numbered(0, 3, rock(1), RAIDER, 1),
        ]);

        assert_eq!(world.count(0, rock(1), CONSTRUCTOR), 1, "it counts home");
        assert_eq!(world.count(0, rock(1), RAIDER), 1, "it counts home");
        assert_eq!(world.count(0, rock(0), CONSTRUCTOR), 0);
        assert_eq!(world.state.frames().len(), 0, "a send fills the shortfall");
        let flights: Vec<Flight> = world
            .state
            .entities()
            .map(|entity| entity.flight().expect("a flight"))
            .collect();
        assert_eq!(flights[0], flights[1], "the two rows fly different sends");
    }

    #[test]
    fn a_send_lands_on_its_destination_rocks_orbit() {
        let mut world = stocked(Roster::shipped(), BTreeMap::from([(CONSTRUCTOR, 1)]));
        world.tick(&[Issued::want(0, rock(0), CONSTRUCTOR, 1)]);
        let unit = world.state.entities().next().expect("the constructor").id();

        world.tick(&[
            Issued::numbered(0, 0, rock(0), CONSTRUCTOR, 0),
            Issued::numbered(0, 1, rock(1), CONSTRUCTOR, 1),
        ]);

        let flight = world.state[unit].flight().expect("a flight");
        world.run(flight.departs().0 - world.state.tick().0);
        let left = world.off_rock(unit, rock(0));
        world.run(flight.arrive().0 - world.state.tick().0);
        assert!(
            !world.state[unit].is_flying(world.state.tick()),
            "it is still flying at its arrival tick"
        );
        let landed = world.off_rock(unit, rock(1));
        assert!(
            landed < left + Schedule::ARRIVAL_POSITION_METERS,
            "it left {left} meters off and arrived {landed} off"
        );

        world.run(60 * u64::from(TICKS_PER_SECOND));

        let held = world.off_rock(unit, rock(1));
        assert!(
            held < Belt::ZONE_RADIUS_METERS,
            "it holds {held} meters off its rock"
        );
    }

    #[test]
    fn units_re_homed_within_the_window_fly_one_send() {
        let mut world = stocked(
            Roster::shipped(),
            BTreeMap::from([(CONSTRUCTOR, 1), (RAIDER, 1)]),
        );
        world.tick(&[
            Issued::numbered(0, 0, rock(0), CONSTRUCTOR, 1),
            Issued::numbered(0, 1, rock(0), RAIDER, 1),
        ]);
        let flight_of = |state: &State, row| {
            state
                .entities()
                .find(|entity| entity.row() == row)
                .and_then(Entity::flight)
        };

        world.tick(&[
            Issued::numbered(0, 0, rock(0), CONSTRUCTOR, 0),
            Issued::numbered(0, 1, rock(1), CONSTRUCTOR, 1),
        ]);
        let first = flight_of(&world.state, CONSTRUCTOR).expect("the first send formed");
        world.run(Send::FORMING_TICKS - 1);
        world.tick(&[
            Issued::numbered(0, 2, rock(0), RAIDER, 0),
            Issued::numbered(0, 3, rock(1), RAIDER, 1),
        ]);

        assert_eq!(
            flight_of(&world.state, RAIDER),
            Some(first),
            "a unit re-homed inside the window opened a send of its own"
        );
        assert_eq!(world.state.tick(), first.departs());
        assert!(
            world
                .state
                .entities()
                .all(|entity| entity.is_flying(world.state.tick())),
            "the window closed and the send did not depart"
        );
    }

    #[test]
    fn a_unit_re_homed_after_the_window_flies_its_own_send() {
        let mut world = stocked(Roster::shipped(), BTreeMap::from([(CONSTRUCTOR, 2)]));
        world.tick(&[Issued::want(0, rock(0), CONSTRUCTOR, 2)]);
        world.tick(&[
            Issued::numbered(0, 0, rock(0), CONSTRUCTOR, 1),
            Issued::numbered(0, 1, rock(1), CONSTRUCTOR, 1),
        ]);
        let first = world
            .state
            .entities()
            .find_map(Entity::flight)
            .expect("the first send formed");

        world.run(Send::FORMING_TICKS + 1);
        world.tick(&[
            Issued::numbered(0, 0, rock(0), CONSTRUCTOR, 0),
            Issued::numbered(0, 1, rock(1), CONSTRUCTOR, 2),
        ]);

        let second = world
            .state
            .entities()
            .filter_map(Entity::flight)
            .find(|flight| *flight != first)
            .expect("the second unit flies a send of its own");
        assert!(
            second.departs() > first.departs(),
            "it joined a send that had already departed"
        );
    }

    #[test]
    fn a_unit_whose_send_is_forming_stands_at_the_rock_it_leaves() {
        let mut world = World::started(&[TeamId(0), TeamId(1)]);
        let prey = world.fix(1, STORAGE, rock(0));
        world.tick(&[
            Issued::numbered(0, 0, rock(0), SHIPYARD, 1),
            Issued::numbered(0, 1, rock(0), FRIGATE, 1),
        ]);
        let building = world.state[FRIGATE].cost.total() / 15.0;
        world.run((building * f64::from(TICKS_PER_SECOND)) as u64 + 2);
        let shooter = world
            .state
            .entities()
            .find(|entity| entity.row() == FRIGATE)
            .expect("the frigate")
            .id();

        world.tick(&[
            Issued::numbered(0, 2, rock(0), FRIGATE, 0),
            Issued::numbered(0, 3, rock(1), FRIGATE, 1),
        ]);

        let now = world.state.tick();
        assert!(
            world.state[shooter].flight().is_some(),
            "its send is forming"
        );
        assert!(!world.state[shooter].is_flying(now));
        assert_eq!(world.state[shooter].standing(now), Some(rock(0)));
        assert_eq!(world.count(0, rock(1), FRIGATE), 1, "it counts toward it");
        assert!(
            world
                .shot_at(prey, u64::from(TICKS_PER_SECOND) / 2 + 1)
                .is_some(),
            "a forming unit stopped shooting"
        );
    }

    #[test]
    fn a_send_the_movement_limit_cannot_fly_leaves_its_units_home_and_opens_frames() {
        let mut world = stocked(
            Roster::shipped().moving_at(Real(1e-6)),
            BTreeMap::from([(CONSTRUCTOR, 1)]),
        );
        world.tick(&[Issued::want(0, rock(0), CONSTRUCTOR, 1)]);
        let unit = world.state.entities().next().expect("the constructor").id();

        world.tick(&[
            Issued::numbered(0, 0, rock(0), CONSTRUCTOR, 0),
            Issued::numbered(0, 1, rock(1), CONSTRUCTOR, 1),
        ]);

        assert!(
            world.state[unit].flight().is_none(),
            "a unit that cannot fly was sent"
        );
        assert_eq!(world.state[unit].home(), rock(0), "it left home");
        assert_eq!(world.count(0, rock(1), CONSTRUCTOR), 0);
        assert_eq!(
            world.state.frames().len(),
            1,
            "the shortfall opened no frame"
        );
    }

    #[test]
    fn a_surplus_unit_with_no_shortfall_is_never_scrapped() {
        let mut world = World::started(&[TeamId(0)]);
        world.tick(&[
            Issued::numbered(0, 0, rock(0), SHIPYARD, 1),
            Issued::numbered(0, 1, rock(0), CONSTRUCTOR, 1),
        ]);
        let stock = world.state[SeatId(0)].stockpile().stock();
        let unit = world
            .state
            .entities()
            .find(|entity| entity.row() == CONSTRUCTOR)
            .expect("the constructor")
            .id();

        world.tick(&[Issued::want(0, rock(0), CONSTRUCTOR, 0)]);
        world.run(8 * u64::from(TICKS_PER_SECOND));

        let state = &world.state;
        assert!(state.entity(unit).is_some(), "the surplus was scrapped");
        assert_eq!(state[unit].home(), rock(0), "it left the rock it stands on");
        assert_eq!(state[unit].hp(), state[CONSTRUCTOR].hp.0);
        let gained = state[SeatId(0)].stockpile().stock() - stock;
        assert!(
            gained.total() < state[CONSTRUCTOR].cost.total(),
            "a whole cost came back: {gained:?}"
        );
    }

    #[test]
    fn a_post_opens_one_frame_of_a_row_at_a_time_and_rows_build_in_parallel() {
        let mut world = World::started(&[TeamId(0)]);
        world.tick(&[Issued::want(0, rock(0), SHIPYARD, 1)]);

        world.tick(&[
            Issued::numbered(0, 0, rock(0), STORAGE, 3),
            Issued::numbered(0, 1, rock(0), EXTRACTOR, 2),
        ]);

        let open = |world: &World, row| world.frames(0, rock(0), row);
        assert_eq!(open(&world, STORAGE), 1, "a shortfall of three opened more");
        assert_eq!(open(&world, EXTRACTOR), 1, "rows build in parallel");

        for _ in 0..30 * u64::from(TICKS_PER_SECOND) {
            if world.count(0, rock(0), STORAGE) == 1 {
                break;
            }
            world.tick(&[]);
        }
        assert_eq!(world.count(0, rock(0), STORAGE), 1, "none completed");
        assert_eq!(open(&world, STORAGE), 0, "its frame closed on completion");

        world.tick(&[]);

        assert_eq!(open(&world, STORAGE), 1, "the next frame opened");
        assert_eq!(world.count(0, rock(0), STORAGE), 1, "and only the next");
    }

    #[test]
    fn a_send_arrives_at_the_earliest_tick_a_schedule_exists() {
        let mut world = World::started(&[TeamId(0)]);
        world.tick(&[Issued::want(0, rock(0), CONSTRUCTOR, 1)]);
        let depart = Tick(world.state.tick().0 + Send::FORMING_TICKS).next();

        world.tick(&[
            Issued::numbered(0, 0, rock(0), CONSTRUCTOR, 0),
            Issued::numbered(0, 1, rock(1), CONSTRUCTOR, 1),
        ]);

        let state = &world.state;
        let unit = state.entities().next().expect("the constructor").id();
        let flight = state[unit].flight().expect("a flight");
        let arrive = flight.arrive();
        assert_eq!(flight.departs(), depart);
        let gravity = state.gravity();
        let source = state[rock(0)].orbit().at(depart, gravity);
        let limit = state.roster().movement_limit().0;
        let second = u64::from(TICKS_PER_SECOND);
        assert!(arrive.0 - depart.0 > second, "the first candidate answered");
        for step in (second..arrive.0 - depart.0).step_by(second as usize) {
            let earlier = Tick(depart.0 + step);
            let target = state[rock(1)].orbit().at(earlier, gravity);
            assert_eq!(
                Schedule::between(source, target, depart, earlier, limit, gravity),
                None,
                "arriving at {earlier:?} would have fitted"
            );
        }
    }

    #[test]
    fn an_armed_unit_kills_an_unarmed_enemy_at_its_rock() {
        let mut world = World::started(&[TeamId(0), TeamId(1)]);

        let prey = world.fix(1, STORAGE, rock(0));
        world.tick(&[
            Issued::numbered(0, 0, rock(0), SHIPYARD, 1),
            Issued::numbered(0, 1, rock(0), FRIGATE, 1),
        ]);

        let building = world.state[FRIGATE].cost.total() / 15.0;
        world.run((building * f64::from(TICKS_PER_SECOND)) as u64 + 2);
        assert_eq!(world.count(0, rock(0), FRIGATE), 1);

        let interval = u64::from(TICKS_PER_SECOND) / 2;
        let hit = world
            .shot_at(prey, interval + 1)
            .expect("the frigate fired");
        assert_eq!(hit.damage, 6.0, "six damage through no plating");

        let shots = (world.state[prey].hp() / hit.damage).ceil();
        world.run((shots / 2.0 * f64::from(TICKS_PER_SECOND)) as u64 - interval);
        assert!(world.state.entity(prey).is_some(), "it died too soon");

        world.run(2 * interval);

        assert_eq!(world.state.entity(prey), None, "it outlived its hit points");
        assert!(
            !world
                .state
                .ready()
                .iter()
                .any(|ready| ready.entity() == prey),
            "a dead entity kept its weapons"
        );
    }

    #[test]
    fn a_weapon_fires_at_every_enemy_in_range_at_its_rock() {
        let mut world = World::started(&[TeamId(0), TeamId(1)]);
        let reach = world.state[LANCER].max_damage_range();
        let shooter = world.hold(0, LANCER, rock(0), 0.0);
        let near = world.hold(1, STORAGE, rock(0), reach - 1.0);
        let far = world.hold(1, STORAGE, rock(0), reach + 1.0);

        let hits = world.shots().hits;

        assert_eq!(hits.len(), 1, "one weapon fires once a tick");
        assert_eq!(hits[0].shooter, shooter);
        assert_eq!(hits[0].target, near);
        assert!(
            world.shot_at(far, u64::from(TICKS_PER_SECOND)).is_none(),
            "an enemy beyond the weapon's range was fired on"
        );
    }

    #[test]
    fn fire_never_targets_a_flying_unit() {
        let mut world = World::started(&[TeamId(0), TeamId(1)]);
        world.tick(&[
            Issued::numbered(0, 0, rock(0), SHIPYARD, 1),
            Issued::numbered(0, 1, rock(0), FRIGATE, 1),
            Issued::want(1, rock(0), CONSTRUCTOR, 1),
        ]);
        let building = world.state[FRIGATE].cost.total() / 15.0;
        world.run((building * f64::from(TICKS_PER_SECOND)) as u64 + 2);
        let prey = world
            .state
            .entities()
            .find(|entity| entity.seat() == SeatId(1))
            .expect("the enemy constructor")
            .id();
        let interval = u64::from(TICKS_PER_SECOND) / 2;
        assert!(
            world.shot_at(prey, interval + 1).is_some(),
            "the enemy was not a target while holding"
        );

        world.tick(&[
            Issued::numbered(1, 1, rock(0), CONSTRUCTOR, 0),
            Issued::numbered(1, 2, rock(1), CONSTRUCTOR, 1),
        ]);
        world.run(Send::FORMING_TICKS + 1);

        assert!(world.state[prey].is_flying(world.state.tick()));
        assert_eq!(
            world.shot_at(prey, interval + 1),
            None,
            "a flying unit was fired on"
        );
    }

    #[test]
    fn a_seat_with_no_entity_and_an_empty_reserve_is_eliminated() {
        let mut world = World::seated(vec![
            Seat::new(TeamId(0), Materials::ZERO, BTreeMap::new()),
            Seat::new(TeamId(1), Materials::ZERO, BTreeMap::from([(SHIPYARD, 1)])),
        ]);

        world.tick(&[]);

        assert!(!world.state[SeatId(0)].alive());
        assert!(world.state[SeatId(1)].alive());
        assert_eq!(
            world.refusal(Issued::want(0, rock(0), SHIPYARD, 1)),
            Some(Rejected::DeadSeat)
        );
    }

    #[test]
    fn a_want_the_state_cannot_take_is_rejected_by_name_and_changes_nothing() {
        let world = World::started(&[TeamId(0)]);

        let refusals = [
            (Issued::want(9, rock(0), SHIPYARD, 1), Rejected::NoSuchSeat),
            (Issued::want(0, rock(99), SHIPYARD, 1), Rejected::NoSuchRock),
            (
                Issued::want(0, rock(0), RowId(u16::MAX), 1),
                Rejected::NoSuchRow,
            ),
            (
                Issued::want(0, rock(0), FRIGATE, MAX_WANT + 1),
                Rejected::TooMany,
            ),
        ];

        for (issued, why) in refusals {
            assert_eq!(world.refusal(issued), Some(why), "{issued:?}");
            let (next, _) = world.state.step(&Batch::of(&[issued]));
            assert_eq!(next.posts().count(), 0, "{issued:?} left a want behind");
        }
        assert_eq!(
            world.refusal(Issued::want(0, rock(0), FRIGATE, MAX_WANT)),
            None
        );
    }

    #[test]
    fn a_tick_lands_the_same_state_and_hash_however_its_commands_arrived() {
        let world = World::started(&[TeamId(0), TeamId(1)]);

        let issued = [
            Issued::numbered(0, 0, rock(0), FRIGATE, 3),
            Issued::numbered(0, 1, rock(0), FRIGATE, 7),
            Issued::numbered(1, 0, rock(2), CONSTRUCTOR, 1),
            Issued::numbered(0, 2, rock(1), SHIPYARD, 1),
        ];

        let (ordered, _) = world.state.step(&Batch::of(&issued));
        let mut scrambled = issued;
        scrambled.reverse();
        let (arrived, _) = world.state.step(&Batch::of(&scrambled));

        assert_eq!(ordered.hash(), arrived.hash());
        assert_eq!(ordered, arrived);
        assert_eq!(
            ordered
                .wants(Post::of(0, rock(0)))
                .map(|wants| wants.get(FRIGATE)),
            Some(7),
            "the seat's later command is the one that stands"
        );
    }

    #[test]
    fn each_unit_spawns_one_spacing_further_out_than_the_last_clear_of_the_rock() {
        let mut world = World::ring(Gravity::new(4.0e13), 1, &[TeamId(0)]);
        let home = world.state.rock_body(rock(0));
        let radial = home.pos.normalized().expect("a radius");
        let floor = world.state[rock(0)].radius() + Belt::SPACING_METERS;
        for already in 0..3 {
            let spawn = spawn_body(&world.state, rock(0), world.state.tick());
            let out = floor + Belt::SPACING_METERS * f64::from(already);
            assert!(
                spawn.pos.distance(home.pos + radial * out) < 1e-9,
                "unit {already} spawns at {spawn:?}"
            );
            assert_eq!(spawn.vel, home.vel);
            world.free(0, FRIGATE, rock(0), spawn);
        }
    }

    #[test]
    fn a_structure_at_the_rock_does_not_move_a_spawn() {
        let mut world = World::ring(Gravity::new(4.0e13), 1, &[TeamId(0)]);
        world.fix(0, FRIGATE, rock(0));
        let home = world.state.rock_body(rock(0));
        let floor = world.state[rock(0)].radius() + Belt::SPACING_METERS;

        let spawn = spawn_body(&world.state, rock(0), world.state.tick());

        assert!(spawn.pos.distance(home.pos) - floor < 1e-9);
    }

    #[test]
    #[ignore = "cost report: cargo test -p probe-sim --release -- --ignored --nocapture"]
    fn one_tick_of_a_crowded_rock_fits_the_budget() {
        for units in [100u32, 1000] {
            let mut world = World::started(&[TeamId(0), TeamId(1)]);
            let home = rock(0);
            for at in 0..units {
                let body = spawn_body(&world.state, home, world.state.tick());
                world.free((at % 2) as u8, FRIGATE, home, body);
            }
            let mut state = world.state;
            let over = 100;

            #[expect(
                clippy::disallowed_types,
                reason = "a test measuring wall time is not the sim reading a clock"
            )]
            let started = std::time::Instant::now();
            let quiet = Batch::new();
            for _ in 0..over {
                let (next, _) = state.step(&quiet);
                state = next;
            }
            let each = started.elapsed().as_secs_f64() / f64::from(over);

            println!(
                "{units} units at one rock: {:.3} ms a tick, against a budget of {:.3} ms",
                each * 1e3,
                Tick(1).seconds() * 1e3
            );
        }
    }
}
