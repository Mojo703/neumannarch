//! The target composition a decision aims at, and the wants that close the
//! gap to it.

use std::collections::BTreeMap;

use probe_sim::roster::Roster;
use probe_sim::state::view::View;
use probe_sim::state::{Command, MAX_WANT};
use probe_sim::{Materials, Place, RockId, RowId};

use crate::dice::Dice;
use crate::memory::Memory;
use crate::personality::Personality;
use crate::survey::Survey;

/// How many seconds of income a plan spends ahead of the stockpile. Build
/// is flow, so a want past this only slows every frame it shares a builder
/// with. A hypothesis.
const BUILD_HORIZON: f64 = 20.0;

/// How full the stockpile must be, as a fraction of capacity, before the
/// plan buys another store. A hypothesis.
const STORE_TRIGGER: f64 = 0.9;

/// The distance a claim's value halves over, in meters: one rock spacing,
/// since a send is slow and a far rock is a rock held late. A hypothesis.
const REACH: f64 = 2_000.0;

/// How long since a rock was watched before a scout is worth sending back
/// to it, in seconds. A hypothesis.
const STALE: f64 = 60.0;

/// How many of the best rocks a claim picks between: two, so an agent's
/// dice vary its expansion without sending a builder past a nearer rock.
/// A hypothesis.
const CHOICES: usize = 2;

/// How many scouts a place gets when a row wanted there shoots farther than
/// it sees. Fire needs sight, so a long row without a spotter never fires.
/// A hypothesis.
const SPOTTERS: u32 = 1;

/// Why a want is in the plan. Wants are issued in this order, so a
/// truncated decision drops the least urgent.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
    /// The reserve placed, which starts the match.
    Opening,
    /// A force answering an enemy seen at one of the agent's rocks.
    Defence,
    /// Extraction, capacity and builders.
    Economy,
    /// A cheap unit walking unseen rocks.
    Scouting,
    /// The army the agent keeps and the rock it takes with it.
    Army,
}

/// One row wanted at one place.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Target {
    priority: Priority,
    count: u32,
}

/// What the agent wants everywhere, this decision: a count per row per
/// place, and what is left of the stockpile to promise.
#[derive(Clone, Debug, PartialEq)]
pub struct Plan {
    targets: BTreeMap<(Place, RowId), Target>,
    /// How many of each row the plan has asked for, over every place.
    promised: BTreeMap<RowId, u32>,
    /// What the plan may still promise, in materials.
    budget: Materials,
}

impl Plan {
    /// The composition `personality` aims at, given what `survey` reads.
    /// Steps `memory` with the claims, scouting and commitment it decides.
    pub fn of(
        survey: &Survey,
        personality: &Personality,
        memory: &mut Memory,
        dice: &mut Dice,
    ) -> Plan {
        let mut plan = Plan {
            targets: BTreeMap::new(),
            promised: BTreeMap::new(),
            budget: survey.view.stockpile.stock() + survey.income * BUILD_HORIZON,
        };
        plan.opening(survey);
        plan.economy(survey, personality, memory, dice);
        plan.scouting(survey, personality, memory);
        plan.army(survey, personality, memory);
        plan
    }

    /// The wants that turn what the view holds into this plan: every place
    /// the plan dropped set to nothing first, then every target that
    /// differs, in priority order.
    pub fn commands(&self, view: &View) -> Vec<Command> {
        let mut standing: BTreeMap<(Place, RowId), u32> = BTreeMap::new();
        for composition in &view.compositions {
            for row in &composition.rows {
                standing.insert((composition.place, row.row), row.want);
            }
        }
        let dropped = standing
            .iter()
            .filter(|(at, want)| **want > 0 && !self.targets.contains_key(at))
            .map(|((place, row), _)| Command::Want {
                place: *place,
                row: *row,
                count: 0,
            });
        let mut changes: Vec<(Priority, Command)> = self
            .targets
            .iter()
            .filter(|(at, target)| standing.get(at).copied().unwrap_or_default() != target.count)
            .map(|((place, row), target)| {
                (
                    target.priority,
                    Command::Want {
                        place: *place,
                        row: *row,
                        count: target.count,
                    },
                )
            })
            .collect();
        changes.sort_by_key(|(priority, _)| *priority);
        dropped
            .chain(changes.into_iter().map(|(_, command)| command))
            .collect()
    }

