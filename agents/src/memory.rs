//! What an agent remembers between decisions, since the view carries no
//! history: when it last watched each rock, what the enemy had there, the
//! rocks it has claimed, and the choices it holds steady.

use std::collections::BTreeMap;

use probe_sim::roster::MassClass;
use probe_sim::roster::{Kind, Roster};
use probe_sim::state::view::View;
use probe_sim::{RockId, Tick, Vec3};

/// How long a claimed rock may go without one of the agent's own entities
/// present before the claim lapses, in seconds. A send the sim could not
/// plan is silent, so this is how an agent learns of one. A hypothesis.
const CLAIM_PATIENCE: f64 = 90.0;

/// How long a rock stays barred after a claim on it lapsed, in seconds. A
/// hypothesis.
const CLAIM_BAR: f64 = 120.0;

/// How close to a rock's body a radar contact counts as being at that rock,
/// in meters: past both band amplitudes, so a force in either band and the
/// space between them counts. A hypothesis.
const NEAR_A_ROCK: f64 = 60.0;

/// What a radar contact is worth as army value, per mass class step, in
/// cost units. Radar gives no row, so a contact counts by its mass alone.
/// A hypothesis.
const BLIP_VALUE: f64 = 40.0;

/// What one rock's last observation said.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Watched {
    /// The tick the agent last had a sensor over the rock.
    pub at: Tick,
    /// The enemy army value there then, in cost units.
    pub enemy_army: f64,
    /// Whether an enemy structure stood there then.
    pub enemy_holds: bool,
}

/// Everything an agent carries from one decision to the next.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Memory {
    watched: BTreeMap<RockId, Watched>,
    /// The tick each rock the agent has sent a builder to was claimed at.
    claims: BTreeMap<RockId, Tick>,
    /// The tick each rock's lapsed claim barred it at.
    barred: BTreeMap<RockId, Tick>,
    /// The rock each scout is walking to, one entry per scout wanted.
    pub scouting: Vec<RockId>,
    /// The rock the army is committed to taking, held until it is taken.
    pub committed: Option<RockId>,
    /// The strongest enemy plating seen, in damage per hit.
    pub enemy_plating: f64,
    /// The longest enemy weapon reach seen, in meters.
    pub enemy_range: f64,
}

impl Memory {
    /// Folds `view` in: what every watched rock holds, what the enemy
    /// fields, the claims that arrived or lapsed, and the bars that expired.
    pub fn observe(&mut self, view: &View, roster: &Roster) {
        for rock in view.terrain.iter().map(|terrain| terrain.rock) {
            if let Some(watched) = watch(view, roster, rock) {
                self.watched.insert(rock, watched);
            }
        }
        self.learn(view, roster);
        self.sweep(view, roster);
    }

    /// What one rock's last observation says; the default before any.
    pub fn watched(&self, rock: RockId) -> Watched {
        self.watched.get(&rock).copied().unwrap_or_default()
    }

    /// Every rock the agent believes an enemy holds, in id order.
    pub fn enemy_rocks(&self) -> Vec<RockId> {
        self.watched
            .iter()
            .filter(|(_, watched)| watched.enemy_holds)
            .map(|(rock, _)| *rock)
            .collect()
    }

    /// The enemy army value remembered over every rock, in cost units.
    pub fn enemy_army(&self) -> f64 {
        self.watched
            .values()
            .map(|watched| watched.enemy_army)
            .sum()
    }

    /// Whether `rock` is claimed and not yet arrived at.
    pub fn claimed(&self, rock: RockId) -> bool {
        self.claims.contains_key(&rock)
    }

    /// How many claims are outstanding.
    pub fn claims(&self) -> usize {
        self.claims.len()
    }

    /// Whether a lapsed claim still bars `rock`.
    pub fn barred(&self, rock: RockId) -> bool {
        self.barred.contains_key(&rock)
    }

    /// Claims `rock` as of `at`.
    pub fn claim(&mut self, rock: RockId, at: Tick) {
        self.claims.insert(rock, at);
    }

    /// The strongest plating and longest reach the enemy has shown.
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

    /// A claim ends when a structure of the agent's stands at the rock, and
    /// lapses into a bar when nothing of its own is even on the way after
    /// [`CLAIM_PATIENCE`], which is how it learns of a send that the sim
    /// could not plan.
    fn sweep(&mut self, view: &View, roster: &Roster) {
        let mut mine = Vec::new();
        let mut held = Vec::new();
        for seen in view.seen.iter().filter(|seen| seen.seat == view.seat) {
            let Some(home) = seen.home else {
                continue;
            };
            mine.push(home.rock);
            if roster
                .get(seen.row)
                .is_some_and(|row| row.kind() == Kind::Structure)
            {
                held.push(home.rock);
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

/// What the agent sees at `rock` now, or `None` when nothing of its own is
/// close enough to watch it.
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
    let theirs = || rows(false).filter(|(seen, _)| seen.home.is_some_and(|at| at.rock == rock));
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

/// The army value the radar contacts around `pos` stand for, in cost units.
fn blips_near(view: &View, pos: Vec3) -> f64 {
    view.blips
        .iter()
        .filter(|blip| blip.body.pos.distance(pos) <= NEAR_A_ROCK)
        .map(|blip| BLIP_VALUE * steps(blip.mass))
        .sum()
}

/// How many mass class steps a contact of `mass` counts as.
fn steps(mass: MassClass) -> f64 {
    match mass {
        MassClass::Light => 1.0,
        MassClass::Medium => 2.0,
        MassClass::Heavy => 3.0,
    }
}
