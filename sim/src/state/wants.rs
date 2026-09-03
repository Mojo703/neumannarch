//! One composition's counts.

use std::collections::BTreeMap;

use crate::ids::RowId;

/// A count per row at one post. Holds no zero entries, so a row absent from
/// it is wanted zero times and an empty `Wants` wants nothing.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Wants(BTreeMap<RowId, u32>);

impl Wants {
    /// Sets `row`'s count; zero removes the row.
    pub fn set(&mut self, row: RowId, count: u32) {
        if count == 0 {
            self.0.remove(&row);
        } else {
            self.0.insert(row, count);
        }
    }

    /// The count wanted of `row`; zero when absent.
    pub fn get(&self, row: RowId) -> u32 {
        self.0.get(&row).copied().unwrap_or(0)
    }

    /// Every wanted row with its count, in row order.
    pub fn iter(&self) -> impl Iterator<Item = (RowId, u32)> + '_ {
        self.0.iter().map(|(row, count)| (*row, *count))
    }

    /// True when no row is wanted.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn setting_zero_removes_the_row() {
        let mut wants = Wants::default();
        wants.set(RowId(3), 4);
        wants.set(RowId(1), 2);
        assert_eq!(wants.get(RowId(3)), 4);
        assert_eq!(
            wants.iter().collect::<Vec<_>>(),
            [(RowId(1), 2), (RowId(3), 4)]
        );
        wants.set(RowId(3), 0);
        wants.set(RowId(1), 0);
        assert_eq!(wants.get(RowId(3)), 0);
        assert!(wants.is_empty());
    }
}
