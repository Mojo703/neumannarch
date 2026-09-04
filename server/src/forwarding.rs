use std::collections::BTreeMap;

use neumannarch_protocol::{Message, PlayerId, Record, Relayed, Seating, Started};
use neumannarch_sim::{SeatId, Setup, Stamped, Tick};

use crate::records::Log;
use crate::rooms::{Outbound, Recipient};

const FOREVER: Tick = Tick(u64::MAX);

const KEPT_REPORTS: usize = 64;

pub(crate) struct Forwarding {
    setup: Setup,
    seating: Seating,
    ledger: Log,
    reports: BTreeMap<Tick, Vec<(PlayerId, u64)>>,
    desynced: Option<Tick>,
    gone: Vec<PlayerId>,
}

impl Forwarding {
    pub(crate) fn started(started: Started) -> Forwarding {
        let (setup, seating) = started.parts();
        Forwarding {
            ledger: Log::of(setup.clock()),
            setup,
            seating,
            reports: BTreeMap::new(),
            desynced: None,
            gone: Vec::new(),
        }
    }

    pub(crate) fn acknowledged(&self, from: PlayerId, seat: SeatId, up_to: Tick) -> Vec<Outbound> {
        match self.seating.owner(seat) == Some(from) {
            true => vec![Outbound::to(
                Recipient::EveryoneElse,
                Message::Relayed(Relayed::Acknowledge { seat, up_to }),
            )],
            false => Vec::new(),
        }
    }

    pub(crate) fn commanded(&mut self, from: PlayerId, stamped: Stamped) -> Vec<Outbound> {
        match self.seating.owner(stamped.issued.seat) == Some(from)
            && self.ledger.take(stamped).is_ok()
        {
            true => vec![Outbound::to(
                Recipient::EveryoneElse,
                Message::Relayed(Relayed::Command(stamped)),
            )],
            false => Vec::new(),
        }
    }

    pub(crate) fn left(&mut self, who: PlayerId) -> Vec<Outbound> {
        if !self.gone.contains(&who) {
            self.gone.push(who);
        }
        self.seating
            .seats_of(who)
            .map(|seat| {
                Outbound::to(
                    Recipient::Everyone,
                    Message::Relayed(Relayed::Acknowledge {
                        seat,
                        up_to: FOREVER,
                    }),
                )
            })
            .collect()
    }

    pub(crate) fn record(&self) -> Record {
        self.ledger.record(self.setup.clone())
    }

    pub(crate) fn reported(&mut self, from: PlayerId, tick: Tick, hash: u64) -> Vec<Outbound> {
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
                vec![Outbound::to(
                    Recipient::Everyone,
                    Message::Relayed(Relayed::Desync { tick }),
                )]
            }
            (false, true) => vec![Outbound::to(
                Recipient::Everyone,
                Message::Relayed(Relayed::Hash { tick, hash }),
            )],
            (false, false) => Vec::new(),
        }
    }

    pub(crate) fn members(&self) -> Vec<PlayerId> {
        self.seating
            .players()
            .into_iter()
            .filter(|player| !self.gone.contains(player))
            .collect()
    }
}
