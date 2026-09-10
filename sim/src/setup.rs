use serde::{Deserialize, Serialize};

use crate::ids::TeamId;
use crate::state::Draft;
use crate::time::{Tick, Time};

pub const MAX_SEATS: usize = 4;

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Hash, Serialize)]
#[serde(try_from = "Fields")]
pub struct Setup {
    teams: Vec<TeamId>,
    seed: u64,
    clock: Time,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BadSetup {
    NoSeats,
    TooManySeats,
}

#[derive(Deserialize)]
struct Fields {
    teams: Vec<TeamId>,
    seed: u64,
    clock: Time,
}

impl Setup {
    pub fn new(teams: Vec<TeamId>, seed: u64, clock: Time) -> Result<Setup, BadSetup> {
        match teams.len() {
            0 => Err(BadSetup::NoSeats),
            count if count > MAX_SEATS => Err(BadSetup::TooManySeats),
            _ => Ok(Setup { teams, seed, clock }),
        }
    }

    pub fn teams(&self) -> &[TeamId] {
        &self.teams
    }

    pub fn seed(&self) -> u64 {
        self.seed
    }

    pub fn clock(&self) -> Time {
        self.clock
    }

    pub fn ends_by(&self) -> Tick {
        Draft::ends_by(self.teams.len()).after(self.clock)
    }
}

impl core::fmt::Display for BadSetup {
    fn fmt(&self, out: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            BadSetup::NoSeats => out.write_str("a match with no seats"),
            BadSetup::TooManySeats => write!(out, "a match of more than {MAX_SEATS} seats"),
        }
    }
}

impl TryFrom<Fields> for Setup {
    type Error = BadSetup;

    fn try_from(fields: Fields) -> Result<Setup, BadSetup> {
        Setup::new(fields.teams, fields.seed, fields.clock)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TICKS_PER_SECOND;
    use crate::state::{Batch, State};

    fn teams(count: usize) -> Vec<TeamId> {
        (0..count).map(|at| TeamId(at as u8)).collect()
    }

    #[test]
    fn a_setup_seats_one_to_four_teams_and_refuses_any_other_count_by_name() {
        assert_eq!(Setup::new(Vec::new(), 0, Time(1)), Err(BadSetup::NoSeats));
        assert_eq!(
            Setup::new(teams(MAX_SEATS), 0, Time(1))
                .expect("four seats are a match")
                .teams()
                .len(),
            MAX_SEATS
        );
        assert_eq!(
            Setup::new(teams(MAX_SEATS + 1), 0, Time(1)),
            Err(BadSetup::TooManySeats)
        );
    }

    #[test]
    fn a_match_nobody_places_in_runs_to_the_tick_its_setup_ends_by_and_no_further() {
        let clock = Time(2 * u64::from(TICKS_PER_SECOND));
        for seats in 1..=MAX_SEATS {
            let setup = Setup::new(teams(seats), 0, clock).expect("one to four seats are a match");
            let nothing = Batch::new();
            let mut state = State::start(&setup);
            while !state.standings().over() {
                let (next, _) = state.step(&nothing);
                state = next;
            }

            assert_eq!(
                state.draft().ended(),
                Some(Draft::ends_by(seats)),
                "a draft no seat places in runs its longest at {seats} seats"
            );
            assert_eq!(state.tick(), setup.ends_by());
        }
    }
}
