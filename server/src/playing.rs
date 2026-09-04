//! The match a room is forwarding: who owns each seat, what has been
//! forwarded, and the hashes the members report.

use std::collections::BTreeMap;

use probe_protocol::{Message, PlayerId, Record, Seating, Started};
use probe_sim::{SeatId, Setup, Stamped, Tick};

use crate::records::Ledger;
use crate::rooms::{Post, To};

/// What a seat whose machine has left is acknowledged to: no command of it
/// will ever arrive, so the room acknowledges it on the seat's behalf and
/// the match settles without it.
const FOREVER: Tick = Tick(u64::MAX);

/// How many settled ticks a room keeps hash reports for. Members report at
/// their own pace, so a tick is held until the others reach it; past this
/// the oldest tick is dropped.
const KEPT_REPORTS: usize = 64;

/// A match in progress, from the room's side. It holds no tick clock and
/// steps no sim: it forwards, it collects hashes, and it declares a desync.
pub(crate) struct Playing {
    setup: Setup,
    /// Who holds each seat, which says whose commands the room forwards.
    seating: Seating,
    ledger: Ledger,
    /// What each member reported at a settled tick.
    reports: BTreeMap<Tick, Vec<(PlayerId, u64)>>,
    /// The tick two members' hashes first differed at.
    desynced: Option<Tick>,
    /// The machines that have left, whose reports never come.
    gone: Vec<PlayerId>,
}

impl Playing {
    /// The match `started` names, with each seat run by the machine its
    /// seating gives it.
    pub(crate) fn started(started: Started) -> Playing {
        let (setup, seating) = started.parts();
        Playing {
            ledger: Ledger::of(setup.clock()),
            setup,
            seating,
            reports: BTreeMap::new(),
            desynced: None,
            gone: Vec::new(),
        }
    }

    /// Passes `up_to` on to every other machine, where `from` owns `seat`.
    pub(crate) fn acknowledged(&self, from: PlayerId, seat: SeatId, up_to: Tick) -> Vec<Post> {
        match self.seating.owner(seat) == Some(from) {
            true => vec![Post::to(
                To::EveryoneElse,
                Message::Acknowledge { seat, up_to },
            )],
            false => Vec::new(),
        }
    }

    /// Takes `stamped` into the record and passes it on to every other
    /// machine, where `from` owns its seat and the log will hold it.
    ///
    /// A command of a seat its sender does not own is dropped rather than
    /// refused: a machine that forges another's seat has not made a
    /// mistake to be told about.
    pub(crate) fn commanded(&mut self, from: PlayerId, stamped: Stamped) -> Vec<Post> {
        match self.seating.owner(stamped.issued.seat) == Some(from)
            && self.ledger.take(stamped).is_ok()
        {
            true => vec![Post::to(To::EveryoneElse, Message::Command(stamped))],
            false => Vec::new(),
        }
    }

    /// Acknowledges every seat `who` owned for the rest of the match, so
    /// the machines still playing settle without it.
    pub(crate) fn left(&mut self, who: PlayerId) -> Vec<Post> {
        if !self.gone.contains(&who) {
            self.gone.push(who);
        }
        self.seating
            .seats_of(who)
            .map(|seat| {
                Post::to(
                    To::Everyone,
                    Message::Acknowledge {
                        seat,
                        up_to: FOREVER,
                    },
                )
            })
            .collect()
    }

    /// The record of what the room has forwarded.
    pub(crate) fn record(&self) -> Record {
        self.ledger.record(self.setup.clone())
    }

    /// Takes `from`'s hash at `tick`: the tick is declared desynced where
    /// the hash is the first to differ from another machine's there, and
    /// agreed where it completes a set every machine reported the same.
    ///
    /// An agreed hash is the room's own word that every machine has reached
    /// that tick with the same state, which is what a machine waits on
    /// before it starts playing.
    pub(crate) fn reported(&mut self, from: PlayerId, tick: Tick, hash: u64) -> Vec<Post> {
        if self.desynced.is_some() || tick > self.setup.clock() {
            return Vec::new();
        }
        let machines = self.members().len();
        let at = self.reports.entry(tick).or_default();
        let differs = at.iter().any(|(_, held)| *held != hash);
        if !at.iter().any(|(who, _)| *who == from) {
            at.push((from, hash));
        }
        let agreed = !differs && at.len() == machines;
        while self.reports.len() > KEPT_REPORTS {
            self.reports.pop_first();
        }
        match (differs, agreed) {
            (true, _) => {
                self.desynced = Some(tick);
                vec![Post::to(To::Everyone, Message::Desync { tick })]
            }
            (false, true) => vec![Post::to(To::Everyone, Message::Hash { tick, hash })],
            (false, false) => Vec::new(),
        }
    }

    /// The machines still in the match: those the seating gives a seat to
    /// and that have not left. How many of them there are is how many
    /// reports one tick is agreed by.
    pub(crate) fn members(&self) -> Vec<PlayerId> {
        self.seating
            .players()
            .into_iter()
            .filter(|player| !self.gone.contains(player))
            .collect()
    }
}
