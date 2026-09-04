use probe_protocol::{Lobby, LobbyEdit, Message, Notice, PlayerId, Refused, Relayed, Request};

use crate::forwarding::Forwarding;
use crate::records::Records;

pub struct Joined {
    pub player: PlayerId,
    pub posts: Vec<Outbound>,
}

pub struct Outbound {
    pub to: Recipient,
    pub message: Message,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Recipient {
    Everyone,
    EveryoneElse,
    Sender,
    One(PlayerId),
}

pub struct Room {
    next: PlayerId,
    phase: Phase,
    records: Records,
}

enum Phase {
    Lobby {
        lobby: Lobby,
        members: Vec<PlayerId>,
    },
    Playing {
        playing: Box<Forwarding>,
        lobby: Lobby,
    },
}

impl Room {
    pub fn opened() -> Room {
        Room {
            next: PlayerId::HOST,
            phase: Phase::opened(),
            records: Records::default(),
        }
    }

    pub fn receive(&mut self, from: PlayerId, message: Message) -> Vec<Outbound> {
        if !self.members().contains(&from) {
            return Vec::new();
        }
        match message {
            Message::Request(Request::Join { .. }) => {
                vec![Outbound::to(
                    Recipient::Sender,
                    Message::Notice(Notice::Full),
                )]
            }
            Message::Request(Request::Edit(edit)) => self.edits(from, edit),
            Message::Request(Request::Start) => self.starts(from),
            Message::Request(Request::Rematch) => self.rematches(from),
            Message::Request(Request::Leave) => self.leaves(from),
            Message::Relayed(relayed) => match self.playing() {
                Some(playing) => match relayed {
                    Relayed::Command(stamped) => playing.commanded(from, stamped),
                    Relayed::Acknowledge { seat, up_to } => playing.acknowledged(from, seat, up_to),
                    Relayed::Hash { tick, hash } => playing.reported(from, tick, hash),
                    Relayed::Desync { .. } => Vec::new(),
                },
                None => Vec::new(),
            },
            Message::Notice(_) => Vec::new(),
        }
    }

    pub fn join(&mut self, version: u32) -> Result<Joined, Notice> {
        if version != probe_protocol::VERSION {
            return Err(Notice::Version);
        }
        let player = self.next;
        let Phase::Lobby { lobby, members } = &mut self.phase else {
            return Err(Notice::Full);
        };
        if !lobby.admit(player) {
            return Err(Notice::Full);
        }
        let lobby = lobby.clone();
        members.push(player);
        self.next = PlayerId(player.0 + 1);
        Ok(Joined {
            player,
            posts: vec![
                Outbound::to(
                    Recipient::Sender,
                    Message::Notice(Notice::Welcome {
                        player,
                        lobby: lobby.clone(),
                    }),
                ),
                Outbound::to(
                    Recipient::EveryoneElse,
                    Message::Notice(Notice::Lobby(lobby)),
                ),
            ],
        })
    }

    pub fn leaves(&mut self, who: PlayerId) -> Vec<Outbound> {
        match &mut self.phase {
            Phase::Lobby { lobby, .. } if lobby.host() == who => {
                self.phase = Phase::opened();
                vec![Outbound::to(
                    Recipient::Everyone,
                    Message::Notice(Notice::Left),
                )]
            }
            Phase::Lobby { lobby, members } => {
                members.retain(|member| *member != who);
                match lobby.release(who) {
                    true => vec![Outbound::to(
                        Recipient::Everyone,
                        Message::Notice(Notice::Lobby(lobby.clone())),
                    )],
                    false => Vec::new(),
                }
            }
            Phase::Playing { playing, .. } => {
                let posts = playing.left(who);
                if playing.members().is_empty() {
                    let record = playing.record();
                    self.records.keep(record);
                    self.phase = Phase::opened();
                }
                posts
            }
        }
    }

    pub fn members(&self) -> Vec<PlayerId> {
        match &self.phase {
            Phase::Lobby { members, .. } => members.clone(),
            Phase::Playing { playing, .. } => playing.members(),
        }
    }

    pub fn records(&self) -> &Records {
        &self.records
    }

