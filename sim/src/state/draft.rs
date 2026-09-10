use crate::TICKS_PER_SECOND;
use crate::ids::{AsteroidId, SeatId};
use crate::pattern::EntityPattern;
use crate::state::hash;
use crate::state::seat::Seat;
use crate::time::Tick;

pub const STAGE_SPAN: Tick = Tick(10 * TICKS_PER_SECOND as u64);

pub const GRACE: Tick = Tick(30 * TICKS_PER_SECOND as u64);

pub const STAGES_PER_SEAT: usize = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PlacementStage {
    pub seat: SeatId,
    pub pattern: EntityPattern,
    pub placed: Option<AsteroidId>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Draft {
    stages: Vec<PlacementStage>,
    running: usize,
    began: Tick,
    ended: Option<Tick>,
}

impl Draft {
    pub(crate) fn of(seed: u64, seats: &[Seat]) -> Draft {
        let mut order: Vec<SeatId> = (0..seats.len())
            .filter_map(|at| u8::try_from(at).ok())
            .map(SeatId)
            .collect();
        order.sort_by_key(|seat| (hash::digest(&(seed, seat.0)), seat.0));
        let held = |seat: SeatId, round: usize| {
            let mut patterns: Vec<EntityPattern> = seats[usize::from(seat.0)]
                .reserve()
                .keys()
                .copied()
                .collect();
            patterns.sort_by(|a, b| b.build_rate().total_cmp(&a.build_rate()).then(a.cmp(b)));
            patterns.get(round).copied()
        };
        let mut stages = Vec::new();
        for round in 0..STAGES_PER_SEAT {
            for at in 0..order.len() {
                let seat = order[match round % 2 {
                    0 => at,
                    _ => order.len() - 1 - at,
                }];
                if let Some(pattern) = held(seat, round) {
                    stages.push(PlacementStage {
                        seat,
                        pattern,
                        placed: None,
                    });
                }
            }
        }
        Draft {
            stages,
            running: 0,
            began: Tick::ZERO,
            ended: None,
        }
    }

    pub(crate) fn ends_by(seats: usize) -> Tick {
        Tick(STAGE_SPAN.0 * (seats * STAGES_PER_SEAT) as u64 + GRACE.0)
    }

    pub fn stages(&self) -> &[PlacementStage] {
        &self.stages
    }

    pub fn running(&self) -> Option<PlacementStage> {
        self.stages.get(self.running).copied()
    }

    pub fn began(&self) -> Tick {
        self.began
    }

    pub fn ended(&self) -> Option<Tick> {
        self.ended
    }

    pub fn awaits(&self, seat: SeatId, pattern: EntityPattern) -> Option<bool> {
        Some(self.waiting(seat, pattern)? <= self.running)
    }

    pub fn over(&self, tick: Tick) -> bool {
        self.stages.iter().all(|stage| stage.placed.is_some())
            || (self.running >= self.stages.len() && tick.0 >= self.began.0 + GRACE.0)
    }

    fn waiting(&self, seat: SeatId, pattern: EntityPattern) -> Option<usize> {
        self.stages.iter().position(|stage| {
            stage.seat == seat && stage.pattern == pattern && stage.placed.is_none()
        })
    }

    pub fn place(
        &mut self,
        asteroid: AsteroidId,
        seat: SeatId,
        pattern: EntityPattern,
        tick: Tick,
    ) {
        let Some(at) = self.waiting(seat, pattern) else {
            return;
        };
        self.stages[at].placed = Some(asteroid);
        if at == self.running {
            self.running += 1;
            self.began = tick;
        }
    }

    pub fn pass(&mut self, tick: Tick) {
        if self.running < self.stages.len() && tick.0 >= self.began.0 + STAGE_SPAN.0 {
            self.running += 1;
            self.began = tick;
        }
    }

    pub fn end(&mut self, tick: Tick) {
        self.ended = Some(tick);
    }
}

#[cfg(test)]
mod tests {
    use crate::pattern::EntityPattern as P;
    use std::collections::BTreeMap;

    use super::*;
    use crate::fixture::{CLOCK, World};
    use crate::ids::TeamId;
    use crate::materials::Materials;
    use crate::state::{Entity, Issued, Rejected};
    use crate::time::Time;

    fn drafting(teams: usize) -> World {
        let teams: Vec<TeamId> = (0..teams)
            .filter_map(|at| u8::try_from(at).ok().map(TeamId))
            .collect();
        World::drafting(&teams, CLOCK)
    }

    fn running(world: &World) -> PlacementStage {
        world.state.draft().running().expect("a stage is running")
    }

    fn placing(world: &World, asteroid: AsteroidId) -> Issued {
        let stage = running(world);
        Issued::want(stage.seat.0, asteroid, stage.pattern, 1)
    }

    #[test]
    fn two_seeds_draw_two_orders_and_one_seed_draws_one() {
        let seats = |seed| {
            let world = drafting(4);
            let staged = Draft::of(seed, world.state.seats());
            staged
                .stages()
                .iter()
                .map(|stage| stage.seat.0)
                .collect::<Vec<u8>>()
        };
        let orders: Vec<Vec<u8>> = (0..8).map(seats).collect();

        assert_eq!(orders[0], seats(0), "the seed alone decides");
        assert!(
            orders.iter().any(|order| *order != orders[0]),
            "eight seeds drew one order: {orders:?}"
        );
        for order in &orders {
            let mut seated = order.clone();
            seated.sort_unstable();
            assert_eq!(
                seated,
                vec![0, 0, 1, 1, 2, 2, 3, 3],
                "two a seat: {order:?}"
            );
        }
    }

    #[test]
    fn the_second_rounds_stages_run_in_the_reverse_order_of_the_first() {
        let world = drafting(4);
        let stages = world.state.draft().stages();
        let seats: Vec<u8> = stages.iter().map(|stage| stage.seat.0).collect();
        let (first, second) = seats.split_at(seats.len() / STAGES_PER_SEAT);

        assert_eq!(stages.len(), 4 * STAGES_PER_SEAT, "a stage per structure");
        assert_eq!(
            second.iter().rev().copied().collect::<Vec<u8>>(),
            first.to_vec(),
            "the seat that went first goes last"
        );
        let patterns: Vec<EntityPattern> = stages.iter().map(|stage| stage.pattern).collect();
        assert_eq!(
            patterns.split_at(first.len()).0.to_vec(),
            vec![patterns[0]; first.len()],
            "the first round places the same structure for every seat"
        );
    }

    #[test]
    fn a_stage_ends_on_its_placement_and_the_next_begins_the_same_tick() {
        let mut world = drafting(2);
        let first = running(&world);

        world.tick(&[placing(&world, AsteroidId(0))]);

        let draft = world.state.draft();
        assert_eq!(draft.stages()[0].placed, Some(AsteroidId(0)));
        assert_eq!(draft.began(), Tick::ZERO, "the next began where it landed");
        let second = draft.running().expect("the second stage runs");
        assert_ne!(second.seat, first.seat, "the next seat is picking");
        assert_eq!(second.placed, None);
    }

    #[test]
    fn a_stage_that_places_nothing_ends_at_its_span_and_the_next_begins() {
        let mut world = drafting(2);
        let first = running(&world);

        world.run(STAGE_SPAN.0);
        assert_eq!(running(&world), first, "the span has not run out");
        world.run(1);

        assert_eq!(world.state.draft().began(), Tick(STAGE_SPAN.0));
        assert_ne!(running(&world).seat, first.seat, "the next seat is picking");
    }

    #[test]
    fn a_seat_whose_stage_ran_out_places_while_another_stage_runs() {
        let mut world = drafting(2);
        let missed = running(&world);
        world.run(STAGE_SPAN.0 + 1);
        let now = running(&world);
        assert_ne!(now.seat, missed.seat, "another seat is picking");

        world.tick(&[Issued::want(
            missed.seat.0,
            AsteroidId(4),
            missed.pattern,
            1,
        )]);

        assert_eq!(world.state.draft().stages()[0].placed, Some(AsteroidId(4)));
        assert_eq!(running(&world), now, "the running stage is untouched");
        assert_eq!(
            world.refusal(Issued::want(now.seat.0, AsteroidId(4), now.pattern, 1)),
            Some(Rejected::AsteroidTaken),
            "the asteroid it took is spoken for"
        );
    }

    #[test]
    fn a_want_of_a_reserve_pattern_at_a_taken_asteroid_is_refused_whatever_its_count() {
        let mut world = drafting(2);
        world.tick(&[placing(&world, AsteroidId(0))]);
        let waiting = running(&world);

        for count in [1, 2] {
            assert_eq!(
                world.refusal(Issued::want(
                    waiting.seat.0,
                    AsteroidId(0),
                    waiting.pattern,
                    count
                )),
                Some(Rejected::AsteroidTaken),
                "a want of {count} reached an asteroid another seat holds"
            );
        }
    }

    #[test]
    fn a_seat_whose_reserve_is_spent_wants_one_pattern_where_something_stands() {
        let stock = Materials::new(1e4, 1e4, 1e4);
        let mut world = World::stocked(stock, BTreeMap::from([(P::Storage, 2)]));

        world.tick(&[Issued::want(0, AsteroidId(0), P::Storage, 2)]);

        assert_eq!(
            world.state[SeatId(0)].reserved(P::Storage),
            0,
            "the want emptied the reserve"
        );
        assert!(world.state.is_taken(AsteroidId(0)));
        assert_eq!(
            world.refusal(Issued::want(0, AsteroidId(0), P::Storage, 1)),
            None,
            "a seat holding no reserve of the pattern wants one like any other"
        );
    }

    #[test]
    fn a_reserve_want_before_the_seats_first_stage_is_refused_by_name() {
        let world = drafting(2);
        let waiting = world.state.draft().stages()[1];

        assert_eq!(
            world.refusal(Issued::want(
                waiting.seat.0,
                AsteroidId(0),
                waiting.pattern,
                1
            )),
            Some(Rejected::NotYet),
            "its stage has not begun"
        );
        assert_eq!(world.refusal(placing(&world, AsteroidId(0))), None);
    }

    #[test]
    fn the_clock_starts_on_the_tick_the_last_placement_lands() {
        let mut world = drafting(1);
        let asteroids = [AsteroidId(0), AsteroidId(1)];
        world.tick(&[placing(&world, asteroids[0])]);
        assert!(world.state.drafting(), "a stage is still to run");

        let landing = world.state.tick();
        world.tick(&[placing(&world, asteroids[1])]);

        assert_eq!(world.state.draft().ended(), Some(landing));
        assert_eq!(
            world.state.time(),
            Time(1),
            "the clock runs from the landing"
        );
        world.run(STAGE_SPAN.0);
        assert_eq!(
            world.state.draft().ended(),
            Some(landing),
            "the end is fixed"
        );
        assert_eq!(world.state.length(), CLOCK, "the length never moves");
        assert_eq!(world.state.entities().count(), 2, "both stand at once");
    }

    #[test]
    fn the_belt_and_the_clock_stand_still_through_a_draft_that_ends_at_the_grace() {
        let mut world = drafting(2);
        let standing = world.state.asteroid_body(AsteroidId(0));

        world.start_the_clock();

        assert!(world.state.tick() > Tick::ZERO, "the steps were counted");
        assert_eq!(
            world.state.time(),
            Time::ZERO,
            "no tick of it is match time"
        );
        assert_eq!(
            world.state.asteroid_body(AsteroidId(0)),
            standing,
            "nor turns an asteroid"
        );

        world.run(1);
        let ended = world.state.draft().ended().expect("the grace ran out");
        world.run(STAGE_SPAN.0);

        assert_eq!(ended.0, STAGE_SPAN.0 * 4 + GRACE.0);
        assert_ne!(
            world.state.asteroid_body(AsteroidId(0)),
            standing,
            "the belt turns"
        );
    }

    #[test]
    fn the_reserve_fills_a_want_at_once_and_any_other_want_waits_for_the_clock() {
        let mut world = drafting(1);
        let stage = running(&world);
        let seat = stage.seat.0;
        world.tick(&[
            Issued::numbered(seat, 0, AsteroidId(0), P::Storage, 1),
            Issued::numbered(seat, 1, AsteroidId(0), stage.pattern, 1),
        ]);
        let stock = world.state[SeatId(seat)].stockpile().stock();

        world.run(STAGE_SPAN.0 - 1);

        let standing: Vec<EntityPattern> = world.state.entities().map(Entity::pattern).collect();
        assert_eq!(
            standing,
            vec![stage.pattern],
            "the reserve pattern stands, complete"
        );
        assert_eq!(world.state[SeatId(seat)].reserved(stage.pattern), 0);
        assert!(world.state.is_taken(AsteroidId(0)));
        assert_eq!(world.state.frames().len(), 0, "nothing builds");
        assert_eq!(
            world.state[SeatId(seat)].stockpile().stock(),
            stock,
            "nothing is spent"
        );

        world.tick(&[Issued::numbered(
            seat,
            2,
            AsteroidId(1),
            running(&world).pattern,
            1,
        )]);

        assert_eq!(world.state.entities().count(), 2, "the reserve stands");
        assert_eq!(
            world.state.frames().len(),
            0,
            "the clock starts on this tick and has run none of it"
        );

        world.tick(&[]);

        assert_eq!(
            world.state.frames().len(),
            1,
            "the storage waited for the first tick of match time"
        );
    }

    #[test]
    fn every_stage_the_view_carries_is_the_stage_the_state_runs() {
        let mut world = drafting(2);
        world.tick(&[placing(&world, AsteroidId(0))]);
        world.run(STAGE_SPAN.0);

        let view = world.view(0);

        assert_eq!(view.draft.stages(), world.state.draft().stages());
        assert_eq!(view.draft.running(), world.state.draft().running());
        assert_eq!(view.draft.began(), world.state.draft().began());
        assert_eq!(
            view.draft.stages()[0].placed,
            Some(AsteroidId(0)),
            "one placed"
        );
        assert_eq!(view.draft.stages()[1].placed, None, "one ran out");
        assert_eq!(view.draft.running(), Some(view.draft.stages()[2]));
    }
}
