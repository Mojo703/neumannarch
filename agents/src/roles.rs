use neumannarch_sim::RowId;
use neumannarch_sim::roster::{Kind, Roster, Row};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Roles {
    pub yards: Vec<RowId>,
    pub masons: Vec<RowId>,
    pub extractors: Vec<RowId>,
    pub stores: Vec<RowId>,
    pub army: Vec<RowId>,
}

impl Roles {
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
            army: ranked(roster, |row| {
                (row.kind() == Kind::Unit && row.is_armed())
                    .then(|| row.dps_through(0.0) / row.cost.total())
            }),
        }
    }
}

fn builds(row: &Row) -> f64 {
    row.builds().sum()
}

fn extracts(row: &Row) -> f64 {
    row.extracts().sum()
}

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
    use neumannarch_sim::roster::{
        CONSTRUCTOR, EXTRACTOR, FRIGATE, LANCER, RAIDER, SHIPYARD, STORAGE,
    };

    #[test]
    fn the_shipped_roster_fills_every_role() {
        let roles = Roles::of(&Roster::shipped());
        assert_eq!(roles.yards, vec![SHIPYARD]);
        assert_eq!(roles.masons, vec![CONSTRUCTOR]);
        assert_eq!(roles.extractors, vec![EXTRACTOR]);
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
}
