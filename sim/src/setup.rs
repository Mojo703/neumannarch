//! What a match starts from, the same value on every machine.

use serde::{Deserialize, Serialize};

use crate::ids::TeamId;
use crate::time::Tick;

/// The most seats a match holds.
pub const MAX_SEATS: usize = 4;

/// What every machine of a match builds its initial state from: who sits
/// where, the map's seed, and the tick the match ends at.
///
/// Deserialising goes through [`Setup::new`], so a setup off the wire is
/// checked for its seat count once, wherever it came from.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Hash, Serialize)]
#[serde(try_from = "Fields")]
pub struct Setup {
    teams: Vec<TeamId>,
    seed: u64,
    clock: Tick,
}

/// Why a setup is not a match.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BadSetup {
    /// No seats at all.
    NoSeats,
    /// More seats than [`MAX_SEATS`].
    TooManySeats,
}

/// A [`Setup`]'s fields as they travel, before the seat count is checked.
#[derive(Deserialize)]
struct Fields {
    teams: Vec<TeamId>,
    seed: u64,
    clock: Tick,
}

impl Setup {
    /// A match of one seat per entry of `teams`, in seat order, ending at
    /// `clock`.
    pub fn new(teams: Vec<TeamId>, seed: u64, clock: Tick) -> Result<Setup, BadSetup> {
        match teams.len() {
            0 => Err(BadSetup::NoSeats),
            count if count > MAX_SEATS => Err(BadSetup::TooManySeats),
            _ => Ok(Setup { teams, seed, clock }),
        }
    }

    /// Each seat's team, in seat order; never empty.
    pub fn teams(&self) -> &[TeamId] {
        &self.teams
    }

    /// The map's seed.
    pub fn seed(&self) -> u64 {
        self.seed
    }

    /// The tick the match ends at.
    pub fn clock(&self) -> Tick {
        self.clock
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

    fn teams(count: usize) -> Vec<TeamId> {
        (0..count).map(|at| TeamId(at as u8)).collect()
    }

    #[test]
    fn a_setup_seats_one_to_four_teams_and_refuses_any_other_count_by_name() {
        assert_eq!(Setup::new(Vec::new(), 0, Tick(1)), Err(BadSetup::NoSeats));
        assert_eq!(
            Setup::new(teams(MAX_SEATS), 0, Tick(1))
                .expect("four seats are a match")
                .teams()
                .len(),
            MAX_SEATS
        );
        assert_eq!(
            Setup::new(teams(MAX_SEATS + 1), 0, Tick(1)),
            Err(BadSetup::TooManySeats)
        );
    }
}
