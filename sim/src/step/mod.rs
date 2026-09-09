use crate::ids::SeatId;
use crate::state::{Batch, Frame, Issued, Rejected, Rolls, State};
use crate::step::construction::{Construction, Progress};
use crate::step::extraction::Income;
use crate::step::fire::{Fire, Shots};
use crate::step::fulfilment::{Assigned, Fulfilment};
use crate::step::holding::{Holding, Steering};
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
        applied.close_draft();
        if applied.drafting() {
            applied.advance();
            return (
                applied,
                Outcome {
                    rejected,
                    shots: Shots::default(),
                },
            );
        }
        let snap = &applied;
        let rolls = Rolls::called(snap);
        let shots = Fire::of(snap, &rolls).run();
        let steering = Holding::of(snap, &rolls, &shots).run();
        let moved = Propagation::of(snap, &steering.thrusts).run();
        let filled = Fulfilment::of(snap).run();
        let income = Income::extracted(snap, &rolls);
        let work = Construction::of(snap, &rolls).run();
        let next = State::next(snap, &moved, &steering, &filled, &income, &work, &shots);
        (next, Outcome { rejected, shots })
    }

    fn next(
        snap: &State,
        moved: &Moved,
        steering: &Steering,
        filled: &Assigned,
        income: &Income,
        work: &Progress,
        shots: &Shots,
    ) -> State {
        let mut next = snap.clone();
        let mut closing = Vec::new();
        move_bodies(&mut next, moved);
        steering.set_passes(&mut next);
        fulfil(&mut next, snap, filled, &mut closing);
        income.apply(&mut next);
        build(&mut next, snap, work, &mut closing);
        resolve(&mut next, shots);
        next.close_frames(&closing);
        next.reap();
        eliminate(&mut next);
        next.refresh_capacities();
        next.advance();
        next
    }
}

fn move_bodies(next: &mut State, moved: &Moved) {
    for step in moved.iter() {
        next.steer(step.entity, step.body);
    }
    for arrival in moved.arrived() {
        next.arrive(*arrival);
    }
}

fn fulfil(next: &mut State, snap: &State, filled: &Assigned, closing: &mut Vec<usize>) {
    for placement in &filled.placements {
        next.place_from_reserve(placement.post(), placement.row());
    }
    for (entity, destination) in &filled.sent_to {
        next.re_home(*entity, *destination);
    }
    for opening in &filled.openings {
        next.add_frame(Frame::new(opening.post(), opening.row(), 0.0, snap.time()));
    }
    for cancellation in &filled.cancellations {
        next[cancellation.posting.seat()].refund(cancellation.refund(snap));
        closing.push(cancellation.frame);
    }
}

fn build(next: &mut State, snap: &State, work: &Progress, closing: &mut Vec<usize>) {
    for spend in &work.spends {
        let frame = &snap.frames()[spend.frame];
        if let Some(seat) = next.seat_mut(frame.post().seat) {
            seat.drain(spend.materials);
        }
        if let Some(open) = next.frame_mut(spend.frame) {
            match spend.materials.total() > 0.0 {
                true => open.build(spend.materials.total(), snap.time()),
                false => open.short_of(spend.short),
            }
        }
        if spend.completed {
            next.spawn_at(frame.post(), frame.row());
            closing.push(spend.frame);
        }
    }
    for repair in &work.repairs {
        next.heal(repair.entity, repair.hp);
    }
}

fn resolve(next: &mut State, shots: &Shots) {
    for (target, damage) in shots.damage() {
        next.entities.hurt(target, damage);
    }
    for ready in &shots.ready {
        next.set_ready(ready.entity(), ready.weapon(), ready.at(), ready.kept());
    }
}

