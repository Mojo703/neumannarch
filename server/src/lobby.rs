//! The lobby a room is the authority on, and who may say what about it.

use probe_protocol::{Control, Lobby, LobbyEdit, PlayerId, Refusal};
use probe_sim::Setup;

/// The lobby of one room, applied to in arrival order.
#[derive(Clone)]
pub(crate) struct Seating {
    lobby: Lobby,
}

impl Seating {
    /// A room's opening shape: the host's slot and one open seat.
    pub(crate) fn opened() -> Seating {
        Seating {
            lobby: Lobby::room(PlayerId::HOST),
        }
    }

    /// Applies `edit` as `by`, or answers why the room did not.
    pub(crate) fn edit(&mut self, by: PlayerId, edit: LobbyEdit) -> Result<(), Refusal> {
        self.lobby.edit(by, edit).map_err(Refusal::Edit)
    }

    /// The match the lobby starts, or why it is not one yet.
    pub(crate) fn freeze(&self) -> Result<Setup, Refusal> {
        self.lobby.freeze().map_err(Refusal::NotReady)
    }

    /// The lobby as it stands, which every member is sent after an edit.
    pub(crate) fn lobby(&self) -> &Lobby {
        &self.lobby
    }

    /// Every player holding a slot but the host, in slot order.
    pub(crate) fn guests(&self) -> Vec<PlayerId> {
        self.lobby
            .slots()
            .iter()
            .filter_map(|slot| match slot.control {
                Control::Player { player, .. } if player != self.lobby.host() => Some(player),
                Control::Player { .. } | Control::Open | Control::Closed | Control::Bot(_) => None,
            })
            .collect()
    }

    /// Opens `who`'s slot again, so another machine may take it. False
    /// where the lobby held no slot for it.
    pub(crate) fn release(&mut self, who: PlayerId) -> bool {
        let host = self.lobby.host();
        let Some(slot) = self.lobby.slot_of(who) else {
            return false;
        };
        self.lobby
            .edit(
                host,
                LobbyEdit::SetSlot {
                    slot,
                    control: Control::Open,
                },
            )
            .is_ok()
    }

    /// Seats `player`: the slot the lobby already holds for it, else the
    /// first open slot. `None` once every slot is held, which is a room
    /// with nowhere to sit.
    pub(crate) fn seat(&mut self, player: PlayerId) -> Option<usize> {
        if let Some(slot) = self.lobby.slot_of(player) {
            return Some(slot);
        }
        let host = self.lobby.host();
        let open = self
            .lobby
            .slots()
            .iter()
            .position(|slot| slot.control == Control::Open)?;
        self.lobby
            .edit(
                host,
                LobbyEdit::SetSlot {
                    slot: open,
                    control: Control::Player {
                        player,
                        ready: false,
                    },
                },
            )
            .ok()?;
        Some(open)
    }
}