    fn edits(&mut self, from: PlayerId, edit: LobbyEdit) -> Vec<Outbound> {
        let Phase::Lobby {
            lobby: held,
            members,
        } = &mut self.phase
        else {
            return Vec::new();
        };
        if let Err(why) = held.edit(from, edit) {
            return vec![Outbound::to(
                Recipient::Sender,
                Message::Notice(Notice::Refused(why)),
            )];
        }
        let lobby = Message::Notice(Notice::Lobby(held.clone()));
        match edit {
            LobbyEdit::Kick(who) => {
                members.retain(|member| *member != who);
                vec![
                    Outbound::to(Recipient::One(who), Message::Notice(Notice::Removed)),
                    Outbound::to(Recipient::Everyone, lobby),
                ]
            }
            LobbyEdit::SetSlot { .. }
            | LobbyEdit::SetTeam { .. }
            | LobbyEdit::SetSeed(_)
            | LobbyEdit::SetClock(_)
            | LobbyEdit::SetReady { .. } => vec![Outbound::to(Recipient::Everyone, lobby)],
        }
    }

    fn rematches(&mut self, from: PlayerId) -> Vec<Outbound> {
        let Phase::Playing { lobby, playing } = &self.phase else {
            return Vec::new();
        };
        if lobby.host() != from {
            return vec![Outbound::to(
                Recipient::Sender,
                Message::Notice(Notice::Refused(Refused::NotHost)),
            )];
        }
        let members = playing.members();
        let mut opened = lobby.clone();
        for player in opened.players() {
            if !members.contains(&player) {
                opened.release(player);
            }
        }
        let sent = opened.clone();
        self.phase = Phase::Lobby {
            lobby: opened,
            members,
        };
        vec![Outbound::to(
            Recipient::Everyone,
            Message::Notice(Notice::Lobby(sent)),
        )]
    }

    fn playing(&mut self) -> Option<&mut Forwarding> {
        match &mut self.phase {
            Phase::Playing { playing, .. } => Some(playing),
            Phase::Lobby { .. } => None,
        }
    }

    fn starts(&mut self, from: PlayerId) -> Vec<Outbound> {
        let Phase::Lobby { lobby: held, .. } = &self.phase else {
            return Vec::new();
        };
        if held.host() != from {
            return vec![Outbound::to(
                Recipient::Sender,
                Message::Notice(Notice::Refused(Refused::NotHost)),
            )];
        }
        let started = match held.freeze() {
            Ok(started) => started,
            Err(why) => {
                return vec![Outbound::to(
                    Recipient::Sender,
                    Message::Notice(Notice::NotReady(why)),
                )];
            }
        };
        let lobby = held.clone();
        let playing = Forwarding::started(started.clone());
        self.phase = Phase::Playing {
            playing: Box::new(playing),
            lobby,
        };
        vec![Outbound::to(
            Recipient::Everyone,
            Message::Notice(Notice::Started(started)),
        )]
    }
}

impl Phase {
    fn opened() -> Phase {
        Phase::Lobby {
            lobby: Lobby::room(PlayerId::HOST),
            members: Vec::new(),
        }
    }
}

impl Outbound {
    pub fn to(to: Recipient, message: Message) -> Outbound {
        Outbound { to, message }
    }
}

#[cfg(test)]
mod tests {
    use probe_protocol::{Bot, Holder, Lobby};
    use probe_sim::roster::SHIPYARD;
    use probe_sim::state::{Command, Issued};
    use probe_sim::{Band, Place, RockId, SeatId, Stamped, TeamId, Tick};

    use super::*;

    const GUEST: PlayerId = PlayerId(1);

    fn command(seat: u8, tick: u64) -> Stamped {
        Stamped {
            tick: Tick(tick),
            issued: Issued {
                seat: SeatId(seat),
                seq: 0,
                command: Command::Want {
                    place: Place {
                        rock: RockId(0),
                        band: Band::Inner,
                    },
                    row: SHIPYARD,
                    count: 1,
                },
            },
        }
    }

    fn lobby(room: &Room) -> Lobby {
        let Phase::Lobby { lobby, .. } = &room.phase else {
            panic!("the room is not seating");
        };
        lobby.clone()
    }

    fn joined() -> Room {
        let mut room = Room::opened();
        assert_eq!(
            room.join(probe_protocol::VERSION)
                .map(|joined| joined.player),
            Ok(PlayerId::HOST),
            "the first machine in hosts"
        );
        assert_eq!(
            room.join(probe_protocol::VERSION)
                .map(|joined| joined.player),
            Ok(GUEST)
        );
        room.receive(
            GUEST,
            Message::Request(Request::Edit(LobbyEdit::SetReady { ready: true })),
        );
        room
    }

