//! What one machine of a match says to another.

use probe_sim::{SeatId, Setup, Stamped, Tick};
use serde::{Deserialize, Serialize};

use crate::ids::PlayerId;
use crate::lobby::{Lobby, LobbyEdit, NotReady, Refused};

/// Why the room did not do what a machine asked.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Hash, Serialize)]
pub enum Refusal {
    /// The edit was not this machine's to make.
    Edit(Refused),
    /// The lobby is not a match yet, so it does not start.
    NotReady(NotReady),
    /// Every slot is held, so a joining machine has nowhere to sit.
    Full,
}

/// One message between a member of a room and the room. Everything before
/// [`Message::Start`] is about the lobby; everything after is the match.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum Message {
    /// A machine asking to enter the room.
    Join,
    /// The id the room gave the joiner, and the lobby as it stands.
    Welcome { player: PlayerId, lobby: Lobby },
    /// An edit a member asks the room to apply.
    Edit(LobbyEdit),
    /// The lobby after an edit landed, to every member.
    Lobby(Lobby),
    /// Why the room did nothing, to the machine that asked.
    Refused(Refusal),
    /// The frozen lobby: every machine builds its initial state from this.
    Start(Setup),
    /// A command, at the tick it takes effect at.
    Command(Stamped),
    /// No command of `seat` before `up_to` is unknown to the sender.
    Acknowledge { seat: SeatId, up_to: Tick },
    /// The sender's state hash at a settled tick.
    Hash { tick: Tick, hash: u64 },
    /// Two members' hashes at this tick differ, which ends the match.
    Desync { tick: Tick },
    /// The sender is leaving the room.
    Leave,
}

#[cfg(test)]
mod tests {
    use probe_sim::state::{Command, Issued};
    use probe_sim::{Band, Place, RockId, RowId, TeamId};

    use super::*;
    use crate::lobby::Control;
    use crate::wire::Wire;

    /// One of every message, so a variant added without a wire form fails
    /// here rather than on the wire.
    fn every_message() -> Vec<Message> {
        let lobby = crate::lobby::Lobby::skirmish(PlayerId::HOST);
        vec![
            Message::Join,
            Message::Welcome {
                player: PlayerId(3),
                lobby: lobby.clone(),
            },
            Message::Edit(LobbyEdit::SetSlot {
                slot: 2,
                control: Control::Open,
            }),
            Message::Lobby(lobby.clone()),
            Message::Refused(Refusal::Edit(Refused::NotHost)),
            Message::Refused(Refusal::NotReady(NotReady::OpenSeat { slot: 1 })),
            Message::Refused(Refusal::Full),
            Message::Start(lobby.freeze().expect("a skirmish is a match")),
            Message::Command(Stamped {
                tick: Tick(9),
                issued: Issued {
                    seat: SeatId(1),
                    seq: 4,
                    command: Command::Want {
                        place: Place {
                            rock: RockId(6),
                            band: Band::Outer,
                        },
                        row: RowId(2),
                        count: 3,
                    },
                },
            }),
            Message::Acknowledge {
                seat: SeatId(0),
                up_to: Tick(120),
            },
            Message::Hash {
                tick: Tick(120),
                hash: 0xDEAD_BEEF,
            },
            Message::Desync { tick: Tick(121) },
            Message::Leave,
        ]
    }

    #[test]
    fn every_message_comes_back_off_the_wire_as_it_went_on() {
        for message in every_message() {
            assert_eq!(
                Message::decode(&message.encoded()),
                Ok(message.clone()),
                "{message:?}"
            );
        }
    }

    #[test]
    fn bytes_that_are_not_a_message_are_refused_rather_than_read() {
        assert!(Message::decode(&[0xFF, 0x00, 0x42]).is_err());
        assert!(
            Message::decode(&TeamId(3).encoded()).is_err(),
            "another value's bytes are not a message"
        );
    }
}
