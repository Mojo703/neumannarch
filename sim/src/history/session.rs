use std::borrow::Cow;
use std::collections::BTreeMap;

use crate::history::log::Log;
use crate::history::snapshots::{Retention, Snapshots};
use crate::ids::SeatId;
use crate::setup::Setup;
use crate::state::{Refused, Stamped, State};
use crate::step::Outcome;
use crate::time::Tick;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rewound {
    Nothing,
    From(Tick),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Unseated {
    pub seat: SeatId,
}

pub struct Session {
    setup: Setup,
    initial: State,
    live: State,
    snapshots: Snapshots,
    log: Log,
    outcomes: BTreeMap<Tick, Outcome>,
    acknowledged: Vec<Tick>,
    local: Vec<SeatId>,
}

impl Session {
    pub fn new(setup: Setup, retention: Retention, local: &[SeatId]) -> Result<Session, Unseated> {
        let seats = setup.teams().len();
        if let Some(seat) = local.iter().find(|seat| usize::from(seat.0) >= seats) {
            return Err(Unseated { seat: *seat });
        }
        let mut local = local.to_vec();
        local.sort_unstable();
        local.dedup();
        let initial = State::start(&setup);
        Ok(Session {
            setup,
            live: initial.clone(),
            initial,
            snapshots: Snapshots::new(retention),
            log: Log::default(),
            outcomes: BTreeMap::new(),
            acknowledged: vec![Tick::ZERO; seats],
            local,
        })
    }

    pub fn state(&self) -> &State {
        &self.live
    }

    pub fn advance(&mut self) -> &Outcome {
        let stepped = self.live.tick();
        self.stepped();
        let latest = self.live.tick();
        for seat in &self.local {
            if let Some(known) = self.acknowledged.get_mut(usize::from(seat.0)) {
                *known = (*known).max(latest);
            }
        }
        self.forget();
        self.outcomes
            .get(&stepped)
            .expect("the tick just stepped keeps its outcome")
    }

    pub fn insert(&mut self, stamped: Stamped) -> Result<Rewound, Refused> {
        let latest = self.live.tick();
        if stamped.tick < self.oldest() {
            return Err(Refused::Late);
        }
        if stamped.tick > latest.ahead(self.snapshots.span()) {
            return Err(Refused::Ahead);
        }
        self.log.insert(stamped)?;
        if stamped.tick >= latest {
            return Ok(Rewound::Nothing);
        }
        self.rewind(stamped.tick);
        Ok(Rewound::From(stamped.tick))
    }

    pub fn acknowledge(&mut self, seat: SeatId, up_to: Tick) {
        if let Some(known) = self.acknowledged.get_mut(usize::from(seat.0)) {
            *known = (*known).max(up_to);
        }
    }

    pub fn acknowledged(&self, seat: SeatId) -> Option<Tick> {
        self.acknowledged.get(usize::from(seat.0)).copied()
    }

    pub fn settled(&self) -> Tick {
        self.acknowledged
            .iter()
            .copied()
            .min()
            .unwrap_or(Tick::ZERO)
    }

    pub fn hash_at(&self, tick: Tick) -> Option<u64> {
        self.at(tick).map(|state| state.hash())
    }

    pub fn outcome_at(&self, tick: Tick) -> Option<&Outcome> {
        self.outcomes.get(&tick)
    }

    pub fn outcome(&self) -> Option<&Outcome> {
        self.live
            .tick()
            .previous()
            .and_then(|at| self.outcome_at(at))
    }

    pub fn setup(&self) -> &Setup {
        &self.setup
    }

    pub fn commands(&self) -> Vec<Stamped> {
        self.log.until(self.settled()).stamped().collect()
    }

    fn stepped(&mut self) {
        let tick = self.live.tick();
        self.snapshots.keep(&self.live);
        let (next, outcome) = self.live.step(self.log.at(tick));
        self.live = next;
        self.outcomes.insert(tick, outcome);
    }

    fn rewind(&mut self, from: Tick) {
        let restored = self
            .snapshots
            .at_or_before(from)
            .unwrap_or(&self.initial)
            .clone();
        self.snapshots.discard_after(from);
        self.outcomes.retain(|tick, _| *tick < from);
        let latest = self.live.tick();
        self.live = restored;
        while self.live.tick() < latest {
            self.stepped();
        }
    }

    fn at(&self, tick: Tick) -> Option<Cow<'_, State>> {
        if tick > self.live.tick() || tick < self.oldest() {
            return None;
        }
        if tick == self.live.tick() {
            return Some(Cow::Borrowed(&self.live));
        }
        let from = self.snapshots.at_or_before(tick).unwrap_or(&self.initial);
        if from.tick() == tick {
            return Some(Cow::Borrowed(from));
        }
        let mut state = from.clone();
        while state.tick() < tick {
            let (next, _) = state.step(self.log.at(state.tick()));
            state = next;
        }
        Some(Cow::Owned(state))
    }

    fn oldest(&self) -> Tick {
        self.live.tick().back(self.snapshots.span())
    }

    fn forget(&mut self) {
        let oldest = self.oldest();
        self.snapshots.prune(oldest);
        self.outcomes.retain(|tick, _| tick.next() >= oldest);
    }
}

