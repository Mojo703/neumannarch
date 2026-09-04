use crate::ids::{RockId, SeatId};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Post {
    pub rock: RockId,
    pub seat: SeatId,
}
