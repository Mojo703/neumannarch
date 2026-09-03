//! The spatial sweep: every entity's position sorted along the belt plane's
//! `x`, so a range query reads one window instead of every entity.

use crate::ids::EntityId;
use crate::vec3::Vec3;

/// Every entity's position, sorted by belt-plane `x` then id; built once per
/// step from the snapshot, and read by every rule that needs what is near a
/// point.
#[derive(Clone, Debug)]
pub struct Sweep {
    items: Vec<Item>,
}

/// One swept entity.
#[derive(Clone, Copy, Debug)]
struct Item {
    id: EntityId,
    /// Position in meters.
    pos: Vec3,
}

impl Sweep {
    /// Sweeps `items`, one per entity, in any order; positions in meters.
    pub fn build(items: impl IntoIterator<Item = (EntityId, Vec3)>) -> Sweep {
        let mut items: Vec<Item> = items
            .into_iter()
            .map(|(id, pos)| Item { id, pos })
            .collect();
        items.sort_by(|a, b| a.pos.x.total_cmp(&b.pos.x).then(a.id.cmp(&b.id)));
        Sweep { items }
    }

    /// Every entity whose distance to `center` is at most `range`, both in
    /// meters, in ascending id order. A negative range finds nothing.
    pub fn within(&self, center: Vec3, range: f64) -> impl Iterator<Item = EntityId> + '_ {
        let mut found: Vec<EntityId> = self
            .window(center, range)
            .iter()
            .filter(|item| item.pos.distance(center) <= range)
            .map(|item| item.id)
            .collect();
        found.sort_unstable();
        found.into_iter()
    }

    /// The number of entities swept.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Whether no entity was swept.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// The run of items whose `x` displacement from `center` lies in
    /// `-range..=range`, found by two binary searches. The displacement is
    /// the one `distance` measures, so no rounding excludes a hit.
    fn window(&self, center: Vec3, range: f64) -> &[Item] {
        let dx = |item: &Item| item.pos.x - center.x;
        let start = self.items.partition_point(|item| dx(item) < -range);
        let rest = &self.items[start..];
        &rest[..rest.partition_point(|item| dx(item) <= range)]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 64-bit linear congruential generator, the tests' only source of
    /// variety; the same seed yields the same points on every target.
    struct Lcg(u64);

    impl Lcg {
        /// The next coordinate, in `-100.0..100.0` meters.
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

        /// Five hundred points, with ids given in descending order so the
        /// sweep's id ordering is its own work.
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
            let found: Vec<EntityId> = sweep.within(center, range).collect();
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
    fn results_are_in_ascending_id_order() {
        let mut lcg = Lcg(11);
        let sweep = Sweep::build(lcg.cloud());
        let found: Vec<EntityId> = sweep.within(Vec3::ZERO, 80.0).collect();
        assert!(found.len() > 1);
        assert!(found.windows(2).all(|pair| pair[0] < pair[1]));
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
