use crate::ids::{EntityId, SeatId};
use crate::materials::Materials;
use crate::roster::Kind;
use crate::state::{Batch, Flight, Frame, Issued, Motion, Rejected, State};
use crate::step::construction::{Construction, Progress};
use crate::step::extraction::{Extraction, Income};
use crate::step::fire::{Fire, Shots};
use crate::step::fulfilment::{Assigned, Fulfilment};
use crate::step::maneuver::Maneuver;
use crate::step::propagation::{Moved, Propagation};

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
        let thrusts = Maneuver::of(snap).run();
        let moved = Propagation::of(snap, &thrusts).run();
        let filled = Fulfilment::of(snap).run();
        let income = Extraction::of(snap).run();
        let work = Construction::of(snap).run();
        let shots = Fire::of(snap).run();
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
            Motion::Free {
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
        for (row, schedule) in &send.schedules {
            let flight = Flight::new(send.source, *schedule);
            for member in send.members.iter().filter(|id| snap[**id].row() == *row) {
                join(next, *member, send.destination, flight);
            }
        }
    }
    let ids: Vec<EntityId> = next.entities().map(|entity| entity.id()).collect();
    for id in ids {
        let marked = filled.marked.binary_search(&id).is_ok();
        if let Some(entity) = next.entity_mut(id) {
            if marked {
                entity.mark_surplus();
            } else {
                entity.clear_surplus();
            }
        }
    }
    for opening in &filled.openings {
        for _ in 0..opening.count {
            next.add_frame(Frame::new(opening.post, opening.row, 0.0, snap.tick()));
        }
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
    for scrapping in &work.scrapping {
        let Some(entity) = next.entity_mut(scrapping.entity) else {
            continue;
        };
        entity.scrap(scrapping.units);
        if scrapping.completed {
            let (seat, row) = (entity.seat(), entity.row());
            refund(next, seat, snap[row].cost);
            next.remove_entity(scrapping.entity);
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

fn spawn(next: &mut State, post: crate::place::Post, row: crate::ids::RowId) {
    let motion = if next[row].kind() == Kind::Structure {
        Motion::Fixed
    } else {
        Motion::Free {
            body: Maneuver::spawn_body(next, post.place, next.tick().next()),
            flight: None,
        }
    };
    next.spawn(post.seat, row, post.place, motion);
}

fn join(next: &mut State, entity: EntityId, destination: crate::place::Place, flight: Flight) {
    let Some(target) = next.entity_mut(entity) else {
        return;
    };
    let Motion::Free { body, .. } = target.motion() else {
        return;
    };
    target.set_home(destination);
    target.set_motion(Motion::Free {
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
pub mod maneuver;
pub mod propagation;

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::belt::Belt;
    use crate::ids::{RockId, RowId, TeamId};
    use crate::materials::Material;
    use crate::place::{Band, Place, Post};
    use crate::real::Real;
    use crate::roster::{CONSTRUCTOR, FRIGATE, LANCER, SHIPYARD, STORAGE};
    use crate::roster::{Roster, Row};
    use crate::setup::Setup;
    use crate::state::view::View;
    use crate::state::{Command, MAX_WANT, Schedule, Seat};
    use crate::step::fire::Hit;
    use crate::time::Tick;
    use crate::{Materials, TICKS_PER_SECOND};

    const CLOCK: Tick = Tick(15 * 60 * TICKS_PER_SECOND as u64);

    fn start(teams: &[TeamId]) -> State {
        let setup = Setup::new(teams.to_vec(), 0, CLOCK).expect("a match of these teams");
        State::start(&setup)
    }

    fn inner(rock: u32) -> Place {
        Place {
            rock: RockId(rock),
            band: Band::Inner,
        }
    }

    fn post(seat: u8, place: Place) -> Post {
        Post {
            place,
            seat: SeatId(seat),
        }
    }

    fn want(seat: u8, place: Place, row: RowId, count: u32) -> Issued {
        numbered(seat, 0, place, row, count)
    }

    fn numbered(seat: u8, seq: u32, place: Place, row: RowId, count: u32) -> Issued {
        Issued {
            seat: SeatId(seat),
            seq,
            command: Command::Want { place, row, count },
        }
    }

    fn batch(issued: &[Issued]) -> Batch {
        let mut batch = Batch::new();
        for issued in issued {
            assert_eq!(batch.insert(*issued), Ok(()));
        }
        batch
    }

    fn tick(state: State, issued: &[Issued]) -> State {
        let (next, outcome) = state.step(&batch(issued));
        assert_eq!(outcome.rejected, Vec::new(), "the commands were rejected");
        next
    }

    fn refusal(state: &State, issued: Issued) -> Option<Rejected> {
        let (_, outcome) = state.step(&batch(&[issued]));
        outcome.rejected.first().map(|(_, why)| *why)
    }

    fn run(mut state: State, ticks: u64) -> State {
        for _ in 0..ticks {
            state = tick(state, &[]);
        }
        state
    }

    fn count(state: &State, seat: u8, place: Place, row: RowId) -> u32 {
        state.count(post(seat, place), row)
    }

    fn shot_at(state: &State, target: EntityId, ticks: u64) -> Option<Hit> {
        let mut state = state.clone();
        for _ in 0..ticks {
            let hit = Fire::of(&state)
                .run()
                .hits
                .into_iter()
                .find(|hit| hit.target == target);
            if hit.is_some() {
                return hit;
            }
            state = tick(state, &[]);
        }
        None
    }

    #[test]
    fn a_first_want_places_the_reserve_shipyard_at_once() {
        let state = start(&[TeamId(0)]);
        let seconds = TICKS_PER_SECOND as u64;

        let state = tick(state, &[want(0, inner(0), SHIPYARD, 1)]);

        assert_eq!(count(&state, 0, inner(0), SHIPYARD), 1);
        assert_eq!(state[SeatId(0)].reserved(SHIPYARD), 0);
        assert_eq!(state[SeatId(0)].reserved(CONSTRUCTOR), 1);
        assert_eq!(state.frames().len(), 0, "the reserve needs no frame");
        let shipyard = state.entities().next().expect("the shipyard");
        assert_eq!(shipyard.motion(), Motion::Fixed);
        assert_eq!(state.body_of(shipyard), state.rock_body(RockId(0)));

        let state = run(state, seconds);
        assert_eq!(
            state[SeatId(0)].stockpile().capacity(),
            state[SeatId(0)].base_capacity() + state[SHIPYARD].capacity
        );
    }

    #[test]
    fn a_frame_that_spends_nothing_for_a_second_names_the_material_it_wants() {
        let stocked = |stock| {
            State::new(
                CLOCK,
                0,
                Belt::GRAVITY,
                Roster::shipped(),
                Belt::fixed(Belt::GRAVITY),
                vec![Seat::new(TeamId(0), stock, BTreeMap::from([(SHIPYARD, 1)]))],
            )
        };

        let a_second_in = |stock| {
            let state = tick(stocked(stock), &[want(0, inner(0), SHIPYARD, 1)]);
            let state = tick(state, &[want(0, inner(0), LANCER, 1)]);
            let state = run(state, TICKS_PER_SECOND as u64 + 1);
            View::of(&state, SeatId(0), &Shots::default())
                .compositions
                .iter()
                .flat_map(|composition| &composition.rows)
                .find(|wanted| wanted.row == LANCER)
                .and_then(|wanted| wanted.frames.first().copied())
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
    fn a_shortfall_with_a_builder_opens_one_frame_and_completes_it() {
        let state = start(&[TeamId(0)]);
        let state = tick(state, &[want(0, inner(0), SHIPYARD, 1)]);
        let stock = state[SeatId(0)].stockpile().stock();

        let state = tick(state, &[want(0, inner(0), STORAGE, 1)]);
        assert_eq!(state.frames().len(), 1);
        let state = run(state, 10);
        assert_eq!(state.frames().len(), 1, "a shortfall opens one frame only");
        assert!(state.frames()[0].progress() > 0.0);

        let cost = state[STORAGE].cost;
        let seconds = cost.total() / 15.0;
        let state = run(
            state,
            (seconds * f64::from(TICKS_PER_SECOND)).ceil() as u64 + 2,
        );

        assert_eq!(count(&state, 0, inner(0), STORAGE), 1);
        assert_eq!(state.frames().len(), 0);
        let spent = stock - state[SeatId(0)].stockpile().stock();
        assert!(
            (spent.total() - cost.total()).abs() < 1e-6,
            "spent {spent:?} on a {cost:?} extractor"
        );
    }

    #[test]
    fn a_surplus_flies_to_the_nearest_shortfall_and_arrives_on_its_anchors_orbit() {
        let state = start(&[TeamId(0)]);
        let state = tick(state, &[want(0, inner(0), CONSTRUCTOR, 1)]);
        assert_eq!(count(&state, 0, inner(0), CONSTRUCTOR), 1);

        let state = tick(
            state,
            &[
                numbered(0, 0, inner(0), CONSTRUCTOR, 0),
                numbered(0, 1, inner(1), CONSTRUCTOR, 1),
            ],
        );

        assert_eq!(count(&state, 0, inner(1), CONSTRUCTOR), 1, "it counts home");
        assert_eq!(count(&state, 0, inner(0), CONSTRUCTOR), 0);
        assert_eq!(state.frames().len(), 0, "a send fills the shortfall");
        let unit = state.entities().next().expect("the constructor").id();
        let arrive = state[unit].flight().expect("a flight").arrive();
        let flying = arrive.0 - state.tick().0;

        let state = run(state, flying);

        assert!(
            !state[unit].is_flying(),
            "it is still flying at its arrival"
        );
        let anchor = state.anchor(inner(1)).at(state.tick(), state.gravity());
        let body = state.body_of(&state[unit]);
        let off = body.pos.distance(anchor.pos);
        let drift = body.vel.distance(anchor.vel);
        assert!(
            off <= Schedule::ARRIVAL_POSITION_METERS,
            "it arrived {off} meters off"
        );
        assert!(
            drift <= Schedule::ARRIVAL_SPEED_METERS_PER_SECOND,
            "it arrived {drift} meters per second across"
        );
    }

    #[test]
    fn a_send_no_row_can_fly_leaves_its_units_home_and_opens_frames() {
        let mut roster = Roster::shipped();
        let crawler = roster.add(Row {
            name: "crawler",
            accel: Real(1e-6),
            maneuver: Real(2.5e-7),
            ..roster[CONSTRUCTOR].clone()
        });
        let state = State::new(
            CLOCK,
            0,
            Belt::GRAVITY,
            roster,
            Belt::fixed(Belt::GRAVITY),
            vec![Seat::new(
                TeamId(0),
                Materials::new(1e4, 1e4, 1e4),
                BTreeMap::from([(crawler, 1)]),
            )],
        );
        let state = tick(state, &[want(0, inner(0), crawler, 1)]);
        let unit = state.entities().next().expect("the crawler").id();

        let state = tick(
            state,
            &[
                numbered(0, 0, inner(0), crawler, 0),
                numbered(0, 1, inner(1), crawler, 1),
            ],
        );

        assert!(
            !state[unit].is_flying(),
            "a crawler that cannot fly was sent"
        );
        assert_eq!(state[unit].home(), inner(0), "it left home");
        assert_eq!(count(&state, 0, inner(1), crawler), 0);
        assert_eq!(state.frames().len(), 1, "the shortfall opened no frame");
        assert!(
            !state[unit].is_surplus(),
            "it was scrapped while a shortfall wants it"
        );
    }

    #[test]
    fn a_send_arrives_at_the_earliest_tick_every_rows_schedule_exists() {
        let state = start(&[TeamId(0)]);
        let state = tick(state, &[want(0, inner(0), CONSTRUCTOR, 1)]);
        let depart = state.tick().next();

        let state = tick(
            state,
            &[
                numbered(0, 0, inner(0), CONSTRUCTOR, 0),
                numbered(0, 1, inner(1), CONSTRUCTOR, 1),
            ],
        );

        let unit = state.entities().next().expect("the constructor").id();
        let arrive = state[unit].flight().expect("a flight").arrive();
        let gravity = state.gravity();
        let source = state.anchor(inner(0)).at(depart, gravity);
        let limit = state[CONSTRUCTOR].accel.0;
        let second = u64::from(TICKS_PER_SECOND);
        assert!(arrive.0 - depart.0 > second, "the first candidate answered");
        for step in (second..arrive.0 - depart.0).step_by(second as usize) {
            let earlier = Tick(depart.0 + step);
            let target = state.anchor(inner(1)).at(earlier, gravity);
            assert_eq!(
                Schedule::between(source, target, depart, earlier, limit, gravity),
                None,
                "arriving at {earlier:?} would have fitted"
            );
        }
    }

    #[test]
    fn a_surplus_with_no_shortfall_is_scrapped_and_refunded() {
        let state = start(&[TeamId(0)]);
        let state = tick(
            state,
            &[
                numbered(0, 0, inner(0), SHIPYARD, 1),
                numbered(0, 1, inner(0), CONSTRUCTOR, 1),
            ],
        );
        let stock = state[SeatId(0)].stockpile().stock();
        let unit = state
            .entities()
            .find(|entity| entity.row() == CONSTRUCTOR)
            .expect("the constructor")
            .id();

        let state = tick(state, &[want(0, inner(0), CONSTRUCTOR, 0)]);
        assert!(state[unit].is_surplus(), "it was not marked");

        let state = run(state, 4 * u64::from(TICKS_PER_SECOND));

        assert_eq!(state.entity(unit), None, "it was not scrapped");
        let refunded = state[SeatId(0)].stockpile().stock() - stock;
        let cost = state[CONSTRUCTOR].cost;
        assert!(
            (refunded.total() - cost.total()).abs() < 1e-6,
            "refunded {refunded:?} of a {cost:?} constructor"
        );
    }

    #[test]
    fn a_want_that_comes_back_lifts_the_surplus_mark() {
        let state = start(&[TeamId(0)]);
        let state = tick(state, &[want(0, inner(0), CONSTRUCTOR, 1)]);
        let unit = state.entities().next().expect("the constructor").id();
        let state = tick(state, &[want(0, inner(0), CONSTRUCTOR, 0)]);
        assert!(state[unit].is_surplus());

        let state = tick(state, &[want(0, inner(0), CONSTRUCTOR, 1)]);

        assert!(!state[unit].is_surplus());
    }

    #[test]
    fn an_armed_unit_kills_an_unarmed_enemy_at_its_rock() {
        let mut state = start(&[TeamId(0), TeamId(1)]);

        let prey = state.spawn(SeatId(1), STORAGE, inner(0), Motion::Fixed);
        let state = tick(
            state,
            &[
                numbered(0, 0, inner(0), SHIPYARD, 1),
                numbered(0, 1, inner(0), FRIGATE, 1),
            ],
        );

        let building = state[FRIGATE].cost.total() / 15.0;
        let state = run(state, (building * f64::from(TICKS_PER_SECOND)) as u64 + 2);
        assert_eq!(count(&state, 0, inner(0), FRIGATE), 1);

        let interval = u64::from(TICKS_PER_SECOND) / 2;
        let hit = shot_at(&state, prey, interval + 1).expect("the frigate fired");
        assert_eq!(hit.damage, 6.0, "six damage through no plating");

        let shots = (state[prey].hp() / hit.damage).ceil();
        let early = run(
            state,
            (shots / 2.0 * f64::from(TICKS_PER_SECOND)) as u64 - interval,
        );
        assert!(early.entity(prey).is_some(), "it died too soon");
        let late = run(early, 2 * interval);
        assert_eq!(late.entity(prey), None, "it outlived its hit points");
        assert!(
            !late.ready().iter().any(|ready| ready.entity() == prey),
            "a dead entity kept its weapons"
        );
    }

    #[test]
    fn fire_never_targets_a_flying_unit() {
        let state = start(&[TeamId(0), TeamId(1)]);
        let state = tick(
            state,
            &[
                numbered(0, 0, inner(0), SHIPYARD, 1),
                numbered(0, 1, inner(0), FRIGATE, 1),
                want(1, inner(0), CONSTRUCTOR, 1),
            ],
        );
        let building = state[FRIGATE].cost.total() / 15.0;
        let state = run(state, (building * f64::from(TICKS_PER_SECOND)) as u64 + 2);
        let prey = state
            .entities()
            .find(|entity| entity.seat() == SeatId(1))
            .expect("the enemy constructor")
            .id();
        let interval = u64::from(TICKS_PER_SECOND) / 2;
        assert!(
            shot_at(&state, prey, interval + 1).is_some(),
            "the enemy was not a target while holding"
        );

        let state = tick(
            state,
            &[
                numbered(1, 1, inner(0), CONSTRUCTOR, 0),
                numbered(1, 2, inner(1), CONSTRUCTOR, 1),
            ],
        );

        assert!(state[prey].is_flying());
        assert_eq!(
            shot_at(&state, prey, interval + 1),
            None,
            "a flying unit was fired on"
        );
    }

    #[test]
    fn a_seat_with_no_entity_and_an_empty_reserve_is_eliminated() {
        let rocks = Belt::fixed(Belt::GRAVITY);
        let seats = vec![
            Seat::new(TeamId(0), Materials::ZERO, BTreeMap::new()),
            Seat::new(TeamId(1), Materials::ZERO, BTreeMap::from([(SHIPYARD, 1)])),
        ];
        let state = State::new(CLOCK, 0, Belt::GRAVITY, Roster::shipped(), rocks, seats);

        let state = tick(state, &[]);

        assert!(!state[SeatId(0)].alive());
        assert!(state[SeatId(1)].alive());
        assert_eq!(
            refusal(&state, want(0, inner(0), SHIPYARD, 1)),
            Some(Rejected::DeadSeat)
        );
    }

    #[test]
    fn a_want_the_state_cannot_take_is_rejected_by_name_and_changes_nothing() {
        let state = start(&[TeamId(0)]);
        let outer = Place {
            rock: RockId(0),
            band: Band::Outer,
        };

        let refusals = [
            (want(9, inner(0), SHIPYARD, 1), Rejected::NoSuchSeat),
            (want(0, inner(99), SHIPYARD, 1), Rejected::NoSuchRock),
            (want(0, inner(0), RowId(u16::MAX), 1), Rejected::NoSuchRow),
            (want(0, outer, SHIPYARD, 1), Rejected::StructureOutside),
            (want(0, inner(0), FRIGATE, MAX_WANT + 1), Rejected::TooMany),
        ];

        for (issued, why) in refusals {
            assert_eq!(refusal(&state, issued), Some(why), "{issued:?}");
            let (next, _) = state.step(&batch(&[issued]));
            assert_eq!(next.posts().count(), 0, "{issued:?} left a want behind");
        }
        assert_eq!(refusal(&state, want(0, inner(0), FRIGATE, MAX_WANT)), None);
    }

    #[test]
    fn a_tick_lands_the_same_state_and_hash_however_its_commands_arrived() {
        let state = start(&[TeamId(0), TeamId(1)]);

        let issued = [
            numbered(0, 0, inner(0), FRIGATE, 3),
            numbered(0, 1, inner(0), FRIGATE, 7),
            numbered(1, 0, inner(2), CONSTRUCTOR, 1),
            numbered(0, 2, inner(1), SHIPYARD, 1),
        ];

        let (ordered, _) = state.step(&batch(&issued));
        let mut scrambled = issued;
        scrambled.reverse();
        let (arrived, _) = state.step(&batch(&scrambled));

        assert_eq!(ordered.hash(), arrived.hash());
        assert_eq!(ordered, arrived);
        assert_eq!(
            ordered
                .wants(post(0, inner(0)))
                .map(|wants| wants.get(FRIGATE)),
            Some(7),
            "the seat's later command is the one that stands"
        );
    }

    #[test]
    #[ignore = "cost report: cargo test -p probe-sim --release -- --ignored --nocapture"]
    fn one_tick_of_a_full_belt_fits_the_budget() {
        let mut state = start(&[TeamId(0), TeamId(1)]);
        for at in 0..100u32 {
            let place = inner(at % 21);
            let seat = SeatId((at / 21 % 2) as u8);
            let motion = Motion::Free {
                body: Maneuver::spawn_body(&state, place, state.tick()),
                flight: None,
            };
            state.spawn(seat, FRIGATE, place, motion);
        }
        let entities = state.entities().count();
        let rocks = state.rocks().len();
        let over = 200;

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
            "{entities} entities on {rocks} rocks: {:.3} ms a tick, against a budget of {:.3} ms",
            each * 1e3,
            Tick(1).seconds() * 1e3
        );
    }
}
