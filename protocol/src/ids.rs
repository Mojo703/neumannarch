use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub struct PlayerId(pub u32);

impl PlayerId {
    pub const HOST: PlayerId = PlayerId(0);
}
