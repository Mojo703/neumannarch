//! The roster read as roles, so an agent names no row by id.

use probe_sim::RowId;
use probe_sim::roster::{Kind, Roster, Row};

/// Which rows of a roster do which job, each list in the order an agent
/// prefers: builders and extractors by rate, stores by capacity per cost,
/// scouts by sight, army by damage per cost. Derived from the roster's own
/// fields, so a roster variant needs no edit here.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Roles {
    /// Structures that build, fastest first.
    pub yards: Vec<RowId>,
    /// Units that build, fastest first: what claims a rock.
    pub masons: Vec<RowId>,
    /// Rows that extract, highest rate first.
    pub extractors: Vec<RowId>,
    /// Unarmed rows that carry capacity and do not build, most capacity per
    /// cost first.
    pub stores: Vec<RowId>,
    /// Unarmed units that neither build nor extract, longest sight first.
    pub scouts: Vec<RowId>,
    /// Armed units, most damage per cost first.
    pub army: Vec<RowId>,
}

impl Roles {
    /// The roles `roster`'s rows fill.
    pub fn of(roster: &Roster) -> Roles {
        Roles {
            yards: ranked(roster, |row| {
                (row.kind() == Kind::Structure && builds(row) > 0.0).then(|| builds(row))
            }),
            masons: ranked(roster, |row| {
                (row.kind() == Kind::Unit && builds(row) > 0.0).then(|| builds(row))
            }),
            extractors: ranked(roster, |row| (extracts(row) > 0.0).then(|| extracts(row))),
            stores: ranked(roster, |row| {
                (!row.is_armed() && builds(row) == 0.0 && row.capacity.total() > 0.0)
                    .then(|| row.capacity.total() / row.cost.total())
            }),
            scouts: ranked(roster, |row| {
                (row.kind() == Kind::Unit
                    && !row.is_armed()
                    && builds(row) == 0.0
                    && extracts(row) == 0.0)
                    .then_some(row.sight.0)
            }),
            army: ranked(roster, |row| {
                (row.kind() == Kind::Unit && row.is_armed())
                    .then(|| row.dps_through(0.0) / row.cost.total())
            }),
        }
    }
}

/// A row's combined build rate, in cost units per second.
fn builds(row: &Row) -> f64 {
    row.builds().sum()
}

/// A row's combined extraction rate, in units per second of each material.
fn extracts(row: &Row) -> f64 {
    row.extracts().sum()
}

/// Every row `score` rates, best first, ties by row id.
fn ranked(roster: &Roster, score: impl Fn(&Row) -> Option<f64>) -> Vec<RowId> {
    let mut rated: Vec<(RowId, f64)> = roster
        .iter()
        .filter_map(|(id, row)| score(row).map(|score| (id, score)))
        .collect();
    rated.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    rated.into_iter().map(|(id, _)| id).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use probe_sim::roster::{
        CONSTRUCTOR, EXTRACTOR, FRIGATE, LANCER, RAIDER, SCOUT, SHIPYARD, STORAGE,
    };

    #[test]
    fn the_shipped_roster_fills_every_role() {
        let roles = Roles::of(&Roster::shipped());
        assert_eq!(roles.yards, vec![SHIPYARD]);
        assert_eq!(roles.masons, vec![CONSTRUCTOR]);
        assert_eq!(roles.extractors, vec![EXTRACTOR]);
        assert_eq!(roles.stores, vec![STORAGE]);
        assert_eq!(roles.scouts, vec![SCOUT]);
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
}
