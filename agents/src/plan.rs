use std::collections::BTreeMap;

use probe_sim::roster::Roster;
use probe_sim::state::view::View;
use probe_sim::state::{Command, MAX_WANT};
use probe_sim::{Materials, Place, RockId, RowId};

use crate::dice::Dice;
use crate::memory::Memory;
use crate::personality::Personality;
use crate::survey::Survey;

const BUILD_HORIZON: f64 = 20.0;

const STORE_TRIGGER: f64 = 0.9;

const REACH: f64 = 2_000.0;

const STALE: f64 = 60.0;

const CHOICES: usize = 2;

const SPOTTERS: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
    Opening,
    Defence,
    Economy,
    Scouting,
    Army,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Target {
    priority: Priority,
    count: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Plan {
    targets: BTreeMap<(Place, RowId), Target>,
    promised: BTreeMap<RowId, u32>,
    budget: Materials,
}

impl Plan {
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

    fn stand(&mut self, survey: &Survey, place: Place) {
        let standing = survey.mine.get(&place).into_iter().flatten();
        for (row, count) in standing.filter(|(row, _)| is_structure(survey.roster, **row)) {
            self.keep(Priority::Economy, place, *row, *count);
        }
    }

    fn stores(&mut self, survey: &Survey, personality: &Personality, home: RockId) {
        let stockpile = &survey.view.stockpile;
        let full = stockpile.stock().total() >= STORE_TRIGGER * stockpile.capacity().total();
        for row in first(&survey.roles.stores) {
            let place = Survey::inner(home);
            let count = personality.stores + u32::from(full);
            self.want(survey, Priority::Economy, place, row, count);
        }
    }

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

        if spare > 0
            && wants_another(survey, personality, memory)
            && let Some(rock) = expansion(survey, memory, dice)
        {
            spare -= 1;
            sent += 1;
            memory.claim(rock, survey.view.tick);
            self.want(survey, Priority::Economy, Survey::inner(rock), mason, 1);
        }

        let wanted = short > 0 || wants_another(survey, personality, memory);
        let replacing = u32::from(sent == 0 && spare == 0 && wanted);
        let staying = at_home.saturating_sub(sent).max(personality.masons);
        self.want(survey, Priority::Economy, place, mason, staying + replacing);
    }

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

    fn planned(&self, place: Place, row: RowId) -> u32 {
        self.targets
            .get(&(place, row))
            .map_or(0, |target| target.count)
    }

    fn promised(&self, row: RowId) -> u32 {
        self.promised.get(&row).copied().unwrap_or_default()
    }

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

fn covers(budget: Materials, cost: Materials) -> bool {
    let left = budget - cost;
    left.metals >= 0.0 && left.volatiles >= 0.0 && left.energy >= 0.0
}

fn is_structure(roster: &Roster, row: RowId) -> bool {
    roster
        .get(row)
        .is_some_and(|row| row.kind() == probe_sim::roster::Kind::Structure)
}

fn first(rows: &[RowId]) -> impl Iterator<Item = RowId> + '_ {
    rows.iter().copied().take(1)
}

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

fn claimed(survey: &Survey, memory: &Memory) -> Vec<RockId> {
    survey
        .view
        .terrain
        .iter()
        .map(|terrain| terrain.rock)
        .filter(|rock| memory.claimed(*rock))
        .collect()
}

fn wants_another(survey: &Survey, personality: &Personality, memory: &Memory) -> bool {
    survey.held.len() + memory.claims() < personality.rocks && memory.claims() < personality.claims
}

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

fn defended(survey: &Survey, personality: &Personality, rock: RockId) -> f64 {
    survey
        .threats
        .get(&rock)
        .copied()
        .unwrap_or_default()
        .max(personality.army_floor)
}

fn nearest(survey: &Survey, rocks: &[RockId]) -> Option<RockId> {
    let from = survey.home?;
    rocks.iter().copied().min_by(|a, b| {
        survey
            .between(from, *a)
            .total_cmp(&survey.between(from, *b))
    })
}

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
