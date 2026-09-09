use neumannarch_sim::roster::{
    CONSTRUCTOR, ENERGY_EXTRACTOR, FRIGATE, LANCER, METALS_EXTRACTOR, RAIDER, SHIPYARD, STORAGE,
    VOLATILES_EXTRACTOR,
};

use neumannarch_sim::roster::Roster;
use neumannarch_sim::{Material, RowId};

use crate::bots::scripted::roles::*;

#[test]
fn the_shipped_roster_fills_every_role() {
    let roles = Roles::of(&Roster::shipped());
    assert_eq!(roles.yards, vec![SHIPYARD]);
    assert_eq!(roles.masons, vec![CONSTRUCTOR]);
    assert_eq!(
        roles.extractors,
        vec![
            (Material::Metals, METALS_EXTRACTOR),
            (Material::Volatiles, VOLATILES_EXTRACTOR),
            (Material::Energy, ENERGY_EXTRACTOR),
        ]
    );
    assert_eq!(roles.stores, vec![STORAGE]);
    assert_eq!(roles.army.len(), 3);
    for row in [RAIDER, FRIGATE, LANCER] {
        assert!(roles.army.contains(&row));
    }
}

#[test]
fn army_is_ranked_by_damage_per_cost() {
    let roster = Roster::shipped();
    let roles = Roles::of(&roster);
    let per_cost = |row: RowId| roster[row].dps_through(0.0) / roster[row].cost.total();
    for pair in roles.army.windows(2) {
        assert!(per_cost(pair[0]) >= per_cost(pair[1]));
    }
}