fn eliminate(next: &mut State) {
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

pub(crate) mod construction;
pub(crate) mod extraction;
pub mod fire;
pub(crate) mod fulfilment;
pub(crate) mod holding;
pub(crate) mod propagation;

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::belt::Belt;
    use crate::fixture::World;
    use crate::ids::{AsteroidId, EntityId, RowId, TeamId};
    use crate::materials::Material;
    use crate::orbit::body::{Body, Gravity};
    use crate::post::Post;
    use crate::posting::Posting;
    use crate::roster::Roster;
    use crate::roster::{
        CONSTRUCTOR, FRIGATE, LANCER, METALS_EXTRACTOR, RAIDER, SHIPYARD, STORAGE,
    };
    use crate::state::{Entity, MAX_WANT, Ready, Seat};
    use crate::time::Time;
    use crate::transfer::Transfer;
    use crate::{Materials, TICKS_PER_SECOND};

    fn asteroid(at: u32) -> AsteroidId {
        AsteroidId(at)
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

        world.tick(&[Issued::want(0, asteroid(0), SHIPYARD, 1)]);

        assert_eq!(world.count(0, asteroid(0), SHIPYARD), 1);
        let state = &world.state;
        assert_eq!(state[SeatId(0)].reserved(SHIPYARD), 0);
        assert_eq!(state[SeatId(0)].reserved(CONSTRUCTOR), 1);
        assert_eq!(state.frames().len(), 0, "the reserve needs no frame");
        let shipyard = state.entities().next().expect("the shipyard");
        assert_eq!(shipyard.steered(), None);
        assert_eq!(state.body_of(shipyard), state.asteroid_body(AsteroidId(0)));

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
            world.tick(&[Issued::want(0, asteroid(0), SHIPYARD, 1)]);
            world.tick(&[Issued::want(0, asteroid(0), LANCER, 1)]);
            world.run(TICKS_PER_SECOND as u64 + 1);
            world
                .view(0)
                .plan_of(Posting::of(asteroid(0), SeatId(0), LANCER))
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
        world.tick(&[Issued::want(0, asteroid(0), SHIPYARD, 1)]);
        let stock = world.state[SeatId(0)].stockpile().stock();

        world.tick(&[Issued::want(0, asteroid(0), STORAGE, 1)]);
        assert_eq!(world.state.frames().len(), 1);
        world.run(10);
        assert_eq!(world.state.frames().len(), 1);
        assert!(world.state.frames()[0].progress() > 0.0);

        let cost = world.state[STORAGE].cost;
        let seconds = cost.total() / 15.0;
        world.run((seconds * f64::from(TICKS_PER_SECOND)).ceil() as u64 + 2);

        assert_eq!(world.count(0, asteroid(0), STORAGE), 1);
        assert_eq!(world.state.frames().len(), 0);
        let spent = stock - world.state[SeatId(0)].stockpile().stock();
        assert!(
            (spent.total() - cost.total()).abs() < 1e-6,
            "spent {spent:?} on a {cost:?} extractor"
        );
    }

    #[test]
    fn a_builder_reaches_the_frames_at_its_own_asteroid_and_no_others() {
        let mut world = World::started(&[TeamId(0)]);
        world.tick(&[Issued::want(0, asteroid(0), SHIPYARD, 1)]);

        world.tick(&[
            Issued::numbered(0, 0, asteroid(0), STORAGE, 1),
            Issued::numbered(0, 1, asteroid(5), STORAGE, 1),
        ]);
        world.run(u64::from(TICKS_PER_SECOND));

        assert!(
            world.progress(0, asteroid(0)) > 0.0,
            "the builder built nothing"
        );
        assert_eq!(
            world.progress(0, asteroid(5)),
            0.0,
            "a builder reached a frame an asteroid away"
        );
    }

    #[test]
    fn a_surplus_of_two_rows_fills_the_nearest_shortfall_by_one_send() {
        let mut world = stocked(Roster::shipped(), BTreeMap::from([(STORAGE, 1)]));
        world.hold(0, CONSTRUCTOR, asteroid(0), 5.0);
        world.hold(0, RAIDER, asteroid(0), 6.0);
        assert_eq!(world.count(0, asteroid(0), CONSTRUCTOR), 1);
        assert_eq!(world.count(0, asteroid(0), RAIDER), 1);

        world.tick(&[
            Issued::numbered(0, 0, asteroid(0), CONSTRUCTOR, 0),
            Issued::numbered(0, 1, asteroid(0), RAIDER, 0),
            Issued::numbered(0, 2, asteroid(1), CONSTRUCTOR, 1),
            Issued::numbered(0, 3, asteroid(1), RAIDER, 1),
        ]);

        assert_eq!(
            world.count(0, asteroid(1), CONSTRUCTOR),
            1,
            "it counts home"
        );
        assert_eq!(world.count(0, asteroid(1), RAIDER), 1, "it counts home");
        assert_eq!(world.count(0, asteroid(0), CONSTRUCTOR), 0);
        assert_eq!(world.state.frames().len(), 0, "a send fills the shortfall");
        assert!(
            world.state.entities().all(Entity::is_flying),
            "a re-homed unit waits before it flies"
        );
    }

    #[test]
    fn a_re_homed_unit_flies_from_that_tick_and_lands_in_its_destinations_zone() {
        let mut world = stocked(Roster::shipped(), BTreeMap::from([(CONSTRUCTOR, 1)]));
        world.tick(&[Issued::want(0, asteroid(0), CONSTRUCTOR, 1)]);
        let unit = world.state.entities().next().expect("the constructor").id();

        world.tick(&[
            Issued::numbered(0, 0, asteroid(0), CONSTRUCTOR, 0),
            Issued::numbered(0, 1, asteroid(1), CONSTRUCTOR, 1),
        ]);

        assert!(world.state.entity(unit).is_flying(), "it is still home");
        assert_eq!(world.state.entity(unit).home(), asteroid(1));
        assert_eq!(
            world.count(0, asteroid(1), CONSTRUCTOR),
            1,
            "it counts toward its destination from the tick it is re-homed"
        );

        for _ in 0..600 * u64::from(TICKS_PER_SECOND) {
            if !world.state.entity(unit).is_flying() {
                break;
            }
            world.tick(&[]);
        }

        assert_eq!(world.state.entity(unit).standing(), Some(asteroid(1)));
        let rim = Belt::ZONE_RADIUS_METERS + Transfer::ARRIVAL_POSITION_METERS;
        let landed = world.off_asteroid(unit, asteroid(1));
        assert!(
            landed <= rim,
            "it arrived {landed} off, past the zone's rim"
        );

        world.run(60 * u64::from(TICKS_PER_SECOND));

        let held = world.off_asteroid(unit, asteroid(1));
        assert!(held <= rim, "it holds {held} meters off its asteroid");
    }

    #[test]
    fn a_surplus_unit_with_no_shortfall_is_never_scrapped() {
        let mut world = World::started(&[TeamId(0)]);
        world.tick(&[Issued::want(0, asteroid(0), SHIPYARD, 1)]);
        let unit = world.hold(0, CONSTRUCTOR, asteroid(0), 5.0);
        let stock = world.state[SeatId(0)].stockpile().stock();

        world.run(8 * u64::from(TICKS_PER_SECOND));

        let state = &world.state;
        assert!(world.still_holds(unit), "the surplus was scrapped");
        assert_eq!(
            state.entity(unit).home(),
            asteroid(0),
            "it left the asteroid it stands on"
        );
        assert_eq!(state.entity(unit).hp(), state[CONSTRUCTOR].hp.0);
        let gained = state[SeatId(0)].stockpile().stock() - stock;
        assert!(
            gained.total() < state[CONSTRUCTOR].cost.total(),
            "a whole cost came back: {gained:?}"
        );
    }

    fn a_unit_and_a_frame_of_one_row_at_one_asteroid() -> (World, EntityId) {
        let mut world = stocked(Roster::shipped(), BTreeMap::new());
        world.fix(0, SHIPYARD, asteroid(0));
        let unit = world.hold(0, CONSTRUCTOR, asteroid(0), 5.0);
        world.tick(&[Issued::want(0, asteroid(0), CONSTRUCTOR, 2)]);
        world.run(20);
        assert_eq!(world.frames(0, asteroid(0), CONSTRUCTOR), 1);
        (world, unit)
    }

    #[test]
    fn lowering_a_want_a_shortfall_elsewhere_wants_sends_the_unit_and_keeps_the_frame() {
        let (mut world, unit) = a_unit_and_a_frame_of_one_row_at_one_asteroid();
        let progress = world.progress(0, asteroid(0));

        world.tick(&[
            Issued::numbered(0, 0, asteroid(0), CONSTRUCTOR, 1),
            Issued::numbered(0, 1, asteroid(1), CONSTRUCTOR, 1),
        ]);

        assert_eq!(
            world.state.entity(unit).home(),
            asteroid(1),
            "the unit stayed home"
        );
        let kept = world.frames(0, asteroid(0), CONSTRUCTOR);
        assert_eq!(kept, 1, "the send cancelled a frame the want covers");
        let building = world.progress(0, asteroid(0)) > progress;
        assert!(building, "the frame stopped building");
        let opened = world.frames(0, asteroid(1), CONSTRUCTOR);
        assert_eq!(opened, 0, "the send opened a frame it fills itself");
    }

    #[test]
    fn lowering_a_want_nothing_else_wants_cancels_the_frame_and_keeps_the_unit() {
        let (mut world, unit) = a_unit_and_a_frame_of_one_row_at_one_asteroid();

        world.tick(&[Issued::want(0, asteroid(0), CONSTRUCTOR, 1)]);

        assert_eq!(
            world.state.entity(unit).home(),
            asteroid(0),
            "the unit left"
        );
        let left = world.frames(0, asteroid(0), CONSTRUCTOR);
        assert_eq!(left, 0, "a frame nothing wants kept building");
    }

    #[test]
    fn a_post_opens_one_frame_of_a_row_at_a_time_and_rows_build_in_parallel() {
        let mut world = World::started(&[TeamId(0)]);
        world.tick(&[Issued::want(0, asteroid(0), SHIPYARD, 1)]);

        world.tick(&[
            Issued::numbered(0, 0, asteroid(0), STORAGE, 3),
            Issued::numbered(0, 1, asteroid(0), METALS_EXTRACTOR, 2),
        ]);

        let open = |world: &World, row| world.frames(0, asteroid(0), row);
        assert_eq!(open(&world, STORAGE), 1, "a shortfall of three opened more");
        assert_eq!(open(&world, METALS_EXTRACTOR), 1, "rows build in parallel");

        for _ in 0..30 * u64::from(TICKS_PER_SECOND) {
            if world.count(0, asteroid(0), STORAGE) == 1 {
                break;
            }
            world.tick(&[]);
        }
        assert_eq!(world.count(0, asteroid(0), STORAGE), 1, "none completed");
        assert_eq!(open(&world, STORAGE), 0, "its frame closed on completion");

        world.tick(&[]);

        assert_eq!(open(&world, STORAGE), 1, "the next frame opened");
        assert_eq!(world.count(0, asteroid(0), STORAGE), 1, "and only the next");
    }

    #[test]
    fn an_armed_unit_kills_an_unarmed_enemy_at_its_asteroid() {
        let mut world = World::started(&[TeamId(0), TeamId(1)]);

        world.tick(&[
            Issued::numbered(0, 0, asteroid(0), SHIPYARD, 1),
            Issued::numbered(0, 1, asteroid(0), FRIGATE, 1),
        ]);
        let prey = world.fix(1, STORAGE, asteroid(0));

        let building = world.state[FRIGATE].cost.total() / 15.0;
        world.run((building * f64::from(TICKS_PER_SECOND)) as u64 + 2);
        assert_eq!(world.count(0, asteroid(0), FRIGATE), 1);

        let interval = u64::from(TICKS_PER_SECOND) / 2;
        let hit = world
            .shot_at(prey, interval + 1)
            .expect("the frigate fired");
        assert_eq!(hit.damage, 6.0, "six damage through no plating");

        let shots = (world.state.entity(prey).hp() / hit.damage).ceil() as u64;
        let firing = shots * interval;
        world.run(firing - interval);
        assert!(
            world.still_holds(prey),
            "it died faster than a frigate fires"
        );

        for _ in 0..firing {
            if !world.still_holds(prey) {
                break;
            }
            world.tick(&[]);
        }

        assert!(
            !world.still_holds(prey),
            "it outlived its hit points: the kill took more than twice the {firing} ticks \
             a frigate's rate alone needs for {shots} shots"
        );
        assert!(
            !world.state.ready().any(|ready| ready.entity() == prey),
            "a dead entity kept its weapons"
        );
    }

    #[test]
    fn a_weapon_fires_at_every_enemy_in_range_at_its_asteroid() {
        let mut world = World::started(&[TeamId(0), TeamId(1)]);
        let reach = world.state[LANCER].max_damage_range();
        let shooter = world.hold(0, LANCER, asteroid(0), 0.0);
        let near = world.hold(1, STORAGE, asteroid(0), reach - 1.0);
        let far = world.hold(1, STORAGE, asteroid(0), reach + 1.0);

        let hits = world.shots().hits;

        assert_eq!(hits.len(), 1, "one weapon fires once a tick");
        assert_eq!(hits[0].shooter, shooter);
        assert_eq!(hits[0].target, near);
        assert!(
            world.shot_at(far, u64::from(TICKS_PER_SECOND)).is_none(),
            "an enemy beyond the weapon's range was fired on"
        );
    }

    fn kept_by(world: &World, shooter: EntityId) -> Option<EntityId> {
        world
            .state
            .ready()
            .find(|ready| ready.entity() == shooter)
            .and_then(Ready::kept)
    }

    #[test]
    fn a_weapon_keeps_its_target_from_one_tick_to_the_next_until_it_dies() {
        let mut world = World::started(&[TeamId(0), TeamId(1)]);
        let shooter = world.hold(0, LANCER, asteroid(0), 0.0);
        let first = world.hold(1, STORAGE, asteroid(0), 2.0);
        let second = world.hold(1, STORAGE, asteroid(0), 3.0);

        let held: Vec<EntityId> = (0..5)
            .filter_map(|_| {
                world.run(u64::from(TICKS_PER_SECOND));
                kept_by(&world, shooter)
            })
            .collect();

        let kept = held.first().copied().expect("the lancer fired");
        assert_eq!(held.len(), 5, "the lancer let its target go: {held:?}");
        assert!(
            held.iter().all(|target| *target == kept),
            "it wandered off its target: {held:?}"
        );
        let other = if kept == first { second } else { first };
        assert!(world.still_holds(other), "it split its fire");

        let mut ticks = 0;
        while world.still_holds(kept) && ticks < 120 * u64::from(TICKS_PER_SECOND) {
            world.tick(&[]);
            ticks += 1;
        }
        assert!(!world.still_holds(kept), "its target outlived the test");
        assert_eq!(
            kept_by(&world, shooter),
            None,
            "the keep outlived the target the reap took"
        );

        world.run(2 * u64::from(TICKS_PER_SECOND));

        assert_eq!(
            kept_by(&world, shooter),
            Some(other),
            "it did not take the survivor once its target died"
        );
    }

    #[test]
    fn a_weapon_lets_a_target_go_the_tick_it_leaves_the_weapons_range() {
        let mut world = World::started(&[TeamId(0), TeamId(1)]);
        let range = world.state[LANCER].max_damage_range();
        let shooter = world.hold(0, LANCER, asteroid(0), 0.0);
        let strays = world.hold(1, CONSTRUCTOR, asteroid(0), range - 1.0);
        world.run(u64::from(TICKS_PER_SECOND) + 1);
        assert_eq!(kept_by(&world, shooter), Some(strays));

        let body = world.state.body_of(world.state.entity(strays));
        let away = world.state.asteroid_body(asteroid(0));
        let radial = away.pos.normalized().expect("a radius");
        world.state.steer(
            strays,
            Body::new(away.pos + radial * (range + 5.0), body.vel),
        );
        world.tick(&[]);

        assert_eq!(
            kept_by(&world, shooter),
            None,
            "the weapon kept a target its range no longer covers"
        );
    }

    fn two_lancers_over_one_dying_store() -> (World, [EntityId; 2], [EntityId; 2]) {
        let mut world = World::started(&[TeamId(0), TeamId(1)]);
        let sooner = world.hold(0, LANCER, asteroid(0), 0.0);
        let later = world.hold(0, LANCER, asteroid(0), 0.5);
        let doomed = world.hold(1, STORAGE, asteroid(0), 1.0);
        let spared = world.hold(1, STORAGE, asteroid(0), 1.5);
        let hp = world.state.entity(doomed).hp();
        world.state.entities.hurt(doomed, hp - 1.0);
        assert!(sooner < later, "the shooters were spawned out of id order");
        (world, [sooner, later], [doomed, spared])
    }

    #[test]
    fn shots_resolve_in_ready_time_order_then_by_shooter_id() {
        let (world, [sooner, later], [doomed, spared]) = two_lancers_over_one_dying_store();

        let tied = world.shots().hits;

        assert_eq!(tied.len(), 2, "both lancers fired once: {tied:?}");
        assert_eq!(
            (tied[0].shooter, tied[0].target),
            (sooner, doomed),
            "the shooters tied on their ready instant and the higher id took the kill"
        );
        assert_eq!(
            (tied[1].shooter, tied[1].target),
            (later, spared),
            "the shooter that fired second was not spread off the target already dead"
        );

        let (mut world, [sooner, later], [doomed, spared]) = two_lancers_over_one_dying_store();
        let at = world
            .state
            .ready()
            .find(|ready| ready.entity() == later)
            .expect("an armed lancer carries its ready instant")
            .at();
        world.state.set_ready(later, 0, at.after(-1.0), None);

        let staggered = world.shots().hits;

        assert_eq!(staggered.len(), 2, "both lancers fired once: {staggered:?}");
        assert_eq!(
            (staggered[0].shooter, staggered[0].target),
            (later, doomed),
            "the lancer ready a second sooner did not fire first"
        );
        assert_eq!(
            (staggered[1].shooter, staggered[1].target),
            (sooner, spared),
            "the lancer ready later was not spread off the target already dead"
        );
    }

    #[test]
    fn a_force_spreads_its_fire_past_a_target_this_ticks_damage_already_kills() {
        let mut world = World::started(&[TeamId(0), TeamId(1)]);
        let range = world.state[LANCER].max_damage_range();
        for at in 0..6 {
            world.hold(0, LANCER, asteroid(0), f64::from(at) * 0.1);
        }
        let near = world.hold(1, CONSTRUCTOR, asteroid(0), range - 2.0);
        let far = world.hold(1, CONSTRUCTOR, asteroid(0), range - 1.0);
        let hp = world.state.entity(near).hp();

        let hits = world.shots().hits;

        let dealt: Vec<f64> = hits
            .iter()
            .filter(|hit| hit.target == near)
            .map(|hit| hit.damage)
            .collect();
        let last = dealt
            .last()
            .copied()
            .expect("the force fired at its target");
        let total: f64 = dealt.iter().sum();
        assert!(
            total >= hp,
            "the force left {near:?} alive, dealing {total} of {hp}"
        );
        assert!(
            total - last < hp,
            "a shot landed on {near:?} after this tick's damage already killed it"
        );
        assert!(
            hits.iter().any(|hit| hit.target == far),
            "every shot piled on one target"
        );
    }

    #[test]
    fn fire_never_targets_a_flying_unit() {
        let mut world = World::started(&[TeamId(0), TeamId(1)]);
        world.tick(&[
            Issued::numbered(0, 0, asteroid(0), SHIPYARD, 1),
            Issued::numbered(0, 1, asteroid(0), FRIGATE, 1),
        ]);
        world.draft(1, asteroid(5));
        let prey = world.hold(1, CONSTRUCTOR, asteroid(0), 5.0);
        let building = world.state[FRIGATE].cost.total() / 15.0;
        world.run((building * f64::from(TICKS_PER_SECOND)) as u64 + 2);
        let interval = u64::from(TICKS_PER_SECOND) / 2;
        assert!(
            world.shot_at(prey, interval + 1).is_some(),
            "the enemy was not a target while holding"
        );

        world.tick(&[
            Issued::numbered(1, 1, asteroid(0), CONSTRUCTOR, 0),
            Issued::numbered(1, 2, asteroid(1), CONSTRUCTOR, 1),
        ]);

        assert!(world.state.entity(prey).is_flying());
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
            world.refusal(Issued::want(0, asteroid(0), SHIPYARD, 1)),
            Some(Rejected::DeadSeat)
        );
    }

    #[test]
    fn a_want_the_state_cannot_take_is_rejected_by_name_and_changes_nothing() {
        let world = World::started(&[TeamId(0)]);

        let refusals = [
            (
                Issued::want(9, asteroid(0), SHIPYARD, 1),
                Rejected::NoSuchSeat,
            ),
            (
                Issued::want(0, asteroid(u32::MAX), SHIPYARD, 1),
                Rejected::NoSuchAsteroid,
            ),
            (
                Issued::want(0, asteroid(0), RowId(u16::MAX), 1),
                Rejected::NoSuchRow,
            ),
            (
                Issued::want(0, asteroid(0), FRIGATE, MAX_WANT + 1),
                Rejected::TooMany,
            ),
        ];

        for (issued, why) in refusals {
            assert_eq!(world.refusal(issued), Some(why), "{issued:?}");
            let (next, _) = world.state.step(&Batch::of(&[issued]));
            assert_eq!(next.posts().count(), 0, "{issued:?} left a want behind");
        }
        assert_eq!(
            world.refusal(Issued::want(0, asteroid(0), FRIGATE, MAX_WANT)),
            None
        );
    }

    #[test]
    fn a_tick_lands_the_same_state_and_hash_however_its_commands_arrived() {
        let world = World::started(&[TeamId(0), TeamId(1)]);

        let issued = [
            Issued::numbered(0, 0, asteroid(0), FRIGATE, 3),
            Issued::numbered(0, 1, asteroid(0), FRIGATE, 7),
            Issued::numbered(1, 0, asteroid(2), CONSTRUCTOR, 1),
            Issued::numbered(0, 2, asteroid(1), SHIPYARD, 1),
        ];

        let (ordered, _) = world.state.step(&Batch::of(&issued));
        let mut scrambled = issued;
        scrambled.reverse();
        let (arrived, _) = world.state.step(&Batch::of(&scrambled));

        assert_eq!(ordered.hash(), arrived.hash());
        assert_eq!(ordered, arrived);
        assert_eq!(
            ordered
                .wants(Post::of(0, asteroid(0)))
                .map(|wants| wants.get(FRIGATE)),
            Some(7),
            "the seat's later command is the one that stands"
        );
    }

    #[test]
    fn each_unit_spawns_one_spacing_further_out_than_the_last_clear_of_the_asteroid() {
        let mut world = World::ring(Gravity::new(4.0e13), 1, &[TeamId(0)]);
        let home = world.state.asteroid_body(asteroid(0));
        let radial = home.pos.normalized().expect("a radius");
        let floor = world.state[asteroid(0)].radius() + Belt::SPACING_METERS;
        for already in 0..3 {
            let spawn = world.state.spawn_body(asteroid(0), world.state.time());
            let out = floor + Belt::SPACING_METERS * f64::from(already);
            assert!(
                spawn.pos.distance(home.pos + radial * out) < 1e-9,
                "unit {already} spawns at {spawn:?}"
            );
            assert_eq!(spawn.vel, home.vel);
            world.free(0, FRIGATE, asteroid(0), spawn);
        }
    }

    #[test]
    fn a_structure_at_the_asteroid_does_not_move_a_spawn() {
        let mut world = World::ring(Gravity::new(4.0e13), 1, &[TeamId(0)]);
        world.fix(0, FRIGATE, asteroid(0));
        let home = world.state.asteroid_body(asteroid(0));
        let floor = world.state[asteroid(0)].radius() + Belt::SPACING_METERS;

        let spawn = world.state.spawn_body(asteroid(0), world.state.time());

        assert!(spawn.pos.distance(home.pos) - floor < 1e-9);
    }

    #[test]
    #[ignore = "cost report: cargo test -p neumannarch-sim --release -- --ignored --nocapture"]
    fn one_tick_of_a_crowded_asteroid_fits_the_budget() {
        for units in [100u32, 1000] {
            let mut world = World::started(&[TeamId(0), TeamId(1)]);
            let home = asteroid(0);
            for at in 0..units {
                let body = world.state.spawn_body(home, world.state.time());
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
                "{units} units at one asteroid: {:.3} ms a tick, against a budget of {:.3} ms",
                each * 1e3,
                Time(1).seconds() * 1e3
            );
        }
    }
}