#[cfg(test)]
mod tests {
    use core::num::NonZeroU32;

    use super::*;
    use crate::TICKS_PER_SECOND;
    use crate::ids::{AsteroidId, RowId, TeamId};
    use crate::roster::{FRIGATE, METALS_EXTRACTOR, SHIPYARD};
    use crate::state::{Command, Issued, MAX_COMMANDS_PER_TICK, Motion, STAGE_SPAN, Stage};
    use crate::time::Time;

    const CLOCK: Tick = Tick(15 * 60 * TICKS_PER_SECOND as u64);

    const BOTH: [SeatId; 2] = [SeatId(0), SeatId(1)];

    const UNTIL: Tick = Tick(40);

    const LAST_STAGE: Tick = Tick(STAGE_SPAN.0 * (2 * BOTH.len() as u64 - 1));

    const SECONDS_CLOSED: Tick = Tick(LAST_STAGE.0 + 5 * TICKS_PER_SECOND as u64);

    const SPAN: u32 = SECONDS_CLOSED.0 as u32;

    fn setup() -> Setup {
        Setup::new(vec![TeamId(0), TeamId(1)], 11, CLOCK).expect("two seats are a match")
    }

    fn session(span: u32) -> Session {
        Session::new(setup(), window(span, 1), &BOTH).expect("both seats are seated")
    }

    fn kept(span: u32) -> Session {
        Session::new(setup(), window(span, 8), &BOTH).expect("both seats are seated")
    }

    fn one_local() -> Session {
        Session::new(setup(), Retention::shipped(), &[SeatId(0)]).expect("seat zero is seated")
    }

    fn window(ticks: u32, every: u32) -> Retention {
        Retention::Window {
            ticks,
            every: NonZeroU32::new(every).expect("a stride of no ticks keeps nothing"),
        }
    }

    fn asteroid(at: u32) -> AsteroidId {
        AsteroidId(at)
    }

    fn want(seat: u8, seq: u32, asteroid: AsteroidId, row: RowId, count: u32) -> Issued {
        Issued {
            seat: SeatId(seat),
            seq,
            command: Command::Want {
                asteroid,
                row,
                count,
            },
        }
    }

    fn stamped(tick: u64, issued: Issued) -> Stamped {
        Stamped {
            tick: Tick(tick),
            issued,
        }
    }

    fn script() -> Vec<Stamped> {
        let mut script: Vec<Stamped> = stages()
            .iter()
            .enumerate()
            .map(|(at, stage)| {
                let seq = u32::try_from(at).expect("a stage a seat");
                let placing = want(stage.seat.0, seq, asteroid(at as u32), stage.row, 1);
                stamped(STAGE_SPAN.0 * at as u64, placing)
            })
            .collect();
        script.push(stamped(3, want(0, 8, worked(), METALS_EXTRACTOR, 2)));
        script.push(stamped(9, want(0, 9, worked(), FRIGATE, 1)));
        script.sort_by_key(|stamped| (stamped.tick, stamped.issued.seat, stamped.issued.seq));
        script
    }