    /// The reserve placed at the rock the agent opens on, which is the only
    /// thing it can do before it holds anything.
    fn opening(&mut self, survey: &Survey) {
        if !survey.mine.is_empty() {
            return;
        }
        let Some(rock) = opening_rock(survey) else {
            return;
        };
        for (row, count) in &survey.view.reserve {
            self.keep(Priority::Opening, Survey::inner(rock), *row, *count);
        }
    }

    /// Extraction, capacity and builders at every rock the agent has, and
    /// one more rock claimed while it wants more.
    fn economy(
        &mut self,
        survey: &Survey,
        personality: &Personality,
        memory: &mut Memory,
        dice: &mut Dice,
    ) {
        let Some(home) = survey.home else {
            return;
        };
        for rock in yards(survey, personality) {
            for row in first(&survey.roles.yards) {
                self.want(survey, Priority::Economy, Survey::inner(rock), row, 1);
            }
        }
        for rock in survey.developed.iter().copied() {
            let place = Survey::inner(rock);
            self.stand(survey, place);
            for row in first(&survey.roles.extractors) {
                let count = extractors(survey, personality, rock, row);
                self.want(survey, Priority::Economy, place, row, count);
            }
        }
        self.stores(survey, personality, home);
        self.expand(survey, personality, memory, dice, home);
    }

    /// Every structure the agent already has at `place`, so a plan never
    /// scraps what holds a rock.
    fn stand(&mut self, survey: &Survey, place: Place) {
        let standing = survey.mine.get(&place).into_iter().flatten();
        for (row, count) in standing.filter(|(row, _)| is_structure(survey.roster, **row)) {
            self.keep(Priority::Economy, place, *row, *count);
        }
    }

    /// The stores at the home rock: what the personality keeps, and one
    /// more whenever the stockpile is nearly full.
    fn stores(&mut self, survey: &Survey, personality: &Personality, home: RockId) {
        let stockpile = &survey.view.stockpile;
        let full = stockpile.stock().total() >= STORE_TRIGGER * stockpile.capacity().total();
        for row in first(&survey.roles.stores) {
            let place = Survey::inner(home);
            let count = personality.stores + u32::from(full);
            self.want(survey, Priority::Economy, place, row, count);
        }
    }

    /// The rocks claimed: a mobile builder moved to each, an extractor
    /// wanted once it lands, and one more rock claimed while the
    /// personality wants more and the agent has a spare builder.
    fn expand(
        &mut self,
        survey: &Survey,
        personality: &Personality,
        memory: &mut Memory,
        dice: &mut Dice,
        home: RockId,
    ) {
        let Some(mason) = first(&survey.roles.masons).next() else {
            return;
        };
        let place = Survey::inner(home);
        let at_home = survey.count(place, mason);
        let mut spare = at_home.saturating_sub(personality.masons);
        let mut sent = 0;
        let mut short = 0;
        for rock in claimed(survey, memory) {
            let claim = Survey::inner(rock);
            let arriving = survey.count(claim, mason);
            if arriving > 0 {
                self.keep(Priority::Economy, claim, mason, arriving);
            } else if spare > 0 {
                spare -= 1;
                sent += 1;
                self.want(survey, Priority::Economy, claim, mason, 1);
            } else {
                short += 1;
            }
        }
        // A claim is the send, so it waits until a builder is free to fly
        // it: a claim nobody is on the way to lapses and bars the rock.
        if spare > 0
            && wants_another(survey, personality, memory)
            && let Some(rock) = expansion(survey, memory, dice)
        {
            spare -= 1;
            sent += 1;
            memory.claim(rock, survey.view.tick);
            self.want(survey, Priority::Economy, Survey::inner(rock), mason, 1);
        }
        // What flies out this decision must leave home wanting fewer, or
        // nothing there is surplus and the send never happens. The
        // replacement is built the decision after.
        let wanted = short > 0 || wants_another(survey, personality, memory);
        let replacing = u32::from(sent == 0 && spare == 0 && wanted);
        let staying = at_home.saturating_sub(sent).max(personality.masons);
        self.want(survey, Priority::Economy, place, mason, staying + replacing);
    }