    fn started() -> Room {
        let mut room = joined();
        let started = lobby(&room).freeze().expect("both machines are seated");
        let posts = room.receive(PlayerId::HOST, Message::Request(Request::Start));

        assert_eq!(posts.len(), 1);
        assert_eq!(posts[0].to, Recipient::Everyone);
        assert_eq!(posts[0].message, Message::Notice(Notice::Started(started)));
        room
    }

    fn to(posts: &[Outbound], to: Recipient) -> Vec<Message> {
        posts
            .iter()
            .filter(|post| post.to == to)
            .map(|post| post.message.clone())
            .collect()
    }

    #[test]
    fn the_room_applies_the_hosts_edits_and_each_guests_own_and_refuses_the_rest_by_name() {
        let mut room = joined();

        let seeded = room.receive(
            PlayerId::HOST,
            Message::Request(Request::Edit(LobbyEdit::SetSeed(42))),
        );
        let moved = room.receive(
            GUEST,
            Message::Request(Request::Edit(LobbyEdit::SetTeam {
                slot: 1,
                team: TeamId(3),
            })),
        );
        let forged = room.receive(
            GUEST,
            Message::Request(Request::Edit(LobbyEdit::SetSlot {
                slot: 0,
                control: Holder::Bot(Bot::Turtle),
            })),
        );

        assert_eq!(lobby(&room).seed(), 42);
        assert_eq!(lobby(&room).slots()[1].team, TeamId(3));
        assert_eq!(
            lobby(&room).slots()[0].control,
            Holder::Player {
                player: PlayerId::HOST,
                ready: true
            }
        );
        assert!(
            matches!(
                to(&seeded, Recipient::Everyone).as_slice(),
                [Message::Notice(Notice::Lobby(sent))] if sent.seed() == 42
            ),
            "an applied edit reaches every member as the lobby it made"
        );
        assert!(matches!(
            to(&moved, Recipient::Everyone).as_slice(),
            [Message::Notice(Notice::Lobby(sent))] if sent.slots()[1].team == TeamId(3)
        ));
        assert_eq!(
            to(&forged, Recipient::Sender),
            vec![Message::Notice(Notice::Refused(Refused::NotHost))]
        );
        assert!(to(&forged, Recipient::Everyone).is_empty());
    }

    #[test]
    fn a_started_room_forwards_a_seats_own_traffic_to_every_machine_but_the_sender() {
        let mut room = started();

        let commanded = room.receive(GUEST, Message::Relayed(Relayed::Command(command(1, 4))));
        let acknowledged = room.receive(
            GUEST,
            Message::Relayed(Relayed::Acknowledge {
                seat: SeatId(1),
                up_to: Tick(4),
            }),
        );
        let forged = room.receive(GUEST, Message::Relayed(Relayed::Command(command(0, 4))));

        assert_eq!(
            to(&commanded, Recipient::EveryoneElse),
            vec![Message::Relayed(Relayed::Command(command(1, 4)))]
        );
        assert_eq!(
            to(&acknowledged, Recipient::EveryoneElse),
            vec![Message::Relayed(Relayed::Acknowledge {
                seat: SeatId(1),
                up_to: Tick(4)
            })]
        );
        assert!(
            forged.is_empty(),
            "a command of a seat the sender does not own goes nowhere"
        );
    }

    #[test]
    fn the_room_declares_a_desync_at_the_first_settled_tick_two_machines_hashes_differ_at() {
        let mut room = started();

        for tick in 1..4 {
            let first = room.receive(
                PlayerId::HOST,
                Message::Relayed(Relayed::Hash {
                    tick: Tick(tick),
                    hash: tick,
                }),
            );
            let second = room.receive(
                GUEST,
                Message::Relayed(Relayed::Hash {
                    tick: Tick(tick),
                    hash: tick,
                }),
            );

            assert!(first.is_empty(), "one report agrees with nothing yet");
            assert_eq!(
                to(&second, Recipient::Everyone),
                vec![Message::Relayed(Relayed::Hash {
                    tick: Tick(tick),
                    hash: tick
                })],
                "a tick every machine reported the same is agreed"
            );
        }
        let host = room.receive(
            PlayerId::HOST,
            Message::Relayed(Relayed::Hash {
                tick: Tick(4),
                hash: 7,
            }),
        );
        let guest = room.receive(
            GUEST,
            Message::Relayed(Relayed::Hash {
                tick: Tick(4),
                hash: 8,
            }),
        );
        let after = room.receive(
            PlayerId::HOST,
            Message::Relayed(Relayed::Hash {
                tick: Tick(5),
                hash: 9,
            }),
        );

        assert!(host.is_empty(), "one report has nothing to differ from");
        assert_eq!(
            to(&guest, Recipient::Everyone),
            vec![Message::Relayed(Relayed::Desync { tick: Tick(4) })]
        );
        assert!(after.is_empty(), "a desync is declared once");
    }

