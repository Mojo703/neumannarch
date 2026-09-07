use std::collections::BTreeMap;

use crate::ids::{RowId, TeamId};
use crate::materials::{Materials, PerSecond, Stockpile};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Seat {
    team: TeamId,
    alive: bool,
    stockpile: Stockpile,
    base_capacity: Materials,
    reserve: BTreeMap<RowId, u32>,
    income: PerSecond,
    spend: PerSecond,
}

impl Seat {
    pub fn new(team: TeamId, stock: Materials, reserve: BTreeMap<RowId, u32>) -> Seat {
        Seat {
            team,
            alive: true,
            stockpile: Stockpile::new(stock, stock),
            base_capacity: stock,
            reserve,
            income: PerSecond::default(),
            spend: PerSecond::default(),
        }
    }

    pub fn team(&self) -> TeamId {
        self.team
    }

    pub(crate) fn alive(&self) -> bool {
        self.alive
    }

    pub fn stockpile(&self) -> &Stockpile {
        &self.stockpile
    }

    pub fn base_capacity(&self) -> Materials {
        self.base_capacity
    }

    pub fn income(&self) -> Materials {
        self.income.completed()
    }

    pub fn spend(&self) -> Materials {
        self.spend.completed()
    }

    pub(crate) fn reserve(&self) -> &BTreeMap<RowId, u32> {
        &self.reserve
    }

    pub fn reserved(&self, row: RowId) -> u32 {
        self.reserve.get(&row).copied().unwrap_or(0)
    }

    pub(crate) fn reserve_is_empty(&self) -> bool {
        self.reserve.is_empty()
    }

    pub fn stockpile_mut(&mut self) -> &mut Stockpile {
        &mut self.stockpile
    }

    pub fn refund(&mut self, materials: Materials) {
        self.stockpile.add(materials);
    }

    pub fn earn(&mut self, income: Materials) {
        self.stockpile.add(income);
        self.income.fill(income);
    }

    pub fn drain(&mut self, cost: Materials) {
        let taken = self.stockpile.spend(cost);
        self.spend.fill(taken);
    }

    pub(crate) fn close_second(&mut self) {
        self.income.close();
        self.spend.close();
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
