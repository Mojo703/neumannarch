//! The match as it is played: the live state, the log that reproduces it,
//! and the rewind that takes a command learned late.

use std::borrow::Cow;
use std::collections::BTreeMap;

use crate::history::log::Log;
use crate::history::snapshots::{Retention, Snapshots};
use crate::ids::SeatId;
use crate::setup::Setup;
use crate::state::{Refused, Stamped, State};
use crate::step::Outcome;
use crate::time::Tick;

/// What an insert did to the ticks a session had already stepped.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rewound {
    /// The command's tick was not stepped yet, so nothing was re-stepped.
    Nothing,
    /// This tick's outcome and every later one may now differ, and so may
    /// every state after it.
    From(Tick),
}

/// A local seat the setup does not seat, which no frozen lobby produces.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Unseated {
    pub seat: SeatId,
}

/// One match in progress: what every frontend drives.
pub struct Session {
    setup: Setup,
    initial: State,
    live: State,
    snapshots: Snapshots,
    log: Log,
    outcomes: BTreeMap<Tick, Outcome>,
    /// Per seat, the tick before which none of its commands are unknown.
    acknowledged: Vec<Tick>,
    /// The seats this machine owns, in id order.
    local: Vec<SeatId>,
}

impl Session {
    /// A session over the match `setup` names, keeping the states
    /// `retention` names and acknowledging the seats `local` names as it
    /// advances. A local seat the setup does not seat is refused, since a
    /// match would answer it by never settling.
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

    /// The state at the tick this session shows.
    pub fn state(&self) -> &State {
        &self.live
    }

    /// One tick on, with the commands logged for the tick it leaves, and
    /// that tick's outcome. Acknowledges every local seat up to the tick it
    /// arrives at.
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

    /// Takes `stamped` into the log at the tick it names, re-stepping from
    /// there when that tick is already stepped, or refuses it by name.
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

    /// Records that no command of `seat` before `up_to` is unknown. A
    /// seat's acknowledgement never goes backward, and a seat the match
    /// does not have holds nothing to record.
    pub fn acknowledge(&mut self, seat: SeatId, up_to: Tick) {
        if let Some(known) = self.acknowledged.get_mut(usize::from(seat.0)) {
            *known = (*known).max(up_to);
        }
    }

    /// The smallest acknowledgement over the seats: the state at it, and
    /// every state before it, is final. It can name a tick this session has
    /// not stepped, when every seat's next command is still ahead.
    pub fn settled(&self) -> Tick {
        self.acknowledged
            .iter()
            .copied()
            .min()
            .unwrap_or(Tick::ZERO)
    }

    /// The desync hash at `tick`. `None` past the tick this session shows,
    /// or further back than it can rewind.
    pub fn hash_at(&self, tick: Tick) -> Option<u64> {
        self.at(tick).map(|state| state.hash())
    }

    /// The outcome of stepping `tick`, whose shots belong to a view of the
    /// state at the next tick. `None` for a tick this session has not
    /// stepped or has forgotten.
    pub fn outcome_at(&self, tick: Tick) -> Option<&Outcome> {
        self.outcomes.get(&tick)
    }

    /// The outcome of the step that produced the tick this session shows.
    pub fn outcome(&self) -> Option<&Outcome> {
        self.live
            .tick()
            .previous()
            .and_then(|at| self.outcome_at(at))
    }

    /// What the match was set up as, the same value on every machine.
    pub fn setup(&self) -> &Setup {
        &self.setup
    }

    /// Every command stamped before the settled tick, in tick then
    /// `(seat, seq)` order: what a record of this match holds.
    pub fn commands(&self) -> Vec<Stamped> {
        self.log.until(self.settled()).stamped().collect()
    }

    /// One tick from the live state: its snapshot kept if the policy names
    /// it, its logged commands applied, its outcome recorded.
    fn stepped(&mut self) {
        let tick = self.live.tick();
        self.snapshots.keep(&self.live);
        let (next, outcome) = self.live.step(self.log.at(tick));
        self.live = next;
        self.outcomes.insert(tick, outcome);
    }

    /// The state at `from` restored and re-stepped to the tick the session
    /// shows, replacing the outcomes on the way.
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

    /// The state at `tick`, borrowed where the session holds it and
    /// re-stepped from the newest kept state before it where it does not.
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

    /// The oldest tick a command can still be inserted at.
    fn oldest(&self) -> Tick {
        self.live.tick().back(self.snapshots.span())
    }

    /// Drops the states that have left the window and the outcomes of the
    /// ticks they produced.
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
    use crate::ids::{RockId, RowId, TeamId};
    use crate::place::{Band, Place};
    use crate::roster::{CONSTRUCTOR, FRIGATE, SHIPYARD};
    use crate::state::{Command, Issued, MAX_COMMANDS_PER_TICK, Motion};
    use crate::step::maneuver::Maneuver;

    /// A clock no test reaches.
    const CLOCK: Tick = Tick(15 * 60 * TICKS_PER_SECOND as u64);

    const BOTH: [SeatId; 2] = [SeatId(0), SeatId(1)];

    /// How far a test match plays.
    const UNTIL: Tick = Tick(40);

    fn setup() -> Setup {
        Setup::new(vec![TeamId(0), TeamId(1)], 11, CLOCK).expect("two seats are a match")
    }