    fn stages() -> Vec<Stage> {
        State::start(&setup()).draft().stages().to_vec()
    }

    fn worked() -> AsteroidId {
        let mine = |stage: &&Stage| stage.seat == SeatId(0) && stage.row == SHIPYARD;
        let at = stages().iter().position(|stage| mine(&stage));
        asteroid(at.expect("seat zero drafts a shipyard") as u32)
    }

    fn early() -> Vec<Stamped> {
        script()
            .into_iter()
            .filter(|stamped| stamped.tick < UNTIL)
            .collect()
    }

    fn play(session: &mut Session, until: Tick) {
        while session.state().tick() < until {
            let tick = session.state().tick();
            for stamped in script().into_iter().filter(|stamped| stamped.tick == tick) {
                assert_eq!(session.insert(stamped), Ok(Rewound::Nothing));
            }
            session.advance();
        }
    }

    fn run(session: &mut Session, ticks: u64) {
        for _ in 0..ticks {
            session.advance();
        }
    }

    #[test]
    fn commands_learned_late_and_out_of_order_reach_the_on_time_hash() {
        let mut on_time = kept(SPAN);
        play(&mut on_time, SECONDS_CLOSED);
        assert_eq!(
            on_time.state().draft().ended(),
            Some(LAST_STAGE),
            "the clock starts on the last placement"
        );
        let seat = &on_time.state()[SeatId(0)];
        assert!(seat.income().total() > 0.0, "an extractor stands");
        assert!(seat.spend().total() > 0.0, "a frame drains the stockpile");
        assert!(
            on_time.state()[worked()].pull().total() > 0.0,
            "the asteroid is worked"
        );

        let mut late = kept(SPAN);
        run(&mut late, SECONDS_CLOSED.0);
        assert_ne!(
            late.hash_at(SECONDS_CLOSED),
            on_time.hash_at(SECONDS_CLOSED),
            "the commands did nothing, so nothing is proven"
        );
        for stamped in script().into_iter().rev() {
            assert_eq!(late.insert(stamped), Ok(Rewound::From(stamped.tick)));
        }

        assert_eq!(
            late.hash_at(SECONDS_CLOSED),
            on_time.hash_at(SECONDS_CLOSED)
        );
        assert_eq!(late.state(), on_time.state());
    }

    #[test]
    fn a_command_the_session_will_not_take_is_refused_by_name() {
        let span = 8;
        let mut session = session(span);
        run(&mut session, 20);
        let latest = session.state().tick();
        let issued = want(0, 0, asteroid(0), SHIPYARD, 1);

        assert_eq!(
            session.insert(stamped(latest.back(span).0 - 1, issued)),
            Err(Refused::Late)
        );
        assert_eq!(
            session.insert(stamped(latest.ahead(span).0 + 1, issued)),
            Err(Refused::Ahead)
        );
        assert!(session.insert(stamped(latest.0, issued)).is_ok());
        assert_eq!(
            session.insert(stamped(latest.0, issued)),
            Err(Refused::Duplicate)
        );
        for seq in 1..MAX_COMMANDS_PER_TICK as u32 {
            assert!(
                session
                    .insert(stamped(latest.0, want(0, seq, asteroid(0), SHIPYARD, 1)))
                    .is_ok()
            );
        }
        let over = MAX_COMMANDS_PER_TICK as u32;
        assert_eq!(
            session.insert(stamped(latest.0, want(0, over, asteroid(0), SHIPYARD, 1))),
            Err(Refused::TooMany)
        );
        assert!(
            session
                .insert(stamped(latest.0, want(1, over, asteroid(0), SHIPYARD, 1)))
                .is_ok(),
            "the cap is one seat's"
        );
    }

