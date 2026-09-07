use crate::ids::EntityId;
use crate::vec3::Vec3;

#[derive(Clone, Debug)]
pub(crate) struct Sweep {
    items: Vec<Item>,
}

#[derive(Clone, Copy, Debug)]
struct Item {
    id: EntityId,
    pos: Vec3,
}

impl Sweep {
    pub(crate) fn build(items: impl IntoIterator<Item = (EntityId, Vec3)>) -> Sweep {
        let mut items: Vec<Item> = items
            .into_iter()
            .map(|(id, pos)| Item { id, pos })
            .collect();
        items.sort_by(|a, b| a.pos.x.total_cmp(&b.pos.x).then(a.id.cmp(&b.id)));
        Sweep { items }
    }

    pub(crate) fn within(&self, center: Vec3, range: f64) -> impl Iterator<Item = EntityId> + '_ {
        self.window(center, range)
            .iter()
            .filter(move |item| item.pos.distance(center) <= range)
            .map(|item| item.id)
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.items.len()
    }

    #[cfg(test)]
    pub(crate) fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    fn window(&self, center: Vec3, range: f64) -> &[Item] {
        let dx = |item: &Item| item.pos.x - center.x;
        let start = self.items.partition_point(|item| dx(item) < -range);
        let rest = &self.items[start..];
        &rest[..rest.partition_point(|item| dx(item) <= range)]
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    struct Lcg(u64);

    impl Lcg {
        fn coordinate(&mut self) -> f64 {
            self.0 = self
                .0
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            let unit = (self.0 >> 11) as f64 / (1u64 << 53) as f64;
            unit * 200.0 - 100.0
        }

        fn point(&mut self) -> Vec3 {
            Vec3::new(self.coordinate(), self.coordinate(), self.coordinate())
        }

        fn cloud(&mut self) -> Vec<(EntityId, Vec3)> {
            (0..500)
                .rev()
                .map(|i| (EntityId(i), self.point()))
                .collect()
        }
    }

    fn brute_force(cloud: &[(EntityId, Vec3)], center: Vec3, range: f64) -> Vec<EntityId> {
        let mut found: Vec<EntityId> = cloud
            .iter()
            .filter(|(_, pos)| pos.distance(center) <= range)
            .map(|(id, _)| *id)
            .collect();
        found.sort_unstable();
        found
    }

    #[test]
    fn within_matches_a_brute_force_filter() {
        let mut lcg = Lcg(7);
        let cloud = lcg.cloud();
        let sweep = Sweep::build(cloud.iter().copied());
        assert_eq!(sweep.len(), cloud.len());
        for _ in 0..64 {
            let center = lcg.point();
            let range = (lcg.coordinate() + 100.0) * 0.4;
            let mut found: Vec<EntityId> = sweep.within(center, range).collect();
            found.sort_unstable();
            assert_eq!(
                found,
                brute_force(&cloud, center, range),
                "{center:?} {range}"
            );
        }
        let far = Vec3::new(1000.0, 0.0, 0.0);
        assert_eq!(sweep.within(far, 2000.0).count(), cloud.len());
        assert_eq!(sweep.within(far, 100.0).count(), 0);
    }

    #[test]
    fn results_come_back_in_the_sweeps_own_order() {
        let mut lcg = Lcg(11);
        let cloud = lcg.cloud();
        let sweep = Sweep::build(cloud.clone());
        let placed: BTreeMap<EntityId, Vec3> = cloud.into_iter().collect();
        let found: Vec<EntityId> = sweep.within(Vec3::ZERO, 80.0).collect();
        assert!(found.len() > 1);
        assert!(found.windows(2).all(|pair| {
            let (first, second) = (placed[&pair[0]], placed[&pair[1]]);
            (first.x, pair[0]) < (second.x, pair[1])
        }));
    }

    #[test]
    fn a_range_of_zero_returns_only_exact_coincidences() {
        let mut lcg = Lcg(3);
        let mut cloud = lcg.cloud();
        let spot = Vec3::new(1.0, 2.0, 3.0);
        cloud.push((EntityId(700), spot));
        cloud.push((EntityId(600), spot));
        cloud.push((EntityId(800), Vec3::new(1.0, 2.0, 3.0_f64.next_up())));
        let sweep = Sweep::build(cloud);
        let found: Vec<EntityId> = sweep.within(spot, 0.0).collect();
        assert_eq!(found, [EntityId(600), EntityId(700)]);
        assert_eq!(
            sweep.within(Vec3::new(1.0, 2.0, 3.0 - 1e-9), 0.0).count(),
            0
        );
    }

    #[test]
    fn a_negative_range_finds_nothing() {
        let sweep = Sweep::build([(EntityId(0), Vec3::ZERO)]);
        assert_eq!(sweep.within(Vec3::ZERO, -1.0).count(), 0);
    }

    #[test]
    fn an_empty_sweep_returns_nothing() {
        let sweep = Sweep::build([]);
        assert!(sweep.is_empty());
        assert_eq!(sweep.len(), 0);
        assert_eq!(sweep.within(Vec3::ZERO, f64::INFINITY).count(), 0);
    }
}
