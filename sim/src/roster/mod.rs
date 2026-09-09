use core::ops::Index;

pub use glyph::{Frame, Glyph, Role, Tier};
pub use row::{Kind, Row, Weapon, Weights};
pub use shipped::{
    CONSTRUCTOR, ENERGY_EXTRACTOR, FRIGATE, LANCER, METALS_EXTRACTOR, RAIDER, SHIPYARD, STORAGE,
    VOLATILES_EXTRACTOR,
};

use crate::belt::Belt;
use crate::ids::RowId;
use crate::real::Real;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Roster {
    movement_limit: Real,
    rows: Vec<Row>,
}

impl Roster {
    pub fn shipped() -> Roster {
        Roster {
            movement_limit: Real(shipped::MOVEMENT_LIMIT_METERS_PER_SECOND_SQUARED),
            rows: shipped::rows(),
        }
        .checked()
    }

    pub fn units_by(self, adjust: impl Fn(Row) -> Row) -> Roster {
        Roster {
            rows: self
                .rows
                .into_iter()
                .map(|row| match row.kind() {
                    Kind::Structure => row,
                    Kind::Unit => adjust(row),
                })
                .collect(),
            ..self
        }
        .checked()
    }

    fn checked(self) -> Roster {
        self.check();
        self
    }

    fn check(&self) {
        let reach = self.longest_damage_range();
        for row in &self.rows {
            let Some(standoff) = row.standoff() else {
                continue;
            };
            assert!(
                row.fires_past_its_lines(),
                "{} fires {} meters, no further than the {} meters its lines stand inside their range",
                row.name,
                row.max_damage_range(),
                Belt::LINES_INSIDE_RANGE_METERS
            );
            let furthest = Belt::furthest_station_meters(reach, standoff);
            assert!(
                furthest < Belt::ZONE_RADIUS_METERS,
                "{} stands its furthest station {furthest} meters off the body, outside the {} meter zone",
                row.name,
                Belt::ZONE_RADIUS_METERS
            );
        }
    }

    pub(crate) fn movement_limit(&self) -> Real {
        self.movement_limit
    }

    pub(crate) fn longest_damage_range(&self) -> f64 {
        self.rows
            .iter()
            .map(Row::max_damage_range)
            .fold(0.0, f64::max)
    }

    #[cfg(test)]
    pub(crate) fn add(&mut self, row: Row) -> RowId {
        let id = RowId(u16::try_from(self.rows.len()).expect("a roster holds at most 65536 rows"));
        self.rows.push(row);
        self.check();
        id
    }

    pub fn get(&self, id: RowId) -> Option<&Row> {
        self.rows.get(usize::from(id.0))
    }

    pub fn iter(&self) -> impl Iterator<Item = (RowId, &Row)> {
        (0..=u16::MAX).map(RowId).zip(&self.rows)
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.rows.len()
    }

    #[cfg(test)]
    pub(crate) fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
}

impl Index<RowId> for Roster {
    type Output = Row;

    fn index(&self, id: RowId) -> &Row {
        &self.rows[usize::from(id.0)]
    }
}

mod glyph;
mod row;
mod shipped;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_returns_the_id_that_reads_the_row_back() {
        let mut roster = Roster::shipped();
        let variant = Row {
            name: "variant",
            ..roster[LANCER].clone()
        };
        let id = roster.add(variant.clone());
        assert_eq!(roster[id], variant);
        assert_eq!(roster.get(id), Some(&variant));
        assert_eq!(roster.iter().last(), Some((id, &variant)));
        assert_eq!(roster.len(), Roster::shipped().len() + 1);
    }

    #[test]
    fn units_by_adjusts_every_unit_row_and_leaves_the_structures_alone() {
        let shipped = Roster::shipped();
        let frailer = shipped.clone().units_by(|row| Row {
            hp: Real(row.hp.0 * 0.5),
            ..row
        });
        assert_eq!(frailer[RAIDER].hp, Real(shipped[RAIDER].hp.0 * 0.5));
        assert_eq!(frailer[RAIDER].manoeuvring, shipped[RAIDER].manoeuvring);
        assert_eq!(frailer[SHIPYARD].hp, shipped[SHIPYARD].hp);
    }

    #[test]
    fn iter_walks_ids_in_order_from_zero() {
        let roster = Roster::shipped();
        let ids: Vec<_> = roster.iter().map(|(id, _)| id).collect();
        let all = (0..roster.len() as u16).map(RowId);
        assert_eq!(ids, all.collect::<Vec<_>>());
        for (id, row) in roster.iter() {
            assert_eq!(&roster[id], row);
        }
    }

    fn reaching(roster: &Roster, meters: f64) -> Row {
        Row {
            name: "variant",
            weapons: vec![Weapon::Damage {
                range: Real(meters),
                rate: Real(1.0),
                damage: Real(1.0),
                falloff: Real(0.0),
            }],
            ..roster[RAIDER].clone()
        }
    }

    #[test]
    #[should_panic(expected = "no further than")]
    fn a_row_that_fires_no_further_than_its_lines_stand_inside_their_range_is_refused() {
        let mut roster = Roster::shipped();
        let short = reaching(&roster, Belt::LINES_INSIDE_RANGE_METERS);

        roster.add(short);
    }

    #[test]
    #[should_panic(expected = "outside the")]
    fn a_row_whose_stations_would_stand_outside_the_zone_is_refused() {
        let mut roster = Roster::shipped();
        let far = reaching(&roster, 4.0 * Belt::ZONE_RADIUS_METERS);

        roster.add(far);
    }

    #[test]
    fn an_unknown_id_reads_nothing() {
        let roster = Roster::shipped();
        assert!(!roster.is_empty());
        let past = u16::try_from(roster.len()).expect("a roster of at most 65536 rows");
        assert_eq!(roster.get(RowId(past)), None);
        assert_eq!(roster.get(RowId(u16::MAX)), None);
    }
}