    #[test]
    fn a_machine_that_leaves_a_started_room_has_its_seats_acknowledged_for_the_rest_of_the_match() {
        let mut room = started();

        let posts = room.receive(GUEST, Message::Request(Request::Leave));

        assert_eq!(
            to(&posts, Recipient::Everyone),
            vec![Message::Relayed(Relayed::Acknowledge {
                seat: SeatId(1),
                up_to: Tick(u64::MAX)
            })]
        );
        assert_eq!(room.members(), [PlayerId::HOST]);
        assert!(
            room.receive(GUEST, Message::Relayed(Relayed::Command(command(1, 9))))
                .is_empty(),
            "a command from a machine the room has dropped goes nowhere"
        );
    }

    #[test]
    fn a_machine_joining_a_room_with_every_slot_held_is_refused_by_name() {
        let mut room = joined();

        assert_eq!(
            room.join(probe_protocol::VERSION).err(),
            Some(Notice::Full),
            "a room opens one seat beside the host's, and both are held"
        );
        assert_eq!(
            to(
                &room.receive(
                    PlayerId::HOST,
                    Message::Request(Request::Join {
                        version: probe_protocol::VERSION
                    })
                ),
                Recipient::Sender
            ),
            vec![Message::Notice(Notice::Full)],
            "a machine already in the room is refused the same way"
        );
    }

    #[test]
    fn a_machine_of_another_version_of_the_protocol_is_refused_by_name() {
        let mut room = Room::opened();

        assert_eq!(
            room.join(probe_protocol::VERSION + 1).err(),
            Some(Notice::Version)
        );
        assert!(room.members().is_empty(), "and is not taken in");
    }

    #[test]
    fn the_host_removes_a_guest_from_the_room_and_the_slot_it_held_opens() {
        let mut room = joined();

        let kicked = room.receive(
            PlayerId::HOST,
            Message::Request(Request::Edit(LobbyEdit::Kick(GUEST))),
        );

        assert_eq!(
            to(&kicked, Recipient::One(GUEST)),
            vec![Message::Notice(Notice::Removed)]
        );
        assert!(matches!(
            to(&kicked, Recipient::Everyone).as_slice(),
            [Message::Notice(Notice::Lobby(sent))] if sent.slots()[1].control == Holder::Open
        ));
        assert_eq!(room.members(), [PlayerId::HOST]);
        assert_eq!(
            to(
                &room.receive(
                    GUEST,
                    Message::Request(Request::Edit(LobbyEdit::SetReady { ready: true }))
                ),
                Recipient::Everyone
            ),
            Vec::new(),
            "a machine the room has removed changes nothing"
        );
        assert_eq!(
            room.join(probe_protocol::VERSION)
                .map(|joined| joined.player),
            Ok(PlayerId(2)),
            "and another machine may take the slot"
        );
    }

    #[test]
    fn the_hosts_rematch_opens_the_lobby_the_match_was_set_up_in_and_nobody_elses_does() {
        let mut room = started();
        let shape = match &room.phase {
            Phase::Playing { lobby, .. } => lobby.clone(),
            Phase::Lobby { .. } => panic!("the room is playing"),
        };

        let refused = room.receive(GUEST, Message::Request(Request::Rematch));
        let again = room.receive(PlayerId::HOST, Message::Request(Request::Rematch));

        assert_eq!(
            to(&refused, Recipient::Sender),
            vec![Message::Notice(Notice::Refused(Refused::NotHost))]
        );
        assert_eq!(
            to(&again, Recipient::Everyone),
            vec![Message::Notice(Notice::Lobby(shape.clone()))]
        );
        assert_eq!(lobby(&room), shape, "the shape is kept");
    }

    #[test]
    fn a_rematch_opens_the_slots_of_the_machines_that_have_left() {
        let mut room = started();
        room.receive(GUEST, Message::Request(Request::Leave));

        room.receive(PlayerId::HOST, Message::Request(Request::Rematch));

        assert_eq!(lobby(&room).slots()[1].control, Holder::Open);
        assert_eq!(
            lobby(&room).slots()[0].control,
            Holder::Player {
                player: PlayerId::HOST,
                ready: true
            },
            "the host's own slot is untouched"
        );
    }
}
