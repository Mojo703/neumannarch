use std::collections::BTreeMap;

use crate::ids::RowId;

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Wants(BTreeMap<RowId, u32>);

impl Wants {
    pub(crate) fn set(&mut self, row: RowId, count: u32) {
        if count == 0 {
            self.0.remove(&row);
        } else {
            self.0.insert(row, count);
        }
    }

    pub(crate) fn get(&self, row: RowId) -> u32 {
        self.0.get(&row).copied().unwrap_or(0)
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = (RowId, u32)> + '_ {
        self.0.iter().map(|(row, count)| (*row, *count))
    }

    pub(crate) fn is_empty(&self) -> bool {
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
