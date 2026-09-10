use core::fmt::{self, Display, Formatter};
use core::hash::{Hash, Hasher};

pub(crate) use effect::{Effect, Extraction, Hitscan};
pub use glyph::{Frame, Glyph, Role, Tier};

pub(crate) use effect::Slot;
use serde::{Deserialize, Serialize};

use crate::belt::Belt;
use crate::materials::{Material, Materials};
use crate::real::Real;

const STORE_CAPACITY: Materials = Materials::new(500.0, 500.0, 500.0);

const HOLDING_WEIGHTS: Weights = Weights {
    wander: Real(0.3),
    returning: Real(1.0),
    separation: Real(20.0),
    station: Real(1.0),
};

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(into = "u8", try_from = "u8")]
pub enum EntityPattern {
    Constructor,
    Extractor(Material),
    Storage,
    Shipyard,
    Raider,
    Frigate,
    Lancer,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    Structure,
    Unit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct Weights {
    pub wander: Real,
    pub returning: Real,
    pub separation: Real,
    pub station: Real,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UnknownPattern(pub u8);

impl EntityPattern {
    pub const EVERY: [EntityPattern; 9] = [
        Self::Constructor,
        Self::Extractor(Material::Metals),
        Self::Extractor(Material::Volatiles),
        Self::Extractor(Material::Energy),
        Self::Storage,
        Self::Shipyard,
        Self::Raider,
        Self::Frigate,
        Self::Lancer,
    ];

    pub const fn code(self) -> u8 {
        match self {
            Self::Constructor => 0,
            Self::Extractor(Material::Metals) => 1,
            Self::Extractor(Material::Volatiles) => 2,
            Self::Extractor(Material::Energy) => 3,
            Self::Storage => 4,
            Self::Shipyard => 5,
            Self::Raider => 6,
            Self::Frigate => 7,
            Self::Lancer => 8,
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::Constructor => "constructor",
            Self::Extractor(Material::Metals) => "metals extractor",
            Self::Extractor(Material::Volatiles) => "volatiles extractor",
            Self::Extractor(Material::Energy) => "energy extractor",
            Self::Storage => "storage",
            Self::Shipyard => "shipyard",
            Self::Raider => "raider",
            Self::Frigate => "frigate",
            Self::Lancer => "lancer",
        }
    }

    pub const fn role(self) -> Role {
        match self {
            Self::Constructor | Self::Shipyard => Role::Build,
            Self::Extractor(_) => Role::Extract,
            Self::Storage => Role::Store,
            Self::Raider | Self::Frigate => Role::ShortFire,
            Self::Lancer => Role::LongFire,
        }
    }

    pub const fn tier(self) -> Tier {
        match self {
            Self::Constructor
            | Self::Extractor(_)
            | Self::Storage
            | Self::Shipyard
            | Self::Raider => Tier::ONE,
            Self::Frigate => Tier::TWO,
            Self::Lancer => Tier::THREE,
        }
    }

    pub const fn cost(self) -> Materials {
        match self {
            Self::Constructor => Materials::new(30.0, 10.0, 10.0),
            Self::Extractor(_) => Materials::new(20.0, 0.0, 5.0),
            Self::Storage => Materials::new(30.0, 0.0, 10.0),
            Self::Shipyard => Materials::new(100.0, 0.0, 40.0),
            Self::Raider => Materials::new(20.0, 20.0, 5.0),
            Self::Frigate => Materials::new(80.0, 10.0, 30.0),
            Self::Lancer => Materials::new(40.0, 5.0, 40.0),
        }
    }

    pub const fn manoeuvring(self) -> Real {
        match self {
            Self::Constructor => Real(1.0),
            Self::Extractor(_) | Self::Storage | Self::Shipyard => Real(0.0),
            Self::Raider => Real(2.0),
            Self::Frigate => Real(1.25),
            Self::Lancer => Real(0.75),
        }
    }

    pub(crate) const fn steering(self) -> Weights {
        match self {
            Self::Constructor | Self::Frigate | Self::Lancer => HOLDING_WEIGHTS,
            Self::Extractor(_) | Self::Storage | Self::Shipyard => Weights::STILL,
            Self::Raider => Weights {
                wander: Real(0.5),
                ..HOLDING_WEIGHTS
            },
        }
    }

    pub const fn hp(self) -> Real {
        match self {
            Self::Constructor => Real(50.0),
            Self::Extractor(_) => Real(120.0),
            Self::Storage | Self::Frigate => Real(150.0),
            Self::Shipyard => Real(300.0),
            Self::Raider => Real(40.0),
            Self::Lancer => Real(60.0),
        }
    }

    pub const fn plating(self) -> Real {
        match self {
            Self::Constructor
            | Self::Extractor(_)
            | Self::Storage
            | Self::Shipyard
            | Self::Raider
            | Self::Lancer => Real(0.0),
            Self::Frigate => Real(1.0),
        }
    }

    pub const fn capacity(self) -> Materials {
        match self {
            Self::Constructor
            | Self::Extractor(_)
            | Self::Raider
            | Self::Frigate
            | Self::Lancer => Materials::ZERO,
            Self::Storage | Self::Shipyard => STORE_CAPACITY,
        }
    }

    pub(crate) const fn effects(self) -> [Effect; 2] {
        match self {
            Self::Constructor => [Effect::Build, Effect::None],
            Self::Extractor(material) => [Effect::Extract(material), Effect::None],
            Self::Storage => [Effect::None, Effect::None],
            Self::Shipyard => [Effect::FastBuild, Effect::None],
            Self::Raider => [Effect::ShortFire, Effect::None],
            Self::Frigate => [Effect::PlatedFire, Effect::None],
            Self::Lancer => [Effect::LongFire, Effect::None],
        }
    }

    pub(crate) const fn effect_at(self, slot: Slot) -> Effect {
        self.effects()[slot as usize]
    }

    pub fn does_damage(self) -> bool {
        self.hitscans().next().is_some()
    }

    pub fn max_damage_range(self) -> f64 {
        self.hitscans()
            .map(|(_, hitscan)| hitscan.range.0)
            .fold(0.0, f64::max)
    }

    pub fn dps_through(self, plating: f64) -> f64 {
        self.hitscans()
            .map(|(_, hitscan)| hitscan.dps_through(plating))
            .sum()
    }

    pub fn build_rate(self) -> f64 {
        self.effects()
            .into_iter()
            .map(|effect| effect.build_rate().0)
            .sum()
    }

    pub fn extracts(self, material: Material) -> f64 {
        self.extractions()
            .filter(|extraction| extraction.material == material)
            .map(|extraction| extraction.rate.0)
            .sum()
    }

    pub(crate) fn extractions(self) -> impl Iterator<Item = Extraction> {
        self.effects().into_iter().filter_map(Effect::extraction)
    }

    pub(crate) fn hitscans(self) -> impl Iterator<Item = (Slot, Hitscan)> {
        Slot::EVERY.into_iter().filter_map(move |slot| {
            self.effect_at(slot)
                .hitscan()
                .map(|hitscan| (slot, hitscan))
        })
    }

    pub const LONGEST_DAMAGE_RANGE_METERS: f64 = {
        let mut longest = 0.0;
        let mut at = 0;
        while at < Self::EVERY.len() {
            let mut slot = 0;
            let effects = Self::EVERY[at].effects();
            while slot < effects.len() {
                if let Some(hitscan) = effects[slot].hitscan()
                    && hitscan.range.0 > longest
                {
                    longest = hitscan.range.0;
                }
                slot += 1;
            }
            at += 1;
        }
        longest
    };

    pub const fn kind(self) -> Kind {
        match self {
            Self::Extractor(_) | Self::Storage | Self::Shipyard => Kind::Structure,
            Self::Constructor | Self::Raider | Self::Frigate | Self::Lancer => Kind::Unit,
        }
    }

    pub(crate) fn standoff(self) -> Option<f64> {
        self.does_damage()
            .then(|| 0.5 * (self.max_damage_range() - Belt::LINES_INSIDE_RANGE_METERS))
    }

    pub(crate) fn runs_passes(self) -> bool {
        self.role() == Role::ShortFire && self.does_damage()
    }

    #[cfg(test)]
    fn fires_past_its_lines(self) -> bool {
        self.hitscans()
            .all(|(_, hitscan)| hitscan.range.0 > Belt::LINES_INSIDE_RANGE_METERS)
    }
}

impl Weights {
    pub const STILL: Weights = Weights {
        wander: Real(0.0),
        returning: Real(0.0),
        separation: Real(0.0),
        station: Real(0.0),
    };
}

impl Hash for EntityPattern {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.code().hash(state);
    }
}

impl PartialOrd for EntityPattern {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for EntityPattern {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.code().cmp(&other.code())
    }
}

impl From<EntityPattern> for u8 {
    fn from(pattern: EntityPattern) -> u8 {
        pattern.code()
    }
}

impl TryFrom<u8> for EntityPattern {
    type Error = UnknownPattern;

