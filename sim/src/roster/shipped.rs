use super::glyph::{Role, Tier};
use super::row::{Row, Weapon, Weights};
use crate::ids::RowId;
use crate::materials::{Material, Materials};
use crate::real::Real;

pub const CONSTRUCTOR: RowId = RowId(0);
pub const METALS_EXTRACTOR: RowId = RowId(1);
pub const VOLATILES_EXTRACTOR: RowId = RowId(2);
pub const ENERGY_EXTRACTOR: RowId = RowId(3);
pub const STORAGE: RowId = RowId(4);
pub const SHIPYARD: RowId = RowId(5);
pub const RAIDER: RowId = RowId(6);
pub const FRIGATE: RowId = RowId(7);
pub const LANCER: RowId = RowId(8);

const STORE: Materials = Materials::new(500.0, 500.0, 500.0);

pub(super) const MOVEMENT_LIMIT_METERS_PER_SECOND_SQUARED: f64 = 80.0;

const HELD: Weights = Weights {
    wander: Real(0.3),
    returning: Real(1.0),
    separation: Real(20.0),
    station: Real(1.0),
};

pub(super) fn rows() -> Vec<Row> {
    vec![
        Row {
            manoeuvring: Real(1.0),
            steering: Weights {
                station: Real(0.0),
                ..HELD
            },
            ..row(
                "constructor",
                Role::Build,
                Tier::ONE,
                Materials::new(30.0, 10.0, 10.0),
                50.0,
                vec![Weapon::Build { rate: Real(3.0) }],
            )
        },
        extractor("metals extractor", Material::Metals),
        extractor("volatiles extractor", Material::Volatiles),
        extractor("energy extractor", Material::Energy),
        Row {
            capacity: STORE,
            ..row(
                "storage",
                Role::Store,
                Tier::ONE,
                Materials::new(30.0, 0.0, 10.0),
                150.0,
                vec![],
            )
        },
        Row {
            capacity: STORE,
            ..row(
                "shipyard",
                Role::Build,
                Tier::ONE,
                Materials::new(100.0, 0.0, 40.0),
                300.0,
                vec![Weapon::Build { rate: Real(15.0) }],
            )
        },
        Row {
            manoeuvring: Real(2.0),
            steering: Weights {
                wander: Real(0.5),
                ..HELD
            },
            ..row(
                "raider",
                Role::ShortFire,
                Tier::ONE,
                Materials::new(20.0, 20.0, 5.0),
                40.0,
                vec![Weapon::Damage {
                    range: Real(3.0),
                    rate: Real(4.0),
                    damage: Real(3.0),
                    falloff: Real(0.5),
                }],
            )
        },
        Row {
            plating: Real(1.0),
            manoeuvring: Real(1.25),
            steering: HELD,
            ..row(
                "frigate",
                Role::ShortFire,
                Tier::TWO,
                Materials::new(80.0, 10.0, 30.0),
                150.0,
                vec![Weapon::Damage {
                    range: Real(6.0),
                    rate: Real(2.0),
                    damage: Real(6.0),
                    falloff: Real(0.0),
                }],
            )
        },
        Row {
            manoeuvring: Real(0.75),
            steering: HELD,
            ..row(
                "lancer",
                Role::LongFire,
                Tier::THREE,
                Materials::new(40.0, 5.0, 40.0),
                60.0,
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

fn extractor(name: &'static str, material: Material) -> Row {
    row(
        name,
        Role::Extract,
        Tier::ONE,
        Materials::new(20.0, 0.0, 5.0),
        120.0,
        vec![Weapon::Extract {
            material,
            rate: Real(2.0),
        }],
    )
}

fn row(
    name: &'static str,
    role: Role,
    tier: Tier,
    cost: Materials,
    hp: f64,
    weapons: Vec<Weapon>,
) -> Row {
    Row {
        name,
        role,
        tier,
        cost,
        manoeuvring: Real(0.0),
        steering: Weights::STILL,
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

    const NAMED: [(RowId, &str); 9] = [
        (CONSTRUCTOR, "constructor"),
        (METALS_EXTRACTOR, "metals extractor"),
        (VOLATILES_EXTRACTOR, "volatiles extractor"),
        (ENERGY_EXTRACTOR, "energy extractor"),
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
            let structures = [
                METALS_EXTRACTOR,
                VOLATILES_EXTRACTOR,
                ENERGY_EXTRACTOR,
                STORAGE,
                SHIPYARD,
            ];
            let expected = if structures.contains(&id) {
                Kind::Structure
            } else {
                Kind::Unit
            };
            assert_eq!(row.kind(), expected, "{}", row.name);
        }
    }

    #[test]
    fn one_shipped_row_extracts_each_material_and_none_extracts_another() {
        let roster = Roster::shipped();
        let extracting = |material| {
            roster
                .iter()
                .filter(|(_, row)| row.extracts_of(material) > 0.0)
                .map(|(id, _)| id)
                .collect::<Vec<RowId>>()
        };
        assert_eq!(extracting(Material::Metals), [METALS_EXTRACTOR]);
        assert_eq!(extracting(Material::Volatiles), [VOLATILES_EXTRACTOR]);
        assert_eq!(extracting(Material::Energy), [ENERGY_EXTRACTOR]);
        for (_, row) in roster.iter() {
            assert!(row.extracts().count() <= 1, "{} pulls two", row.name);
        }
    }

    #[test]
    fn every_row_costs_something() {
        for row in rows() {
            assert!(row.cost.total() > 0.0, "{}", row.name);
        }
    }

    #[test]
    fn a_structure_holds_by_nothing_and_a_unit_by_a_station_only_where_it_takes_one() {
        for (_, row) in Roster::shipped().iter() {
            match row.kind() {
                Kind::Structure => assert_eq!(row.steering, Weights::STILL, "{}", row.name),
                Kind::Unit => {
                    assert!(row.steering.returning.0 > 0.0, "{}", row.name);
                    assert!(row.steering.wander.0 > 0.0, "{}", row.name);
                    assert!(row.steering.separation.0 > 0.0, "{}", row.name);
                    assert_eq!(
                        row.steering.station.0 > 0.0,
                        row.is_armed(),
                        "{} weighs a station it never takes",
                        row.name
                    );
                }
            }
        }
    }

    #[test]
    fn every_shipped_row_that_is_armed_stands_off_the_stage_and_no_other_does() {
        for (_, row) in Roster::shipped().iter() {
            assert_eq!(
                row.standoff().is_some(),
                row.is_armed(),
                "{} stands off the stage without a weapon to reach across it",
                row.name
            );
            assert!(
                row.standoff().is_none_or(|standoff| standoff > 0.0),
                "{} stands off {:?}, on the far side of the stage's centre",
                row.name,
                row.standoff()
            );
        }
    }
}
