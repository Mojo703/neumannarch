//! The unit table: rows, weapons, and the shipped roster.

use core::ops::Index;

pub use row::{Kind, Row, Weapon};
pub use shipped::{CONSTRUCTOR, EXTRACTOR, FRIGATE, LANCER, RAIDER, SCOUT, SHIPYARD, STORAGE};

use crate::ids::RowId;

/// The rows of a match, indexed by `RowId` in the order they were added.
/// The only place a row lives; entities refer to theirs by id.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Roster {
    rows: Vec<Row>,
}

impl Roster {
    /// The eight shipped rows, at the ids of the shipped constants.
    pub fn shipped() -> Roster {
        Roster {
            rows: shipped::rows(),
        }
    }

    /// Appends `row` and returns its id, the next after the last.
    pub fn add(&mut self, row: Row) -> RowId {
        // Rows come from code, never from a command, so a roster past
        // `RowId`'s range is a programming error the type of `rows` cannot
        // rule out.
        let id = RowId(u16::try_from(self.rows.len()).expect("a roster holds at most 65536 rows"));
        self.rows.push(row);
        id
    }

    /// The row at `id`, or `None` when no such row was added.
    pub fn get(&self, id: RowId) -> Option<&Row> {
        self.rows.get(usize::from(id.0))
    }

    /// Every row with its id, in id order.
    pub fn iter(&self) -> impl Iterator<Item = (RowId, &Row)> {
        (0..=u16::MAX).map(RowId).zip(&self.rows)
    }

    /// The number of rows.
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    /// Whether no row was added.
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
}

/// Panics when `id` is not a row; a command's row is checked by `get` before
/// a rule reads it.
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
        assert_eq!(roster.len(), 9);
    }

    #[test]
    fn iter_walks_ids_in_order_from_zero() {
        let roster = Roster::shipped();
        let ids: Vec<_> = roster.iter().map(|(id, _)| id).collect();
        assert_eq!(ids, (0..8).map(RowId).collect::<Vec<_>>());
        for (id, row) in roster.iter() {
            assert_eq!(&roster[id], row);
        }
    }

    #[test]
    fn an_unknown_id_reads_nothing() {
        let roster = Roster::shipped();
        assert!(!roster.is_empty());
        assert_eq!(roster.get(RowId(8)), None);
        assert_eq!(roster.get(RowId(u16::MAX)), None);
    }
}
