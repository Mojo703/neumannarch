use std::collections::BTreeMap;

use neumannarch_sim::roster::{Roster, Row};
use neumannarch_sim::state::view::View;
use neumannarch_sim::state::{Command, MAX_WANT};
use neumannarch_sim::{Materials, RockId, RowId};

use crate::commitments::Commitments;
use crate::dice::Dice;
use crate::personality::Personality;
use crate::survey::Survey;

const BUILD_HORIZON: f64 = 20.0;

const STORE_TRIGGER: f64 = 0.9;

const REACH: f64 = 2_000.0;

const CHOICES: usize = 2;

const PAYBACK_HORIZON: f64 = 60.0;

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
            budget: survey.view.stockpile.stock() + survey.view.income * BUILD_HORIZON,
        };
        plan.draft(survey, personality, commitments);
        plan.economy(survey, personality, commitments, dice);
        plan.army(survey, personality, commitments);
        plan
    }

    pub fn commands(&self, view: &View) -> Vec<Command> {
        let mut standing: BTreeMap<(RockId, RowId), u32> = BTreeMap::new();
        for plan in &view.plans {
            standing.insert((plan.rock, plan.row), plan.want);
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

    fn draft(&mut self, survey: &Survey, personality: &Personality, commitments: &mut Commitments) {
        let draft = &survey.view.draft;
        let seat = survey.view.seat;
        for (rock, row) in draft.placements(seat) {
            if survey.roles.masons.contains(&row) {
                commitments.claim(rock, survey.view.time);
            }
        }
        if draft.ended().is_some() && !survey.mine.is_empty() {
            return;
        }
        for (rock, row) in draft.placements(seat) {
            self.keep(Priority::Opening, rock, row, 1);
        }
        let Some(stage) = draft.running().filter(|stage| stage.seat == seat) else {
            return;
        };
        let Some(rock) = self.fittest(survey, personality) else {
            return;
        };
        self.keep(Priority::Opening, rock, stage.row, 1);
    }

    fn wanted_shares(&self, survey: &Survey, personality: &Personality) -> Materials {
        let shares = |of: Materials| of * (1.0 / of.total().max(f64::MIN_POSITIVE));
        let drafted = survey
            .view
            .draft
            .placements(survey.view.seat)
            .filter_map(|(rock, _)| survey.view.terrain_of(rock))
            .fold(Materials::ZERO, |caps, terrain| caps + terrain.caps);
        let held = shares(drafted);
        let mut wanted = Materials::ZERO;
        for (material, share) in shares(self.intended(survey, personality)).amounts() {
            wanted[material] = share * (1.0 - held[material]);
        }
        wanted
    }

    fn fittest(&self, survey: &Survey, personality: &Personality) -> Option<RockId> {
        let wanted = self.wanted_shares(survey, personality);
        let nearest = |from: RockId, enemy: bool| -> Option<f64> {
            survey
                .view
                .draft
                .stages()
                .iter()
                .filter(|stage| match enemy {
                    true => survey.view.is_enemy(stage.seat),
                    false => stage.seat == survey.view.seat,
                })
                .filter_map(|stage| Some(survey.between(from, stage.placed?)))
                .min_by(f64::total_cmp)
        };
        survey
            .view
            .terrain
            .iter()
            .filter(|terrain| survey.view.draft.took(terrain.rock).is_none())
            .map(|terrain| {
                let rock = terrain.rock;
                let fit: f64 = terrain
                    .caps
                    .amounts()
                    .map(|(material, cap)| cap * wanted[material])
                    .sum();
                let away = nearest(rock, true).map_or(1.0, |gap| gap / (gap + REACH));
                let near = nearest(rock, false).unwrap_or_default();
                (rock, fit * away / (1.0 + near / REACH))
            })
            .max_by(|a, b| a.1.total_cmp(&b.1).then(b.0.cmp(&a.0)))
            .map(|(rock, _)| rock)
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
        for rock in survey.developed.iter().copied() {
            self.stand(survey, rock);
        }
        self.expand(survey, personality, commitments, dice, home);
        self.extractors(survey, personality);
        for rock in yards(survey, personality) {
            for row in first(&survey.roles.yards) {
                self.want(survey, Priority::Economy, rock, row, 1);
            }
        }
        self.stores(survey, personality, home);
    }

    fn demand(&self, survey: &Survey, personality: &Personality) -> Materials {
        let mix = self.intended(survey, personality);
        match mix.total() {
            total if total > 0.0 => mix * (self.building_rate(survey) / total),
            _ => Materials::ZERO,
        }
    }

    fn intended(&self, survey: &Survey, personality: &Personality) -> Materials {
        let weights = personality.weights(
            survey.roster,
            survey.roles,
            survey.enemy_plating,
            survey.enemy_range,
        );
        let army = counted(survey, personality.army_value(survey.enemy), &weights);
        let army = army
            .iter()
            .filter_map(|(row, count)| survey.roster.get(*row).zip(Some(*count)));
        self.wanted(survey)
            .chain(army)
            .map(|(row, count)| row.cost * f64::from(count))
            .fold(Materials::ZERO, |mix, cost| mix + cost)
    }

    fn building_rate(&self, survey: &Survey) -> f64 {
        self.wanted(survey)
            .map(|(row, count)| row.builds().sum::<f64>() * f64::from(count))
            .sum()
    }

    fn wanted<'a>(&'a self, survey: &'a Survey) -> impl Iterator<Item = (&'a Row, u32)> {
        self.targets
            .iter()
            .filter_map(|((_, row), target)| survey.roster.get(*row).zip(Some(target.count)))
    }

    fn extractors(&mut self, survey: &Survey, personality: &Personality) {
        let wanted = self.demand(survey, personality);
        let left = survey.view.length.since(survey.view.time).seconds();
        let repaying = left.min(PAYBACK_HORIZON);
        for (material, row) in &survey.roles.extractors {
            let Some(stats) = survey.roster.get(*row) else {
                continue;
            };
            let rate = stats.extracts_of(*material);
            let mut short = (wanted[*material] - survey.view.income[*material]).max(0.0);
            let mut spare: BTreeMap<RockId, f64> = BTreeMap::new();
            let mut counts: BTreeMap<RockId, u32> = BTreeMap::new();
            for rock in survey.developed.iter().copied() {
                let Some(terrain) = survey.view.terrain_of(rock) else {
                    continue;
                };
                let standing = survey.count(rock, *row);
                let mine = (rate * f64::from(standing)).min(terrain.caps[*material]);
                let others = (terrain.pull[*material] - mine).max(0.0);
                spare.insert(rock, terrain.caps[*material] - others - mine);
                counts.insert(rock, standing);
            }
            while short > 0.0 {
                let Some(rock) = widest(&spare) else {
                    break;
                };
                let pulls = rate.min(spare[&rock]);
                if pulls * repaying < stats.cost.total() {
                    break;
                }
                short -= pulls;
                *spare.entry(rock).or_default() -= pulls;
                *counts.entry(rock).or_default() += 1;
            }
            for (rock, count) in counts.into_iter().filter(|(_, count)| *count > 0) {
                self.want(survey, Priority::Economy, rock, *row, count);
            }
        }
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
            commitments.claim(rock, survey.view.time);
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
        let mut left = personality.army_value(survey.enemy);
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
        for (row, count) in counted(survey, value, weights) {
            self.want(survey, priority, rock, row, count);
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
        while bought < count && affords(self.budget, cost) {
            self.budget -= cost;
            bought += 1;
        }
        bought
    }
}

fn counted(survey: &Survey, value: f64, weights: &[(RowId, f64)]) -> Vec<(RowId, u32)> {
    weights
        .iter()
        .filter_map(|(row, share)| {
            let count = (value * share / survey.roster.get(*row)?.cost.total()) as u32;
            (count > 0).then_some((*row, count))
        })
        .collect()
}

fn widest(spare: &BTreeMap<RockId, f64>) -> Option<RockId> {
    spare
        .iter()
        .filter(|(_, room)| **room > 0.0)
        .max_by(|a, b| a.1.total_cmp(b.1).then(b.0.cmp(a.0)))
        .map(|(rock, _)| *rock)
}

fn affords(budget: Materials, cost: Materials) -> bool {
    (budget - cost).amounts().all(|(_, left)| left >= 0.0)
}

fn is_structure(roster: &Roster, row: RowId) -> bool {
    roster
        .get(row)
        .is_some_and(|row| row.kind() == neumannarch_sim::roster::Kind::Structure)
}

fn first(rows: &[RowId]) -> impl Iterator<Item = RowId> + '_ {
    rows.iter().copied().take(1)
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

    use neumannarch_sim::belt::Belt;
    use neumannarch_sim::roster::{
        CONSTRUCTOR, ENERGY_EXTRACTOR, METALS_EXTRACTOR, RAIDER, SHIPYARD, VOLATILES_EXTRACTOR,
    };
    use neumannarch_sim::state::view::View;
    use neumannarch_sim::state::{Batch, Issued, Seat, State};
    use neumannarch_sim::step::fire::Shots;
    use neumannarch_sim::{Material, Materials, SeatId, TICKS_PER_SECOND, TeamId, Tick, Time};

    use super::*;
    use crate::personality::Personality;
    use crate::roles::Roles;

    const CLOCK: Tick = Tick(15 * 60 * TICKS_PER_SECOND as u64);

    const ALLY_ROCK: RockId = RockId(4);

    const ENEMY_ROCK: RockId = RockId(9);

    const HOME: RockId = RockId(0);

    const ALIKE_ROCK: RockId = RockId(1);

    const UNLIKE_ROCK: RockId = RockId(2);

    const RICH: Materials = Materials::new(1e4, 1e4, 1e4);

    const EXTRACTORS: [(Material, RowId); 3] = [
        (Material::Metals, METALS_EXTRACTOR),
        (Material::Volatiles, VOLATILES_EXTRACTOR),
        (Material::Energy, ENERGY_EXTRACTOR),
    ];

    fn mining(standing: u32) -> View {
        let mut batch = Batch::new();
        for (seq, (row, count)) in [(SHIPYARD, 1), (METALS_EXTRACTOR, standing)]
            .into_iter()
            .enumerate()
        {
            let seq = u32::try_from(seq).expect("two commands");
            assert_eq!(batch.insert(want(0, seq, HOME, row, count)), Ok(()));
        }
        let reserve = BTreeMap::from([(SHIPYARD, 1), (METALS_EXTRACTOR, standing)]);
        let (state, outcome) = opened(sided(reserve)).step(&batch);
        assert_eq!(outcome.rejected, Vec::new());
        View::of(&state, SeatId(0), &Shots::default())
    }

    fn opened(state: State) -> State {
        let mut state = state;
        while state.drafting() {
            let (next, _) = state.step(&Batch::new());
            state = next;
        }
        state
    }

    fn planned(view: &View) -> Plan {
        let roster = Roster::shipped();
        let roles = Roles::of(&roster);
        let survey = Survey::of(view, &roster, &roles);
        Plan::of(
            &survey,
            &Personality::expand(),
            &mut Commitments::default(),
            &mut Dice::new(0),
        )
    }

    fn sided(reserve: BTreeMap<RowId, u32>) -> State {
        let seat = |team| Seat::new(TeamId(team), RICH, reserve.clone());
        State::new(
            Time(CLOCK.0),
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
    fn a_bots_second_draft_pick_complements_its_first() {
        let mut state = sided(BTreeMap::from([(SHIPYARD, 1), (CONSTRUCTOR, 1)]));
        let opening = state.draft().stages()[2];
        while state.draft().running() != Some(opening) {
            state = state.step(&Batch::new()).0;
        }
        let mut batch = Batch::new();
        let placing = want(opening.seat.0, 0, HOME, opening.row, 1);
        assert_eq!(batch.insert(placing), Ok(()));
        let mut view = View::of(&state.step(&batch).0, opening.seat, &Shots::default());
        let metals = Materials::new(RICH[Material::Metals], 0.0, 0.0);
        let beside = view.terrain_of(ALIKE_ROCK).expect("a free rock").orbit;
        for terrain in &mut view.terrain {
            terrain.caps = match terrain.rock {
                HOME | ALIKE_ROCK => metals,
                UNLIKE_ROCK => RICH - metals,
                _ => Materials::ZERO,
            };
        }
        view.terrain[UNLIKE_ROCK.0 as usize].orbit = beside;

        let picked = planned(&view).commands(&view);

        assert_eq!(
            picked.first().map(|Command::Want { rock, .. }| *rock),
            Some(UNLIKE_ROCK),
            "it took the rock as poor as the one it opened on, not the one that completes it"
        );
    }

    #[test]
    fn no_extractor_is_wanted_at_a_rock_whose_headroom_cannot_feed_it() {
        let roster = Roster::shipped();
        for standing in [0, 3] {
            let view = mining(standing);
            let plan = planned(&view);

            let terrain = view.terrain_of(HOME).expect("the home rock");
            for (material, row) in EXTRACTORS {
                let rate = roster[row].extracts_of(material);
                let headroom = terrain.caps[material] - terrain.pull[material];
                let fed = f64::from(plan.planned(HOME, row).saturating_sub(1)) * rate;
                assert!(fed < headroom, "{material:?}: {fed} past {headroom}");
            }
        }
    }

    #[test]
    fn a_bot_whose_income_covers_its_builders_wants_no_further_extractor() {
        let mut view = mining(0);
        view.income = RICH;

        let plan = planned(&view);

        for (_, row) in EXTRACTORS {
            assert_eq!(plan.planned(HOME, row), 0, "{row:?} wanted");
        }
    }

    #[test]
    fn a_bot_near_the_clocks_end_wants_no_extractor_that_cannot_repay() {
        let mut view = mining(0);
        assert!(
            planned(&view).planned(HOME, METALS_EXTRACTOR) > 0,
            "with a match to play"
        );
        view.time = view.length.since(Time(1));

        let plan = planned(&view);

        for (_, row) in EXTRACTORS {
            assert_eq!(plan.planned(HOME, row), 0, "{row:?} wanted");
        }
    }

    #[test]
    fn a_standing_extractor_is_never_wanted_away_by_the_economy() {
        let mut covered = mining(3);
        covered.income = RICH;

        assert_eq!(planned(&covered).planned(HOME, METALS_EXTRACTOR), 3);
        assert!(
            planned(&mining(3)).planned(HOME, METALS_EXTRACTOR) >= 3,
            "a shortfall asks for more, never for fewer"
        );
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
        let reserve = BTreeMap::from([(SHIPYARD, 1), (RAIDER, 4)]);
        let (state, outcome) = opened(sided(reserve)).step(&batch);
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
