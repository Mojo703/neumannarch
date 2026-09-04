//! This machine's part of one match: every seat stepped, its own seats
//! spoken for, and what its peers are told.

use probe_protocol::{Crew, Message, Started};
use probe_sim::state::view::View;
use probe_sim::step::fire::Shots;
use probe_sim::{Retention, Rewound, SeatId, Session, Tick};

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

/// One match as this machine plays it: the session every seat is stepped
/// in, one controller per seat, and the pace it keeps with its peers. The
/// transport is the caller's, so a room outlives the match played in it.
pub struct Machine {
    /// The seats this machine runs, and the one it watches.
    crew: Crew,
    session: Session,
    controllers: Vec<Controller>,
    pace: Pace,
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
    /// The match `started` names, as the machine `crew` is the seats of
    /// plays it, speaking to its peers through `transport`.
    ///
    /// It reports its own hash at tick zero at once, which is what every
    /// machine agrees the match on before the first step.
    pub fn of(started: Started, crew: &Crew, transport: &mut dyn Transport) -> Machine {
        let (setup, seating) = started.parts();
        let session = Session::new(setup, Retention::shipped(), crew.seats())
            .expect("a crew's seats are the seats of the setup it was frozen with");
        let controllers = Controller::of(&seating, crew, session.state().roster());
        let peers = seating.peers_of(crew.player());
        transport.report(Tick::ZERO, session.state().hash());
        Machine {
            crew: crew.clone(),
            session,
            controllers,
            pace: Pace::shipped(),
            peers,
            agreed: peers == 0,
            desynced: None,
            reported: Tick::ZERO,
        }
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
        let seat = self.crew.watched();
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

    /// The seat this machine watches: the person's own where they hold one,
    /// else the first seat the machine runs.
    pub fn seat(&self) -> SeatId {
        self.crew.watched()
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
        View::of(self.session.state(), self.crew.watched(), shots)
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
            for seat in self.crew.seats() {
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

#[cfg(test)]
mod tests {
    use probe_protocol::{Bot, Control, Lobby, LobbyEdit, PlayerId};

    use super::*;
    use crate::net::local::Local;

    const GUEST: PlayerId = PlayerId(1);

    #[test]
    fn a_guest_of_a_host_that_runs_a_bot_from_a_closed_slot_waits_for_the_first_agreed_hash() {
        let mut lobby = Lobby::room(PlayerId::HOST);
        for edit in [
            LobbyEdit::SetSlot {
                slot: 1,
                control: Control::Player {
                    player: GUEST,
                    ready: true,
                },
            },
            LobbyEdit::SetSlot {
                slot: 2,
                control: Control::Bot(Bot::Turtle),
            },
            LobbyEdit::SetSlot {
                slot: 0,
                control: Control::Closed,
            },
        ] {
            lobby.edit(PlayerId::HOST, edit).expect("the host's shape");
        }
        let started = lobby.freeze().expect("a ready guest and the host's bot");
        let crew = started
            .seating()
            .run_by(GUEST)
            .expect("the guest holds a seat");

        let machine = Machine::of(started, &crew, &mut Local);

        assert!(
            !machine.alone(),
            "the host's machine runs the bot, so the guest has a peer"
        );
        assert!(
            !machine.agreed(),
            "a match with a peer starts once both machines report the same first hash"
        );
    }
}
