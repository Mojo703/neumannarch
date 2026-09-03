//! The eight shipped rows.

use super::row::{Row, Weapon};
use crate::ids::RowId;
use crate::materials::Materials;
use crate::real::Real;

pub const CONSTRUCTOR: RowId = RowId(0);
pub const EXTRACTOR: RowId = RowId(1);
pub const STORAGE: RowId = RowId(2);
pub const SHIPYARD: RowId = RowId(3);
pub const SCOUT: RowId = RowId(4);
pub const RAIDER: RowId = RowId(5);
pub const FRIGATE: RowId = RowId(6);
pub const LANCER: RowId = RowId(7);

/// The stockpile capacity a storage-class row contributes, per material.
const STORE: Materials = Materials::new(500.0, 500.0, 500.0);

/// Every shipped row's manoeuvring limit as a fraction of its movement
/// limit. One quarter throughout, a hypothesis the harness confirms or
/// kills.
const MANEUVER_SHARE: f64 = 0.25;

/// The shipped rows, in the order of the id constants.
pub(super) fn rows() -> Vec<Row> {
    vec![
        row(
            "constructor",
            Materials::new(30.0, 10.0, 10.0),
            50.0,
            4.0,
            6.0,
            vec![Weapon::Build { rate: Real(3.0) }],
        ),
        row(
            "extractor",
            Materials::new(40.0, 0.0, 10.0),
            120.0,
            0.0,
            3.0,
            vec![Weapon::Extract { rate: Real(2.0) }],
        ),
        Row {
            capacity: STORE,
            ..row(
                "storage",
                Materials::new(30.0, 0.0, 10.0),
                150.0,
                0.0,
                2.0,
                vec![],
            )
        },
        Row {
            capacity: STORE,
            ..row(
                "shipyard",
                Materials::new(100.0, 0.0, 40.0),
                300.0,
                0.0,
                6.0,
                vec![Weapon::Build { rate: Real(15.0) }],
            )
        },
        Row {
            radar: Real(50.0),
            ..row(
                "scout",
                Materials::new(5.0, 10.0, 0.0),
                15.0,
                10.0,
                20.0,
                vec![],
            )
        },
        row(
            "raider",
            Materials::new(20.0, 20.0, 5.0),
            40.0,
            8.0,
            12.0,
            vec![Weapon::Damage {
                range: Real(3.0),
                rate: Real(4.0),
                damage: Real(3.0),
                falloff: Real(0.5),
            }],
        ),
        Row {
            plating: Real(1.0),
            ..row(
                "frigate",
                Materials::new(80.0, 10.0, 30.0),
                150.0,
                2.0,
                8.0,
                vec![Weapon::Damage {
                    range: Real(6.0),
                    rate: Real(2.0),
                    damage: Real(6.0),
                    falloff: Real(0.0),
                }],
            )
        },
        Row {
            radar: Real(10.0),
            ..row(
                "lancer",
                Materials::new(40.0, 5.0, 40.0),
                60.0,
                3.0,
                5.0,
                vec![Weapon::Damage {
                    range: Real(14.0),
                    rate: Real(1.0),
                    damage: Real(20.0),
                    falloff: Real(0.0),
                }],
            )
        },
    ]
}

/// A row with the table's defaults: mass equals the metals cost, the
/// manoeuvring limit is `MANEUVER_SHARE` of `accel`, radar is twice
/// `sight`, no plating, no capacity.
fn row(
    name: &'static str,
    cost: Materials,
    hp: f64,
    accel: f64,
    sight: f64,
    weapons: Vec<Weapon>,
) -> Row {
    Row {
        name,
        cost,
        mass: Real(cost.metals),
        accel: Real(accel),
        maneuver: Real(accel * MANEUVER_SHARE),
        hp: Real(hp),
        plating: Real(0.0),
        sight: Real(sight),
        radar: Real(sight * 2.0),
        capacity: Materials::ZERO,
        weapons,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::roster::{Kind, Roster};

    const NAMED: [(RowId, &str); 8] = [
        (CONSTRUCTOR, "constructor"),
        (EXTRACTOR, "extractor"),
        (STORAGE, "storage"),
        (SHIPYARD, "shipyard"),
        (SCOUT, "scout"),
        (RAIDER, "raider"),
        (FRIGATE, "frigate"),
        (LANCER, "lancer"),
    ];

    #[test]
    fn each_constant_indexes_its_named_row() {
        let roster = Roster::shipped();
        assert_eq!(roster.len(), NAMED.len());
        for (id, name) in NAMED {
            assert_eq!(roster[id].name, name);
        }
    }

    #[test]
    fn names_are_distinct() {
        let mut names: Vec<_> = rows().iter().map(|row| row.name).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), rows().len());
    }

    #[test]
    fn structures_are_the_rows_that_cannot_accelerate() {
        for (id, row) in Roster::shipped().iter() {
            let expected = if [EXTRACTOR, STORAGE, SHIPYARD].contains(&id) {
                Kind::Structure
            } else {
                Kind::Unit
            };
            assert_eq!(row.kind(), expected, "{}", row.name);
        }
    }

    #[test]
    fn every_row_costs_something() {
        for row in rows() {
            assert!(row.cost.total() > 0.0, "{}", row.name);
        }
    }

    #[test]
    fn radar_is_twice_sight_unless_the_table_states_it() {
        let roster = Roster::shipped();
        assert_eq!(roster[SCOUT].radar, Real(50.0));
        assert_eq!(roster[LANCER].radar, Real(10.0));
        for (id, row) in roster
            .iter()
            .filter(|(id, _)| ![SCOUT, LANCER].contains(id))
        {
            assert_eq!(row.radar.0, row.sight.0 * 2.0, "{id:?}");
        }
    }

    #[test]
    fn every_row_manoeuvres_at_a_quarter_of_its_movement_limit() {
        for row in rows() {
            assert_eq!(row.maneuver.0, row.accel.0 * MANEUVER_SHARE, "{}", row.name);
        }
    }

    #[test]
    fn mass_equals_the_metals_cost() {
        for row in rows() {
            assert_eq!(row.mass.0, row.cost.metals, "{}", row.name);
        }
    }
}
