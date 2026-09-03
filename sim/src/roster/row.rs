//! A row of the roster and its weapons.

use crate::materials::Materials;
use crate::real::Real;

/// The mass, on the roster's scale, below which radar reports a contact
/// as light. A hypothesis the display confirms or kills.
const LIGHT_MASS: f64 = 30.0;

/// The mass, on the roster's scale, below which radar reports a contact as
/// medium and at or above which it reports heavy. A hypothesis.
const HEAVY_MASS: f64 = 100.0;

/// Whether a row's copies can move; a structure holds its rock's body.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    Structure,
    Unit,
}

/// How much a radar contact weighs, as much as radar can tell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MassClass {
    Light,
    Medium,
    Heavy,
}

/// One row of the roster. An entity is a copy of its row plus position,
/// velocity, HP and home.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Row {
    /// The row's name; shipped names are distinct.
    pub name: &'static str,
    /// What a frame spends to complete one copy.
    pub cost: Materials,
    /// Mass, on the roster's own scale; radar reports it as a rough class.
    pub mass: Real,
    /// The movement limit in m/s²: the thrust a burn of a send uses. Zero
    /// for a structure.
    pub accel: Real,
    /// The manoeuvring limit in m/s²: the thrust holding position uses,
    /// far below `accel`. Zero for a structure.
    pub maneuver: Real,
    /// Hit points of a fresh copy.
    pub hp: Real,
    /// Damage subtracted from each hit taken.
    pub plating: Real,
    /// Sight range in meters; exact detection, and the leash.
    pub sight: Real,
    /// Radar range in meters; detection as a blip.
    pub radar: Real,
    /// Stockpile capacity one copy contributes, per material.
    pub capacity: Materials,
    /// Weapons, indexed by position.
    pub weapons: Vec<Weapon>,
}

/// One weapon of a row; each kind carries its own fields.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Weapon {
    /// Hitscan at the highest-threat enemy in range.
    Damage {
        /// Reach in meters.
        range: Real,
        /// Shots per second.
        rate: Real,
        /// Damage per hit at zero distance, before plating.
        damage: Real,
        /// Fraction of `damage` lost at `range`.
        falloff: Real,
    },
    /// Spends stockpile toward frames at the home rock, repairs and scraps
    /// there.
    Build {
        /// Cost units per second.
        rate: Real,
    },
    /// Pulls materials from the home rock.
    Extract {
        /// Units per second of each material, before the rock's cap.
        rate: Real,
    },
}

impl Row {
    /// `Structure` exactly when acceleration is zero, else `Unit`.
    pub fn kind(&self) -> Kind {
        if self.accel.0 == 0.0 {
            Kind::Structure
        } else {
            Kind::Unit
        }
    }

    /// The class radar reports a copy of this row as.
    pub fn mass_class(&self) -> MassClass {
        if self.mass.0 < LIGHT_MASS {
            MassClass::Light
        } else if self.mass.0 < HEAVY_MASS {
            MassClass::Medium
        } else {
            MassClass::Heavy
        }
    }

    /// Whether any weapon deals damage.
    pub fn is_armed(&self) -> bool {
        self.weapons.iter().any(|weapon| weapon.range().is_some())
    }

    /// The longest damage weapon's reach in meters; zero when unarmed.
    pub fn max_damage_range(&self) -> f64 {
        self.weapons
            .iter()
            .filter_map(Weapon::range)
            .fold(0.0, f64::max)
    }

    /// Damage per second at zero distance against `plating`, each hit
    /// floored at zero.
    pub fn dps_through(&self, plating: f64) -> f64 {
        self.weapons
            .iter()
            .map(|weapon| weapon.dps_through(plating))
            .sum()
    }

    /// The index of each damage weapon in this row.
    pub fn damage_weapons(&self) -> impl Iterator<Item = u8> + '_ {
        self.weapons
            .iter()
            .enumerate()
            .filter(|(_, weapon)| weapon.range().is_some())
            .filter_map(|(at, _)| u8::try_from(at).ok())
    }

    /// The rate of each build weapon, in cost units per second.
    pub fn builds(&self) -> impl Iterator<Item = f64> + '_ {
        self.weapons.iter().filter_map(Weapon::build_rate)
    }

    /// The rate of each extract weapon, in units per second of each
    /// material.
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
