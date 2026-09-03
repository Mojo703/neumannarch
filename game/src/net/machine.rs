//! This machine's part of one match: every seat stepped, its own seats
//! spoken for, and what its peers are told.

use probe_protocol::{Control, Lobby, Message, PlayerId};
use probe_sim::state::view::View;
use probe_sim::step::fire::Shots;
use probe_sim::{Retention, Rewound, SeatId, Session, Setup, Tick, Unseated};

use crate::net::controller::{Controller, Human};
use crate::net::pace::{ACKNOWLEDGE_INTERVAL, Allowed, Pace, REPORT_INTERVAL};
use crate::net::transport::Transport;

/// What one tick of a machine did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ticked {
    /// What the pace allowed.
    pub pace: Allowed,
    /// Whether a command learned late rewound the session, so a memory the
    /// client kept of what it saw may be stale.
    pub rewound: bool,
}

/// Why this machine cannot play the match it was given.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unplayable {
    /// The setup does not seat a seat this machine owns.
    NoSeat(Unseated),
    /// The lobby holds no slot for the person at this machine.
    NoSlot,
}

/// One match as this machine plays it: the session every seat is stepped
/// in, one controller per seat, and the pace it keeps with its peers. The
/// transport is the caller's, so a room outlives the match played in it.
pub struct Machine {
    seat: SeatId,
    session: Session,
    controllers: Vec<Controller>,
    pace: Pace,
    /// The seats this machine speaks for, in seat order.
    local: Vec<SeatId>,
    /// How many other machines are in this match.
    peers: usize,
    /// Whether every machine has built the match and agreed its first hash.
    agreed: bool,
    /// The tick this machine's history differed from another's at.
    desynced: Option<Tick>,
    /// The settled tick last reported, so each is reported once.
    reported: Tick,
}

impl Machine {
    /// The match `setup` names, as the person holding `me`'s slot of
    /// `lobby` plays it, speaking to its peers through `transport`.
    ///
    /// It reports its own hash at tick zero at once, which is what every
    /// machine agrees the match on before the first step.
    pub fn of(
        lobby: &Lobby,
        setup: Setup,
        me: PlayerId,
        transport: &mut dyn Transport,
    ) -> Result<Machine, Unplayable> {
        let seat = lobby
            .slot_of(me)
            .and_then(|slot| lobby.seat_of(slot))
            .ok_or(Unplayable::NoSlot)?;
        let local: Vec<SeatId> = lobby
            .slots()
            .iter()
            .enumerate()
            .filter(|(_, slot)| mine(lobby, me, slot.control))
            .filter_map(|(at, _)| lobby.seat_of(at))
            .collect();
        let session =
            Session::new(setup, Retention::shipped(), &local).map_err(Unplayable::NoSeat)?;
        let controllers = Controller::of(lobby, me, session.state().roster());
        let peers = peers(lobby, me);
        transport.report(Tick::ZERO, session.state().hash());
        Ok(Machine {
            seat,
            session,
            controllers,
            pace: Pace::shipped(),
            local,
            peers,
            agreed: peers == 0,
            desynced: None,
            reported: Tick::ZERO,
        })
    }

    /// Whether every machine has built the match and agreed its first hash,
    /// which is what the loading screen waits on.
    pub fn agreed(&self) -> bool {
        self.agreed
    }

    /// Whether this machine is the only one in the match, which is what a
    /// skirmish is.
    pub fn alone(&self) -> bool {
        self.peers == 0
    }

    /// The tick this machine's history differed from another's at, after
    /// which the match holds and cannot go on.
    pub fn desynced(&self) -> Option<Tick> {
        self.desynced
    }

    /// The person at this machine, to take a command for the next tick.
    pub fn human(&mut self) -> Option<&mut Human> {
        let seat = self.seat;
        self.controllers
            .iter_mut()
            .find(|controller| controller.seat() == seat)
            .and_then(Controller::human)
    }

    /// Takes in what the peers have said, without stepping the sim: what a
    /// machine does while it waits for the others to build the match.
    pub fn listen(&mut self, transport: &mut dyn Transport) -> bool {
        let mut rewound = false;
        for message in transport.received() {
            rewound |= self.heard(message);
        }
        rewound
    }

    /// The seat the person at this machine plays.
    pub fn seat(&self) -> SeatId {
        self.seat
    }

    /// The session at the tick it shows.
    pub fn session(&self) -> &Session {
        &self.session
    }