    #[test]
    fn the_commands_a_session_hands_out_are_those_stamped_before_its_settled_tick() {
        let mut session = one_local();
        for stamped in early() {
            assert!(session.insert(stamped).is_ok());
        }
        run(&mut session, UNTIL.0);
        session.acknowledge(SeatId(1), Tick(10));
        let settled = session.settled();

        assert_eq!(session.setup(), &setup());
        assert_eq!(
            session.commands(),
            early()
                .into_iter()
                .filter(|stamped| stamped.tick < settled)
                .collect::<Vec<_>>()
        );
        assert!(
            settled < UNTIL,
            "an unsettled tail is what makes the filter mean anything"
        );
    }

    #[test]
    fn a_local_seat_the_setup_does_not_seat_is_refused_by_name() {
        let beyond = SeatId(setup().teams().len() as u8);

        assert_eq!(
            Session::new(setup(), Retention::shipped(), &[SeatId(0), beyond]).err(),
            Some(Unseated { seat: beyond })
        );
    }

    #[test]
    fn settling_is_the_smallest_acknowledgement_over_the_seats() {
        let mut session = one_local();
        run(&mut session, 10);

        assert_eq!(
            session.settled(),
            Tick::ZERO,
            "the remote seat has acknowledged nothing"
        );
        session.acknowledge(SeatId(1), Tick(4));
        assert_eq!(session.settled(), Tick(4));
        session.acknowledge(SeatId(1), Tick(2));
        assert_eq!(session.settled(), Tick(4));
        session.acknowledge(SeatId(9), Tick(1));
        session.acknowledge(SeatId(1), Tick(10));
        assert_eq!(
            session.settled(),
            Tick(10),
            "the local seat is acknowledged as it advances"
        );
    }

    #[test]
    fn a_hash_at_a_tick_no_state_was_kept_for_is_the_one_a_kept_tick_holds() {
        let mut dense = session(240);
        play(&mut dense, UNTIL);
        let mut sparse =
            Session::new(setup(), window(240, 7), &BOTH).expect("both seats are seated");
        play(&mut sparse, UNTIL);

        for at in 0..UNTIL.0 {
            assert_eq!(
                sparse.hash_at(Tick(at)),
                dense.hash_at(Tick(at)),
                "the state re-stepped at {at} is not the one that was kept"
            );
        }
        assert_eq!(sparse.hash_at(UNTIL.next()), None);
    }

    #[test]
    fn a_session_holds_no_state_or_outcome_from_outside_its_window() {
        let span = 8;
        let mut session = session(span);
        run(&mut session, 40);

        let oldest = session.state().tick().back(span);
        assert!(session.hash_at(oldest).is_some());
        assert!(session.hash_at(oldest.back(1)).is_none());
        assert!(session.outcome_at(oldest).is_some());
        assert!(session.outcome_at(oldest.back(2)).is_none());
    }

    #[test]
    #[ignore = "cost report: cargo test -p neumannarch-sim --release -- --ignored --nocapture"]
    fn the_worst_rewind_the_window_allows_fits_the_budget() {
        let span = Retention::shipped().span();
        let mut session =
            Session::new(setup(), Retention::shipped(), &BOTH).expect("both seats are seated");
        for at in 0..100u32 {
            let place = asteroid(at % 21);
            let body = session.live.spawn_body(place, Time::ZERO);
            session.live.spawn(
                SeatId((at / 21 % 2) as u8),
                FRIGATE,
                place,
                Motion::Steered { body },
            );
        }
        let entities = session.live.entities().count();
        run(&mut session, u64::from(span) + 1);
        let oldest = session.oldest();

        #[expect(
            clippy::disallowed_types,
            reason = "a test measuring wall time is not the sim reading a clock"
        )]
        let started = std::time::Instant::now();
        let rewound = session.insert(stamped(oldest.0, want(1, 0, asteroid(4), FRIGATE, 1)));
        let took = started.elapsed().as_secs_f64();

        assert_eq!(rewound, Ok(Rewound::From(oldest)));
        println!(
            "a rewind of the whole {span}-tick window at {entities} entities: {:.1} ms, against {:.1} ms of sim time",
            took * 1e3,
            Tick(u64::from(span)).seconds() * 1e3
        );
    }
}