    /// A session of two seats, both this machine's, keeping every tick of
    /// `span` ticks.
    fn session(span: u32) -> Session {
        Session::new(setup(), window(span, 1), &BOTH).expect("both seats are seated")
    }

    /// A session of the same match with seat one another machine's, so it
    /// settles only as that seat is acknowledged.
    fn one_local() -> Session {
        Session::new(setup(), Retention::shipped(), &[SeatId(0)]).expect("seat zero is seated")
    }

    fn window(ticks: u32, every: u32) -> Retention {
        Retention::Window {
            ticks,
            every: NonZeroU32::new(every).expect("a stride of no ticks keeps nothing"),
        }
    }

    fn inner(rock: u32) -> Place {
        Place {
            rock: RockId(rock),
            band: Band::Inner,
        }
    }

    fn want(seat: u8, seq: u32, place: Place, row: RowId, count: u32) -> Issued {
        Issued {
            seat: SeatId(seat),
            seq,
            command: Command::Want { place, row, count },
        }
    }

    fn stamped(tick: u64, issued: Issued) -> Stamped {
        Stamped {
            tick: Tick(tick),
            issued,
        }
    }

    /// The commands a test match plays: both seats place their reserve,
    /// then seat zero builds a frigate and moves its constructor on. Every
    /// one of them changes the state it is applied to.
    fn script() -> Vec<Stamped> {
        vec![
            stamped(3, want(0, 0, inner(0), SHIPYARD, 1)),
            stamped(3, want(0, 1, inner(0), CONSTRUCTOR, 1)),
            stamped(3, want(1, 0, inner(4), CONSTRUCTOR, 1)),
            stamped(9, want(0, 2, inner(0), FRIGATE, 1)),
            stamped(20, want(0, 3, inner(0), CONSTRUCTOR, 0)),
            stamped(20, want(0, 4, inner(1), CONSTRUCTOR, 1)),
            stamped(31, want(1, 1, inner(4), SHIPYARD, 1)),
        ]
    }

    /// The script played to `until` with every command inserted at the tick
    /// it takes effect at, as a local frontend's own commands are.
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
        let mut on_time = session(240);
        play(&mut on_time, UNTIL);

        let mut late = session(240);
        run(&mut late, UNTIL.0);
        assert_ne!(
            late.hash_at(UNTIL),
            on_time.hash_at(UNTIL),
            "the commands did nothing, so nothing is proven"
        );
        for stamped in script().into_iter().rev() {
            assert_eq!(late.insert(stamped), Ok(Rewound::From(stamped.tick)));
        }

        assert_eq!(late.hash_at(UNTIL), on_time.hash_at(UNTIL));
        assert_eq!(late.state(), on_time.state());
    }

    #[test]
    fn a_command_the_session_will_not_take_is_refused_by_name() {
        let span = 8;
        let mut session = session(span);
        run(&mut session, 20);
        let latest = session.state().tick();
        let issued = want(0, 0, inner(0), SHIPYARD, 1);

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
                    .insert(stamped(latest.0, want(0, seq, inner(0), SHIPYARD, 1)))
                    .is_ok()
            );
        }
        let over = MAX_COMMANDS_PER_TICK as u32;
        assert_eq!(
            session.insert(stamped(latest.0, want(0, over, inner(0), SHIPYARD, 1))),
            Err(Refused::TooMany)
        );
        assert!(
            session
                .insert(stamped(latest.0, want(1, over, inner(0), SHIPYARD, 1)))
                .is_ok(),
            "the cap is one seat's"
        );
    }

    #[test]
    fn the_commands_a_session_hands_out_are_those_stamped_before_its_settled_tick() {
        let mut session = one_local();
        for stamped in script() {
            assert!(session.insert(stamped).is_ok());
        }
        run(&mut session, UNTIL.0);
        session.acknowledge(SeatId(1), Tick(10));
        let settled = session.settled();

        assert_eq!(session.setup(), &setup());
        assert_eq!(
            session.commands(),
            script()
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
    #[ignore = "cost report: cargo test -p probe-sim --release -- --ignored --nocapture"]
    fn the_worst_rewind_the_window_allows_fits_the_budget() {
        let span = Retention::shipped().span();
        let mut session =
            Session::new(setup(), Retention::shipped(), &BOTH).expect("both seats are seated");
        for at in 0..100u32 {
            let place = inner(at % 21);
            let body = Maneuver::spawn_body(&session.live, place, Tick::ZERO);
            session.live.spawn(
                SeatId((at / 21 % 2) as u8),
                FRIGATE,
                place,
                Motion::Free { body, flight: None },
            );
        }
        let entities = session.live.entities().count();
        run(&mut session, u64::from(span) + 1);
        let oldest = session.oldest();

        // The sim has no clock of its own; a cost report needs one.
        #[expect(
            clippy::disallowed_types,
            reason = "a test measuring wall time is not the sim reading a clock"
        )]
        let started = std::time::Instant::now();
        let rewound = session.insert(stamped(oldest.0, want(1, 0, inner(4), FRIGATE, 1)));
        let took = started.elapsed().as_secs_f64();

        assert_eq!(rewound, Ok(Rewound::From(oldest)));
        println!(
            "a rewind of the whole {span}-tick window at {entities} entities: {:.1} ms, against {:.1} ms of sim time",
            took * 1e3,
            Tick(u64::from(span)).seconds() * 1e3
        );
    }
}