    /// One tick of the match: what the peers said, what the pace allows,
    /// what every local seat issues, the step, and what the peers are told.
    ///
    /// A desynced match steps no further: the two histories have already
    /// parted, and stepping either one only widens it.
    pub fn tick(&mut self, transport: &mut dyn Transport) -> Ticked {
        let rewound = self.listen(transport);
        if self.desynced.is_some() {
            return Ticked {
                pace: Allowed::Held,
                rewound,
            };
        }
        let pace = self
            .pace
            .allows(self.session.state().tick(), self.session.settled());
        if pace == Allowed::Advance {
            self.speak(transport);
            self.session.advance();
            self.tell(transport);
        }
        Ticked { pace, rewound }
    }

    /// The fogged view of the tick the session shows, as this machine's
    /// own seat sees it.
    pub fn view(&self) -> View {
        let quiet = Shots::default();
        let shots = self
            .session
            .outcome()
            .map_or(&quiet, |outcome| &outcome.shots);
        View::of(self.session.state(), self.seat, shots)
    }

    /// The seats the match is waiting on: those acknowledged no further
    /// than the settled tick, while that is behind the tick shown.
    pub fn waiting(&self) -> Vec<SeatId> {
        let settled = self.session.settled();
        if settled >= self.session.state().tick() {
            return Vec::new();
        }
        self.controllers
            .iter()
            .map(Controller::seat)
            .filter(|seat| self.session.acknowledged(*seat) == Some(settled))
            .collect()
    }

    /// Takes one message from a peer, and answers whether it rewound the
    /// session.
    fn heard(&mut self, message: Message) -> bool {
        match message {
            Message::Command(stamped) => {
                // A peer outside the window has already been held by the
                // pacing rule, so a refusal here is a peer that was cut off.
                matches!(self.session.insert(stamped), Ok(Rewound::From(_)))
            }
            Message::Acknowledge { seat, up_to } => {
                self.session.acknowledge(seat, up_to);
                false
            }
            // The room sends a hash only once every machine has reported
            // the same one, so one that matches is the match agreed.
            Message::Hash { tick, hash } => {
                match self.session.hash_at(tick) {
                    Some(held) if held != hash => self.desynced = Some(tick),
                    Some(_) => self.agreed |= tick == Tick::ZERO,
                    // A tick this machine keeps no state for is one it
                    // cannot answer for; the room holds every machine's
                    // report and declares the desync itself.
                    None => {}
                }
                false
            }
            Message::Desync { tick } => {
                self.desynced = Some(tick);
                false
            }
            // The room's lobby phase is over once a match is running, and
            // a rematch is asked for from the results.
            Message::Join { .. }
            | Message::Welcome { .. }
            | Message::Edit(_)
            | Message::Lobby(_)
            | Message::Refused(_)
            | Message::Start(_)
            | Message::Rematch
            | Message::Removed
            | Message::Leave => false,
        }
    }

    /// Every local controller's commands, into this session at once and on
    /// to the peers.
    fn speak(&mut self, transport: &mut dyn Transport) {
        let issued: Vec<_> = self
            .controllers
            .iter_mut()
            .flat_map(|controller| controller.issue(&self.session))
            .collect();
        for stamped in issued {
            // A controller stamps the tick the session shows and yields at
            // most that tick's cap, so the batch takes every one.
            self.session
                .insert(stamped)
                .expect("a controller's own command is one this tick takes");
            transport.send(stamped);
        }
    }

    /// What this machine now knows, on the cadences pacing states: how far
    /// its own seats are acknowledged, and its hash at the settled tick.
    fn tell(&mut self, transport: &mut dyn Transport) {
        let latest = self.session.state().tick();
        if latest.0.is_multiple_of(ACKNOWLEDGE_INTERVAL) {
            for seat in &self.local {
                transport.acknowledge(*seat, latest);
            }
        }
        let settled = self.session.settled();
        if settled > self.reported && settled.0.is_multiple_of(REPORT_INTERVAL) {
            self.reported = settled;
            if let Some(hash) = self.session.hash_at(settled) {
                transport.report(settled, hash);
            }
        }
    }
}

/// Whether the machine `me` is at plays `control`: its own slot, and every
/// bot, which only a host seats and only a host runs.
fn mine(lobby: &Lobby, me: PlayerId, control: Control) -> bool {
    match control {
        Control::Player { player, .. } => player == me,
        Control::Bot(_) => lobby.host() == me,
        Control::Open | Control::Closed => false,
    }
}

/// How many other machines play the match `lobby` freezes into.
fn peers(lobby: &Lobby, me: PlayerId) -> usize {
    let mut players: Vec<PlayerId> = lobby
        .slots()
        .iter()
        .filter_map(|slot| match slot.control {
            Control::Player { player, .. } if player != me => Some(player),
            Control::Player { .. } | Control::Bot(_) | Control::Open | Control::Closed => None,
        })
        .collect();
    players.sort_unstable();
    players.dedup();
    players.len()
}
