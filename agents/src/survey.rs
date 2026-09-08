use std::collections::BTreeMap;

use neumannarch_sim::roster::{Kind, Roster, Row};
use neumannarch_sim::state::view::{Present, View};
use neumannarch_sim::{AsteroidId, RowId};

use crate::roles::Roles;

pub struct Survey<'a> {
    pub view: &'a View,
    pub roster: &'a Roster,
    pub roles: &'a Roles,
    pub mine: BTreeMap<AsteroidId, BTreeMap<RowId, u32>>,
}

impl<'a> Survey<'a> {
    pub fn of(view: &'a View, roster: &'a Roster, roles: &'a Roles) -> Survey<'a> {
        let mut mine: BTreeMap<AsteroidId, BTreeMap<RowId, u32>> = BTreeMap::new();
        for present in view.present.iter().filter(|it| it.seat == view.seat) {
            *mine
                .entry(present.home)
                .or_default()
                .entry(present.row)
                .or_default() += 1;
        }
        Survey {
            view,
            roster,
            roles,
            mine,
        }
    }

    pub fn count(&self, asteroid: AsteroidId, row: RowId) -> u32 {
        self.mine
            .get(&asteroid)
            .and_then(|rows| rows.get(&row))
            .copied()
            .unwrap_or_default()
    }

    pub fn owned(&self, row: RowId) -> u32 {
        self.mine.values().filter_map(|rows| rows.get(&row)).sum()
    }

    pub fn between(&self, from: AsteroidId, to: AsteroidId) -> f64 {
        match (self.view.asteroid_body(from), self.view.asteroid_body(to)) {
            (Some(from), Some(to)) => from.pos.distance(to.pos),
            _ => 0.0,
        }
    }

    pub fn is_structure(&self, row: RowId) -> bool {
        self.roster
            .get(row)
            .is_some_and(|row| row.kind() == Kind::Structure)
    }

    pub fn held(&self) -> Vec<AsteroidId> {
        self.mine
            .iter()
            .filter(|(_, rows)| rows.keys().any(|row| self.is_structure(*row)))
            .map(|(asteroid, _)| *asteroid)
            .collect()
    }

    pub fn occupied(&self) -> Vec<AsteroidId> {
        self.mine.keys().copied().collect()
    }

    pub fn building(&self) -> Vec<AsteroidId> {
        let mut asteroids: Vec<AsteroidId> = self
            .view
            .present
            .iter()
            .filter(|mine| mine.seat == self.view.seat && mine.at.standing().is_some())
            .filter(|mine| {
                self.roster
                    .get(mine.row)
                    .is_some_and(|row| row.builds().sum::<f64>() > 0.0)
            })
            .map(|mine| mine.home)
            .collect();
        asteroids.sort_unstable();
        asteroids.dedup();
        asteroids
    }

    pub fn developed(&self) -> Vec<AsteroidId> {
        let mut asteroids = self.held();
        asteroids.extend(self.building());
        asteroids.sort_unstable();
        asteroids.dedup();
        asteroids
    }

    pub fn home(&self) -> Option<AsteroidId> {
        let rate = |asteroid: AsteroidId| -> f64 {
            self.mine
                .get(&asteroid)
                .into_iter()
                .flatten()
                .filter_map(|(row, count)| self.roster.get(*row).zip(Some(*count)))
                .map(|(row, count)| row.builds().sum::<f64>() * f64::from(count))
                .sum()
        };
        self.building()
            .into_iter()
            .max_by(|a, b| rate(*a).total_cmp(&rate(*b)).then(b.cmp(a)))
    }

    pub fn army(&self) -> f64 {
        self.view
            .present
            .iter()
            .filter(|present| present.seat == self.view.seat)
            .filter_map(|present| self.roster.get(present.row))
            .filter(|row| row.is_armed())
            .map(|row| row.cost.total())
            .sum()
    }

    pub fn enemy(&self) -> f64 {
        self.threats().values().sum()
    }

    pub fn enemy_asteroids(&self) -> Vec<AsteroidId> {
        let mut asteroids: Vec<AsteroidId> = self
            .enemies()
            .filter(|(_, row)| row.kind() == Kind::Structure)
            .map(|(present, _)| present.home)
            .collect();
        asteroids.sort_unstable();
        asteroids.dedup();
        asteroids
    }

    pub fn taken(&self) -> Vec<AsteroidId> {
        let mut asteroids: Vec<AsteroidId> = self
            .view
            .present
            .iter()
            .map(|present| present.home)
            .collect();
        asteroids.sort_unstable();
        asteroids.dedup();
        asteroids
    }

    pub fn enemy_plating(&self) -> f64 {
        self.worst(|row| row.plating.0)
    }

    pub fn enemy_range(&self) -> f64 {
        self.worst(Row::max_damage_range)
    }

    pub fn threats(&self) -> BTreeMap<AsteroidId, f64> {
        let mut threats: BTreeMap<AsteroidId, f64> = BTreeMap::new();
        for (present, row) in self.enemies().filter(|(_, row)| row.is_armed()) {
            *threats.entry(present.home).or_default() += row.cost.total();
        }
        threats
    }

    fn enemies(&self) -> impl Iterator<Item = (&'a Present, &'a Row)> {
        let (view, roster) = (self.view, self.roster);
        view.present
            .iter()
            .filter(move |present| view.is_enemy(present.seat))
            .filter_map(move |present| roster.get(present.row).map(|row| (present, row)))
    }

    fn worst(&self, of: impl Fn(&Row) -> f64) -> f64 {
        self.enemies().map(|(_, row)| of(row)).fold(0.0, f64::max)
    }
}