    fn try_from(code: u8) -> Result<EntityPattern, UnknownPattern> {
        EntityPattern::EVERY
            .into_iter()
            .find(|pattern| pattern.code() == code)
            .ok_or(UnknownPattern(code))
    }
}

impl Display for UnknownPattern {
    fn fmt(&self, out: &mut Formatter<'_>) -> fmt::Result {
        write!(out, "no pattern has code {}", self.0)
    }
}

mod effect;
mod glyph;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pattern::EntityPattern as P;

    #[test]
    fn every_pattern_stands_in_code_order_and_reads_back_from_its_code() {
        for (at, pattern) in P::EVERY.into_iter().enumerate() {
            let code = u8::try_from(at).expect("nine patterns");
            assert_eq!(pattern.code(), code, "{}", pattern.name());
            assert_eq!(P::try_from(code), Ok(pattern));
        }
        let past = u8::try_from(P::EVERY.len()).expect("nine patterns");
        assert_eq!(P::try_from(past), Err(UnknownPattern(past)));
        assert_eq!(
            UnknownPattern(past).to_string(),
            "no pattern has code 9",
            "an unknown code names itself"
        );
    }

    #[test]
    fn no_pattern_carries_an_effect_behind_an_empty_slot() {
        for pattern in P::EVERY {
            let [first, second] = pattern.effects();
            assert!(
                first != Effect::None || second == Effect::None,
                "{} holds {second:?} behind an empty first slot",
                pattern.name()
            );
        }
    }

