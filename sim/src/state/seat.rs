//! One player's side, materials and reserve.

use std::collections::BTreeMap;

use crate::ids::{RowId, TeamId};
use crate::materials::{Materials, Stockpile};

/// A player. Alive until it has no entities and an empty reserve; the
/// reserve is a count per row of entities that appear complete when wanted.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Seat {
    team: TeamId,
    alive: bool,
    stockpile: Stockpile,
    base_capacity: Materials,
    reserve: BTreeMap<RowId, u32>,
}

impl Seat {
    /// A living seat on `team` holding `stock` and `reserve`. Its capacity
    /// starts at `stock`, so nothing it begins with is lost, and the rows
    /// that carry capacity add to that.
    pub fn new(team: TeamId, stock: Materials, reserve: BTreeMap<RowId, u32>) -> Seat {
        Seat {
            team,
            alive: true,
            stockpile: Stockpile::new(stock, stock),
            base_capacity: stock,
            reserve,
        }
    }

    pub fn team(&self) -> TeamId {
        self.team
    }

    /// False once the seat is out of the match.
    pub fn alive(&self) -> bool {
        self.alive
    }

    pub fn stockpile(&self) -> &Stockpile {
        &self.stockpile
    }

    /// The capacity the seat has before any entity adds to it.
    pub fn base_capacity(&self) -> Materials {
        self.base_capacity
    }

    /// Entities held back per row, in row order.
    pub fn reserve(&self) -> &BTreeMap<RowId, u32> {
        &self.reserve
    }

    /// How many of `row` the reserve holds.
    pub fn reserved(&self, row: RowId) -> u32 {
        self.reserve.get(&row).copied().unwrap_or(0)
    }

    /// True while the reserve holds nothing.
    pub fn reserve_is_empty(&self) -> bool {
        self.reserve.is_empty()
    }

    /// The seat's materials, to spend from or add to.
    pub(crate) fn stockpile_mut(&mut self) -> &mut Stockpile {
        &mut self.stockpile
    }

    /// Takes one of `row` from the reserve; false when it holds none.
    pub(crate) fn take_reserved(&mut self, row: RowId) -> bool {
        match self.reserve.get_mut(&row) {
            Some(count) if *count > 0 => {
                *count -= 1;
                if *count == 0 {
                    self.reserve.remove(&row);
                }
                true
            }
            _ => false,
        }
    }

    /// Puts the seat out of the match; it never comes back.
    pub(crate) fn eliminate(&mut self) {
        self.alive = false;
        self.reserve.clear();
    }
}
