//! Where things are wanted: a rock's band, and a seat's composition there.

use crate::ids::{RockId, SeatId};

/// How far ahead of a rock the inner band's anchor sits, in meters. A
/// hypothesis the harness and the display confirm or kill.
const INNER_AMPLITUDE: f64 = 4.0;

/// How far ahead of a rock the outer band's anchor sits, in meters. It
/// exceeds `INNER_AMPLITUDE` by more than the longest weapon range plus
/// twice the manoeuvring cutoff, so the bands never fight each other. A
/// hypothesis the harness and the display confirm or kill.
const OUTER_AMPLITUDE: f64 = 30.0;

/// The two anchors around a rock a force can hold: the inner band fights at
/// the rock, the outer band stages out of its range.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Band {
    Inner,
    Outer,
}

impl Band {
    /// How far ahead of the rock, along its orbit, this band's anchor sits,
    /// in meters.
    pub fn amplitude(self) -> f64 {
        match self {
            Band::Inner => INNER_AMPLITUDE,
            Band::Outer => OUTER_AMPLITUDE,
        }
    }
}

/// A rock's band. A unit's home, and the only thing a want names.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Place {
    pub rock: RockId,
    pub band: Band,
}

/// One seat's composition at a place.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Post {
    pub place: Place,
    pub seat: SeatId,
}
