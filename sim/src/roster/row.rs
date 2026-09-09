use crate::belt::Belt;
use crate::materials::{Material, Materials};
use crate::real::Real;
use crate::roster::glyph::{Role, Tier};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    Structure,
    Unit,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Row {
    pub name: &'static str,
    pub role: Role,
    pub tier: Tier,
    pub cost: Materials,
    pub manoeuvring: Real,
    pub steering: Weights,
    pub hp: Real,
    pub plating: Real,
    pub capacity: Materials,
    pub weapons: Vec<Weapon>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Weights {
    pub wander: Real,
    pub returning: Real,
    pub separation: Real,
    pub station: Real,
}

impl Weights {
    pub const STILL: Weights = Weights {
        wander: Real(0.0),
        returning: Real(0.0),
        separation: Real(0.0),
        station: Real(0.0),
    };
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Weapon {
    Damage {
        range: Real,
        rate: Real,
        damage: Real,
        falloff: Real,
    },
    Build {
        rate: Real,
    },
    Extract {
        material: Material,
        rate: Real,
    },
}

impl Row {
    pub fn kind(&self) -> Kind {
        if self.manoeuvring.0 == 0.0 {
            Kind::Structure
        } else {
            Kind::Unit
        }
    }

    pub fn is_armed(&self) -> bool {
        self.weapons.iter().any(|weapon| weapon.range().is_some())
    }

    pub(crate) fn standoff(&self) -> Option<f64> {
        self.is_armed()
            .then(|| 0.5 * (self.max_damage_range() - Belt::LINES_INSIDE_RANGE_METERS))
    }

    pub(crate) fn fires_past_its_lines(&self) -> bool {
        self.damage_ranges()
            .all(|range| range > Belt::LINES_INSIDE_RANGE_METERS)
    }

    pub(crate) fn runs_passes(&self) -> bool {
        self.role == Role::ShortFire && self.is_armed()
    }

    pub fn max_damage_range(&self) -> f64 {
        self.damage_ranges().fold(0.0, f64::max)
    }

    pub(crate) fn damage_ranges(&self) -> impl Iterator<Item = f64> + '_ {
        self.weapons.iter().filter_map(Weapon::range)
    }

    pub(crate) fn damage_range(&self, weapon: u8) -> Option<f64> {
        self.weapons
            .get(usize::from(weapon))
            .and_then(Weapon::range)
    }

    pub fn dps_through(&self, plating: f64) -> f64 {
        self.weapons
            .iter()
            .map(|weapon| weapon.dps_through(plating))
            .sum()
    }

    pub(crate) fn damage_weapons(&self) -> impl Iterator<Item = u8> + '_ {
        self.weapons
            .iter()
            .enumerate()
            .filter(|(_, weapon)| weapon.range().is_some())
            .filter_map(|(at, _)| u8::try_from(at).ok())
    }

    pub fn builds(&self) -> impl Iterator<Item = f64> + '_ {
        self.weapons.iter().filter_map(Weapon::build_rate)
    }

    pub fn extracts(&self) -> impl Iterator<Item = (Material, f64)> + '_ {
        self.weapons.iter().filter_map(Weapon::extraction)
    }

    pub fn extracts_of(&self, material: Material) -> f64 {
        self.extracts()
            .filter(|(pulled, _)| *pulled == material)
            .map(|(_, rate)| rate)
            .sum()
    }
}

impl Weapon {
    fn range(&self) -> Option<f64> {
        match *self {
            Weapon::Damage { range, .. } => Some(range.0),
            Weapon::Build { .. } | Weapon::Extract { .. } => None,
        }
    }

    fn dps_through(&self, plating: f64) -> f64 {
        match *self {
            Weapon::Damage { rate, damage, .. } => (damage.0 - plating).max(0.0) * rate.0,
            Weapon::Build { .. } | Weapon::Extract { .. } => 0.0,
        }
    }

    fn build_rate(&self) -> Option<f64> {
        match *self {
            Weapon::Build { rate } => Some(rate.0),
            Weapon::Damage { .. } | Weapon::Extract { .. } => None,
        }
    }

    fn extraction(&self) -> Option<(Material, f64)> {
        match *self {
            Weapon::Extract { material, rate } => Some((material, rate.0)),
            Weapon::Damage { .. } | Weapon::Build { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(manoeuvring: f64, weapons: Vec<Weapon>) -> Row {
        Row {
            name: "test",
            role: Role::Scout,
            tier: Tier::ONE,
            cost: Materials::new(1.0, 0.0, 0.0),
            manoeuvring: Real(manoeuvring),
            steering: Weights::STILL,
            hp: Real(1.0),
            plating: Real(0.0),
            capacity: Materials::ZERO,
            weapons,
        }
    }

    fn extract(material: Material, rate: f64) -> Weapon {
        Weapon::Extract {
            material,
            rate: Real(rate),
        }
    }

    fn damage(damage: f64, range: f64, rate: f64) -> Weapon {
        Weapon::Damage {
            range: Real(range),
            rate: Real(rate),
            damage: Real(damage),
            falloff: Real(0.0),
        }
    }

    #[test]
    fn a_row_with_no_manoeuvring_is_a_structure_and_any_other_a_unit() {
        assert_eq!(row(0.0, vec![]).kind(), Kind::Structure);
        assert_eq!(row(0.5, vec![]).kind(), Kind::Unit);
    }

    #[test]
    fn dps_through_subtracts_plating_per_hit_and_floors_at_zero() {
        let armed = row(1.0, vec![damage(6.0, 6.0, 2.0), damage(20.0, 14.0, 1.0)]);
        assert_eq!(armed.dps_through(0.0), 32.0);
        assert_eq!(armed.dps_through(1.0), 29.0);
        assert_eq!(armed.dps_through(10.0), 10.0);
        assert_eq!(armed.dps_through(100.0), 0.0);
    }

    #[test]
    fn range_and_arms_come_from_damage_weapons_only() {
        let unarmed = row(1.0, vec![Weapon::Build { rate: Real(3.0) }]);
        assert!(!unarmed.is_armed());
        assert_eq!(unarmed.max_damage_range(), 0.0);
        let armed = row(1.0, vec![damage(3.0, 3.0, 4.0), damage(20.0, 14.0, 1.0)]);
        assert!(armed.is_armed());
        assert_eq!(armed.max_damage_range(), 14.0);
    }

    #[test]
    fn builds_and_extracts_yield_the_matching_rates() {
        let mixed = row(
            0.0,
            vec![
                Weapon::Build { rate: Real(3.0) },
                extract(Material::Volatiles, 2.0),
                damage(1.0, 1.0, 1.0),
                Weapon::Build { rate: Real(15.0) },
                extract(Material::Volatiles, 0.5),
            ],
        );
        assert_eq!(mixed.builds().collect::<Vec<_>>(), [3.0, 15.0]);
        assert_eq!(
            mixed.extracts().collect::<Vec<_>>(),
            [(Material::Volatiles, 2.0), (Material::Volatiles, 0.5)]
        );
        assert_eq!(mixed.extracts_of(Material::Volatiles), 2.5);
        assert_eq!(mixed.extracts_of(Material::Metals), 0.0);
    }
}
