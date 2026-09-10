use neumannarch_protocol::{Crew, PlayerId, Relayed, Seating, Started};
use neumannarch_sim::state::view::View;
use neumannarch_sim::step::fire::Shots;
use neumannarch_sim::{Retention, Rewound, SeatId, Session, Tick};

use crate::net::controller::{Controller, Human};
use crate::net::pace::{ACKNOWLEDGE_INTERVAL, Allowed, Pace, REPORT_INTERVAL};
use crate::net::transport::Transport;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ticked {
    pub pace: Allowed,
    pub rewound: bool,
}

pub struct Machine {
    crew: Crew,
    seating: Seating,
    session: Session,
    controllers: Vec<Controller>,
    pace: Pace,
    peers: usize,
    agreed: bool,
    desynced: Option<Tick>,
    reported: Tick,
}

impl Machine {
    pub fn of(started: Started, crew: &Crew, transport: &mut dyn Transport) -> Machine {
        let (setup, seating) = started.parts();
        let session = Session::new(setup, Retention::shipped(), crew.seats())
            .expect("a crew's seats are the seats of the setup it was frozen with");
        let controllers = Controller::of(&seating, crew);
        let peers = seating.peers_of(crew.player());
        transport.report(Tick::ZERO, session.state().hash());
        Machine {
            crew: crew.clone(),
            seating,
            session,
            controllers,
            pace: Pace::shipped(),
            peers,
            agreed: peers == 0,
            desynced: None,
            reported: Tick::ZERO,
        }
    }

    pub fn agreed(&self) -> bool {
        self.agreed
    }

    pub fn alone(&self) -> bool {
        self.peers == 0
    }

    pub fn desynced(&self) -> Option<Tick> {
        self.desynced
    }

    pub fn human(&mut self) -> Option<&mut Human> {
        let seat = self.crew.watched();
        self.controllers
            .iter_mut()
            .find(|controller| controller.seat() == seat)
            .and_then(Controller::human)
    }

    pub fn receive(&mut self, transport: &mut dyn Transport) -> bool {
        let mut rewound = false;
        for relayed in transport.received() {
            rewound |= self.apply(relayed);
        }
        rewound
    }

    pub fn seat(&self) -> SeatId {
        self.crew.watched()
    }

    pub fn player(&self) -> PlayerId {
        self.crew.player()
    }

    pub fn seating(&self) -> &Seating {
        &self.seating
    }

    pub fn session(&self) -> &Session {
        &self.session
    }

    pub fn tick(&mut self, transport: &mut dyn Transport) -> Ticked {
        let rewound = self.receive(transport);
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
            self.issue_all(transport);
            self.session.advance();
            self.relay(transport);
        }
        Ticked { pace, rewound }
    }

    pub fn view(&self) -> View {
        let quiet = Shots::default();
        let shots = self
            .session
            .outcome()
            .map_or(&quiet, |outcome| &outcome.shots);
        View::of(self.session.state(), self.crew.watched(), shots)
    }

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

    fn apply(&mut self, relayed: Relayed) -> bool {
        match relayed {
            Relayed::Command(stamped) => {
                matches!(self.session.insert(stamped), Ok(Rewound::From(_)))
            }
            Relayed::Acknowledge { seat, up_to } => {
                self.session.acknowledge(seat, up_to);
                false
            }

            Relayed::Hash { tick, hash } => {
                match self.session.hash_at(tick) {
                    Some(held) if held != hash => self.desynced = Some(tick),
                    Some(_) => self.agreed |= tick == Tick::ZERO,
                    None => {}
                }
                false
            }
            Relayed::Desync { tick } => {
                self.desynced = Some(tick);
                false
            }
        }
    }

    fn issue_all(&mut self, transport: &mut dyn Transport) {
        let issued: Vec<_> = self
            .controllers
            .iter_mut()
            .flat_map(|controller| controller.issue(&self.session))
            .collect();
        for stamped in issued {
            self.session
                .insert(stamped)
                .expect("a controller's own command is one this tick takes");
            transport.send(stamped);
        }
    }

    fn relay(&mut self, transport: &mut dyn Transport) {
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
    use neumannarch_protocol::{Bot, Holder, Lobby, LobbyEdit, PlayerId};

    use super::*;
    use crate::net::local::Local;

    const GUEST: PlayerId = PlayerId(1);

    #[test]
    fn a_guest_of_a_host_that_runs_a_bot_from_a_closed_slot_waits_for_the_first_agreed_hash() {
        let mut lobby = Lobby::room(PlayerId::HOST);
        for edit in [
            LobbyEdit::SetSlot {
                slot: 1,
                holder: Holder::Player {
                    player: GUEST,
                    ready: true,
                },
            },
            LobbyEdit::SetSlot {
                slot: 2,
                holder: Holder::Bot(Bot::Turtle),
            },
            LobbyEdit::SetSlot {
                slot: 0,
                holder: Holder::Closed,
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
