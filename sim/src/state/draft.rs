use crate::TICKS_PER_SECOND;
use crate::ids::{RockId, RowId, SeatId};
use crate::roster::Roster;
use crate::state::hash;
use crate::state::seat::Seat;
use crate::time::Tick;

pub const STAGE_SPAN: Tick = Tick(10 * TICKS_PER_SECOND as u64);

pub const GRACE: Tick = Tick(30 * TICKS_PER_SECOND as u64);

pub const STAGES_PER_SEAT: usize = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Stage {
    pub seat: SeatId,
    pub row: RowId,
    pub placed: Option<RockId>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Draft {
    stages: Vec<Stage>,
    running: usize,
    began: Tick,
    ended: Option<Tick>,
}

impl Draft {
    pub fn of(seed: u64, seats: &[Seat], roster: &Roster) -> Draft {
        let mut order: Vec<SeatId> = (0..seats.len())
            .filter_map(|at| u8::try_from(at).ok())
            .map(SeatId)
            .collect();
        order.sort_by_key(|seat| (hash::digest(&(seed, seat.0)), seat.0));
        let builds = |row: &RowId| {
            roster
                .get(*row)
                .map_or(0.0, |row| row.builds().sum::<f64>())
        };
        let held = |seat: SeatId, round: usize| {
            let mut rows: Vec<RowId> = seats[usize::from(seat.0)]
                .reserve()
                .keys()
                .copied()
                .collect();
            rows.sort_by(|a, b| builds(b).total_cmp(&builds(a)).then(a.cmp(b)));
            rows.get(round).copied()
        };
        let mut stages = Vec::new();
        for round in 0..STAGES_PER_SEAT {
            for at in 0..order.len() {
                let seat = order[match round % 2 {
                    0 => at,
                    _ => order.len() - 1 - at,
                }];
                if let Some(row) = held(seat, round) {
                    stages.push(Stage {
                        seat,
                        row,
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

    pub fn stages(&self) -> &[Stage] {
        &self.stages
    }

    pub fn running(&self) -> Option<Stage> {
        self.stages.get(self.running).copied()
    }

    pub fn began(&self) -> Tick {
        self.began
    }

    pub fn ended(&self) -> Option<Tick> {
        self.ended
    }

    pub fn placements(&self, seat: SeatId) -> impl Iterator<Item = (RockId, RowId)> + '_ {
        self.stages
            .iter()
            .filter(move |stage| stage.seat == seat)
            .filter_map(|stage| Some((stage.placed?, stage.row)))
    }

    pub fn took(&self, rock: RockId) -> Option<SeatId> {
        self.stages
            .iter()
            .find(|stage| stage.placed == Some(rock))
            .map(|stage| stage.seat)
    }

    pub fn awaits(&self, seat: SeatId, row: RowId) -> Option<bool> {
        Some(self.waiting(seat, row)? <= self.running)
    }

    pub fn over(&self, tick: Tick) -> bool {
        self.stages.iter().all(|stage| stage.placed.is_some())
            || (self.running >= self.stages.len() && tick.0 >= self.began.0 + GRACE.0)
    }

    fn waiting(&self, seat: SeatId, row: RowId) -> Option<usize> {
        self.stages
            .iter()
            .position(|stage| stage.seat == seat && stage.row == row && stage.placed.is_none())
    }

    pub(crate) fn place(&mut self, rock: RockId, seat: SeatId, row: RowId, tick: Tick) {
        let Some(at) = self.waiting(seat, row) else {
            return;
        };
        self.stages[at].placed = Some(rock);
        if at == self.running {
            self.running += 1;
            self.began = tick;
        }
    }

    pub(crate) fn pass(&mut self, tick: Tick) {
        if self.running < self.stages.len() && tick.0 >= self.began.0 + STAGE_SPAN.0 {
            self.running += 1;
            self.began = tick;
        }
    }

    pub(crate) fn end(&mut self, tick: Tick) {
        self.ended = Some(tick);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixture::{CLOCK, World};
    use crate::ids::TeamId;
    use crate::roster::STORAGE;
    use crate::state::{Issued, Rejected};
    use crate::time::Time;

    fn drafting(teams: usize) -> World {
        let teams: Vec<TeamId> = (0..teams)
            .filter_map(|at| u8::try_from(at).ok().map(TeamId))
            .collect();
        World::drafting(&teams, CLOCK)
    }

    fn running(world: &World) -> Stage {
        world.state.draft().running().expect("a stage is running")
    }

    fn placing(world: &World, rock: RockId) -> Issued {
        let stage = running(world);
        Issued::want(stage.seat.0, rock, stage.row, 1)
    }

    #[test]
    fn two_seeds_draw_two_orders_and_one_seed_draws_one() {
        let seats = |seed| {
            let world = drafting(4);
            let staged = Draft::of(seed, world.state.seats(), world.state.roster());
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
        let rows: Vec<u8> = stages.iter().map(|stage| stage.row.0 as u8).collect();
        assert_eq!(
            rows.split_at(first.len()).0.iter().collect::<Vec<&u8>>(),
            vec![&rows[0]; first.len()],
            "the first round places the same structure for every seat"
        );
    }

    #[test]
    fn a_stage_ends_on_its_placement_and_the_next_begins_the_same_tick() {
        let mut world = drafting(2);
        let first = running(&world);

        world.tick(&[placing(&world, RockId(0))]);

        let draft = world.state.draft();
        assert_eq!(draft.stages()[0].placed, Some(RockId(0)));
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

        world.tick(&[Issued::want(missed.seat.0, RockId(4), missed.row, 1)]);

        assert_eq!(world.state.draft().stages()[0].placed, Some(RockId(4)));
        assert_eq!(running(&world), now, "the running stage is untouched");
        assert_eq!(
            world.refusal(Issued::want(now.seat.0, RockId(4), now.row, 1)),
            Some(Rejected::RockTaken),
            "the rock it took is spoken for"
        );
    }

    #[test]
    fn a_reserve_want_before_the_seats_first_stage_is_refused_by_name() {
        let world = drafting(2);
        let waiting = world.state.draft().stages()[1];

        assert_eq!(
            world.refusal(Issued::want(waiting.seat.0, RockId(0), waiting.row, 1)),
            Some(Rejected::NotYet),
            "its stage has not begun"
        );
        assert_eq!(world.refusal(placing(&world, RockId(0))), None);
    }

    #[test]
    fn the_clock_starts_on_the_tick_the_last_placement_lands() {
        let mut world = drafting(1);
        let rocks = [RockId(0), RockId(1)];
        world.tick(&[placing(&world, rocks[0])]);
        assert!(world.state.drafting(), "a stage is still to run");

        let landing = world.state.tick();
        world.tick(&[placing(&world, rocks[1])]);

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
        let standing = world.state.rock_body(RockId(0));

        world.start_the_clock();

        assert!(world.state.tick() > Tick::ZERO, "the steps were counted");
        assert_eq!(
            world.state.time(),
            Time::ZERO,
            "no tick of it is match time"
        );
        assert_eq!(
            world.state.rock_body(RockId(0)),
            standing,
            "nor turns a rock"
        );

        world.run(1);
        let ended = world.state.draft().ended().expect("the grace ran out");
        world.run(STAGE_SPAN.0);

        assert_eq!(ended.0, STAGE_SPAN.0 * 4 + GRACE.0);
        assert_ne!(world.state.rock_body(RockId(0)), standing, "the belt turns");
    }

    #[test]
    fn a_want_accepted_during_the_draft_is_filled_once_the_clock_runs() {
        let mut world = drafting(1);
        let stage = running(&world);
        let seat = stage.seat.0;
        world.tick(&[
            Issued::numbered(seat, 0, RockId(0), STORAGE, 1),
            Issued::numbered(seat, 1, RockId(0), stage.row, 1),
        ]);
        let stock = world.state[SeatId(seat)].stockpile().stock();

        world.run(STAGE_SPAN.0 - 1);

        assert_eq!(world.state.entities().count(), 0, "nothing stands");
        assert_eq!(world.state.frames().len(), 0, "nothing builds");
        assert_eq!(
            world.state[SeatId(seat)].stockpile().stock(),
            stock,
            "nothing is spent"
        );

        world.tick(&[Issued::numbered(seat, 2, RockId(1), running(&world).row, 1)]);

        assert_eq!(world.state.entities().count(), 2, "the reserve stands");
        assert_eq!(world.state.frames().len(), 1, "the storage builds");
    }

    #[test]
    fn every_stage_the_view_carries_is_the_stage_the_state_runs() {
        let mut world = drafting(2);
        world.tick(&[placing(&world, RockId(0))]);
        world.run(STAGE_SPAN.0);

        let view = world.view(0);

        assert_eq!(view.draft.stages(), world.state.draft().stages());
        assert_eq!(view.draft.running(), world.state.draft().running());
        assert_eq!(view.draft.began(), world.state.draft().began());
        assert_eq!(view.draft.stages()[0].placed, Some(RockId(0)), "one placed");
        assert_eq!(view.draft.stages()[1].placed, None, "one ran out");
        assert_eq!(view.draft.running(), Some(view.draft.stages()[2]));
    }
}
