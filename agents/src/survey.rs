//! One decision's reading of the view: what the agent holds, what it earns,
//! and what it believes the enemy has.

use std::collections::BTreeMap;

use probe_sim::roster::{Kind, Roster};
use probe_sim::state::view::View;
use probe_sim::{Band, Materials, Place, RockId, RowId};

use crate::memory::Memory;
use crate::roles::Roles;

/// The army value the agent credits an enemy it has not scouted, in cost
/// units. A hypothesis.
const ASSUMED_ENEMY_START: f64 = 100.0;

/// How fast the agent assumes an unscouted enemy grows, in cost units per
/// second of the match. A hypothesis.
const ASSUMED_ENEMY_GROWTH: f64 = 1.0;

/// How long an observation counts as a live threat, in seconds. A
/// hypothesis.
const THREAT_MEMORY: f64 = 60.0;

/// What the agent reads out of one view, with its memory folded in.
pub struct Survey<'a> {
    pub view: &'a View,
    pub roster: &'a Roster,
    pub roles: &'a Roles,
    /// Its own entities per place and row, counting those flying in.
    pub mine: BTreeMap<Place, BTreeMap<RowId, u32>>,
    /// Rocks where it has a structure, in id order: what the score counts.
    pub held: Vec<RockId>,
    /// Rocks where it has anything at all, in id order.
    pub occupied: Vec<RockId>,
    /// Rocks where it has a builder, in id order: where a frame can be
    /// filled.
    pub building: Vec<RockId>,
    /// Rocks it has developed, in id order: those it holds and those a
    /// builder of its own stands at, which are the only ones a frame
    /// opened at is ever built.
    pub developed: Vec<RockId>,
    /// The rock it builds fastest at, or `None` before its first placement.
    pub home: Option<RockId>,
    /// What its extractors pull, in units per second of each material.
    pub income: Materials,
    /// The cost total of its armed units, in cost units.
    pub army: f64,
    /// The enemy army value it plays against, in cost units: what it
    /// remembers, or what it assumes of an enemy it has not found.
    pub enemy: f64,
    /// Rocks it believes an enemy holds, in id order.
    pub enemy_rocks: Vec<RockId>,
    /// The enemy army value at each rock it has watched lately, in cost
    /// units.
    pub threats: BTreeMap<RockId, f64>,
}

impl<'a> Survey<'a> {
    /// What `view` says, read through `memory`.
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

    /// How many of `row` it has at `place`, counting those flying in.
    pub fn count(&self, place: Place, row: RowId) -> u32 {
        self.mine
            .get(&place)
            .and_then(|rows| rows.get(&row))
            .copied()
            .unwrap_or_default()
    }

    /// How many of `row` it has anywhere, counting those in flight.
    pub fn owned(&self, row: RowId) -> u32 {
        self.mine.values().filter_map(|rows| rows.get(&row)).sum()
    }

    /// The cost total of its armed units at `place`, in cost units.
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

    /// How far apart two rocks are now, in meters; zero when the map lacks
    /// either.
    pub fn between(&self, from: RockId, to: RockId) -> f64 {
        match (self.view.rock_body(from), self.view.rock_body(to)) {
            (Some(from), Some(to)) => from.pos.distance(to.pos),
            _ => 0.0,
        }
    }

    /// The inner band of `rock`, where every structure stands.
    pub fn inner(rock: RockId) -> Place {
        Place {
            rock,
            band: Band::Inner,
        }
    }

    /// The outer band of `rock`, where a force stages.
    pub fn outer(rock: RockId) -> Place {
        Place {
            rock,
            band: Band::Outer,
        }
    }
}

/// Every own entity counted by place and row, places in key order.
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

/// Every rock where the agent has a builder that has arrived, in id order:
/// a frame anywhere else has nobody to fill it.
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

/// The rock the agent builds fastest at, ties by lowest id.
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

/// What the agent's extractors pull, in units per second of each material:
/// its own rate at every rock, cut by that rock's caps.
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

/// The cost total of the agent's armed units, in cost units.
fn army(view: &View, roster: &Roster) -> f64 {
    view.seen
        .iter()
        .filter(|seen| seen.seat == view.seat)
        .filter_map(|seen| roster.get(seen.row))
        .filter(|row| row.is_armed())
        .map(|row| row.cost.total())
        .sum()
}

/// The enemy army value to play against: what memory holds, or what an
/// unscouted enemy is assumed to have grown to by now.
fn enemy(view: &View, memory: &Memory) -> f64 {
    let assumed = ASSUMED_ENEMY_START + ASSUMED_ENEMY_GROWTH * view.tick.seconds();
    memory.enemy_army().max(assumed)
}

/// The enemy army value at each rock watched inside [`THREAT_MEMORY`].
fn threats(view: &View, memory: &Memory) -> BTreeMap<RockId, f64> {
    view.terrain
        .iter()
        .map(|terrain| (terrain.rock, memory.watched(terrain.rock)))
        .filter(|(_, watched)| view.tick.seconds() - watched.at.seconds() <= THREAT_MEMORY)
        .filter(|(_, watched)| watched.enemy_army > 0.0)
        .map(|(rock, watched)| (rock, watched.enemy_army))
        .collect()
}
