//! One room: the machines in it, the phase it is in, and what it says back.

use probe_protocol::{LobbyEdit, Message, PlayerId, Refusal, Refused};

use crate::lobby::Seating;
use crate::playing::Playing;
use crate::records::Records;

/// A machine taken into a room: the id it holds there, and what the room
/// said about it.
pub struct Joined {
    pub player: PlayerId,
    pub posts: Vec<Post>,
}

/// One message the room sends, and who hears it.
pub struct Post {
    pub to: To,
    pub message: Message,
}

/// Who hears one of the room's messages, from the sender it is answering.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum To {
    /// Every machine connected to the room.
    Everyone,
    /// Every machine but the sender.
    EveryoneElse,
    /// The machine the room is answering.
    Sender,
    /// One named machine, whatever the room is answering.
    One(PlayerId),
}

/// One room of a match server: the authority on its lobby, and the
/// forwarder of the match its members play.
pub struct Room {
    /// The machines in the room, in the order they joined.
    members: Vec<PlayerId>,
    /// The id the next joiner takes.
    next: PlayerId,
    phase: Phase,
    records: Records,
}

/// What a room is doing.
enum Phase {
    /// Setting a match up.
    Seating(Seating),
    /// Forwarding one, over the lobby it was set up in, which a rematch
    /// opens again.
    Playing {
        playing: Box<Playing>,
        seating: Seating,
    },
}

impl Room {
    /// An empty room, open on the shape a host takes.
    pub fn opened() -> Room {
        Room {
            members: Vec::new(),
            next: PlayerId::HOST,
            phase: Phase::Seating(Seating::opened()),
            records: Records::default(),
        }
    }

    /// What the room does with `heard` from `from`. A machine the room has
    /// not taken in is not heard at all.
    pub fn hears(&mut self, from: PlayerId, heard: Message) -> Vec<Post> {
        if !self.members.contains(&from) {
            return Vec::new();
        }
        match heard {
            Message::Join { .. } => vec![Post::to(To::Sender, Message::Refused(Refusal::Full))],
            Message::Edit(edit) => self.edits(from, edit),
            Message::Start(_) => self.starts(from),
            Message::Rematch => self.rematches(from),
            Message::Leave => self.leaves(from),
            Message::Command(stamped) => match self.playing() {
                Some(playing) => playing.commanded(from, stamped),
                None => Vec::new(),
            },
            Message::Acknowledge { seat, up_to } => match self.playing() {
                Some(playing) => playing.acknowledged(from, seat, up_to),
                None => Vec::new(),
            },
            Message::Hash { tick, hash } => match self.playing() {
                Some(playing) => playing.reported(from, tick, hash),
                None => Vec::new(),
            },
            // The room's own words, which a member never speaks.
            Message::Welcome { .. }
            | Message::Lobby(_)
            | Message::Refused(_)
            | Message::Removed
            | Message::Desync { .. } => Vec::new(),
        }
    }

    /// Takes a machine speaking `version` into the room, or answers why it
    /// is not taken in.
    pub fn join(&mut self, version: u32) -> Result<Joined, Refusal> {
        if version != probe_protocol::VERSION {
            return Err(Refusal::Version);
        }
        let Phase::Seating(seating) = &mut self.phase else {
            return Err(Refusal::Full);
        };
        let player = self.next;
        seating.seat(player).ok_or(Refusal::Full)?;
        self.next = PlayerId(player.0 + 1);
        self.members.push(player);
        let lobby = seating.lobby().clone();
        Ok(Joined {
            player,
            posts: vec![
                Post::to(
                    To::Sender,
                    Message::Welcome {
                        player,
                        lobby: lobby.clone(),
                    },
                ),
                Post::to(To::EveryoneElse, Message::Lobby(lobby)),
            ],
        })
    }

