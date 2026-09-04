//! Who speaks for one seat on this machine.

use probe_agents::{Personality, Scripted, Seated};
use probe_protocol::{Crew, Holder, Seating};
use probe_sim::roster::Roster;
use probe_sim::state::{Command, MAX_COMMANDS_PER_TICK};
use probe_sim::{SeatId, Sequence, Session, Stamped};

/// The person at this machine, speaking for one seat.
///
/// It holds what the frame's gestures asked for until the next tick stamps
/// it, and never holds more than a tick can carry.
pub struct Human {
    sequence: Sequence,
    wanted: Vec<Command>,
}

/// Who speaks for one seat of a match.
pub enum Controller {
    /// The person at this machine.
    Human(Human),
    /// A scripted opponent this machine runs, on the agent's own cadence.
    Bot(Box<Seated>),
    /// Another machine's seat: it issues nothing here, and its commands
    /// arrive through the transport.
    Remote(SeatId),
}

impl Controller {
    /// One controller per seat of `seating`, in seat order: the person at
    /// this machine in the seat they hold, a bot in each seat `crew` runs
    /// one for, and the rest remote.
    ///
    /// Only a host seats a bot, so only the host's machine runs one; on
    /// every other machine that seat is another machine's.
    pub fn of(seating: &Seating, crew: &Crew, roster: &Roster) -> Vec<Controller> {
        seating
            .seats()
            .map(|(seat, holder)| match holder {
                Holder::Player(player) if player == crew.player() => {
                    Controller::Human(Human::new(seat))
                }
                Holder::Bot(bot) if crew.seats().contains(&seat) => {
                    Controller::Bot(Box::new(Seated::new(
                        seat,
                        Box::new(Scripted::new(Personality::of(bot), roster.clone())),
                    )))
                }
                Holder::Bot(_) | Holder::Player(_) | Holder::Open => Controller::Remote(seat),
            })
            .collect()
    }

    /// The seat it speaks for.
    pub fn seat(&self) -> SeatId {
        match self {
            Controller::Human(human) => human.sequence.seat(),
            Controller::Bot(seated) => seated.seat(),
            Controller::Remote(seat) => *seat,
        }
    }

    /// The person at this machine, where this controller is one.
    pub fn human(&mut self) -> Option<&mut Human> {
        match self {
            Controller::Human(human) => Some(human),
            Controller::Bot(_) | Controller::Remote(_) => None,
        }
    }

    /// What it issues at the tick `session` shows, stamped with its seat
    /// and its own count. Never more than [`MAX_COMMANDS_PER_TICK`], so
    /// the tick's batch takes every one of them.
    pub fn issue(&mut self, session: &Session) -> Vec<Stamped> {
        match self {
            Controller::Human(human) => human.issue(session),
            Controller::Bot(seated) => seated.issue(session),
            Controller::Remote(_) => Vec::new(),
        }
    }
}

impl Human {
    /// The person holding `seat`, wanting nothing yet.
    pub fn new(seat: SeatId) -> Human {
        Human {
            sequence: Sequence::new(seat),
            wanted: Vec::new(),
        }
    }

    /// Takes `command` for the next tick. One frame's gestures ask for a
    /// wheel edit or a send's row pairs, well inside a tick's cap, so what
    /// a frame asks for is what the next tick issues.
    pub fn want(&mut self, command: Command) {
        if self.wanted.len() < MAX_COMMANDS_PER_TICK {
            self.wanted.push(command);
        }
    }

