use serde::{Deserialize, Serialize};

use crate::ids::{RockId, SeatId};

const INNER_AMPLITUDE: f64 = 4.0;

const OUTER_AMPLITUDE: f64 = 30.0;

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub enum Band {
    Inner,
    Outer,
}

impl Band {
    pub fn amplitude(self) -> f64 {
        match self {
            Band::Inner => INNER_AMPLITUDE,
            Band::Outer => OUTER_AMPLITUDE,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub struct Place {
    pub rock: RockId,
    pub band: Band,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Post {
    pub place: Place,
    pub seat: SeatId,
}
