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
    pub held: Vec<AsteroidId>,
    pub occupied: Vec<AsteroidId>,
    pub building: Vec<AsteroidId>,
    pub developed: Vec<AsteroidId>,
    pub home: Option<AsteroidId>,
    pub army: f64,
    pub enemy: f64,
    pub enemy_asteroids: Vec<AsteroidId>,
    pub enemy_plating: f64,
    pub enemy_range: f64,
    pub threats: BTreeMap<AsteroidId, f64>,
}

impl<'a> Survey<'a> {
    pub fn of(view: &'a View, roster: &'a Roster, roles: &'a Roles) -> Survey<'a> {
        let mine = holdings(view);
        let of = |kind: fn(&Roster, RowId) -> bool| -> Vec<AsteroidId> {
            let mut asteroids: Vec<AsteroidId> = mine
                .iter()
                .filter(|(_, rows)| rows.keys().any(|row| kind(roster, *row)))
                .map(|(asteroid, _)| *asteroid)
                .collect();
            asteroids.dedup();
            asteroids
        };
        let held = of(is_structure);
        let occupied = of(is_anything);
        let building = building(view, roster);
        let mut developed = held.clone();
        developed.extend(building.iter().copied());
        developed.sort_unstable();
        developed.dedup();
        let threats = threats(view, roster);
        Survey {
            view,
            roster,
            roles,
            home: home(&mine, roster, &building),
            army: army(view, roster),
            enemy: threats.values().sum(),
            enemy_asteroids: enemy_asteroids(view, roster),
            enemy_plating: worst(view, roster, |row| row.plating.0),
            enemy_range: worst(view, roster, Row::max_damage_range),
            threats,
            mine,
            held,
            occupied,
            building,
            developed,
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
}

fn holdings(view: &View) -> BTreeMap<AsteroidId, BTreeMap<RowId, u32>> {
    let mut counted: BTreeMap<AsteroidId, BTreeMap<RowId, u32>> = BTreeMap::new();
    for mine in view.present.iter().filter(|it| it.seat == view.seat) {
        *counted
            .entry(mine.home)
            .or_default()
            .entry(mine.row)
            .or_default() += 1;
    }
    counted
}

fn is_structure(roster: &Roster, row: RowId) -> bool {
    roster
        .get(row)
        .is_some_and(|row| row.kind() == Kind::Structure)
}

fn is_anything(_roster: &Roster, _row: RowId) -> bool {
    true
}

fn building(view: &View, roster: &Roster) -> Vec<AsteroidId> {
    let mut asteroids: Vec<AsteroidId> = view
        .present
        .iter()
        .filter(|mine| mine.seat == view.seat && mine.at.standing().is_some())
        .filter(|mine| {
            roster
                .get(mine.row)
                .is_some_and(|row| row.builds().sum::<f64>() > 0.0)
        })
        .map(|mine| mine.home)
        .collect();
    asteroids.sort_unstable();
    asteroids.dedup();
    asteroids
}

fn home(
    mine: &BTreeMap<AsteroidId, BTreeMap<RowId, u32>>,
    roster: &Roster,
    building: &[AsteroidId],
) -> Option<AsteroidId> {
    let rate = |asteroid: AsteroidId| -> f64 {
        mine.iter()
            .filter(|(at, _)| **at == asteroid)
            .flat_map(|(_, rows)| rows)
            .filter_map(|(row, count)| roster.get(*row).map(|row| (row, *count)))
            .map(|(row, count)| row.builds().sum::<f64>() * count as f64)
            .sum()
    };
    building
        .iter()
        .copied()
        .max_by(|a, b| rate(*a).total_cmp(&rate(*b)).then(b.cmp(a)))
}

fn army(view: &View, roster: &Roster) -> f64 {
    view.present
        .iter()
        .filter(|present| present.seat == view.seat)
        .filter_map(|present| roster.get(present.row))
        .filter(|row| row.is_armed())
        .map(|row| row.cost.total())
        .sum()
}

fn enemies<'a>(
    view: &'a View,
    roster: &'a Roster,
) -> impl Iterator<Item = (&'a Present, &'a Row)> + 'a {
    view.present
        .iter()
        .filter(move |present| view.is_enemy(present.seat))
        .filter_map(|present| roster.get(present.row).map(|row| (present, row)))
}

fn enemy_asteroids(view: &View, roster: &Roster) -> Vec<AsteroidId> {
    let mut asteroids: Vec<AsteroidId> = enemies(view, roster)
        .filter(|(_, row)| row.kind() == Kind::Structure)
        .map(|(present, _)| present.home)
        .collect();
    asteroids.sort_unstable();
    asteroids.dedup();
    asteroids
}

fn threats(view: &View, roster: &Roster) -> BTreeMap<AsteroidId, f64> {
    let mut threats: BTreeMap<AsteroidId, f64> = BTreeMap::new();
    for (present, row) in enemies(view, roster).filter(|(_, row)| row.is_armed()) {
        *threats.entry(present.home).or_default() += row.cost.total();
    }
    threats
}

fn worst(view: &View, roster: &Roster, of: impl Fn(&Row) -> f64) -> f64 {
    enemies(view, roster)
        .map(|(_, row)| of(row))
        .fold(0.0, f64::max)
}
