use std::collections::BTreeMap;

use neumannarch_sim::state::Command;
use neumannarch_sim::state::view::View;
use neumannarch_sim::step::fire::Shots;
use neumannarch_sim::{
    AsteroidId, Retention, RowId, SeatId, Sequence, Session, Setup, TICKS_PER_SECOND, TeamId, Time,
};

pub(crate) const PLAYER: SeatId = SeatId(0);

pub(crate) const RIVAL: SeatId = SeatId(1);

const CLOCK: Time = Time(15 * 60 * TICKS_PER_SECOND as u64);

pub(crate) struct Local {
    session: Session,
    sequences: BTreeMap<SeatId, Sequence>,
}

impl Local {
    pub(crate) fn start(teams: u8) -> Local {
        let mut local = Local::drafting(teams);
        local.start_the_clock();
        local
    }

    pub(crate) fn drafting(teams: u8) -> Local {
        let teams = (0..teams).map(TeamId).collect();
        let setup = Setup::new(teams, 0, CLOCK).expect("a match of these teams");
        Local {
            session: Session::new(setup, Retention::shipped(), &[PLAYER])
                .expect("seat zero is seated"),
            sequences: BTreeMap::new(),
        }
    }

    pub(crate) fn start_the_clock(&mut self) {
        while self.session.state().drafting() {
            self.session.advance();
        }
    }

    pub(crate) fn session(&self) -> &Session {
        &self.session
    }

    pub(crate) fn view(&self) -> View {
        self.view_of(PLAYER)
    }

    pub(crate) fn view_of(&self, seat: SeatId) -> View {
        let quiet = Shots::default();
        let shots = self
            .session
            .outcome()
            .map_or(&quiet, |outcome| &outcome.shots);
        View::of(self.session.state(), seat, shots)
    }

    pub(crate) fn want(&mut self, wants: &[(AsteroidId, RowId, u32)]) {
        self.want_of(PLAYER, wants);
    }

    pub(crate) fn want_of(&mut self, seat: SeatId, wants: &[(AsteroidId, RowId, u32)]) {
        for (asteroid, row, count) in wants {
            let command = Command::Want {
                asteroid: *asteroid,
                row: *row,
                count: *count,
            };
            let tick = self.session.state().tick();
            let stamped = self
                .sequences
                .entry(seat)
                .or_insert_with(|| Sequence::new(seat))
                .stamp(tick, command);
            assert!(self.session.insert(stamped).is_ok(), "a want was refused");
        }
        assert!(
            self.session.advance().rejected.is_empty(),
            "a want was rejected"
        );
    }

    pub(crate) fn run(&mut self, ticks: u64) {
        for _ in 0..ticks {
            self.session.advance();
        }
    }
}
