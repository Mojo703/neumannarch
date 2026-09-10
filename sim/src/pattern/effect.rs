use crate::materials::Material;
use crate::real::Real;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Effect {
    None,
    Build,
    FastBuild,
    Extract(Material),
    ShortFire,
    PlatedFire,
    LongFire,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Hitscan {
    pub range: Real,
    pub rate: Real,
    pub damage: Real,
    pub falloff: Real,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Extraction {
    pub material: Material,
    pub rate: Real,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum Slot {
    First,
    Second,
}

impl Effect {
    pub const fn hitscan(self) -> Option<Hitscan> {
        match self {
            Self::ShortFire => Some(Hitscan {
                range: Real(3.0),
                rate: Real(4.0),
                damage: Real(3.0),
                falloff: Real(0.5),
            }),
            Self::PlatedFire => Some(Hitscan {
                range: Real(6.0),
                rate: Real(2.0),
                damage: Real(6.0),
                falloff: Real(0.0),
            }),
            Self::LongFire => Some(Hitscan {
                range: Real(14.0),
                rate: Real(1.0),
                damage: Real(20.0),
                falloff: Real(0.0),
            }),
            Self::None | Self::Build | Self::FastBuild | Self::Extract(_) => None,
        }
    }

    pub const fn build_rate(self) -> Real {
        match self {
            Self::Build => Real(3.0),
            Self::FastBuild => Real(15.0),
            Self::None | Self::Extract(_) | Self::ShortFire | Self::PlatedFire | Self::LongFire => {
                Real(0.0)
            }
        }
    }

    pub const fn extraction(self) -> Option<Extraction> {
        match self {
            Self::Extract(material) => Some(Extraction {
                material,
                rate: Real(2.0),
            }),
            Self::None
            | Self::Build
            | Self::FastBuild
            | Self::ShortFire
            | Self::PlatedFire
            | Self::LongFire => None,
        }
    }
}

impl Hitscan {
    pub fn dps_through(self, plating: f64) -> f64 {
        (self.damage.0 - plating).max(0.0) * self.rate.0
    }
}

impl Slot {
    pub(crate) const EVERY: [Slot; 2] = [Slot::First, Slot::Second];
}

#[cfg(test)]
mod tests {
    use super::*;

    const EVERY: [Effect; 9] = [
        Effect::None,
        Effect::Build,
        Effect::FastBuild,
        Effect::Extract(Material::Metals),
        Effect::Extract(Material::Volatiles),
        Effect::Extract(Material::Energy),
        Effect::ShortFire,
        Effect::PlatedFire,
        Effect::LongFire,
    ];

    #[test]
    fn every_effect_is_one_kind_and_none_is_no_kind() {
        for effect in EVERY {
            let kinds = usize::from(effect.hitscan().is_some())
                + usize::from(effect.build_rate().0 > 0.0)
                + usize::from(effect.extraction().is_some());
            let expected = usize::from(effect != Effect::None);
            assert_eq!(kinds, expected, "{effect:?}");
        }
    }

    #[test]
    fn dps_through_subtracts_plating_per_hit_and_floors_at_zero() {
        let hitscan = Hitscan {
            range: Real(6.0),
            rate: Real(2.0),
            damage: Real(6.0),
            falloff: Real(0.0),
        };
        assert_eq!(hitscan.dps_through(0.0), 12.0);
        assert_eq!(hitscan.dps_through(1.0), 10.0);
        assert_eq!(hitscan.dps_through(6.0), 0.0);
        assert_eq!(hitscan.dps_through(100.0), 0.0);
    }

    #[test]
    fn one_effect_extracts_each_material_and_names_it() {
        for material in [Material::Metals, Material::Volatiles, Material::Energy] {
            let pulling: Vec<Effect> = EVERY
                .into_iter()
                .filter(|effect| {
                    effect
                        .extraction()
                        .is_some_and(|extraction| extraction.material == material)
                })
                .collect();
            assert_eq!(pulling, [Effect::Extract(material)]);
        }
    }
}
