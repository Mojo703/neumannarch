use std::collections::BTreeMap;

use crate::pattern::EntityPattern;

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Wants(BTreeMap<EntityPattern, u32>);

impl Wants {
    pub(crate) fn set(&mut self, pattern: EntityPattern, count: u32) {
        if count == 0 {
            self.0.remove(&pattern);
        } else {
            self.0.insert(pattern, count);
        }
    }

    pub(crate) fn get(&self, pattern: EntityPattern) -> u32 {
        self.0.get(&pattern).copied().unwrap_or(0)
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = (EntityPattern, u32)> + '_ {
        self.0.iter().map(|(pattern, count)| (*pattern, *count))
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pattern::EntityPattern as P;

    #[test]
    fn setting_zero_removes_the_pattern() {
        let mut wants = Wants::default();
        wants.set(P::Storage, 4);
        wants.set(P::Constructor, 2);
        assert_eq!(wants.get(P::Storage), 4);
        assert_eq!(
            wants.iter().collect::<Vec<_>>(),
            [(P::Constructor, 2), (P::Storage, 4)]
        );
        wants.set(P::Storage, 0);
        wants.set(P::Constructor, 0);
        assert_eq!(wants.get(P::Storage), 0);
        assert!(wants.is_empty());
    }
}
