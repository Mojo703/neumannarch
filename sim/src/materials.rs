//! The material triple, and the one place materials are held.

use core::hash::{Hash, Hasher};
use core::ops::{Add, AddAssign, Mul, Sub, SubAssign};

/// Metals, volatiles, energy. A cost, a cap per second, a capacity or a
/// stock, named by its field; the arithmetic is the same. Equal and hashed
/// by bit pattern.
#[derive(Clone, Copy, Debug, Default)]
pub struct Materials {
    pub metals: f64,
    pub volatiles: f64,
    pub energy: f64,
}

/// One of the three materials, for naming the one a spend wanted and did
/// not have.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Material {
    Metals,
    Volatiles,
    Energy,
}

/// A seat's materials. Income clamps to capacity, spending stops at zero,
/// refunds clamp; nothing else reads or writes a stock.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Stockpile {
    stock: Materials,
    capacity: Materials,
}

impl Materials {
    pub const ZERO: Materials = Materials::new(0.0, 0.0, 0.0);

    pub const fn new(metals: f64, volatiles: f64, energy: f64) -> Materials {
        Materials {
            metals,
            volatiles,
            energy,
        }
    }

    /// The sum of the three, in cost units.
    pub fn total(self) -> f64 {
        self.metals + self.volatiles + self.energy
    }

    /// `f` applied to each material.
    pub fn map(self, f: impl Fn(f64) -> f64) -> Materials {
        Materials::new(f(self.metals), f(self.volatiles), f(self.energy))
    }

    /// `f` applied to each material of `self` and `other` in turn.
    pub fn zip(self, other: Materials, f: impl Fn(f64, f64) -> f64) -> Materials {
        Materials::new(
            f(self.metals, other.metals),
            f(self.volatiles, other.volatiles),
            f(self.energy, other.energy),
        )
    }

    /// The smaller of each material.
    pub fn min(self, other: Materials) -> Materials {
        self.zip(other, f64::min)
    }

    /// Per material, the fraction of `demand` this covers, capped at one and
    /// one where nothing is demanded.
    pub fn covers(self, demand: Materials) -> Materials {
        self.zip(demand, |have, need| {
            if need > 0.0 {
                (have / need).min(1.0)
            } else {
                1.0
            }
        })
    }

    /// The smallest of `ratios` among the materials this uses, one if it
    /// uses none.
    pub fn bottleneck(self, ratios: Materials) -> f64 {
        self.amounts()
            .filter(|(_, amount)| *amount > 0.0)
            .map(|(material, _)| ratios[material])
            .fold(1.0, f64::min)
    }

    /// The material this uses whose `ratios` entry is smallest, ties in
    /// field order; `None` where this uses none.
    pub fn binding_material(self, ratios: Materials) -> Option<Material> {
        self.amounts()
            .filter(|(_, amount)| *amount > 0.0)
            .min_by(|(a, _), (b, _)| ratios[*a].total_cmp(&ratios[*b]))
            .map(|(material, _)| material)
    }

    /// Each material and how much of it this holds, in field order.
    fn amounts(self) -> impl Iterator<Item = (Material, f64)> {
        [Material::Metals, Material::Volatiles, Material::Energy]
            .into_iter()
            .map(move |material| (material, self[material]))
    }
}

impl Add for Materials {
    type Output = Materials;
    fn add(self, other: Materials) -> Materials {
        self.zip(other, |a, b| a + b)
    }
}

impl AddAssign for Materials {
    fn add_assign(&mut self, other: Materials) {
        *self = *self + other;
    }
}

impl Eq for Materials {}

impl core::ops::Index<Material> for Materials {
    type Output = f64;
    fn index(&self, material: Material) -> &f64 {
        match material {
            Material::Metals => &self.metals,
            Material::Volatiles => &self.volatiles,
            Material::Energy => &self.energy,
        }
    }
}

impl Hash for Materials {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.metals.to_bits().hash(state);
        self.volatiles.to_bits().hash(state);
        self.energy.to_bits().hash(state);
    }
}

impl Mul<f64> for Materials {
    type Output = Materials;
    fn mul(self, k: f64) -> Materials {
        self.map(|m| m * k)
    }
}

impl PartialEq for Materials {
    fn eq(&self, other: &Self) -> bool {
        self.metals.to_bits() == other.metals.to_bits()
            && self.volatiles.to_bits() == other.volatiles.to_bits()
            && self.energy.to_bits() == other.energy.to_bits()
    }
}

impl Sub for Materials {
    type Output = Materials;
    fn sub(self, other: Materials) -> Materials {
        self.zip(other, |a, b| a - b)
    }
}

impl SubAssign for Materials {
    fn sub_assign(&mut self, other: Materials) {
        *self = *self - other;
    }
}

impl Stockpile {
    /// Holds `stock` clamped to `capacity`.
    pub fn new(stock: Materials, capacity: Materials) -> Stockpile {
        Stockpile {
            stock: stock.min(capacity),
            capacity,
        }
    }

    pub fn stock(&self) -> Materials {
        self.stock
    }

    pub fn capacity(&self) -> Materials {
        self.capacity
    }

    /// Replaces the capacity; stock above it is lost.
    pub fn set_capacity(&mut self, capacity: Materials) {
        self.capacity = capacity;
        self.stock = self.stock.min(capacity);
    }

    /// Adds income or a refund; whatever exceeds capacity is lost.
    pub fn add(&mut self, materials: Materials) {
        self.stock = (self.stock + materials).min(self.capacity);
    }

    /// Takes up to `amount` of each material and returns what was taken.
    pub fn spend(&mut self, amount: Materials) -> Materials {
        let taken = amount.min(self.stock);
        self.stock -= taken;
        taken
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stockpile_never_exceeds_capacity_or_drops_below_zero() {
        let mut pile = Stockpile::new(
            Materials::new(900.0, 0.0, 0.0),
            Materials::new(500.0, 500.0, 500.0),
        );
        assert_eq!(pile.stock(), Materials::new(500.0, 0.0, 0.0));
        pile.add(Materials::new(10.0, 10.0, 10.0));
        assert_eq!(pile.stock(), Materials::new(500.0, 10.0, 10.0));
        let taken = pile.spend(Materials::new(1.0, 50.0, 0.0));
        assert_eq!(taken, Materials::new(1.0, 10.0, 0.0));
        assert_eq!(pile.stock(), Materials::new(499.0, 0.0, 10.0));
    }

    #[test]
    fn the_bottleneck_ignores_materials_a_cost_does_not_use() {
        let cost = Materials::new(40.0, 0.0, 10.0);
        let ratios = Materials::new(1.0, 0.0, 0.5);
        assert_eq!(cost.bottleneck(ratios), 0.5);
        assert_eq!(Materials::ZERO.bottleneck(ratios), 1.0);
    }

    #[test]
    fn covers_is_one_where_nothing_is_demanded() {
        let have = Materials::new(2.0, 0.0, 0.0);
        let need = Materials::new(4.0, 0.0, 3.0);
        assert_eq!(have.covers(need), Materials::new(0.5, 1.0, 0.0));
    }
}
