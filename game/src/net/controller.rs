use neumannarch_agents::{Personality, Scripted, Seated};
use neumannarch_protocol::{Crew, Occupant, Seating};
use neumannarch_sim::roster::Roster;
use neumannarch_sim::state::{Command, MAX_COMMANDS_PER_TICK};
use neumannarch_sim::{SeatId, Sequence, Session, Stamped};

pub struct Human {
    sequence: Sequence,
    wanted: Vec<Command>,
}

pub enum Controller {
    Human(Human),
    Bot(Box<Seated>),
    Remote(SeatId),
}

impl Controller {
    pub fn of(seating: &Seating, crew: &Crew, roster: &Roster) -> Vec<Controller> {
        seating
            .seats()
            .map(|(seat, holder)| match holder {
                Occupant::Player(player) if player == crew.player() => {
                    Controller::Human(Human::new(seat))
                }
                Occupant::Bot(bot) if crew.seats().contains(&seat) => {
                    Controller::Bot(Box::new(Seated::new(
                        seat,
                        Box::new(Scripted::new(Personality::of(bot), roster.clone())),
                    )))
                }
                Occupant::Bot(_) | Occupant::Player(_) => Controller::Remote(seat),
            })
            .collect()
    }

    pub fn seat(&self) -> SeatId {
        match self {
            Controller::Human(human) => human.sequence.seat(),
            Controller::Bot(seated) => seated.seat(),
            Controller::Remote(seat) => *seat,
        }
    }

    pub fn human(&mut self) -> Option<&mut Human> {
        match self {
            Controller::Human(human) => Some(human),
            Controller::Bot(_) | Controller::Remote(_) => None,
        }
    }

    pub fn issue(&mut self, session: &Session) -> Vec<Stamped> {
        match self {
            Controller::Human(human) => human.issue(session),
            Controller::Bot(seated) => seated.issue(session),
            Controller::Remote(_) => Vec::new(),
        }
    }
}

impl Human {
    pub fn new(seat: SeatId) -> Human {
        Human {
            sequence: Sequence::new(seat),
            wanted: Vec::new(),
        }
    }

    pub fn want(&mut self, command: Command) {
        if self.wanted.len() < MAX_COMMANDS_PER_TICK {
            self.wanted.push(command);
        }
    }

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
    use neumannarch_protocol::{Holder, Lobby, LobbyEdit, PlayerId};
    use neumannarch_sim::roster::SHIPYARD;
    use neumannarch_sim::{Retention, RockId};

    use super::*;

    fn command(count: u32) -> Command {
        Command::Want {
            rock: RockId(0),
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
                    holder: Holder::Player {
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
                    holder: Holder::Player {
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
