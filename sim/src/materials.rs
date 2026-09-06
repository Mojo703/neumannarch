use core::hash::{Hash, Hasher};
use core::ops::{Add, AddAssign, Mul, Sub, SubAssign};

#[derive(Clone, Copy, Debug, Default)]
pub struct Materials([f64; 3]);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Material {
    Metals,
    Volatiles,
    Energy,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub(crate) struct PerSecond {
    filling: Materials,
    completed: Materials,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Stockpile {
    stock: Materials,
    capacity: Materials,
}

impl Material {
    pub const EVERY: [Material; 3] = [Material::Metals, Material::Volatiles, Material::Energy];
}

impl Materials {
    pub const ZERO: Materials = Materials([0.0; 3]);

    pub const fn new(metals: f64, volatiles: f64, energy: f64) -> Materials {
        Materials([metals, volatiles, energy])
    }

    pub fn total(self) -> f64 {
        self.0.iter().sum()
    }

    pub(crate) fn map(self, f: impl Fn(f64) -> f64) -> Materials {
        Materials(self.0.map(f))
    }

    fn zip(self, other: Materials, f: impl Fn(f64, f64) -> f64) -> Materials {
        Materials(core::array::from_fn(|at| f(self.0[at], other.0[at])))
    }

    pub fn min(self, other: Materials) -> Materials {
        self.zip(other, f64::min)
    }

    pub(crate) fn covers(self, demand: Materials) -> Materials {
        self.zip(demand, |have, need| match need > 0.0 {
            true => (have / need).min(1.0),
            false => 1.0,
        })
    }

    pub(crate) fn binding(self, ratios: Materials) -> Option<(Material, f64)> {
        self.amounts()
            .filter(|(_, amount)| *amount > 0.0)
            .map(|(material, _)| (material, ratios[material]))
            .min_by(|(_, a), (_, b)| a.total_cmp(b))
    }

    pub fn amounts(self) -> impl Iterator<Item = (Material, f64)> {
        Material::EVERY.into_iter().zip(self.0)
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
        &self.0[material as usize]
    }
}

impl core::ops::IndexMut<Material> for Materials {
    fn index_mut(&mut self, material: Material) -> &mut f64 {
        &mut self.0[material as usize]
    }
}

impl Hash for Materials {
    fn hash<H: Hasher>(&self, state: &mut H) {
        for amount in self.0 {
            amount.to_bits().hash(state);
        }
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
        self.0.map(f64::to_bits) == other.0.map(f64::to_bits)
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

impl PerSecond {
    pub(crate) fn completed(&self) -> Materials {
        self.completed
    }

    pub(crate) fn fill(&mut self, materials: Materials) {
        self.filling += materials;
    }

    pub(crate) fn close(&mut self) {
        self.completed = self.filling;
        self.filling = Materials::ZERO;
    }
}

impl Stockpile {
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

    pub fn set_capacity(&mut self, capacity: Materials) {
        self.capacity = capacity;
        self.stock = self.stock.min(capacity);
    }

    pub fn add(&mut self, materials: Materials) {
        self.stock = (self.stock + materials).min(self.capacity);
    }

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
    fn the_binding_material_ignores_materials_a_cost_does_not_use() {
        let cost = Materials::new(40.0, 0.0, 10.0);
        let ratios = Materials::new(1.0, 0.0, 0.5);
        assert_eq!(cost.binding(ratios), Some((Material::Energy, 0.5)));
        assert_eq!(Materials::ZERO.binding(ratios), None);
    }

    #[test]
    fn covers_is_one_where_nothing_is_demanded() {
        let have = Materials::new(2.0, 0.0, 0.0);
        let need = Materials::new(4.0, 0.0, 3.0);
        assert_eq!(have.covers(need), Materials::new(0.5, 1.0, 0.0));
    }
}
