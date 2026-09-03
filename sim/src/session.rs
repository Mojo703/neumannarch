//! The match as it is played: a state, and the command log that reproduces
//! it.

use crate::state::{Issued, Rejected, State};
use crate::step::Outcome;
use crate::step::fire::Shots;
use crate::time::Tick;

/// One match in progress. Every command issued is logged with the tick it
/// was applied at, so the log and the initial state are the replay.
#[derive(Clone, Debug)]
pub struct Session {
    state: State,
    log: Vec<(Tick, Issued)>,
    outcome: Outcome,
}

impl Session {
    /// A session over `state`, with nothing logged and no step taken.
    pub fn new(state: State) -> Session {
        Session {
            state,
            log: Vec::new(),
            outcome: Outcome::default(),
        }
    }

    /// Logs `issued`, steps one tick, and returns what was rejected. A
    /// rejected command is logged too: a replay rejects it again.
    pub fn advance(&mut self, issued: Vec<Issued>) -> &[(Issued, Rejected)] {
        let tick = self.state.tick();
        self.log.extend(issued.iter().map(|issued| (tick, *issued)));
        let (next, outcome) = self.state.step(&issued);
        self.state = next;
        self.outcome = outcome;
        &self.outcome.rejected
    }

    /// The state at the current tick.
    pub fn state(&self) -> &State {
        &self.state
    }

    /// The shots the last step resolved; none before the first step. A
    /// fogged view of this tick takes it, since the state records no shot.
    pub fn shots(&self) -> &Shots {
        &self.outcome.shots
    }

    /// Every command applied so far, with the tick it was applied at, in
    /// the order applied.
    pub fn log(&self) -> &[(Tick, Issued)] {
        &self.log
    }

    /// `initial` stepped to `until`, applying `log` at the ticks it names.
    /// The log must be in tick order, as a session's own log is.
    pub fn replay(initial: State, log: &[(Tick, Issued)], until: Tick) -> State {
        let mut state = initial;
        let mut at = 0;
        while state.tick() < until {
            let tick = state.tick();
            let from = at;
            while log.get(at).is_some_and(|(logged, _)| *logged == tick) {
                at += 1;
            }
            let issued: Vec<Issued> = log[from..at].iter().map(|(_, issued)| *issued).collect();
            let (next, _) = state.step(&issued);
            state = next;
        }
        state
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TICKS_PER_SECOND;
    use crate::belt::Belt;
    use crate::ids::{RockId, RowId, SeatId, TeamId};
    use crate::place::{Band, Place};
    use crate::roster::{CONSTRUCTOR, FRIGATE, SHIPYARD};
    use crate::state::Command;

    fn inner(rock: u32) -> Place {
        Place {
            rock: RockId(rock),
            band: Band::Inner,
        }
    }

    fn want(seat: u8, place: Place, row: RowId, count: u32) -> Issued {
        Issued {
            seat: SeatId(seat),
            command: Command::Want { place, row, count },
        }
    }

    fn start() -> State {
        State::start(
            Tick(15 * 60 * TICKS_PER_SECOND as u64),
            Belt::GRAVITY,
            Belt::fixed(Belt::GRAVITY),
            &[TeamId(0), TeamId(1)],
        )
    }

    /// A match both sides play: each places its reserve, seat zero sends
    /// its constructor to the next rock and builds a frigate over seat
    /// one's constructor, and the shots land.
    fn scripted() -> Session {
        let mut session = Session::new(start());
        session.advance(vec![
            want(0, inner(0), SHIPYARD, 1),
            want(0, inner(0), CONSTRUCTOR, 1),
            want(1, inner(0), CONSTRUCTOR, 1),
        ]);
        session.advance(vec![want(0, inner(0), FRIGATE, 1)]);
        for _ in 0..100 {
            session.advance(Vec::new());
        }
        session.advance(vec![
            want(0, inner(0), CONSTRUCTOR, 0),
            want(0, inner(1), CONSTRUCTOR, 1),
        ]);
        for _ in 0..1_000 {
            session.advance(Vec::new());
        }
        session.advance(vec![want(0, inner(0), FRIGATE, 2)]);
        for _ in 0..200 {
            session.advance(Vec::new());
        }
        session
    }

    #[test]
    fn a_replay_of_the_log_reproduces_the_live_hash() {
        let session = scripted();
        let live = session.state();
        assert_eq!(live.tick(), Tick(1_304));
        // The match must have done something for the hash to be worth
        // reproducing: a placement, a build, a send and a shot.
        assert!(live.entities().count() >= 3, "the match built nothing");
        assert!(
            live.entities().any(|entity| entity.row() == FRIGATE),
            "the frigate was never built"
        );
        assert!(
            live.entities()
                .any(|entity| entity.seat() == SeatId(0) && entity.home() == inner(1)),
            "the send never happened"
        );
        assert!(
            live.entities()
                .filter(|entity| entity.seat() == SeatId(1))
                .all(|entity| entity.hp() < live[entity.row()].hp.0),
            "no shot landed"
        );
        assert_eq!(live.frames().len(), 1, "the second frigate is building");

        let replayed = Session::replay(start(), session.log(), live.tick());

        assert_eq!(replayed.tick(), live.tick());
        assert_eq!(replayed.hash(), live.hash());
        assert_eq!(&replayed, live);
    }

    #[test]
    fn a_replay_can_stop_short_of_the_log() {
        let session = scripted();
        let part = Session::replay(start(), session.log(), Tick(200));
        assert_eq!(part.tick(), Tick(200));
        let rest = Session::replay(start(), session.log(), Tick(300));
        assert_eq!(rest.tick(), Tick(300));
        assert_ne!(part.hash(), rest.hash());
    }

    #[test]
    fn a_rejected_command_is_reported_and_logged() {
        let mut session = Session::new(start());
        let issued = want(0, inner(0), SHIPYARD, 1_000_000);
        assert_eq!(
            session.advance(vec![issued]),
            [(issued, crate::state::Rejected::TooMany)]
        );
        assert_eq!(session.log().len(), 1);
        let replayed = Session::replay(start(), session.log(), session.state().tick());
        assert_eq!(replayed.hash(), session.state().hash());
    }
}
