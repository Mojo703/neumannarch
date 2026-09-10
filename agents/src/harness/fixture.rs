use std::collections::BTreeMap;

use neumannarch_sim::pattern::EntityPattern;
use neumannarch_sim::state::view::View;
use neumannarch_sim::state::{Command, State};
use neumannarch_sim::step::fire::Shots;
use neumannarch_sim::{
    AsteroidId, Retention, SeatId, Sequence, Session, Setup, Stamped, TICKS_PER_SECOND,
};

use crate::Seated;
use crate::bots::scripted::Scripted;
use crate::bots::scripted::personality::Personality;
use crate::bots::scripted::survey::Survey;
use crate::harness::played_match::{BELT_SEED, free_for_all, minutes};

const CLOCK_MINUTES: u64 = 15;

pub(crate) struct Fixture {
    session: Session,
    playing: Vec<Seated>,
    hands: BTreeMap<SeatId, Sequence>,
}

impl Fixture {
    pub(crate) fn seating(seats: [Option<Personality>; 2]) -> Fixture {
        let setup = Setup::new(free_for_all(seats.len()), BELT_SEED, minutes(CLOCK_MINUTES))
            .expect("a free for all of two seats is a match");
        let ids: Vec<SeatId> = (0..seats.len())
            .map(|at| SeatId(u8::try_from(at).expect("a seat a personality")))
            .collect();
        let mut fixture = Fixture {
            session: Session::new(setup, Retention::shipped(), &ids)
                .expect("this machine holds every seat"),
            playing: Vec::new(),
            hands: BTreeMap::new(),
        };
        for (seat, personality) in ids.into_iter().zip(seats) {
            match personality {
                Some(personality) => fixture
                    .playing
                    .push(Seated::new(seat, Box::new(Scripted::new(personality)))),
                None => {
                    fixture.hands.insert(seat, Sequence::new(seat));
                }
            }
        }
        fixture
    }

    pub(crate) fn drafted(seats: [Option<Personality>; 2]) -> Fixture {
        let mut fixture = Fixture::seating(seats);
        fixture.until(|fixture| !fixture.state().drafting());
        fixture
    }

    pub(crate) fn advance(&mut self) {
        let issued: Vec<Stamped> = self
            .playing
            .iter_mut()
            .flat_map(|seated| seated.issue(&self.session))
            .collect();
        for stamped in issued {
            assert!(
                self.session.insert(stamped).is_ok(),
                "a bot's own command was refused"
            );
        }
        self.session.advance();
    }

    pub(crate) fn run(&mut self, seconds: u64) {
        let until = self
            .state()
            .tick()
            .ahead(u32::try_from(seconds).expect("a span of seconds") * TICKS_PER_SECOND);
        while self.state().tick() < until {
            self.advance();
        }
    }

    pub(crate) fn until(&mut self, wanted: impl Fn(&Fixture) -> bool) {
        while !wanted(self) {
            assert!(
                !self.state().standings().over(),
                "the clock ran out before the match played its way to what the test needs"
            );
            self.advance();
        }
    }

    pub(crate) fn want(
        &mut self,
        seat: u8,
        asteroid: AsteroidId,
        pattern: EntityPattern,
        count: u32,
    ) {
        let tick = self.state().tick();
        let sequence = self
            .hands
            .get_mut(&SeatId(seat))
            .expect("a seat no bot decides for");
        let stamped = sequence.stamp(
            tick,
            Command::Want {
                asteroid,
                pattern,
                count,
            },
        );
        assert!(
            self.session.insert(stamped).is_ok(),
            "the seat's own want was refused"
        );
        self.advance();
    }

    pub(crate) fn state(&self) -> &State {
        self.session.state()
    }

    pub(crate) fn view(&self, seat: u8) -> View {
        let quiet = Shots::default();
        let shots = self
            .session
            .outcome()
            .map_or(&quiet, |outcome| &outcome.shots);
        View::of(self.session.state(), SeatId(seat), shots)
    }

    pub(crate) fn standing(&self, seat: u8) -> Vec<AsteroidId> {
        self.state().occupied_by(SeatId(seat)).collect()
    }

    pub(crate) fn free(&self, count: usize) -> Vec<AsteroidId> {
        self.state()
            .asteroids()
            .map(|(id, _)| id)
            .filter(|asteroid| !self.state().is_taken(*asteroid))
            .take(count)
            .collect()
    }
}

pub(crate) fn surveyed(view: &View) -> Survey<'_> {
    Survey::of(view)
}
