use neumannarch_sim::{SeatId, Stamped, Tick};
use serde::{Deserialize, Serialize};

use crate::ids::PlayerId;
use crate::lobby::{Lobby, LobbyEdit, NotReady, Refused};
use crate::seating::Started;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum Request {
    Join { version: u32 },
    Edit(LobbyEdit),
    Start,
    Rematch,
    Leave,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum Notice {
    Welcome { player: PlayerId, lobby: Lobby },
    Lobby(Lobby),
    Started(Started),
    Refused(Refused),
    NotReady(NotReady),
    Full,
    Version,
    Left,
    Removed,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum Relayed {
    Command(Stamped),
    Acknowledge { seat: SeatId, up_to: Tick },
    Hash { tick: Tick, hash: u64 },
    Desync { tick: Tick },
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum Message {
    Request(Request),
    Notice(Notice),
    Relayed(Relayed),
}

#[cfg(test)]
mod tests {
    use neumannarch_sim::state::{Command, Issued};
    use neumannarch_sim::{RockId, RowId, TeamId};

    use super::*;
    use crate::lobby::Holder;
    use crate::wire::Codec;

    fn every_message() -> Vec<Message> {
        let lobby = crate::lobby::Lobby::skirmish(PlayerId::HOST);
        vec![
            Message::Request(Request::Join {
                version: crate::VERSION,
            }),
            Message::Request(Request::Edit(LobbyEdit::SetSlot {
                slot: 2,
                holder: Holder::Open,
            })),
            Message::Request(Request::Edit(LobbyEdit::Kick(PlayerId(4)))),
            Message::Request(Request::Start),
            Message::Request(Request::Rematch),
            Message::Request(Request::Leave),
            Message::Notice(Notice::Welcome {
                player: PlayerId(3),
                lobby: lobby.clone(),
            }),
            Message::Notice(Notice::Lobby(lobby.clone())),
            Message::Notice(Notice::Started(
                lobby.freeze().expect("a skirmish is a match"),
            )),
            Message::Notice(Notice::Refused(Refused::NotHost)),
            Message::Notice(Notice::NotReady(NotReady::OpenSeat { slot: 1 })),
            Message::Notice(Notice::Full),
            Message::Notice(Notice::Version),
            Message::Notice(Notice::Left),
            Message::Notice(Notice::Removed),
            Message::Relayed(Relayed::Command(Stamped {
                tick: Tick(9),
                issued: Issued {
                    seat: SeatId(1),
                    seq: 4,
                    command: Command::Want {
                        rock: RockId(6),
                        row: RowId(2),
                        count: 3,
                    },
                },
            })),
            Message::Relayed(Relayed::Acknowledge {
                seat: SeatId(0),
                up_to: Tick(120),
            }),
            Message::Relayed(Relayed::Hash {
                tick: Tick(120),
                hash: 0xDEAD_BEEF,
            }),
            Message::Relayed(Relayed::Desync { tick: Tick(121) }),
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
