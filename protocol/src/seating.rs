use probe_sim::{SeatId, Setup};
use serde::{Deserialize, Serialize};

use crate::ids::PlayerId;
use crate::lobby::Bot;

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Hash, Serialize)]
pub enum Occupant {
    Player(PlayerId),
    Bot(Bot),
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(try_from = "Frozen")]
pub struct Started {
    setup: Setup,
    seating: Seating,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Unpaired {
    pub seats: usize,
    pub held: usize,
}

#[derive(Deserialize)]
struct Frozen {
    setup: Setup,
    seating: Seating,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Crew {
    player: PlayerId,
    watched: SeatId,
    seats: Vec<SeatId>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Hash, Serialize)]
pub struct Seating {
    holders: Vec<Occupant>,
    host: PlayerId,
}

impl Occupant {
    pub fn owner(self, host: PlayerId) -> PlayerId {
        match self {
            Occupant::Player(player) => player,
            Occupant::Bot(_) => host,
        }
    }
}

impl Started {
    pub fn new(setup: Setup, seating: Seating) -> Result<Started, Unpaired> {
        let (seats, held) = (setup.teams().len(), seating.seats().count());
        match seats == held {
            true => Ok(Started { setup, seating }),
            false => Err(Unpaired { seats, held }),
        }
    }

    pub fn setup(&self) -> &Setup {
        &self.setup
    }

    pub fn seating(&self) -> &Seating {
        &self.seating
    }

    pub fn parts(self) -> (Setup, Seating) {
        (self.setup, self.seating)
    }
}

impl Crew {
    pub fn seats(&self) -> &[SeatId] {
        &self.seats
    }

    pub fn watched(&self) -> SeatId {
        self.watched
    }

    pub fn player(&self) -> PlayerId {
        self.player
    }
}

impl Seating {
    pub(crate) fn new(holders: Vec<Occupant>, host: PlayerId) -> Seating {
        Seating { holders, host }
    }

    pub fn run_by(&self, player: PlayerId) -> Option<Crew> {
        let seats: Vec<SeatId> = self.seats_of(player).collect();
        let watched = self.seat(player).or_else(|| seats.first().copied())?;
        Some(Crew {
            player,
            watched,
            seats,
        })
    }

    pub fn owner(&self, seat: SeatId) -> Option<PlayerId> {
        self.holders
            .get(usize::from(seat.0))
            .map(|holder| holder.owner(self.host))
    }

    pub fn players(&self) -> Vec<PlayerId> {
        let mut players: Vec<PlayerId> = self
            .holders
            .iter()
            .map(|holder| holder.owner(self.host))
            .collect();
        players.sort_unstable();
        players.dedup();
        players
    }

    pub fn peers_of(&self, player: PlayerId) -> usize {
        self.players()
            .iter()
            .filter(|other| **other != player)
            .count()
    }

    pub fn seat(&self, player: PlayerId) -> Option<SeatId> {
        self.seats()
            .find(|(_, holder)| *holder == Occupant::Player(player))
            .map(|(seat, _)| seat)
    }

    pub fn seats(&self) -> impl Iterator<Item = (SeatId, Occupant)> + '_ {
        self.holders
            .iter()
            .copied()
            .zip(0u8..)
            .map(|(holder, seat)| (SeatId(seat), holder))
    }

    pub fn seats_of(&self, player: PlayerId) -> impl Iterator<Item = SeatId> + '_ {
        self.seats()
            .filter(move |(_, holder)| holder.owner(self.host) == player)
            .map(|(seat, _)| seat)
    }
}

impl core::fmt::Display for Unpaired {
    fn fmt(&self, out: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            out,
            "a match of {} seats seated for {}",
            self.seats, self.held
        )
    }
}

impl TryFrom<Frozen> for Started {
    type Error = Unpaired;

    fn try_from(frozen: Frozen) -> Result<Started, Unpaired> {
        Started::new(frozen.setup, frozen.seating)
    }
}
