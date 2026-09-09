use neumannarch_sim::roster::{Kind, Roster, Row};
use neumannarch_sim::{Material, RowId};

use super::ranking::Ranking;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Roles {
    pub yards: Vec<RowId>,
    pub masons: Vec<RowId>,
    pub extractors: Vec<(Material, RowId)>,
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
            extractors: Material::EVERY
                .into_iter()
                .filter_map(|material| best(roster, material).map(|row| (material, row)))
                .collect(),
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

    pub fn yields_on_completion(&self) -> Vec<RowId> {
        let mut rows: Vec<RowId> = self
            .yards
            .iter()
            .chain(&self.masons)
            .chain(&self.stores)
            .copied()
            .chain(self.extractors.iter().map(|(_, row)| *row))
            .collect();
        rows.sort_unstable();
        rows.dedup();
        rows
    }
}

fn builds(row: &Row) -> f64 {
    row.builds().sum()
}

fn best(roster: &Roster, material: Material) -> Option<RowId> {
    let pulls = |row: &Row| {
        let rate = row.extracts_of(material);
        (rate > 0.0).then_some(rate)
    };
    ranked(roster, pulls).into_iter().next()
}

fn ranked(roster: &Roster, score: impl Fn(&Row) -> Option<f64>) -> Vec<RowId> {
    Ranking::by(roster.iter().map(|(id, _)| id), |id| {
        roster.get(id).and_then(&score)
    })
    .order()
}
