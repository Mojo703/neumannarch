use probe_sim::state::Command;
use probe_sim::state::view::View;
use probe_sim::step::fire::Shots;
use probe_sim::{
    Place, Retention, RowId, SeatId, Sequence, Session, Setup, TICKS_PER_SECOND, TeamId, Tick,
};

pub(crate) const PLAYER: SeatId = SeatId(0);

const CLOCK: Tick = Tick(15 * 60 * TICKS_PER_SECOND as u64);

pub(crate) struct Local {
    session: Session,
    sequence: Sequence,
}

impl Local {
    pub(crate) fn start(teams: u8) -> Local {
        let teams = (0..teams).map(TeamId).collect();
        let setup = Setup::new(teams, 0, CLOCK).expect("a match of these teams");
        Local {
            session: Session::new(setup, Retention::shipped(), &[PLAYER])
                .expect("seat zero is seated"),
            sequence: Sequence::new(PLAYER),
        }
    }

    pub(crate) fn session(&self) -> &Session {
        &self.session
    }

    pub(crate) fn view(&self) -> View {
        let quiet = Shots::default();
        let shots = self
            .session
            .outcome()
            .map_or(&quiet, |outcome| &outcome.shots);
        View::of(self.session.state(), PLAYER, shots)
    }

    pub(crate) fn want(&mut self, wants: &[(Place, RowId, u32)]) {
        for (place, row, count) in wants {
            let command = Command::Want {
                place: *place,
                row: *row,
                count: *count,
            };
            let stamped = self.sequence.stamp(self.session.state().tick(), command);
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