    /// Drops `who` from the room and answers what the rest are told: in a
    /// lobby, the lobby with its slot open again, and in a match, its seats
    /// acknowledged for the rest of the match.
    ///
    /// The shape of a lobby is its host's, so a host that leaves one ends
    /// it: the room reopens and every other machine is sent away.
    pub fn leaves(&mut self, who: PlayerId) -> Vec<Post> {
        self.members.retain(|member| *member != who);
        match &mut self.phase {
            Phase::Seating(seating) if seating.lobby().host() == who => {
                self.reopen();
                vec![Post::to(To::Everyone, Message::Leave)]
            }
            Phase::Seating(seating) => match seating.release(who) {
                true => vec![Post::to(
                    To::Everyone,
                    Message::Lobby(seating.lobby().clone()),
                )],
                false => Vec::new(),
            },
            Phase::Playing { playing, .. } => {
                let posts = playing.left(who);
                if self.members.is_empty() {
                    let record = playing.record();
                    self.records.keep(record);
                    self.reopen();
                }
                posts
            }
        }
    }

    /// The machines in the room, in the order they joined.
    pub fn members(&self) -> &[PlayerId] {
        &self.members
    }

    /// The records of the matches this room has served.
    pub fn records(&self) -> &Records {
        &self.records
    }

    /// Applies `from`'s edit to the lobby, or answers why it did not. A
    /// kick also drops the machine whose slot it opened and tells it so.
    fn edits(&mut self, from: PlayerId, edit: LobbyEdit) -> Vec<Post> {
        let Phase::Seating(seating) = &mut self.phase else {
            return Vec::new();
        };
        if let Err(why) = seating.edit(from, edit) {
            return vec![Post::to(To::Sender, Message::Refused(why))];
        }
        let lobby = Message::Lobby(seating.lobby().clone());
        match edit {
            LobbyEdit::Kick(who) => {
                self.members.retain(|member| *member != who);
                vec![
                    Post::to(To::One(who), Message::Removed),
                    Post::to(To::Everyone, lobby),
                ]
            }
            LobbyEdit::SetSlot { .. }
            | LobbyEdit::SetTeam { .. }
            | LobbyEdit::SetSeed(_)
            | LobbyEdit::SetClock(_)
            | LobbyEdit::SetReady { .. } => vec![Post::to(To::Everyone, lobby)],
        }
    }

    /// Opens the lobby the finished match was set up in, as `from`'s
    /// rematch, or answers why it did not: only the host asks for one, and
    /// only of a match this room forwarded.
    ///
    /// The slots of machines that have left stand open again, so the shape
    /// the host starts from is the shape the room can still play.
    fn rematches(&mut self, from: PlayerId) -> Vec<Post> {
        let Phase::Playing { seating, .. } = &mut self.phase else {
            return Vec::new();
        };
        if seating.lobby().host() != from {
            return vec![Post::to(
                To::Sender,
                Message::Refused(Refusal::Edit(Refused::NotHost)),
            )];
        }
        let mut seating = seating.clone();
        for gone in seating.guests() {
            if !self.members.contains(&gone) {
                seating.release(gone);
            }
        }
        let lobby = seating.lobby().clone();
        self.phase = Phase::Seating(seating);
        vec![Post::to(To::Everyone, Message::Lobby(lobby))]
    }

    /// The match this room is forwarding, where it is forwarding one.
    fn playing(&mut self) -> Option<&mut Playing> {
        match &mut self.phase {
            Phase::Playing { playing, .. } => Some(playing),
            Phase::Seating(_) => None,
        }
    }

    /// Empties the room and opens it on a host's shape again.
    fn reopen(&mut self) {
        self.members.clear();
        self.next = PlayerId::HOST;
        self.phase = Phase::Seating(Seating::opened());
    }

    /// Freezes the lobby as `from`'s start, or answers why it did not.
    ///
    /// The room is the authority on the lobby, so it freezes its own copy;
    /// the setup a host sends with its start is not read.
    fn starts(&mut self, from: PlayerId) -> Vec<Post> {
        let Phase::Seating(seating) = &self.phase else {
            return Vec::new();
        };
        if seating.lobby().host() != from {
            return vec![Post::to(
                To::Sender,
                Message::Refused(Refusal::Edit(Refused::NotHost)),
            )];
        }
        let setup = match seating.freeze() {
            Ok(setup) => setup,
            Err(why) => return vec![Post::to(To::Sender, Message::Refused(why))],
        };
        let playing = Playing::started(seating.lobby(), setup.clone());
        self.phase = Phase::Playing {
            playing: Box::new(playing),
            seating: seating.clone(),
        };
        vec![Post::to(To::Everyone, Message::Start(setup))]
    }
}

impl Post {
    /// `message`, for `to` to hear.
    pub fn to(to: To, message: Message) -> Post {
        Post { to, message }
    }
}