    #[test]
    fn names_are_distinct() {
        let mut names: Vec<&str> = P::EVERY.iter().map(|it| it.name()).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), P::EVERY.len());
    }

    #[test]
    fn the_structures_are_the_patterns_with_no_manoeuvring() {
        for pattern in P::EVERY {
            let expected = match pattern {
                P::Extractor(_) | P::Storage | P::Shipyard => Kind::Structure,
                P::Constructor | P::Raider | P::Frigate | P::Lancer => Kind::Unit,
            };
            assert_eq!(pattern.kind(), expected, "{}", pattern.name());
        }
    }

    #[test]
    fn every_pattern_costs_something() {
        for pattern in P::EVERY {
            assert!(pattern.cost().total() > 0.0, "{}", pattern.name());
        }
    }

    #[test]
    fn one_pattern_extracts_each_material_and_none_extracts_another() {
        for material in Material::EVERY {
            let extracting: Vec<EntityPattern> = P::EVERY
                .into_iter()
                .filter(|pattern| pattern.extracts(material) > 0.0)
                .collect();
            assert_eq!(extracting, [P::Extractor(material)]);
        }
        for pattern in P::EVERY {
            let pulls = pattern.extractions().count();
            assert!(pulls <= 1, "{} pulls two", pattern.name());
        }
    }

    #[test]
    fn a_structure_holds_by_nothing_and_every_unit_by_every_term() {
        for pattern in P::EVERY {
            let steering = pattern.steering();
            match pattern.kind() {
                Kind::Structure => assert_eq!(steering, Weights::STILL, "{}", pattern.name()),
                Kind::Unit => {
                    assert!(steering.returning.0 > 0.0, "{}", pattern.name());
                    assert!(steering.wander.0 > 0.0, "{}", pattern.name());
                    assert!(steering.separation.0 > 0.0, "{}", pattern.name());
                    assert!(
                        steering.station.0 > 0.0,
                        "{} weighs no station, and takes one whether it does damage or not",
                        pattern.name()
                    );
                }
            }
        }
    }

    #[test]
    fn every_pattern_that_does_damage_stands_off_the_stage_and_no_other_does() {
        for pattern in P::EVERY {
            assert_eq!(
                pattern.standoff().is_some(),
                pattern.does_damage(),
                "{} stands off the stage with no damage to reach across it",
                pattern.name()
            );
            assert!(
                pattern.standoff().is_none_or(|standoff| standoff > 0.0),
                "{} stands off {:?}, on the far side of the stage's centre",
                pattern.name(),
                pattern.standoff()
            );
        }
    }

    #[test]
    fn every_pattern_fires_past_its_lines_and_stations_inside_the_zone() {
        let reach = P::LONGEST_DAMAGE_RANGE_METERS;
        for pattern in P::EVERY {
            let Some(standoff) = pattern.standoff() else {
                continue;
            };
            assert!(
                pattern.fires_past_its_lines(),
                "{} fires {} meters, no further than the {} meters its lines stand inside their range",
                pattern.name(),
                pattern.max_damage_range(),
                Belt::LINES_INSIDE_RANGE_METERS
            );
            let furthest = Belt::furthest_station_meters(reach, standoff);
            assert!(
                furthest < Belt::ZONE_RADIUS_METERS,
                "{} stands its furthest station {furthest} meters off the body, outside the {} meter zone",
                pattern.name(),
                Belt::ZONE_RADIUS_METERS
            );
        }
    }
}
