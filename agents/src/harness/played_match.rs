use neumannarch_sim::roster::Roster;
use neumannarch_sim::state::State;
use neumannarch_sim::state::view::View;
use neumannarch_sim::step::fire::Shots;
use neumannarch_sim::{Retention, SeatId, Session, Setup, Stamped, TICKS_PER_SECOND, TeamId, Tick};

use neumannarch_protocol::Bot;

use crate::Seated;
use crate::bots::Shipped;

pub const BELT_SEED: u64 = 1;

pub fn minutes(count: u64) -> Tick {
    Tick(count * 60 * u64::from(TICKS_PER_SECOND))
}

pub fn free_for_all(seats: usize) -> Vec<TeamId> {
    (0..seats)
        .map(|at| TeamId(u8::try_from(at).expect("a team a seat")))
        .collect()
}

pub struct PlayedMatch {
    session: Session,
    seated: Vec<Seated>,
    decided: Vec<SeatId>,
}

impl PlayedMatch {
    pub fn new(setup: Setup, seated: Vec<Seated>) -> PlayedMatch {
        let seats: Vec<SeatId> = (0..setup.teams().len())
            .map(|at| SeatId(u8::try_from(at).expect("a seat a team")))
            .collect();
        PlayedMatch {
            session: Session::new(setup, Retention::shipped(), &seats)
                .expect("one seat per team seats them all"),
            seated,
            decided: Vec::new(),
        }
    }

    pub fn of(seated: &[Bot], clock: Tick) -> PlayedMatch {
        let teams = free_for_all(seated.len());
        let setup = Setup::new(teams, BELT_SEED, clock).expect("a match of these teams");
        let roster = Roster::shipped();
        let playing = seated
            .iter()
            .enumerate()
            .map(|(at, bot)| {
                Seated::new(
                    SeatId(u8::try_from(at).expect("a seat a bot")),
                    Shipped::of(*bot).seated(&roster),
                )
            })
            .collect();
        PlayedMatch::new(setup, playing)
    }

    pub fn advance(&mut self) -> Vec<Stamped> {
        let PlayedMatch {
            session,
            seated,
            decided,
        } = self;
        decided.clear();
        decided.extend(
            seated
                .iter()
                .filter(|seated| seated.deciding(session))
                .map(Seated::seat),
        );
        let issued: Vec<Stamped> = seated
            .iter_mut()
            .flat_map(|seated| seated.issue(session))
            .collect();
        for stamped in &issued {
            let taken = self.session.insert(*stamped);
            assert!(
                taken.is_ok(),
                "an agent's own command was refused: {taken:?}"
            );
        }
        self.session.advance();
        issued
    }

    pub fn decided(&self) -> &[SeatId] {
        &self.decided
    }

    pub fn over(&self) -> bool {
        self.session.state().standings().over()
    }

    pub fn session(&self) -> &Session {
        &self.session
    }

    pub fn state(&self) -> &State {
        self.session.state()
    }

    pub fn view(&self, seat: SeatId) -> View {
        let quiet = Shots::default();
        let shots = self
            .session
            .outcome()
            .map_or(&quiet, |outcome| &outcome.shots);
        View::of(self.session.state(), seat, shots)
    }

    pub fn hits(&self) -> usize {
        self.session
            .outcome()
            .map_or(0, |outcome| outcome.shots.hits.len())
    }
}
