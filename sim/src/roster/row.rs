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
    pub effects: Vec<Effect>,
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
pub struct Hitscan {
    pub range: Real,
    pub rate: Real,
    pub damage: Real,
    pub falloff: Real,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Effect {
    Damage(Hitscan),
    Build { rate: Real },
    Extract { material: Material, rate: Real },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct DamagePlace {
    pub(crate) in_row: u8,
    pub(crate) hitscan: Hitscan,
}

impl Row {
    pub fn kind(&self) -> Kind {
        if self.manoeuvring.0 == 0.0 {
            Kind::Structure
        } else {
            Kind::Unit
        }
    }

    pub fn does_damage(&self) -> bool {
        self.hitscans().next().is_some()
    }

    pub(crate) fn standoff(&self) -> Option<f64> {
        self.does_damage()
            .then(|| 0.5 * (self.max_damage_range() - Belt::LINES_INSIDE_RANGE_METERS))
    }

    pub(crate) fn fires_past_its_lines(&self) -> bool {
        self.damage_ranges()
            .all(|range| range > Belt::LINES_INSIDE_RANGE_METERS)
    }

    pub(crate) fn runs_passes(&self) -> bool {
        self.role == Role::ShortFire && self.does_damage()
    }

    pub fn max_damage_range(&self) -> f64 {
        self.damage_ranges().fold(0.0, f64::max)
    }

    pub(crate) fn damage_ranges(&self) -> impl Iterator<Item = f64> + '_ {
        self.hitscans().map(|hitscan| hitscan.range.0)
    }

    pub fn dps_through(&self, plating: f64) -> f64 {
        self.hitscans()
            .map(|hitscan| hitscan.dps_through(plating))
            .sum()
    }

    pub(crate) fn damage_places(&self) -> impl Iterator<Item = DamagePlace> + '_ {
        self.effects.iter().enumerate().filter_map(|(at, effect)| {
            Some(DamagePlace {
                in_row: u8::try_from(at).ok()?,
                hitscan: effect.hitscan()?,
            })
        })
    }

    pub fn builds(&self) -> impl Iterator<Item = f64> + '_ {
        self.effects.iter().filter_map(Effect::build_rate)
    }

    pub fn extracts(&self) -> impl Iterator<Item = (Material, f64)> + '_ {
        self.effects.iter().filter_map(Effect::extraction)
    }

    pub fn extracts_of(&self, material: Material) -> f64 {
        self.extracts()
            .filter(|(pulled, _)| *pulled == material)
            .map(|(_, rate)| rate)
            .sum()
    }

    fn hitscans(&self) -> impl Iterator<Item = Hitscan> + '_ {
        self.effects.iter().filter_map(Effect::hitscan)
    }
}

impl Hitscan {
    fn dps_through(&self, plating: f64) -> f64 {
        (self.damage.0 - plating).max(0.0) * self.rate.0
    }
}

impl Effect {
    fn hitscan(&self) -> Option<Hitscan> {
        match *self {
            Effect::Damage(hitscan) => Some(hitscan),
            Effect::Build { .. } | Effect::Extract { .. } => None,
        }
    }

    fn build_rate(&self) -> Option<f64> {
        match *self {
            Effect::Build { rate } => Some(rate.0),
            Effect::Damage(_) | Effect::Extract { .. } => None,
        }
    }

    fn extraction(&self) -> Option<(Material, f64)> {
        match *self {
            Effect::Extract { material, rate } => Some((material, rate.0)),
            Effect::Damage(_) | Effect::Build { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(manoeuvring: f64, effects: Vec<Effect>) -> Row {
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
            effects,
        }
    }

    fn extract(material: Material, rate: f64) -> Effect {
        Effect::Extract {
            material,
            rate: Real(rate),
        }
    }

    fn damage(damage: f64, range: f64, rate: f64) -> Effect {
        Effect::Damage(Hitscan {
            range: Real(range),
            rate: Real(rate),
            damage: Real(damage),
            falloff: Real(0.0),
        })
    }

    #[test]
    fn a_row_with_no_manoeuvring_is_a_structure_and_any_other_a_unit() {
        assert_eq!(row(0.0, vec![]).kind(), Kind::Structure);
        assert_eq!(row(0.5, vec![]).kind(), Kind::Unit);
    }

    #[test]
    fn dps_through_subtracts_plating_per_hit_and_floors_at_zero() {
        let two_hitscans = row(1.0, vec![damage(6.0, 6.0, 2.0), damage(20.0, 14.0, 1.0)]);
        assert_eq!(two_hitscans.dps_through(0.0), 32.0);
        assert_eq!(two_hitscans.dps_through(1.0), 29.0);
        assert_eq!(two_hitscans.dps_through(10.0), 10.0);
        assert_eq!(two_hitscans.dps_through(100.0), 0.0);
    }

    #[test]
    fn only_a_hitscan_gives_a_row_damage_and_a_range() {
        let builder = row(1.0, vec![Effect::Build { rate: Real(3.0) }]);
        assert!(!builder.does_damage());
        assert_eq!(builder.max_damage_range(), 0.0);
        let two_hitscans = row(1.0, vec![damage(3.0, 3.0, 4.0), damage(20.0, 14.0, 1.0)]);
        assert!(two_hitscans.does_damage());
        assert_eq!(two_hitscans.max_damage_range(), 14.0);
    }

    #[test]
    fn a_damage_place_carries_the_stats_standing_where_it_names_in_the_row() {
        let mixed = row(
            1.0,
            vec![
                Effect::Build { rate: Real(3.0) },
                damage(3.0, 3.0, 4.0),
                extract(Material::Metals, 1.0),
                damage(20.0, 14.0, 1.0),
            ],
        );

        let places: Vec<DamagePlace> = mixed.damage_places().collect();

        assert_eq!(
            places,
            vec![
                DamagePlace {
                    in_row: 1,
                    hitscan: Hitscan {
                        range: Real(3.0),
                        rate: Real(4.0),
                        damage: Real(3.0),
                        falloff: Real(0.0),
                    },
                },
                DamagePlace {
                    in_row: 3,
                    hitscan: Hitscan {
                        range: Real(14.0),
                        rate: Real(1.0),
                        damage: Real(20.0),
                        falloff: Real(0.0),
                    },
                },
            ]
        );
    }

    #[test]
    fn builds_and_extracts_yield_the_matching_rates() {
        let mixed = row(
            0.0,
            vec![
                Effect::Build { rate: Real(3.0) },
                extract(Material::Volatiles, 2.0),
                damage(1.0, 1.0, 1.0),
                Effect::Build { rate: Real(15.0) },
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
