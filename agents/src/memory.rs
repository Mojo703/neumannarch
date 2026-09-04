use std::collections::BTreeMap;

use probe_sim::roster::MassClass;
use probe_sim::roster::{Kind, Roster};
use probe_sim::state::view::View;
use probe_sim::{RockId, Tick, Vec3};

const CLAIM_PATIENCE: f64 = 90.0;

const CLAIM_BAR: f64 = 120.0;

const NEAR_A_ROCK: f64 = 60.0;

const BLIP_VALUE: f64 = 40.0;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Watched {
    pub at: Tick,
    pub enemy_army: f64,
    pub enemy_holds: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Memory {
    watched: BTreeMap<RockId, Watched>,
    claims: BTreeMap<RockId, Tick>,
    barred: BTreeMap<RockId, Tick>,
    pub scouting: Vec<RockId>,
    pub committed: Option<RockId>,
    pub enemy_plating: f64,
    pub enemy_range: f64,
}

impl Memory {
    pub fn observe(&mut self, view: &View, roster: &Roster) {
        for rock in view.terrain.iter().map(|terrain| terrain.rock) {
            if let Some(watched) = watch(view, roster, rock) {
                self.watched.insert(rock, watched);
            }
        }
        self.learn(view, roster);
        self.sweep(view, roster);
    }

    pub fn watched(&self, rock: RockId) -> Watched {
        self.watched.get(&rock).copied().unwrap_or_default()
    }

    pub fn enemy_rocks(&self) -> Vec<RockId> {
        self.watched
            .iter()
            .filter(|(_, watched)| watched.enemy_holds)
            .map(|(rock, _)| *rock)
            .collect()
    }

    pub fn enemy_army(&self) -> f64 {
        self.watched
            .values()
            .map(|watched| watched.enemy_army)
            .sum()
    }

    pub fn claimed(&self, rock: RockId) -> bool {
        self.claims.contains_key(&rock)
    }

    pub fn claims(&self) -> usize {
        self.claims.len()
    }

    pub fn barred(&self, rock: RockId) -> bool {
        self.barred.contains_key(&rock)
    }

    pub fn claim(&mut self, rock: RockId, at: Tick) {
        self.claims.insert(rock, at);
    }

    fn learn(&mut self, view: &View, roster: &Roster) {
        let theirs = view
            .seen
            .iter()
            .filter(|seen| seen.seat != view.seat)
            .filter_map(|seen| roster.get(seen.row));
        for row in theirs {
            self.enemy_plating = self.enemy_plating.max(row.plating.0);
            self.enemy_range = self.enemy_range.max(row.max_damage_range());
        }
    }

    fn sweep(&mut self, view: &View, roster: &Roster) {
        let mut mine = Vec::new();
        let mut held = Vec::new();
        for seen in view.seen.iter().filter(|seen| seen.seat == view.seat) {
            let Some(home) = seen.home else {
                continue;
            };
            mine.push(home);
            if roster
                .get(seen.row)
                .is_some_and(|row| row.kind() == Kind::Structure)
            {
                held.push(home);
            }
        }
        let now = view.tick.seconds();
        let mut lapsed = Vec::new();
        self.claims.retain(|rock, at| {
            if held.contains(rock) {
                return false;
            }
            if mine.contains(rock) {
                return true;
            }
            if now - at.seconds() >= CLAIM_PATIENCE {
                lapsed.push(*rock);
                return false;
            }
            true
        });
        for rock in lapsed {
            self.barred.insert(rock, view.tick);
        }
        self.barred.retain(|_, at| now - at.seconds() < CLAIM_BAR);
    }
}

fn watch(view: &View, roster: &Roster, rock: RockId) -> Option<Watched> {
    let body = view.rock_body(rock)?;
    let rows = |ours: bool| {
        view.seen
            .iter()
            .filter(move |seen| (seen.seat == view.seat) == ours)
            .filter_map(|seen| roster.get(seen.row).map(|row| (seen, row)))
    };
    let watching = rows(true).any(|(seen, row)| seen.body.pos.distance(body.pos) <= row.sight.0);
    if !watching {
        return None;
    }
    let theirs = || rows(false).filter(|(seen, _)| seen.home == Some(rock));
    Some(Watched {
        at: view.tick,
        enemy_army: theirs()
            .filter(|(_, row)| row.is_armed())
            .map(|(_, row)| row.cost.total())
            .sum::<f64>()
            + blips_near(view, body.pos),
        enemy_holds: theirs().any(|(_, row)| row.kind() == Kind::Structure),
    })
}

fn blips_near(view: &View, pos: Vec3) -> f64 {
    view.blips
        .iter()
        .filter(|blip| blip.body.pos.distance(pos) <= NEAR_A_ROCK)
        .map(|blip| BLIP_VALUE * steps(blip.mass))
        .sum()
}

fn steps(mass: MassClass) -> f64 {
    match mass {
        MassClass::Light => 1.0,
        MassClass::Medium => 2.0,
        MassClass::Heavy => 3.0,
    }
}
