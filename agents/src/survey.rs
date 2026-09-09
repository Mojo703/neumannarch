use std::collections::BTreeMap;

use neumannarch_sim::roster::{Kind, Roster};
use neumannarch_sim::state::Held;
use neumannarch_sim::state::view::View;
use neumannarch_sim::{AsteroidId, RowId};

use crate::ranking::Ranking;
use crate::roles::Roles;

pub struct Survey<'a> {
    pub view: &'a View,
    pub roster: &'a Roster,
    pub roles: &'a Roles,
    pub mine: BTreeMap<AsteroidId, BTreeMap<RowId, Held>>,
    pub taken: Vec<AsteroidId>,
    pub enemy_asteroids: Vec<AsteroidId>,
    pub threats: BTreeMap<AsteroidId, f64>,
    pub enemy_plating: f64,
    pub enemy_range: f64,
}

impl<'a> Survey<'a> {
    pub fn of(view: &'a View, roster: &'a Roster, roles: &'a Roles) -> Survey<'a> {
        let mut mine: BTreeMap<AsteroidId, BTreeMap<RowId, Held>> = BTreeMap::new();
        let mut taken: Vec<AsteroidId> = Vec::new();
        let mut enemy_asteroids: Vec<AsteroidId> = Vec::new();
        let mut threats: BTreeMap<AsteroidId, f64> = BTreeMap::new();
        let mut enemy_plating = 0.0f64;
        let mut enemy_range = 0.0f64;
        for (post, composition) in &view.compositions {
            let homed: u32 = composition
                .rows
                .values()
                .map(|held| held.present + held.arriving)
                .sum();
            if homed > 0 {
                taken.push(post.asteroid);
            }
            if post.seat == view.seat {
                mine.insert(post.asteroid, composition.rows.clone());
                continue;
            }
            if !view.is_enemy(post.seat) {
                continue;
            }
            for (row, held) in &composition.rows {
                let Some(stats) = roster.get(*row) else {
                    continue;
                };
                let count = held.present + held.arriving;
                if count == 0 {
                    continue;
                }
                enemy_plating = enemy_plating.max(stats.plating.0);
                enemy_range = enemy_range.max(stats.max_damage_range());
                if stats.kind() == Kind::Structure {
                    enemy_asteroids.push(post.asteroid);
                }
                if stats.is_armed() {
                    *threats.entry(post.asteroid).or_default() +=
                        stats.cost.total() * f64::from(count);
                }
            }
        }
        taken.sort_unstable();
        taken.dedup();
        enemy_asteroids.sort_unstable();
        enemy_asteroids.dedup();
        Survey {
            view,
            roster,
            roles,
            mine,
            taken,
            enemy_asteroids,
            threats,
            enemy_plating,
            enemy_range,
        }
    }

    pub fn count(&self, asteroid: AsteroidId, row: RowId) -> u32 {
        self.holding(asteroid, row)
            .map_or(0, |held| held.present + held.arriving)
    }

    pub fn standing(&self, asteroid: AsteroidId, row: RowId) -> u32 {
        self.holding(asteroid, row).map_or(0, |held| held.present)
    }

    pub fn owned(&self, row: RowId) -> u32 {
        self.mine
            .keys()
            .map(|asteroid| self.count(*asteroid, row))
            .sum()
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

    pub fn builds(&self, row: RowId) -> bool {
        self.roster
            .get(row)
            .is_some_and(|row| row.builds().next().is_some())
    }

    pub fn is_armed(&self, row: RowId) -> bool {
        self.roster.get(row).is_some_and(|row| row.is_armed())
    }

    pub fn held(&self) -> Vec<AsteroidId> {
        self.mine
            .keys()
            .copied()
            .filter(|asteroid| {
                self.rows(*asteroid)
                    .any(|(row, held)| held.present > 0 && self.is_structure(*row))
            })
            .collect()
    }

    pub fn builders(&self, asteroid: AsteroidId) -> u32 {
        self.rows(asteroid)
            .filter(|(row, _)| self.builds(**row))
            .map(|(_, held)| held.present)
            .sum()
    }

    pub fn builder_arriving(&self, asteroid: AsteroidId) -> bool {
        self.rows(asteroid)
            .any(|(row, held)| self.builds(*row) && held.arriving > 0)
    }

    pub fn builds_at(&self, asteroid: AsteroidId) -> bool {
        self.builders(asteroid) > 0 || self.builder_arriving(asteroid)
    }

    pub fn building(&self) -> Vec<AsteroidId> {
        self.mine
            .keys()
            .copied()
            .filter(|asteroid| self.builders(*asteroid) > 0)
            .collect()
    }

    pub fn developed(&self) -> Vec<AsteroidId> {
        let mut asteroids = self.held();
        asteroids.extend(self.building());
        asteroids.sort_unstable();
        asteroids.dedup();
        asteroids
    }

    pub fn occupied(&self) -> Vec<AsteroidId> {
        self.mine.keys().copied().collect()
    }

    pub fn home(&self) -> Option<AsteroidId> {
        Ranking::by(self.building(), |asteroid| {
            Some(self.build_rate_at(asteroid))
        })
        .best()
    }

    pub fn build_rate(&self) -> f64 {
        self.mine
            .keys()
            .map(|asteroid| self.build_rate_at(*asteroid))
            .sum()
    }

    pub fn build_rate_at(&self, asteroid: AsteroidId) -> f64 {
        self.rows(asteroid)
            .filter_map(|(row, held)| {
                self.roster
                    .get(*row)
                    .map(|stats| stats.builds().sum::<f64>() * f64::from(held.present))
            })
            .sum()
    }

    pub fn army(&self) -> f64 {
        self.mine
            .keys()
            .map(|asteroid| self.armed_value(*asteroid))
            .sum()
    }

    pub fn armed_value(&self, asteroid: AsteroidId) -> f64 {
        self.rows(asteroid)
            .filter_map(|(row, held)| {
                let stats = self.roster.get(*row).filter(|stats| stats.is_armed())?;
                Some(stats.cost.total() * f64::from(held.present + held.arriving))
            })
            .sum()
    }

    pub fn armed_count(&self, asteroid: AsteroidId) -> u32 {
        self.rows(asteroid)
            .filter(|(row, _)| self.is_armed(**row))
            .map(|(_, held)| held.present + held.arriving)
            .sum()
    }

    pub fn enemy(&self) -> f64 {
        self.threats.values().sum()
    }

    pub fn threat_at(&self, asteroid: AsteroidId) -> f64 {
        self.threats.get(&asteroid).copied().unwrap_or_default()
    }

    pub fn frame_open_at(&self, asteroid: AsteroidId) -> bool {
        self.view
            .plans
            .iter()
            .any(|(posting, plan)| posting.asteroid() == asteroid && plan.building.is_some())
    }

    pub fn wanting_more_at(&self, asteroid: AsteroidId) -> bool {
        self.view
            .plans
            .iter()
            .filter(|(posting, _)| posting.asteroid() == asteroid)
            .any(|(posting, plan)| plan.want > self.standing(asteroid, posting.row()))
    }

    pub fn homed_at(&self, asteroid: AsteroidId) -> u32 {
        self.rows(asteroid)
            .map(|(_, held)| held.present + held.arriving)
            .sum()
    }

    pub fn nearest(&self, from: AsteroidId, among: &[AsteroidId]) -> Option<AsteroidId> {
        Ranking::by(among.iter().copied(), |to| Some(-self.between(from, to))).best()
    }

    pub fn staging(&self) -> Option<AsteroidId> {
        let enemies = &self.enemy_asteroids;
        Ranking::by(self.building(), |asteroid| {
            let to = self.nearest(asteroid, enemies)?;
            Some(-self.between(asteroid, to))
        })
        .best()
        .or_else(|| self.home())
    }

    fn rows(&self, asteroid: AsteroidId) -> impl Iterator<Item = (&RowId, &Held)> {
        self.mine.get(&asteroid).into_iter().flatten()
    }

    fn holding(&self, asteroid: AsteroidId, row: RowId) -> Option<&Held> {
        self.mine.get(&asteroid).and_then(|rows| rows.get(&row))
    }
}
