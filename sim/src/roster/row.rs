use crate::materials::Materials;
use crate::real::Real;

const LIGHT_MASS: f64 = 30.0;

const HEAVY_MASS: f64 = 100.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    Structure,
    Unit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MassClass {
    Light,
    Medium,
    Heavy,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Row {
    pub name: &'static str,
    pub cost: Materials,
    pub mass: Real,
    pub accel: Real,
    pub maneuver: Real,
    pub hp: Real,
    pub plating: Real,
    pub sight: Real,
    pub radar: Real,
    pub capacity: Materials,
    pub weapons: Vec<Weapon>,
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
        rate: Real,
    },
}

impl Row {
    pub fn kind(&self) -> Kind {
        if self.accel.0 == 0.0 {
            Kind::Structure
        } else {
            Kind::Unit
        }
    }

    pub fn mass_class(&self) -> MassClass {
        if self.mass.0 < LIGHT_MASS {
            MassClass::Light
        } else if self.mass.0 < HEAVY_MASS {
            MassClass::Medium
        } else {
            MassClass::Heavy
        }
    }

    pub fn is_armed(&self) -> bool {
        self.weapons.iter().any(|weapon| weapon.range().is_some())
    }

    pub fn max_damage_range(&self) -> f64 {
        self.weapons
            .iter()
            .filter_map(Weapon::range)
            .fold(0.0, f64::max)
    }

    pub fn dps_through(&self, plating: f64) -> f64 {
        self.weapons
            .iter()
            .map(|weapon| weapon.dps_through(plating))
            .sum()
    }

    pub fn damage_weapons(&self) -> impl Iterator<Item = u8> + '_ {
        self.weapons
            .iter()
            .enumerate()
            .filter(|(_, weapon)| weapon.range().is_some())
            .filter_map(|(at, _)| u8::try_from(at).ok())
    }

    pub fn builds(&self) -> impl Iterator<Item = f64> + '_ {
        self.weapons.iter().filter_map(Weapon::build_rate)
    }

    pub fn extracts(&self) -> impl Iterator<Item = f64> + '_ {
        self.weapons.iter().filter_map(Weapon::extract_rate)
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

    fn extract_rate(&self) -> Option<f64> {
        match *self {
            Weapon::Extract { rate } => Some(rate.0),
            Weapon::Damage { .. } | Weapon::Build { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(accel: f64, weapons: Vec<Weapon>) -> Row {
        Row {
            name: "test",
            cost: Materials::new(1.0, 0.0, 0.0),
            mass: Real(1.0),
            accel: Real(accel),
            maneuver: Real(accel * 0.25),
            hp: Real(1.0),
            plating: Real(0.0),
            sight: Real(1.0),
            radar: Real(2.0),
            capacity: Materials::ZERO,
            weapons,
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
    fn a_structure_is_exactly_a_row_that_cannot_accelerate() {
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
                Weapon::Extract { rate: Real(2.0) },
                damage(1.0, 1.0, 1.0),
                Weapon::Build { rate: Real(15.0) },
            ],
        );
        assert_eq!(mixed.builds().collect::<Vec<_>>(), [3.0, 15.0]);
        assert_eq!(mixed.extracts().collect::<Vec<_>>(), [2.0]);
    }
}
