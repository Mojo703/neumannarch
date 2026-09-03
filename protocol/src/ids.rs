//! The one id a machine holds that the sim never learns.

use serde::{Deserialize, Serialize};

/// One person in a lobby. The server hands it out with its welcome; a
/// skirmish's host takes [`PlayerId::HOST`] and there is nobody else.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub struct PlayerId(pub u32);

impl PlayerId {
    /// The player who opened the lobby.
    pub const HOST: PlayerId = PlayerId(0);
}
