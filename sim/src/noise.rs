use core::hash::Hash;

use crate::state::hash::digest;
use crate::vec3::Vec3;

const DIGEST_RANGE: f64 = u64::MAX as f64 + 1.0;

pub(crate) struct Noise {
    key: u64,
    cell_meters: f64,
}

impl Noise {
    pub(crate) fn keyed(key: u64, cell_meters: f64) -> Noise {
        Noise { key, cell_meters }
    }

    pub(crate) fn at(&self, point: Vec3) -> f64 {
        let (u, v) = (point.x / self.cell_meters, point.z / self.cell_meters);
        let (cell_u, cell_v) = (u.floor(), v.floor());
        let (eased_u, eased_v) = (eased(u - cell_u), eased(v - cell_v));
        let (cell_u, cell_v) = (cell_u as i64, cell_v as i64);
        let corner = |along: i64, across: i64| self.corner(cell_u + along, cell_v + across);
        let near = between(corner(0, 0), corner(1, 0), eased_u);
        let far = between(corner(0, 1), corner(1, 1), eased_u);
        between(near, far, eased_v)
    }

    fn corner(&self, along: i64, across: i64) -> f64 {
        fraction(&(self.key, along, across))
    }
}

pub(crate) fn fraction<T: Hash + ?Sized>(of: &T) -> f64 {
    digest(of) as f64 / DIGEST_RANGE
}

fn eased(along: f64) -> f64 {
    along * along * (3.0 - 2.0 * along)
}

fn between(from: f64, to: f64, along: f64) -> f64 {
    from + (to - from) * along
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: u64 = 0x9e37_79b9_7f4a_7c15;

    const CELL_METERS: f64 = 8_000.0;

    const STEP_METERS: f64 = 1.0;

    fn walked() -> impl Iterator<Item = Vec3> {
        (0..400).map(|at| {
            let along = at as f64;
            Vec3::new(along * 137.0 - 20_000.0, 0.0, along * 61.0 - 9_000.0)
        })
    }

    #[test]
    fn a_field_moves_little_between_two_points_a_metre_apart() {
        let noise = Noise::keyed(KEY, CELL_METERS);
        for point in walked() {
            let here = noise.at(point);
            let there = noise.at(point + Vec3::new(STEP_METERS, 0.0, STEP_METERS));
            assert!(
                (here - there).abs() < 4.0 * STEP_METERS / CELL_METERS,
                "{here} jumps to {there} a metre away at {point:?}"
            );
        }
    }

    #[test]
    fn a_field_stays_inside_the_unit_range() {
        let noise = Noise::keyed(KEY, CELL_METERS);
        for point in walked() {
            let value = noise.at(point);
            assert!((0.0..=1.0).contains(&value), "{value} at {point:?}");
        }
    }

    #[test]
    fn the_same_key_reads_the_same_value_at_the_same_point() {
        let point = Vec3::new(11_111.0, 0.0, -3_333.0);
        assert_eq!(
            Noise::keyed(KEY, CELL_METERS).at(point),
            Noise::keyed(KEY, CELL_METERS).at(point)
        );
    }

    #[test]
    fn two_keys_read_two_values_at_one_point() {
        let point = Vec3::new(11_111.0, 0.0, -3_333.0);
        assert_ne!(
            Noise::keyed(KEY, CELL_METERS).at(point),
            Noise::keyed(!KEY, CELL_METERS).at(point)
        );
    }

    #[test]
    fn a_field_varies_across_the_belt() {
        let noise = Noise::keyed(KEY, CELL_METERS);
        let read: Vec<f64> = walked().map(|point| noise.at(point)).collect();
        let least = read.iter().copied().fold(f64::INFINITY, f64::min);
        let most = read.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        assert!(
            most - least > 0.4,
            "the field spans {least} to {most} alone"
        );
    }
}
