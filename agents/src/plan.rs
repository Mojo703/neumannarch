use std::collections::BTreeMap;

use probe_sim::roster::Roster;
use probe_sim::state::view::View;
use probe_sim::state::{Command, MAX_WANT};
use probe_sim::{Materials, RockId, RowId};

use crate::commitments::Commitments;
use crate::dice::Dice;
use crate::personality::Personality;
use crate::survey::Survey;

const BUILD_HORIZON: f64 = 20.0;

const STORE_TRIGGER: f64 = 0.9;

const REACH: f64 = 2_000.0;

const CHOICES: usize = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
    Opening,
    Defence,
    Economy,
    Army,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Target {
    priority: Priority,
    count: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Plan {
    targets: BTreeMap<(RockId, RowId), Target>,
    promised: BTreeMap<RowId, u32>,
    budget: Materials,
}

impl Plan {
    pub fn of(
        survey: &Survey,
        personality: &Personality,
        commitments: &mut Commitments,
        dice: &mut Dice,
    ) -> Plan {
        let mut plan = Plan {
            targets: BTreeMap::new(),
            promised: BTreeMap::new(),
            budget: survey.view.stockpile.stock() + survey.income * BUILD_HORIZON,
        };
        plan.opening(survey);
        plan.economy(survey, personality, commitments, dice);
        plan.army(survey, personality, commitments);
        plan
    }

    pub fn commands(&self, view: &View) -> Vec<Command> {
        let mut standing: BTreeMap<(RockId, RowId), u32> = BTreeMap::new();
        for composition in &view.compositions {
            for row in &composition.rows {
                standing.insert((composition.rock, row.row), row.want);
            }
        }
        let dropped = standing
            .iter()
            .filter(|(at, want)| **want > 0 && !self.targets.contains_key(at))
            .map(|((rock, row), _)| Command::Want {
                rock: *rock,
                row: *row,
                count: 0,
            });
        let mut changes: Vec<(Priority, Command)> = self
            .targets
            .iter()
            .filter(|(at, target)| standing.get(at).copied().unwrap_or_default() != target.count)
            .map(|((rock, row), target)| {
                (
                    target.priority,
                    Command::Want {
                        rock: *rock,
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
            self.keep(Priority::Opening, rock, *row, *count);
        }
    }

    fn economy(
        &mut self,
        survey: &Survey,
        personality: &Personality,
        commitments: &mut Commitments,
        dice: &mut Dice,
    ) {
        let Some(home) = survey.home else {
            return;
        };
        for rock in yards(survey, personality) {
            for row in first(&survey.roles.yards) {
                self.want(survey, Priority::Economy, rock, row, 1);
            }
        }
        for rock in survey.developed.iter().copied() {
            self.stand(survey, rock);
            for row in first(&survey.roles.extractors) {
                let count = extractors(survey, personality, rock, row);
                self.want(survey, Priority::Economy, rock, row, count);
            }
        }
        self.stores(survey, personality, home);
        self.expand(survey, personality, commitments, dice, home);
    }

    fn stand(&mut self, survey: &Survey, rock: RockId) {
        let standing = survey.mine.get(&rock).into_iter().flatten();
        for (row, count) in standing.filter(|(row, _)| is_structure(survey.roster, **row)) {
            self.keep(Priority::Economy, rock, *row, *count);
        }
    }

    fn stores(&mut self, survey: &Survey, personality: &Personality, home: RockId) {
        let stockpile = &survey.view.stockpile;
        let full = stockpile.stock().total() >= STORE_TRIGGER * stockpile.capacity().total();
        for row in first(&survey.roles.stores) {
            let count = personality.stores + u32::from(full);
            self.want(survey, Priority::Economy, home, row, count);
        }
    }

    fn expand(
        &mut self,
        survey: &Survey,
        personality: &Personality,
        commitments: &mut Commitments,
        dice: &mut Dice,
        home: RockId,
    ) {
        let Some(mason) = first(&survey.roles.masons).next() else {
            return;
        };
        let at_home = survey.count(home, mason);
        let mut spare = at_home.saturating_sub(personality.masons);
        let mut sent = 0;
        let mut short = 0;
        for claim in claimed(survey, commitments) {
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
            && wants_another(survey, personality, commitments)
            && let Some(rock) = expansion(survey, commitments, dice)
        {
            spare -= 1;
            sent += 1;
            commitments.claim(rock, survey.view.tick);
            self.want(survey, Priority::Economy, rock, mason, 1);
        }

        let wanted = short > 0 || wants_another(survey, personality, commitments);
        let replacing = u32::from(sent == 0 && spare == 0 && wanted);
        let staying = at_home.saturating_sub(sent).max(personality.masons);
        self.want(survey, Priority::Economy, home, mason, staying + replacing);
    }

    fn army(&mut self, survey: &Survey, personality: &Personality, commitments: &mut Commitments) {
        let weights = personality.weights(
            survey.roster,
            survey.roles,
            survey.enemy_plating,
            survey.enemy_range,
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
            self.force(survey, Priority::Defence, rock, value, &weights);
            left -= value;
        }
        if left <= 0.0 {
            return;
        }
        let Some(rock) = staging(survey, personality, commitments) else {
            return;
        };
        self.force(survey, Priority::Army, rock, left, &weights);
    }

    fn force(
        &mut self,
        survey: &Survey,
        priority: Priority,
        rock: RockId,
        value: f64,
        weights: &[(RowId, f64)],
    ) {
        for (row, share) in weights {
            let Some(stats) = survey.roster.get(*row) else {
                continue;
            };
            let count = (value * share / stats.cost.total()) as u32;
            if count == 0 {
                continue;
            }
            self.want(survey, priority, rock, *row, count);
        }
    }

    fn want(&mut self, survey: &Survey, priority: Priority, rock: RockId, row: RowId, count: u32) {
        let planned = self.planned(rock, row);
        let count = count.min(MAX_WANT).max(planned);
        let owned = survey.owned(row);
        let promised = self.promised(row);
        let building = |promised: u32| promised.saturating_sub(owned);
        let extra = building(promised + count - planned) - building(promised);
        let bought = self.affordable(survey.roster, row, extra);
        self.keep(priority, rock, row, count - (extra - bought));
    }

    fn keep(&mut self, priority: Priority, rock: RockId, row: RowId, count: u32) {
        let count = count.min(MAX_WANT);
        let planned = self.planned(rock, row);
        if count <= planned {
            if count > 0
                && let Some(target) = self.targets.get_mut(&(rock, row))
            {
                target.priority = target.priority.min(priority);
            }
            return;
        }
        *self.promised.entry(row).or_default() += count - planned;
        self.targets.insert(
            (rock, row),
            Target {
                priority: self
                    .targets
                    .get(&(rock, row))
                    .map_or(priority, |target| target.priority.min(priority)),
                count,
            },
        );
    }

    fn planned(&self, rock: RockId, row: RowId) -> u32 {
        self.targets
            .get(&(rock, row))
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

fn claimed(survey: &Survey, commitments: &Commitments) -> Vec<RockId> {
    survey
        .view
        .terrain
        .iter()
        .map(|terrain| terrain.rock)
        .filter(|rock| commitments.claimed(*rock))
        .collect()
}

fn wants_another(survey: &Survey, personality: &Personality, commitments: &Commitments) -> bool {
    survey.held.len() + commitments.claims() < personality.rocks
        && commitments.claims() < personality.claims
}

fn expansion(survey: &Survey, commitments: &Commitments, dice: &mut Dice) -> Option<RockId> {
    let from = survey.home?;
    let mut rated: Vec<(RockId, f64)> = survey
        .view
        .terrain
        .iter()
        .map(|terrain| terrain.rock)
        .filter(|rock| !survey.occupied.contains(rock))
        .filter(|rock| !commitments.claimed(*rock) && !commitments.barred(*rock))
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

fn staging(
    survey: &Survey,
    personality: &Personality,
    commitments: &mut Commitments,
) -> Option<RockId> {
    if let Some(rock) = commitments.committed {
        let taken = survey.held.contains(&rock) || !survey.enemy_rocks.contains(&rock);
        if taken {
            commitments.committed = None;
        } else {
            return Some(rock);
        }
    }
    let target = nearest(survey, &survey.enemy_rocks);
    if let Some(rock) = target
        && survey.army > 0.0
        && survey.army >= personality.attack_ratio * defended(survey, rock)
    {
        commitments.committed = Some(rock);
        return Some(rock);
    }
    match target {
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
    }
}

fn defended(survey: &Survey, rock: RockId) -> f64 {
    survey.threats.get(&rock).copied().unwrap_or_default()
}

fn nearest(survey: &Survey, rocks: &[RockId]) -> Option<RockId> {
    let from = survey.home?;
    rocks.iter().copied().min_by(|a, b| {
        survey
            .between(from, *a)
            .total_cmp(&survey.between(from, *b))
    })
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use probe_sim::belt::Belt;
    use probe_sim::roster::{RAIDER, SHIPYARD};
    use probe_sim::state::view::View;
    use probe_sim::state::{Batch, Issued, Seat, State};
    use probe_sim::step::fire::Shots;
    use probe_sim::{Materials, SeatId, TICKS_PER_SECOND, TeamId, Tick};

    use super::*;
    use crate::personality::Personality;
    use crate::roles::Roles;

    const CLOCK: Tick = Tick(15 * 60 * TICKS_PER_SECOND as u64);

    const ALLY_ROCK: RockId = RockId(4);

    const ENEMY_ROCK: RockId = RockId(9);

    fn sided() -> State {
        let reserve = BTreeMap::from([(SHIPYARD, 1), (RAIDER, 4)]);
        let seat = |team| Seat::new(TeamId(team), Materials::new(1e4, 1e4, 1e4), reserve.clone());
        State::new(
            CLOCK,
            0,
            Belt::GRAVITY,
            Roster::shipped(),
            Belt::fixed(Belt::GRAVITY),
            vec![seat(0), seat(0), seat(1)],
        )
    }

    fn want(seat: u8, seq: u32, rock: RockId, row: RowId, count: u32) -> Issued {
        Issued {
            seat: SeatId(seat),
            seq,
            command: Command::Want { rock, row, count },
        }
    }

    #[test]
    fn an_allys_army_is_no_threat_and_its_rock_is_never_attacked() {
        let mut batch = Batch::new();
        for (seat, rock) in [(1, ALLY_ROCK), (2, ENEMY_ROCK)] {
            for (seq, row) in [SHIPYARD, RAIDER].into_iter().enumerate() {
                let count = if row == RAIDER { 4 } else { 1 };
                assert_eq!(
                    batch.insert(want(seat, seq as u32, rock, row, count)),
                    Ok(())
                );
            }
        }
        let (state, outcome) = sided().step(&batch);
        assert_eq!(outcome.rejected, Vec::new());
        let view = View::of(&state, SeatId(0), &Shots::default());
        assert!(
            view.present.iter().any(|it| it.seat == SeatId(1)),
            "the ally's force is in the view like any other"
        );

        let roster = Roster::shipped();
        let roles = Roles::of(&roster);
        let survey = Survey::of(&view, &roster, &roles);

        assert_eq!(survey.enemy_rocks, vec![ENEMY_ROCK]);
        assert_eq!(survey.threats.get(&ALLY_ROCK), None);
        assert!(survey.threats.contains_key(&ENEMY_ROCK));
        assert_eq!(survey.enemy, state[RAIDER].cost.total() * 4.0);

        let plan = Plan::of(
            &survey,
            &Personality::expand(),
            &mut Commitments::default(),
            &mut Dice::new(0),
        );

        assert!(
            plan.commands(&view)
                .iter()
                .all(|Command::Want { rock, .. }| *rock != ALLY_ROCK),
            "the plan asked for something at the ally's rock"
        );
    }
}
