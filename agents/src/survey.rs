use std::collections::BTreeMap;

use probe_sim::roster::{Kind, Roster};
use probe_sim::state::view::View;
use probe_sim::{Band, Materials, Place, RockId, RowId};

use crate::memory::Memory;
use crate::roles::Roles;

const ASSUMED_ENEMY_START: f64 = 100.0;

const ASSUMED_ENEMY_GROWTH: f64 = 1.0;

const THREAT_MEMORY: f64 = 60.0;

pub struct Survey<'a> {
    pub view: &'a View,
    pub roster: &'a Roster,
    pub roles: &'a Roles,
    pub mine: BTreeMap<Place, BTreeMap<RowId, u32>>,
    pub held: Vec<RockId>,
    pub occupied: Vec<RockId>,
    pub building: Vec<RockId>,
    pub developed: Vec<RockId>,
    pub home: Option<RockId>,
    pub income: Materials,
    pub army: f64,
    pub enemy: f64,
    pub enemy_rocks: Vec<RockId>,
    pub threats: BTreeMap<RockId, f64>,
}

impl<'a> Survey<'a> {
    pub fn of(view: &'a View, roster: &'a Roster, roles: &'a Roles, memory: &Memory) -> Survey<'a> {
        let mine = holdings(view);
        let of = |kind: fn(&Roster, RowId) -> bool| -> Vec<RockId> {
            let mut rocks: Vec<RockId> = mine
                .iter()
                .filter(|(_, rows)| rows.keys().any(|row| kind(roster, *row)))
                .map(|(place, _)| place.rock)
                .collect();
            rocks.dedup();
            rocks
        };
        let held = of(is_structure);
        let occupied = of(is_anything);
        let building = building(view, roster);
        let mut developed = held.clone();
        developed.extend(building.iter().copied());
        developed.sort_unstable();
        developed.dedup();
        Survey {
            view,
            roster,
            roles,
            home: home(&mine, roster, &building),
            income: income(view, roster, &mine),
            army: army(view, roster),
            enemy: enemy(view, memory),
            enemy_rocks: memory.enemy_rocks(),
            threats: threats(view, memory),
            mine,
            held,
            occupied,
            building,
            developed,
        }
    }

    pub fn count(&self, place: Place, row: RowId) -> u32 {
        self.mine
            .get(&place)
            .and_then(|rows| rows.get(&row))
            .copied()
            .unwrap_or_default()
    }

    pub fn owned(&self, row: RowId) -> u32 {
        self.mine.values().filter_map(|rows| rows.get(&row)).sum()
    }

    pub fn army_at(&self, place: Place) -> f64 {
        self.mine
            .get(&place)
            .into_iter()
            .flatten()
            .filter_map(|(row, count)| self.roster.get(*row).map(|row| (row, *count)))
            .filter(|(row, _)| row.is_armed())
            .map(|(row, count)| row.cost.total() * count as f64)
            .sum()
    }

    pub fn between(&self, from: RockId, to: RockId) -> f64 {
        match (self.view.rock_body(from), self.view.rock_body(to)) {
            (Some(from), Some(to)) => from.pos.distance(to.pos),
            _ => 0.0,
        }
    }

    pub fn inner(rock: RockId) -> Place {
        Place {
            rock,
            band: Band::Inner,
        }
    }

    pub fn outer(rock: RockId) -> Place {
        Place {
            rock,
            band: Band::Outer,
        }
    }
}

fn holdings(view: &View) -> BTreeMap<Place, BTreeMap<RowId, u32>> {
    let mut counted: BTreeMap<Place, BTreeMap<RowId, u32>> = BTreeMap::new();
    for seen in view.seen.iter().filter(|seen| seen.seat == view.seat) {
        if let Some(home) = seen.home {
            *counted
                .entry(home)
                .or_default()
                .entry(seen.row)
                .or_default() += 1;
        }
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

fn building(view: &View, roster: &Roster) -> Vec<RockId> {
    let mut rocks: Vec<RockId> = view
        .seen
        .iter()
        .filter(|seen| seen.seat == view.seat && !seen.flying)
        .filter(|seen| {
            roster
                .get(seen.row)
                .is_some_and(|row| row.builds().sum::<f64>() > 0.0)
        })
        .filter_map(|seen| seen.home)
        .map(|home| home.rock)
        .collect();
    rocks.sort_unstable();
    rocks.dedup();
    rocks
}

fn home(
    mine: &BTreeMap<Place, BTreeMap<RowId, u32>>,
    roster: &Roster,
    building: &[RockId],
) -> Option<RockId> {
    let rate = |rock: RockId| -> f64 {
        mine.iter()
            .filter(|(place, _)| place.rock == rock)
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

fn income(view: &View, roster: &Roster, mine: &BTreeMap<Place, BTreeMap<RowId, u32>>) -> Materials {
    let mut earned = Materials::ZERO;
    for terrain in &view.terrain {
        let rate: f64 = mine
            .iter()
            .filter(|(place, _)| place.rock == terrain.rock)
            .flat_map(|(_, rows)| rows)
            .filter_map(|(row, count)| roster.get(*row).map(|row| (row, *count)))
            .map(|(row, count)| row.extracts().sum::<f64>() * count as f64)
            .sum();
        earned += terrain.caps.min(Materials::new(rate, rate, rate));
    }
    earned
}

fn army(view: &View, roster: &Roster) -> f64 {
    view.seen
        .iter()
        .filter(|seen| seen.seat == view.seat)
        .filter_map(|seen| roster.get(seen.row))
        .filter(|row| row.is_armed())
        .map(|row| row.cost.total())
        .sum()
}

fn enemy(view: &View, memory: &Memory) -> f64 {
    let assumed = ASSUMED_ENEMY_START + ASSUMED_ENEMY_GROWTH * view.tick.seconds();
    memory.enemy_army().max(assumed)
}

fn threats(view: &View, memory: &Memory) -> BTreeMap<RockId, f64> {
    view.terrain
        .iter()
        .map(|terrain| (terrain.rock, memory.watched(terrain.rock)))
        .filter(|(_, watched)| view.tick.seconds() - watched.at.seconds() <= THREAT_MEMORY)
        .filter(|(_, watched)| watched.enemy_army > 0.0)
        .map(|(rock, watched)| (rock, watched.enemy_army))
        .collect()
}