    /// One scout walking each rock the agent has watched least recently.
    fn scouting(&mut self, survey: &Survey, personality: &Personality, memory: &mut Memory) {
        let Some(scout) = first(&survey.roles.scouts).next() else {
            return;
        };
        let Some(home) = survey.home else {
            return;
        };
        let owned = survey.owned(scout);
        let walking: Vec<RockId> = memory
            .scouting
            .iter()
            .copied()
            .filter(|rock| memory.watched(*rock).at != survey.view.tick)
            .take(owned.min(personality.scouts) as usize)
            .collect();
        memory.scouting = walking;
        while memory.scouting.len() < owned.min(personality.scouts) as usize {
            let Some(rock) = unwatched(survey, memory) else {
                break;
            };
            memory.scouting.push(rock);
        }
        // Every scout is wanted once: those walking at the rock they walk
        // to, the rest at home, where the builders are.
        for rock in memory.scouting.clone() {
            self.want(survey, Priority::Scouting, Survey::inner(rock), scout, 1);
        }
        let walking = memory.scouting.len() as u32;
        let place = Survey::inner(home);
        let count = personality.scouts.saturating_sub(walking);
        if count > 0 {
            self.want(survey, Priority::Scouting, place, scout, count);
        }
    }

    /// The army: a value target against the enemy the agent estimates, a
    /// share of it posted at every rock of its own under threat, and the
    /// rest staged where it means to fight.
    fn army(&mut self, survey: &Survey, personality: &Personality, memory: &mut Memory) {
        let weights = personality.weights(
            survey.roster,
            survey.roles,
            memory.enemy_plating,
            memory.enemy_range,
        );
        let mut left = (personality.army_ratio * survey.enemy).max(personality.army_floor);
        let mut threatened: Vec<(RockId, f64)> = survey
            .threats
            .iter()
            .filter(|(rock, _)| survey.occupied.contains(rock))
            .map(|(rock, threat)| (*rock, *threat))
            .collect();
        threatened.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
        for (rock, threat) in threatened {
            let value = threat * personality.defence_ratio;
            self.force(
                survey,
                Priority::Defence,
                Survey::inner(rock),
                value,
                &weights,
            );
            left -= value;
        }
        if left <= 0.0 {
            return;
        }
        let Some(place) = staging(survey, personality, memory) else {
            return;
        };
        self.force(survey, Priority::Army, place, left, &weights);
    }

    /// `value` cost units of army at `place`, split by `weights`, with a
    /// spotter wherever a row wanted shoots farther than it sees.
    fn force(
        &mut self,
        survey: &Survey,
        priority: Priority,
        place: Place,
        value: f64,
        weights: &[(RowId, f64)],
    ) {
        let mut blind = false;
        for (row, share) in weights {
            let Some(stats) = survey.roster.get(*row) else {
                continue;
            };
            let count = (value * share / stats.cost.total()) as u32;
            if count == 0 {
                continue;
            }
            self.want(survey, priority, place, *row, count);
            blind |= stats.max_damage_range() > stats.sight.0;
        }
        if blind {
            for row in first(&survey.roles.scouts) {
                self.want(survey, priority, place, row, SPOTTERS);
            }
        }
    }

    /// Wants `count` of `row` at `place`, cut to what the budget covers of
    /// the ones the agent does not already own, and never below a want
    /// already planned there. A row it owns elsewhere costs nothing: the
    /// surplus rule flies it in.
    fn want(&mut self, survey: &Survey, priority: Priority, place: Place, row: RowId, count: u32) {
        let planned = self.planned(place, row);
        let count = count.min(MAX_WANT).max(planned);
        let owned = survey.owned(row);
        let promised = self.promised(row);
        let building = |promised: u32| promised.saturating_sub(owned);
        let extra = building(promised + count - planned) - building(promised);
        let bought = self.affordable(survey.roster, row, extra);
        self.keep(priority, place, row, count - (extra - bought));
    }

