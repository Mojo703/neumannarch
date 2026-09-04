//! Who holds and who runs each seat of a match: the one table a machine
//! builds its controllers from.

use probe_sim::{SeatId, Setup};
use serde::{Deserialize, Serialize};

use crate::ids::PlayerId;
use crate::lobby::Bot;

/// What holds one seat of a match. A closed slot holds no seat, so it is
/// not one of these.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Hash, Serialize)]
pub enum Holder {
    /// A player may still take it; the match does not start while one does.
    Open,
    /// A person.
    Player(PlayerId),
    /// A scripted opponent.
    Bot(Bot),
}

/// The match a lobby froze into: the state every machine builds, and who
/// runs each of its seats.
///
/// Deserialising goes through [`Started::new`], so a started match off the
/// wire seats every seat its setup holds, wherever it came from.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(try_from = "Frozen")]
pub struct Started {
    setup: Setup,
    seating: Seating,
}

/// Why a setup and a seating are not one started match.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Unpaired {
    /// Seats the setup holds.
    pub seats: usize,
    /// Seats the seating holds.
    pub held: usize,
}

/// A [`Started`]'s fields as they travel, before the two are paired.
#[derive(Deserialize)]
struct Frozen {
    setup: Setup,
    seating: Seating,
}

/// The seats one machine runs: its own where it holds one, and every bot it
/// was left to run.
///
/// A frozen lobby seats every machine in it, so a machine of a started
/// match always has one, and it always has a seat to play and to watch.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Crew {
    player: PlayerId,
    watched: SeatId,
    seats: Vec<SeatId>,
}

/// Who holds each seat of a match, in seat order, and the host that runs
/// its bots.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Hash, Serialize)]
pub struct Seating {
    holders: Vec<Holder>,
    host: PlayerId,
}

impl Holder {
    /// The machine that runs it: a person's own, and the host's for a bot,
    /// since only the host seats one. `None` for a seat nobody holds.
    pub fn owner(self, host: PlayerId) -> Option<PlayerId> {
        match self {
            Holder::Player(player) => Some(player),
            Holder::Bot(_) => Some(host),
            Holder::Open => None,
        }
    }
}

impl Started {
    /// The match `setup` builds with `seating` running its seats, or why
    /// the two are not one match.
    pub fn new(setup: Setup, seating: Seating) -> Result<Started, Unpaired> {
        let (seats, held) = (setup.teams().len(), seating.seats().count());
        match seats == held {
            true => Ok(Started { setup, seating }),
            false => Err(Unpaired { seats, held }),
        }
    }

    /// What every machine builds its initial state from.
    pub fn setup(&self) -> &Setup {
        &self.setup
    }

    /// Who runs each seat of the match.
    pub fn seating(&self) -> &Seating {
        &self.seating
    }

    /// The setup and the seating, for the machine that plays them.
    pub fn parts(self) -> (Setup, Seating) {
        (self.setup, self.seating)
    }
}

impl Crew {
    /// Every seat it runs, in seat order; never empty.
    pub fn seats(&self) -> &[SeatId] {
        &self.seats
    }

    /// The seat its display follows: the machine's own where it holds one,
    /// else the first seat it runs.
    pub fn watched(&self) -> SeatId {
        self.watched
    }

    /// The machine these seats are run by.
    pub fn player(&self) -> PlayerId {
        self.player
    }
}

impl Seating {
    /// The seats `holders` name in seat order, with `host` running every
    /// bot among them.
    pub(crate) fn new(holders: Vec<Holder>, host: PlayerId) -> Seating {
        Seating { holders, host }
    }

    /// What `player` runs of this match; `None` for a machine with no seat
    /// here, which a frozen lobby holds none of.
    pub fn run_by(&self, player: PlayerId) -> Option<Crew> {
        let seats: Vec<SeatId> = self.seats_of(player).collect();
        let watched = self.seat(player).or_else(|| seats.first().copied())?;
        Some(Crew {
            player,
            watched,
            seats,
        })
    }

    /// The machine that runs `seat`; `None` for a seat nobody holds and for
    /// no such seat.
    pub fn owner(&self, seat: SeatId) -> Option<PlayerId> {
        self.holders.get(usize::from(seat.0))?.owner(self.host)
    }

    /// Every machine with a seat to run, in id order, each once.
    pub fn players(&self) -> Vec<PlayerId> {
        let mut players: Vec<PlayerId> = self
            .holders
            .iter()
            .filter_map(|holder| holder.owner(self.host))
            .collect();
        players.sort_unstable();
        players.dedup();
        players
    }

    /// How many machines other than `player` run a seat of this match.
    pub fn peers_of(&self, player: PlayerId) -> usize {
        self.players()
            .iter()
            .filter(|other| **other != player)
            .count()
    }

    /// The seat `player` holds in person, if any. A bot's seat is run by
    /// the host and held by nobody, so it is not this.
    pub fn seat(&self, player: PlayerId) -> Option<SeatId> {
        self.seats()
            .find(|(_, holder)| *holder == Holder::Player(player))
            .map(|(seat, _)| seat)
    }

    /// Every seat, with what holds it, in seat order.
    pub fn seats(&self) -> impl Iterator<Item = (SeatId, Holder)> + '_ {
        self.holders
            .iter()
            .copied()
            .zip(0u8..)
            .map(|(holder, seat)| (SeatId(seat), holder))
    }

    /// Every seat `player` runs, in seat order: its own, and each bot where
    /// it is the host.
    pub fn seats_of(&self, player: PlayerId) -> impl Iterator<Item = SeatId> + '_ {
        self.seats()
            .filter(move |(_, holder)| holder.owner(self.host) == Some(player))
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