#[cfg(test)]
mod tests {
    use probe_protocol::{Bot, Control, Lobby};
    use probe_sim::roster::SHIPYARD;
    use probe_sim::state::{Command, Issued};
    use probe_sim::{Band, Place, RockId, SeatId, Stamped, TeamId, Tick};

    use super::*;

    const GUEST: PlayerId = PlayerId(1);

    /// A command of `seat`, at `tick`.
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

    /// The lobby every member of `room` was last sent.
    fn lobby(room: &Room) -> Lobby {
        let Phase::Seating(seating) = &room.phase else {
            panic!("the room is not seating");
        };
        seating.lobby().clone()
    }

    /// A room the host and one guest have joined, the guest ready.
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
        room.hears(GUEST, Message::Edit(LobbyEdit::SetReady { ready: true }));
        room
    }

    /// The room started, with the host holding seat zero and the guest
    /// seat one.
    fn started() -> Room {
        let mut room = joined();
        let setup = lobby(&room).freeze().expect("both machines are seated");
        let posts = room.hears(PlayerId::HOST, Message::Start(setup.clone()));

        assert_eq!(posts.len(), 1);
        assert_eq!(posts[0].to, To::Everyone);
        assert_eq!(posts[0].message, Message::Start(setup));
        room
    }

    /// Every message `posts` carries to `to`.
    fn to(posts: &[Post], to: To) -> Vec<Message> {
        posts
            .iter()
            .filter(|post| post.to == to)
            .map(|post| post.message.clone())
            .collect()
    }

    #[test]
    fn the_room_applies_the_hosts_edits_and_each_guests_own_and_refuses_the_rest_by_name() {
        let mut room = joined();

        let seeded = room.hears(PlayerId::HOST, Message::Edit(LobbyEdit::SetSeed(42)));
        let moved = room.hears(
            GUEST,
            Message::Edit(LobbyEdit::SetTeam {
                slot: 1,
                team: TeamId(3),
            }),
        );
        let forged = room.hears(
            GUEST,
            Message::Edit(LobbyEdit::SetSlot {
                slot: 0,
                control: Control::Bot(Bot::Turtle),
            }),
        );

        assert_eq!(lobby(&room).seed(), 42);
        assert_eq!(lobby(&room).slots()[1].team, TeamId(3));
        assert_eq!(
            lobby(&room).slots()[0].control,
            Control::Player {
                player: PlayerId::HOST,
                ready: true
            }
        );
        assert!(
            matches!(
                to(&seeded, To::Everyone).as_slice(),
                [Message::Lobby(sent)] if sent.seed() == 42
            ),
            "an applied edit reaches every member as the lobby it made"
        );
        assert!(matches!(
            to(&moved, To::Everyone).as_slice(),
            [Message::Lobby(sent)] if sent.slots()[1].team == TeamId(3)
        ));
        assert_eq!(
            to(&forged, To::Sender),
            vec![Message::Refused(Refusal::Edit(Refused::NotHost))]
        );
        assert!(to(&forged, To::Everyone).is_empty());
    }

    #[test]
    fn a_started_room_forwards_a_seats_own_words_to_every_machine_but_the_sender() {
        let mut room = started();

        let commanded = room.hears(GUEST, Message::Command(command(1, 4)));
        let acknowledged = room.hears(
            GUEST,
            Message::Acknowledge {
                seat: SeatId(1),
                up_to: Tick(4),
            },
        );
        let forged = room.hears(GUEST, Message::Command(command(0, 4)));

        assert_eq!(
            to(&commanded, To::EveryoneElse),
            vec![Message::Command(command(1, 4))]
        );
        assert_eq!(
            to(&acknowledged, To::EveryoneElse),
            vec![Message::Acknowledge {
                seat: SeatId(1),
                up_to: Tick(4)
            }]
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
            let first = room.hears(
                PlayerId::HOST,
                Message::Hash {
                    tick: Tick(tick),
                    hash: tick,
                },
            );
            let second = room.hears(
                GUEST,
                Message::Hash {
                    tick: Tick(tick),
                    hash: tick,
                },
            );

            assert!(first.is_empty(), "one report agrees with nothing yet");
            assert_eq!(
                to(&second, To::Everyone),
                vec![Message::Hash {
                    tick: Tick(tick),
                    hash: tick
                }],
                "a tick every machine reported the same is agreed"
            );
        }
        let host = room.hears(
            PlayerId::HOST,
            Message::Hash {
                tick: Tick(4),
                hash: 7,
            },
        );
        let guest = room.hears(
            GUEST,
            Message::Hash {
                tick: Tick(4),
                hash: 8,
            },
        );
        let after = room.hears(
            PlayerId::HOST,
            Message::Hash {
                tick: Tick(5),
                hash: 9,
            },
        );

        assert!(host.is_empty(), "one report has nothing to differ from");
        assert_eq!(
            to(&guest, To::Everyone),
            vec![Message::Desync { tick: Tick(4) }]
        );
        assert!(after.is_empty(), "a desync is declared once");
    }

    #[test]
    fn a_machine_that_leaves_a_started_room_has_its_seats_acknowledged_for_the_rest_of_the_match() {
        let mut room = started();

        let posts = room.hears(GUEST, Message::Leave);

        assert_eq!(
            to(&posts, To::Everyone),
            vec![Message::Acknowledge {
                seat: SeatId(1),
                up_to: Tick(u64::MAX)
            }]
        );
        assert_eq!(room.members(), [PlayerId::HOST]);
        assert!(
            room.hears(GUEST, Message::Command(command(1, 9)))
                .is_empty(),
            "a machine the room has dropped is not heard"
        );
    }

    #[test]
    fn a_machine_joining_a_room_with_every_slot_held_is_refused_by_name() {
        let mut room = joined();

        assert_eq!(
            room.join(probe_protocol::VERSION).err(),
            Some(Refusal::Full),
            "a room opens one seat beside the host's, and both are held"
        );
        assert_eq!(
            to(
                &room.hears(
                    PlayerId::HOST,
                    Message::Join {
                        version: probe_protocol::VERSION
                    }
                ),
                To::Sender
            ),
            vec![Message::Refused(Refusal::Full)],
            "a machine already in the room is refused the same way"
        );
    }

    #[test]
    fn a_machine_speaking_another_version_of_the_protocol_is_refused_by_name() {
        let mut room = Room::opened();

        assert_eq!(
            room.join(probe_protocol::VERSION + 1).err(),
            Some(Refusal::Version)
        );
        assert!(room.members().is_empty(), "and is not taken in");
    }

    #[test]
    fn the_host_removes_a_guest_from_the_room_and_the_slot_it_held_opens() {
        let mut room = joined();

        let kicked = room.hears(PlayerId::HOST, Message::Edit(LobbyEdit::Kick(GUEST)));

        assert_eq!(to(&kicked, To::One(GUEST)), vec![Message::Removed]);
        assert!(matches!(
            to(&kicked, To::Everyone).as_slice(),
            [Message::Lobby(sent)] if sent.slots()[1].control == Control::Open
        ));
        assert_eq!(room.members(), [PlayerId::HOST]);
        assert_eq!(
            to(
                &room.hears(GUEST, Message::Edit(LobbyEdit::SetReady { ready: true })),
                To::Everyone
            ),
            Vec::new(),
            "a machine the room has removed is not heard"
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
            Phase::Playing { seating, .. } => seating.lobby().clone(),
            Phase::Seating(_) => panic!("the room is playing"),
        };

        let refused = room.hears(GUEST, Message::Rematch);
        let again = room.hears(PlayerId::HOST, Message::Rematch);

        assert_eq!(
            to(&refused, To::Sender),
            vec![Message::Refused(Refusal::Edit(Refused::NotHost))]
        );
        assert_eq!(
            to(&again, To::Everyone),
            vec![Message::Lobby(shape.clone())]
        );
        assert_eq!(lobby(&room), shape, "the shape is kept");
    }

    #[test]
    fn a_rematch_opens_the_slots_of_the_machines_that_have_left() {
        let mut room = started();
        room.hears(GUEST, Message::Leave);

        room.hears(PlayerId::HOST, Message::Rematch);

        assert_eq!(lobby(&room).slots()[1].control, Control::Open);
        assert_eq!(
            lobby(&room).slots()[0].control,
            Control::Player {
                player: PlayerId::HOST,
                ready: true
            },
            "the host's own slot is untouched"
        );
    }
}
