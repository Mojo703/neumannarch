use super::row::{Row, Weapon, Weights};
use crate::ids::RowId;
use crate::materials::Materials;
use crate::real::Real;

pub const CONSTRUCTOR: RowId = RowId(0);
pub const EXTRACTOR: RowId = RowId(1);
pub const STORAGE: RowId = RowId(2);
pub const SHIPYARD: RowId = RowId(3);
pub const RAIDER: RowId = RowId(4);
pub const FRIGATE: RowId = RowId(5);
pub const LANCER: RowId = RowId(6);

const STORE: Materials = Materials::new(500.0, 500.0, 500.0);

pub(super) const MOVEMENT_LIMIT_METERS_PER_SECOND_SQUARED: f64 = 4.0;

const HELD: Weights = Weights {
    wander: Real(0.3),
    returning: Real(1.0),
    separation: Real(20.0),
    cohesion: Real(2.0),
    caution: Real(8.0),
    chase: Real(1.0),
};

pub(super) fn rows() -> Vec<Row> {
    vec![
        row(
            "constructor",
            Materials::new(30.0, 10.0, 10.0),
            50.0,
            1.0,
            Weights {
                caution: Real(12.0),
                ..HELD
            },
            vec![Weapon::Build { rate: Real(3.0) }],
        ),
        row(
            "extractor",
            Materials::new(40.0, 0.0, 10.0),
            120.0,
            0.0,
            Weights::STILL,
            vec![Weapon::Extract { rate: Real(2.0) }],
        ),
        Row {
            capacity: STORE,
            ..row(
                "storage",
                Materials::new(30.0, 0.0, 10.0),
                150.0,
                0.0,
                Weights::STILL,
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
                Weights::STILL,
                vec![Weapon::Build { rate: Real(15.0) }],
            )
        },
        row(
            "raider",
            Materials::new(20.0, 20.0, 5.0),
            40.0,
            2.0,
            Weights {
                wander: Real(0.5),
                cohesion: Real(1.5),
                caution: Real(5.0),
                chase: Real(1.2),
                ..HELD
            },
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
                1.25,
                Weights {
                    cohesion: Real(2.5),
                    ..HELD
                },
                vec![Weapon::Damage {
                    range: Real(6.0),
                    rate: Real(2.0),
                    damage: Real(6.0),
                    falloff: Real(0.0),
                }],
            )
        },
        row(
            "lancer",
            Materials::new(40.0, 5.0, 40.0),
            60.0,
            0.75,
            Weights {
                cohesion: Real(3.0),
                caution: Real(12.0),
                chase: Real(0.8),
                ..HELD
            },
            vec![Weapon::Damage {
                range: Real(14.0),
                rate: Real(1.0),
                damage: Real(20.0),
                falloff: Real(0.0),
            }],
        ),
    ]
}

fn row(
    name: &'static str,
    cost: Materials,
    hp: f64,
    manoeuvring: f64,
    steering: Weights,
    weapons: Vec<Weapon>,
) -> Row {
    Row {
        name,
        cost,
        manoeuvring: Real(manoeuvring),
        steering,
        hp: Real(hp),
        plating: Real(0.0),
        capacity: Materials::ZERO,
        weapons,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::roster::{Kind, Roster};

    const NAMED: [(RowId, &str); 7] = [
        (CONSTRUCTOR, "constructor"),
        (EXTRACTOR, "extractor"),
        (STORAGE, "storage"),
        (SHIPYARD, "shipyard"),
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
    fn the_shipped_structures_are_the_rows_with_no_manoeuvring() {
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
    fn a_structure_holds_by_nothing_and_a_unit_by_every_term() {
        for (_, row) in Roster::shipped().iter() {
            match row.kind() {
                Kind::Structure => assert_eq!(row.steering, Weights::STILL, "{}", row.name),
                Kind::Unit => {
                    assert!(row.steering.returning.0 > 0.0, "{}", row.name);
                    assert!(row.steering.wander.0 > 0.0, "{}", row.name);
                    assert!(row.steering.separation.0 > 0.0, "{}", row.name);
                    assert!(row.steering.cohesion.0 > 0.0, "{}", row.name);
                    assert!(row.steering.caution.0 > 0.0, "{}", row.name);
                }
            }
        }
    }
}
