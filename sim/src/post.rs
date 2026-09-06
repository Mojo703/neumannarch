use crate::ids::{AsteroidId, SeatId};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Post {
    pub asteroid: AsteroidId,
    pub seat: SeatId,
}