    /// Wants `count` of `row` at `place` without charging the budget: what
    /// the agent already has, or takes from its reserve.
    fn keep(&mut self, priority: Priority, place: Place, row: RowId, count: u32) {
        let count = count.min(MAX_WANT);
        let planned = self.planned(place, row);
        if count <= planned {
            if count > 0
                && let Some(target) = self.targets.get_mut(&(place, row))
            {
                target.priority = target.priority.min(priority);
            }
            return;
        }
        *self.promised.entry(row).or_default() += count - planned;
        self.targets.insert(
            (place, row),
            Target {
                priority: self
                    .targets
                    .get(&(place, row))
                    .map_or(priority, |target| target.priority.min(priority)),
                count,
            },
        );
    }

    /// The count of `row` already planned at `place`.
    fn planned(&self, place: Place, row: RowId) -> u32 {
        self.targets
            .get(&(place, row))
            .map_or(0, |target| target.count)
    }

    /// How many of `row` the plan has asked for, over every place.
    fn promised(&self, row: RowId) -> u32 {
        self.promised.get(&row).copied().unwrap_or_default()
    }

    /// How many of `row` the budget covers, spending what it takes.
    fn affordable(&mut self, roster: &Roster, row: RowId, count: u32) -> u32 {
        let Some(cost) = roster.get(row).map(|row| row.cost) else {
            return 0;
        };
        let mut bought = 0;
        while bought < count && covers(self.budget, cost) {
            self.budget -= cost;
            bought += 1;
        }
        bought
    }
}

/// Whether `budget` covers `cost` in every material.
fn covers(budget: Materials, cost: Materials) -> bool {
    let left = budget - cost;
    left.metals >= 0.0 && left.volatiles >= 0.0 && left.energy >= 0.0
}

fn is_structure(roster: &Roster, row: RowId) -> bool {
    roster
        .get(row)
        .is_some_and(|row| row.kind() == probe_sim::roster::Kind::Structure)
}

/// The best row for a role, if the roster has one.
fn first(rows: &[RowId]) -> impl Iterator<Item = RowId> + '_ {
    rows.iter().copied().take(1)
}

/// The rock the agent opens on: the richest, offset by its seat, so two
/// agents alike do not open on the same rock.
fn opening_rock(survey: &Survey) -> Option<RockId> {
    let mut rocks: Vec<(RockId, f64)> = survey
        .view
        .terrain
        .iter()
        .map(|terrain| (terrain.rock, terrain.caps.total()))
        .collect();
    rocks.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    let at = usize::from(survey.view.seat.0) % rocks.len().max(1);
    rocks.get(at).map(|(rock, _)| *rock)
}

/// The rocks that get a fast builder: the home rock first, then the rest in
/// id order, as many as the personality pays for.
fn yards(survey: &Survey, personality: &Personality) -> Vec<RockId> {
    let mut rocks: Vec<RockId> = survey.home.into_iter().collect();
    rocks.extend(
        survey
            .developed
            .iter()
            .copied()
            .filter(|rock| Some(*rock) != survey.home),
    );
    rocks.truncate(personality.yards);
    rocks
}

/// How many extractors a rock is worth: what the personality wants, cut to
/// what the rock's richest material can feed.
fn extractors(survey: &Survey, personality: &Personality, rock: RockId, row: RowId) -> u32 {
    let Some(caps) = survey.view.terrain_of(rock).map(|terrain| terrain.caps) else {
        return 0;
    };
    let Some(rate) = survey
        .roster
        .get(row)
        .map(|row| row.extracts().sum::<f64>())
    else {
        return 0;
    };
    if rate <= 0.0 {
        return 0;
    }
    let richest = caps.metals.max(caps.volatiles).max(caps.energy);
    let saturating = libm::ceil(richest / rate) as u32;
    personality.extractors_per_rock.min(saturating)
}

/// Every rock the agent has claimed and not yet built on, in id order.
fn claimed(survey: &Survey, memory: &Memory) -> Vec<RockId> {
    survey
        .view
        .terrain
        .iter()
        .map(|terrain| terrain.rock)
        .filter(|rock| memory.claimed(*rock))
        .collect()
}

