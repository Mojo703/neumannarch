use core::ops::Index;

pub use row::{Kind, Row, Weapon, Weights};
pub use shipped::{
    CONSTRUCTOR, ENERGY_EXTRACTOR, FRIGATE, LANCER, METALS_EXTRACTOR, RAIDER, SHIPYARD, STORAGE,
    VOLATILES_EXTRACTOR,
};

use crate::ids::RowId;
use crate::real::Real;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Roster {
    movement_limit: Real,
    rows: Vec<Row>,
}

impl Roster {
    pub fn shipped() -> Roster {
        Roster {
            movement_limit: Real(shipped::MOVEMENT_LIMIT_METERS_PER_SECOND_SQUARED),
            rows: shipped::rows(),
        }
    }

    pub fn moving_at(self, movement_limit: Real) -> Roster {
        Roster {
            movement_limit,
            ..self
        }
    }

    pub fn units_by(self, adjust: impl Fn(Row) -> Row) -> Roster {
        Roster {
            rows: self
                .rows
                .into_iter()
                .map(|row| match row.kind() {
                    Kind::Structure => row,
                    Kind::Unit => adjust(row),
                })
                .collect(),
            ..self
        }
    }

    pub fn movement_limit(&self) -> Real {
        self.movement_limit
    }

    pub fn add(&mut self, row: Row) -> RowId {
        let id = RowId(u16::try_from(self.rows.len()).expect("a roster holds at most 65536 rows"));
        self.rows.push(row);
        id
    }

    pub fn get(&self, id: RowId) -> Option<&Row> {
        self.rows.get(usize::from(id.0))
    }

    pub fn iter(&self) -> impl Iterator<Item = (RowId, &Row)> {
        (0..=u16::MAX).map(RowId).zip(&self.rows)
    }

    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
}

impl Index<RowId> for Roster {
    type Output = Row;

    fn index(&self, id: RowId) -> &Row {
        &self.rows[usize::from(id.0)]
    }
}

mod row;
mod shipped;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_returns_the_id_that_reads_the_row_back() {
        let mut roster = Roster::shipped();
        let variant = Row {
            name: "variant",
            ..roster[LANCER].clone()
        };
        let id = roster.add(variant.clone());
        assert_eq!(roster[id], variant);
        assert_eq!(roster.get(id), Some(&variant));
        assert_eq!(roster.iter().last(), Some((id, &variant)));
        assert_eq!(roster.len(), Roster::shipped().len() + 1);
    }

    #[test]
    fn moving_at_sets_the_movement_limit_and_keeps_the_rows() {
        let shipped = Roster::shipped();
        let slow = shipped.clone().moving_at(Real(1e-6));
        assert_eq!(slow.movement_limit(), Real(1e-6));
        assert_eq!(slow.len(), shipped.len());
        assert_eq!(slow[LANCER], shipped[LANCER]);
    }

    #[test]
    fn units_by_adjusts_every_unit_row_and_leaves_the_structures_alone() {
        let shipped = Roster::shipped();
        let bolder = shipped.clone().units_by(|row| Row {
            steering: Weights {
                chase: Real(row.steering.chase.0 * 3.0),
                ..row.steering
            },
            ..row
        });
        assert_eq!(
            bolder[LANCER].steering.chase,
            Real(shipped[LANCER].steering.chase.0 * 3.0)
        );
        assert_eq!(bolder[LANCER].hp, shipped[LANCER].hp);
        assert_eq!(bolder[SHIPYARD].steering, Weights::STILL);
    }

    #[test]
    fn iter_walks_ids_in_order_from_zero() {
        let roster = Roster::shipped();
        let ids: Vec<_> = roster.iter().map(|(id, _)| id).collect();
        let all = (0..roster.len() as u16).map(RowId);
        assert_eq!(ids, all.collect::<Vec<_>>());
        for (id, row) in roster.iter() {
            assert_eq!(&roster[id], row);
        }
    }

    #[test]
    fn an_unknown_id_reads_nothing() {
        let roster = Roster::shipped();
        assert!(!roster.is_empty());
        let past = u16::try_from(roster.len()).expect("a roster of at most 65536 rows");
        assert_eq!(roster.get(RowId(past)), None);
        assert_eq!(roster.get(RowId(u16::MAX)), None);
    }
}