    /// Everything asked for since the last tick, stamped at the tick
    /// `session` shows.
    fn issue(&mut self, session: &Session) -> Vec<Stamped> {
        let tick = session.state().tick();
        self.wanted
            .drain(..)
            .map(|command| self.sequence.stamp(tick, command))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use probe_protocol::{Control, Lobby, LobbyEdit, PlayerId};
    use probe_sim::roster::SHIPYARD;
    use probe_sim::{Band, Place, Retention, RockId};

    use super::*;

    fn command(count: u32) -> Command {
        Command::Want {
            place: Place {
                rock: RockId(0),
                band: Band::Inner,
            },
            row: SHIPYARD,
            count,
        }
    }

    fn skirmish() -> Lobby {
        Lobby::skirmish(PlayerId::HOST)
    }

    fn session(lobby: &Lobby) -> Session {
        Session::new(
            lobby.freeze().expect("a skirmish is a match").parts().0,
            Retention::shipped(),
            &[],
        )
        .expect("a session owning no seat seats nothing to refuse")
    }

    /// The controllers the machine `me` is at holds of `lobby`'s match.
    fn controllers(lobby: &Lobby, me: PlayerId) -> Vec<Controller> {
        let started = lobby.freeze().expect("the lobby is a match");
        let seating = started.seating();
        let crew = seating.run_by(me).expect("the machine runs a seat");
        Controller::of(seating, &crew, &Roster::shipped())
    }

    #[test]
    fn a_skirmish_seats_the_person_at_this_machine_and_runs_its_bot() {
        let lobby = skirmish();

        let controllers = controllers(&lobby, PlayerId::HOST);

        assert_eq!(controllers.len(), 2, "two held slots are two seats");
        assert!(matches!(controllers[0], Controller::Human(_)));
        assert!(matches!(controllers[1], Controller::Bot(_)));
        assert_eq!(controllers[1].seat(), SeatId(1));
    }

    #[test]
    fn a_bot_is_run_by_the_hosts_machine_and_by_no_other() {
        let mut lobby = skirmish();
        lobby
            .edit(
                PlayerId::HOST,
                LobbyEdit::SetSlot {
                    slot: 2,
                    control: Control::Player {
                        player: PlayerId(4),
                        ready: true,
                    },
                },
            )
            .expect("the host seats a guest");

        let host = controllers(&lobby, PlayerId::HOST);
        let guest = controllers(&lobby, PlayerId(4));

        assert!(matches!(host[1], Controller::Bot(_)));
        assert!(
            matches!(guest[1], Controller::Remote(_)),
            "the guest's machine leaves the bot to the host"
        );
        assert!(matches!(guest[2], Controller::Human(_)));
    }

    #[test]
    fn a_seat_another_machine_holds_issues_nothing_here() {
        let mut lobby = skirmish();
        lobby
            .edit(
                PlayerId::HOST,
                LobbyEdit::SetSlot {
                    slot: 1,
                    control: Control::Player {
                        player: PlayerId(4),
                        ready: true,
                    },
                },
            )
            .expect("the host seats a guest");
        let session = session(&lobby);

        let mut controllers = controllers(&lobby, PlayerId::HOST);

        assert!(matches!(controllers[1], Controller::Remote(_)));
        assert!(controllers[1].issue(&session).is_empty());
    }

    #[test]
    fn what_a_frame_asks_for_is_stamped_at_the_next_tick_and_fits_that_tick() {
        let lobby = skirmish();
        let mut session = session(&lobby);
        let mut human = Human::new(SeatId(0));
        for count in 0..2 * MAX_COMMANDS_PER_TICK as u32 {
            human.want(command(count));
        }

        session.advance();
        let issued = human.issue(&session);

        assert_eq!(issued.len(), MAX_COMMANDS_PER_TICK);
        assert!(
            issued
                .iter()
                .all(|stamped| stamped.tick == session.state().tick()),
            "a controller stamps the tick the session shows"
        );
        assert_eq!(
            issued
                .iter()
                .map(|stamped| stamped.issued.seq)
                .collect::<Vec<_>>(),
            (0..MAX_COMMANDS_PER_TICK as u32).collect::<Vec<_>>()
        );
        assert!(
            human.issue(&session).is_empty(),
            "a tick takes what was asked for once"
        );
    }
}