/// Whether the agent wants another rock and has room for another claim.
fn wants_another(survey: &Survey, personality: &Personality, memory: &Memory) -> bool {
    survey.held.len() + memory.claims() < personality.rocks && memory.claims() < personality.claims
}

/// The next rock to claim: the richest that is close, that nobody holds and
/// no lapsed claim bars, ties broken by the agent's dice.
fn expansion(survey: &Survey, memory: &Memory, dice: &mut Dice) -> Option<RockId> {
    let from = survey.home?;
    let mut rated: Vec<(RockId, f64)> = survey
        .view
        .terrain
        .iter()
        .map(|terrain| terrain.rock)
        .filter(|rock| !survey.occupied.contains(rock))
        .filter(|rock| !memory.claimed(*rock) && !memory.barred(*rock))
        .filter(|rock| !survey.enemy_rocks.contains(rock))
        .filter_map(|rock| {
            let caps = survey.view.terrain_of(rock)?.caps.total();
            let reach = 1.0 + survey.between(from, rock) / REACH;
            Some((rock, caps / reach))
        })
        .collect();
    rated.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    let close = rated.len().min(CHOICES);
    let at = dice.below(close)?;
    rated.get(at).map(|(rock, _)| *rock)
}

/// Where the army that is not defending goes: the rock it is committed to
/// taking, staged in the outer band until it is strong enough to move in,
/// else the agent's own rock nearest the enemy.
fn staging(survey: &Survey, personality: &Personality, memory: &mut Memory) -> Option<Place> {
    if let Some(rock) = memory.committed {
        let taken = survey.held.contains(&rock) || !survey.enemy_rocks.contains(&rock);
        if taken {
            memory.committed = None;
        } else {
            let threat = survey.threats.get(&rock).copied().unwrap_or_default();
            let staged = survey.army_at(Survey::outer(rock));
            let ready = staged >= personality.commit_ratio * threat;
            return Some(match ready {
                true => Survey::inner(rock),
                false => Survey::outer(rock),
            });
        }
    }
    let target = nearest(survey, &survey.enemy_rocks);
    if let Some(rock) = target
        && survey.army >= personality.attack_ratio * defended(survey, personality, rock)
    {
        memory.committed = Some(rock);
        return Some(Survey::outer(rock));
    }
    let frontier = match target {
        Some(rock) => survey
            .building
            .iter()
            .copied()
            .min_by(|a, b| {
                survey
                    .between(*a, rock)
                    .total_cmp(&survey.between(*b, rock))
            })
            .or(survey.home),
        None => survey.home,
    };
    frontier.map(Survey::inner)
}

/// What the agent believes it must beat at `rock`: the enemy army it has
/// watched there, never read as less than the army it keeps anyway, so it
/// does not walk into a rock it has only glanced at.
fn defended(survey: &Survey, personality: &Personality, rock: RockId) -> f64 {
    survey
        .threats
        .get(&rock)
        .copied()
        .unwrap_or_default()
        .max(personality.army_floor)
}

/// The rock of `rocks` nearest the agent's home.
fn nearest(survey: &Survey, rocks: &[RockId]) -> Option<RockId> {
    let from = survey.home?;
    rocks.iter().copied().min_by(|a, b| {
        survey
            .between(from, *a)
            .total_cmp(&survey.between(from, *b))
    })
}

/// The nearest rock the agent has not watched lately, holds, or is already
/// walking a scout to: a scout walks outward, since a far rock is a scout
/// spent for minutes.
fn unwatched(survey: &Survey, memory: &Memory) -> Option<RockId> {
    let from = survey.home?;
    let now = survey.view.tick.seconds();
    survey
        .view
        .terrain
        .iter()
        .map(|terrain| terrain.rock)
        .filter(|rock| !survey.occupied.contains(rock))
        .filter(|rock| !memory.scouting.contains(rock))
        .filter(|rock| now - memory.watched(*rock).at.seconds() >= STALE)
        .min_by(|a, b| {
            survey
                .between(from, *a)
                .total_cmp(&survey.between(from, *b))
        })
}
