use std::collections::BTreeMap;

use crate::ids::{RowId, TeamId};
use crate::materials::{Materials, Stockpile};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Seat {
    team: TeamId,
    alive: bool,
    stockpile: Stockpile,
    base_capacity: Materials,
    reserve: BTreeMap<RowId, u32>,
}

impl Seat {
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

    pub fn alive(&self) -> bool {
        self.alive
    }

    pub fn stockpile(&self) -> &Stockpile {
        &self.stockpile
    }

    pub fn base_capacity(&self) -> Materials {
        self.base_capacity
    }

    pub fn reserve(&self) -> &BTreeMap<RowId, u32> {
        &self.reserve
    }

    pub fn reserved(&self, row: RowId) -> u32 {
        self.reserve.get(&row).copied().unwrap_or(0)
    }

    pub fn reserve_is_empty(&self) -> bool {
        self.reserve.is_empty()
    }

    pub(crate) fn stockpile_mut(&mut self) -> &mut Stockpile {
        &mut self.stockpile
    }

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

    pub(crate) fn eliminate(&mut self) {
        self.alive = false;
        self.reserve.clear();
    }
}
