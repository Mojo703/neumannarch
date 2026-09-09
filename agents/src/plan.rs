use std::collections::BTreeMap;

use neumannarch_sim::roster::Roster;
use neumannarch_sim::state::view::View;
use neumannarch_sim::state::{Command, MAX_WANT};
use neumannarch_sim::{AsteroidId, Material, Materials, Posting, RowId, SeatId};

use crate::commitments::Commitments;
use crate::dice::Dice;
use crate::personality::Personality;
use crate::ranking::Ranking;
use crate::survey::Survey;

const BUILD_HORIZON: f64 = 20.0;

const STORE_TRIGGER: f64 = 0.9;

const NEAR_CAPACITY_SHARE: f64 = 0.8;

const REACH: f64 = 2_000.0;

const CHOICES: usize = 2;

const PAYBACK_HORIZON: f64 = 60.0;

const TO_BUILD_PER_BUILDER: u32 = 2;

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
    seat: SeatId,
    targets: BTreeMap<Posting, Target>,
    promised: BTreeMap<RowId, u32>,
    to_build: BTreeMap<AsteroidId, u32>,
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
            seat: survey.view.seat,
            targets: BTreeMap::new(),
            promised: BTreeMap::new(),
            to_build: BTreeMap::new(),
            budget: survey.view.stockpile.stock() + survey.view.income * BUILD_HORIZON,
        };
        match survey.view.draft.ended() {
            None => plan.draft(survey, personality),
            Some(_) => plan.play(survey, personality, commitments, dice),
        }
        plan
    }

    pub fn commands(&self, view: &View) -> Vec<Command> {
        let dropped = view
            .plans
            .iter()
            .filter(|(posting, plan)| plan.want > 0 && !self.targets.contains_key(posting))
            .map(|(posting, _)| Command::Want {
                asteroid: posting.asteroid(),
                row: posting.row(),
                count: 0,
            });
        let mut changes: Vec<(&Posting, &Target)> = self
            .targets
            .iter()
            .filter(|(posting, target)| view.want_of(**posting) != target.count)
            .collect();
        changes.sort_by_key(|(posting, target)| {
            (target.count > view.want_of(**posting), target.priority)
        });
        dropped
            .chain(changes.into_iter().map(|(posting, target)| Command::Want {
                asteroid: posting.asteroid(),
                row: posting.row(),
                count: target.count,
            }))
            .collect()
    }

    fn play(
        &mut self,
        survey: &Survey,
        personality: &Personality,
        commitments: &mut Commitments,
        dice: &mut Dice,
    ) {
        self.lower_where_no_builder(survey);
        for asteroid in survey.developed() {
            self.stand(survey, asteroid);
        }
        let short_of_room = self.extractors(survey, personality);
        self.expand(survey, personality, commitments, dice, short_of_room);
        self.yards(survey, personality);
        self.stores(survey, personality);
        self.army(survey, personality, commitments);
    }

    fn draft(&mut self, survey: &Survey, personality: &Personality) {
        for asteroid in survey.held() {
            self.stand(survey, asteroid);
        }
        let seat = survey.view.seat;
        let Some(stage) = survey
            .view
            .draft
            .running()
            .filter(|stage| stage.seat == seat)
        else {
            return;
        };
        let Some(asteroid) = self.fittest(survey, personality) else {
            return;
        };
        self.keep(Priority::Opening, asteroid, stage.row, 1);
    }

    fn wanted_shares(&self, survey: &Survey, personality: &Personality) -> Materials {
        let shares = |of: Materials| of * (1.0 / of.total().max(f64::MIN_POSITIVE));
        let drafted = survey
            .held()
            .into_iter()
            .filter_map(|asteroid| survey.view.terrain_of(asteroid))
            .fold(Materials::ZERO, |caps, terrain| caps + terrain.caps);
        let held = shares(drafted);
        let mut wanted = Materials::ZERO;
        for (material, share) in shares(self.to_build(survey, personality)).amounts() {
            wanted[material] = share * (1.0 - held[material]);
        }
        wanted
    }

    fn fittest(&self, survey: &Survey, personality: &Personality) -> Option<AsteroidId> {
        let wanted = self.wanted_shares(survey, personality);
        let held = survey.held();
        let nearest = |from: AsteroidId, among: &[AsteroidId]| -> Option<f64> {
            among
                .iter()
                .map(|to| survey.between(from, *to))
                .min_by(f64::total_cmp)
        };
        Ranking::by(
            survey
                .view
                .terrain
                .iter()
                .map(|terrain| terrain.asteroid)
                .filter(|asteroid| !survey.taken.contains(asteroid)),
            |asteroid| {
                let terrain = survey.view.terrain_of(asteroid)?;
                let fit: f64 = terrain
                    .caps
                    .amounts()
                    .map(|(material, cap)| cap * wanted[material])
                    .sum();
                let away = nearest(asteroid, &survey.enemy_asteroids)
                    .map_or(1.0, |gap| gap / (gap + REACH));
                let near = nearest(asteroid, &held).unwrap_or_default();
                Some(fit * away / (1.0 + near / REACH))
            },
        )
        .best()
    }

    fn to_build(&self, survey: &Survey, personality: &Personality) -> Materials {
        let weights = personality.weights(
            survey.roster,
            survey.roles,
            survey.enemy_plating,
            survey.enemy_range,
        );
        let armed = weights
            .iter()
            .filter_map(|(row, share)| survey.roster.get(*row).map(|row| row.cost * *share))
            .fold(Materials::ZERO, |mix, cost| mix + cost);
        let laid_down = survey.build_rate() * BUILD_HORIZON;
        let units = (laid_down / armed.total().max(f64::MIN_POSITIVE)).max(1.0);
        self.wanted(survey)
            .fold(Materials::ZERO, |mix, cost| mix + cost)
            + armed * units
    }

    fn demand(&self, survey: &Survey, personality: &Personality) -> Materials {
        let mix = self.to_build(survey, personality);
        match mix.total() {
            total if total > 0.0 => mix * (survey.build_rate() / total),
            _ => Materials::ZERO,
        }
    }

    fn wanted<'a>(&'a self, survey: &'a Survey) -> impl Iterator<Item = Materials> + 'a {
        self.targets.iter().filter_map(|(posting, target)| {
            survey
                .roster
                .get(posting.row())
                .map(|row| row.cost * f64::from(target.count))
        })
    }

    fn extractors(&mut self, survey: &Survey, personality: &Personality) -> bool {
        let wanted = self.demand(survey, personality);
        let left = survey.view.length.since(survey.view.time).seconds();
        let repaying = left.min(PAYBACK_HORIZON);
        let mut short_of_room = false;
        for (material, row) in &survey.roles.extractors {
            let Some(stats) = survey.roster.get(*row) else {
                continue;
            };
            let rate = stats.extracts_of(*material);
            let mut short = (wanted[*material] - survey.view.income[*material]).max(0.0);
            let mut spare: BTreeMap<AsteroidId, f64> = BTreeMap::new();
            let mut counts: BTreeMap<AsteroidId, u32> = BTreeMap::new();
            for asteroid in survey.developed() {
                let Some(terrain) = survey.view.terrain_of(asteroid) else {
                    continue;
                };
                let standing = survey.count(asteroid, *row);
                let mine = (rate * f64::from(standing)).min(terrain.caps[*material]);
                let others = (terrain.pull[*material] - mine).max(0.0);
                spare.insert(asteroid, terrain.caps[*material] - others - mine);
                counts.insert(asteroid, standing);
            }
            while short > 0.0 {
                let Some(asteroid) = widest(&spare) else {
                    short_of_room = true;
                    break;
                };
                let pulls = rate.min(spare[&asteroid]);
                if pulls * repaying < stats.cost.total() {
                    break;
                }
                short -= pulls;
                *spare.entry(asteroid).or_default() -= pulls;
                *counts.entry(asteroid).or_default() += 1;
            }
            for (asteroid, count) in counts.into_iter().filter(|(_, count)| *count > 0) {
                self.want(survey, Priority::Economy, asteroid, *row, count);
            }
        }
        short_of_room
    }

    fn lower_where_no_builder(&mut self, survey: &Survey) {
        let wanted: Vec<Posting> = survey
            .view
            .plans
            .iter()
            .filter(|(_, plan)| plan.want > 0)
            .map(|(posting, _)| *posting)
            .collect();
        for posting in wanted {
            let asteroid = posting.asteroid();
            if survey.builds_at(asteroid) {
                continue;
            }
            let standing = survey.standing(asteroid, posting.row());
            self.keep(Priority::Economy, asteroid, posting.row(), standing);
        }
    }

    fn stand(&mut self, survey: &Survey, asteroid: AsteroidId) {
        let standing: Vec<(RowId, u32)> = survey
            .mine
            .get(&asteroid)
            .into_iter()
            .flatten()
            .filter(|(row, _)| survey.is_structure(**row))
            .map(|(row, held)| (*row, held.present))
            .collect();
        for (row, count) in standing {
            self.keep(Priority::Economy, asteroid, row, count);
        }
    }

    fn yards(&mut self, survey: &Survey, personality: &Personality) {
        let Some(row) = survey.roles.yards.first().copied() else {
            return;
        };
        let home = survey.home();
        let mut asteroids: Vec<AsteroidId> = home.into_iter().collect();
        asteroids.extend(
            survey
                .developed()
                .into_iter()
                .filter(|asteroid| Some(*asteroid) != home),
        );
        asteroids.truncate(personality.yards);
        for asteroid in asteroids {
            self.want(survey, Priority::Economy, asteroid, row, 1);
        }
        let Some(staging) = survey.staging().filter(|_| stock_near_capacity(survey)) else {
            return;
        };
        let yard_building = survey
            .view
            .plans
            .get(&self.posting(staging, row))
            .is_some_and(|plan| plan.building.is_some());
        if !yard_building {
            let standing = self.planned(staging, row).max(survey.count(staging, row));
            self.want(survey, Priority::Economy, staging, row, standing + 1);
        }
    }

    fn stores(&mut self, survey: &Survey, personality: &Personality) {
        let (Some(home), Some(row)) = (survey.home(), survey.roles.stores.first().copied()) else {
            return;
        };
        let stockpile = &survey.view.stockpile;
        let full = stockpile.stock().total() >= STORE_TRIGGER * stockpile.capacity().total();
        let count = personality.stores + u32::from(full);
        self.want(survey, Priority::Economy, home, row, count);
    }

    fn expand(
        &mut self,
        survey: &Survey,
        personality: &Personality,
        commitments: &mut Commitments,
        dice: &mut Dice,
        short_of_room: bool,
    ) {
        let (Some(home), Some(mason)) = (survey.home(), survey.roles.masons.first().copied())
        else {
            return;
        };
        for asteroid in survey.occupied().into_iter().filter(|at| *at != home) {
            self.keep(
                Priority::Economy,
                asteroid,
                mason,
                survey.standing(asteroid, mason),
            );
        }
        let at_home = survey.standing(home, mason);
        let mut spare = at_home.saturating_sub(personality.masons);
        let mut sent = 0;
        let mut short = 0;
        for claim in commitments.claimed_asteroids() {
            let arriving = survey.count(claim, mason);
            if arriving > 0 {
                self.keep(Priority::Economy, claim, mason, arriving);
            } else if spare > 0 {
                spare -= 1;
                sent += 1;
                self.keep(Priority::Economy, claim, mason, 1);
            } else {
                short += 1;
            }
        }

        let claiming = short_of_room && commitments.claims() < personality.claims;
        if spare > 0
            && claiming
            && let Some(asteroid) = expansion(survey, commitments, dice)
        {
            spare -= 1;
            sent += 1;
            commitments.claim(asteroid, survey.view.time);
            self.keep(Priority::Economy, asteroid, mason, 1);
        }

        let replacing = u32::from(sent == 0 && spare == 0 && (short > 0 || claiming));
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
        let unit = personality.armed_unit_cost(survey.roster, &weights);
        let Some(staging) = survey.staging().filter(|_| unit > 0.0) else {
            return;
        };
        let marching = self.attacked(survey, personality, commitments, staging, unit);
        commitments.committed = marching;
        if marching.is_some() {
            self.hold_garrison(survey, personality, &weights, staging, unit);
        }
        for asteroid in survey.occupied() {
            if marching.is_some() && asteroid == staging {
                continue;
            }
            for (row, _) in &weights {
                let posting = Posting::of(asteroid, self.seat, *row);
                let carried = survey
                    .view
                    .want_of(posting)
                    .max(survey.count(asteroid, *row));
                self.want(
                    survey,
                    self.rank(asteroid, staging),
                    asteroid,
                    *row,
                    carried,
                );
            }
        }
        if !commitments.holding_back_army {
            self.fill(survey, personality, &weights, staging, unit);
        }
        self.spend_overflow(survey, &weights, staging);
        if let Some(target) = marching {
            self.send_forward(survey, &weights, target);
        }
    }

    fn hold_garrison(
        &mut self,
        survey: &Survey,
        personality: &Personality,
        weights: &[(RowId, f64)],
        staging: AsteroidId,
        unit: f64,
    ) {
        let units = (self.garrison(survey, personality, staging, unit) / unit)
            .ceil()
            .max(0.0);
        for (row, share) in weights {
            let stays = whole(share * units).min(survey.standing(staging, *row));
            self.keep(Priority::Defence, staging, *row, stays);
        }
    }

    fn send_forward(&mut self, survey: &Survey, weights: &[(RowId, f64)], target: AsteroidId) {
        for (row, _) in weights {
            let spare: u32 = survey
                .occupied()
                .into_iter()
                .map(|at| {
                    survey
                        .count(at, *row)
                        .saturating_sub(self.planned(at, *row))
                        .min(survey.standing(at, *row))
                })
                .sum();
            let short: u32 = survey
                .occupied()
                .into_iter()
                .filter(|at| *at != target)
                .map(|at| {
                    self.planned(at, *row)
                        .saturating_sub(survey.count(at, *row))
                })
                .sum();
            let sent = spare.saturating_sub(short);
            self.keep(
                Priority::Army,
                target,
                *row,
                survey.count(target, *row) + sent,
            );
        }
    }

    fn attacked(
        &self,
        survey: &Survey,
        personality: &Personality,
        commitments: &Commitments,
        staging: AsteroidId,
        unit: f64,
    ) -> Option<AsteroidId> {
        if let Some(committed) = commitments.committed {
            return Some(committed);
        }
        let target = survey.nearest(staging, &survey.enemy_asteroids)?;
        let ratio = personality.attack_ratio_at(survey.view.time, survey.view.length);
        let enough = (ratio * survey.threat_at(target))
            .min(f64::from(personality.attack_floor) * unit)
            .max(0.0);
        let offensive =
            survey.armed_value(staging) - self.garrison(survey, personality, staging, unit);
        (offensive > 0.0 && offensive >= enough).then_some(target)
    }

    fn garrison(
        &self,
        survey: &Survey,
        personality: &Personality,
        asteroid: AsteroidId,
        unit: f64,
    ) -> f64 {
        (survey.threat_at(asteroid) * personality.defence_ratio)
            .max(f64::from(personality.garrison_floor) * unit)
    }

    fn wanted_force(
        &self,
        survey: &Survey,
        personality: &Personality,
        asteroid: AsteroidId,
        staging: AsteroidId,
        unit: f64,
    ) -> f64 {
        let garrison = self.garrison(survey, personality, asteroid, unit);
        if asteroid != staging {
            return garrison;
        }
        let ratio = personality.attack_ratio_at(survey.view.time, survey.view.length);
        let target = survey.nearest(staging, &survey.enemy_asteroids);
        let offensive = target
            .map_or(0.0, |at| ratio * survey.threat_at(at))
            .max(f64::from(personality.attack_floor) * unit);
        garrison + offensive
    }

    fn fill(
        &mut self,
        survey: &Survey,
        personality: &Personality,
        weights: &[(RowId, f64)],
        staging: AsteroidId,
        unit: f64,
    ) {
        let needing = Ranking::by(survey.building(), |asteroid| {
            let want = self.wanted_force(survey, personality, asteroid, staging, unit);
            let force = survey.armed_value(asteroid);
            (want > 0.0 && force < want).then_some((want - force) / want)
        })
        .best();
        let Some(asteroid) = needing else {
            return;
        };
        let force = f64::from(survey.armed_count(asteroid) + 1);
        let raising = Ranking::by(weights.iter().copied().map(|(row, _)| row), |row| {
            let share = weights
                .iter()
                .find(|(id, _)| *id == row)
                .map(|(_, share)| *share)?;
            Some(share * force - f64::from(survey.count(asteroid, row)))
        })
        .best();
        let Some(row) = raising else {
            return;
        };
        let rank = self.rank(asteroid, staging);
        self.want(survey, rank, asteroid, row, self.planned(asteroid, row) + 1);
    }

    fn spend_overflow(&mut self, survey: &Survey, weights: &[(RowId, f64)], staging: AsteroidId) {
        let Some(fullest) = material_near_capacity(survey) else {
            return;
        };
        let spending_it = Ranking::by(weights.iter().copied().map(|(row, _)| row), |row| {
            let cost = survey.roster.get(row)?.cost;
            (cost[fullest] > 0.0).then(|| cost[fullest] / cost.total().max(f64::MIN_POSITIVE))
        })
        .best();
        let Some(row) = spending_it else {
            return;
        };
        let idle = Ranking::by(survey.building(), |asteroid| {
            (!survey.frame_open_at(asteroid)).then(|| survey.build_rate_at(asteroid))
        })
        .best()
        .unwrap_or(staging);
        self.want(
            survey,
            Priority::Army,
            idle,
            row,
            self.planned(idle, row) + 1,
        );
    }

    fn rank(&self, asteroid: AsteroidId, staging: AsteroidId) -> Priority {
        match asteroid == staging {
            true => Priority::Army,
            false => Priority::Defence,
        }
    }

    fn want(
        &mut self,
        survey: &Survey,
        priority: Priority,
        asteroid: AsteroidId,
        row: RowId,
        count: u32,
    ) {
        let planned = self.planned(asteroid, row);
        let homed = survey.count(asteroid, row);
        let floor = planned.max(homed);
        let ceiling = match survey.builds_at(asteroid) {
            true => floor.saturating_add(self.room(survey, asteroid)),
            false => floor,
        };
        let count = count.min(MAX_WANT).min(ceiling).max(planned);
        let owned = survey.owned(row);
        let promised = self.promised(row);
        let to_build = |promised: u32| promised.saturating_sub(owned);
        let extra = to_build(promised + count - planned) - to_build(promised);
        let bought = self.affordable(survey.roster, row, extra);
        let count = count - (extra - bought);
        self.keep(priority, asteroid, row, count);
        *self.to_build.entry(asteroid).or_default() += count.saturating_sub(floor);
    }

    fn room(&self, survey: &Survey, asteroid: AsteroidId) -> u32 {
        let opened = self.to_build.get(&asteroid).copied().unwrap_or_default();
        (TO_BUILD_PER_BUILDER * survey.builders(asteroid)).saturating_sub(opened)
    }

    fn keep(&mut self, priority: Priority, asteroid: AsteroidId, row: RowId, count: u32) {
        let count = count.min(MAX_WANT);
        let posting = self.posting(asteroid, row);
        let planned = self.planned(asteroid, row);
        if count <= planned {
            if count > 0
                && let Some(target) = self.targets.get_mut(&posting)
            {
                target.priority = target.priority.min(priority);
            }
            return;
        }
        *self.promised.entry(row).or_default() += count - planned;
        self.targets.insert(
            posting,
            Target {
                priority: self
                    .targets
                    .get(&posting)
                    .map_or(priority, |target| target.priority.min(priority)),
                count,
            },
        );
    }

    fn posting(&self, asteroid: AsteroidId, row: RowId) -> Posting {
        Posting::of(asteroid, self.seat, row)
    }

    fn planned(&self, asteroid: AsteroidId, row: RowId) -> u32 {
        self.targets
            .get(&self.posting(asteroid, row))
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

fn stock_near_capacity(survey: &Survey) -> bool {
    material_near_capacity(survey).is_some()
}

fn material_near_capacity(survey: &Survey) -> Option<Material> {
    let stockpile = &survey.view.stockpile;
    Ranking::by(Material::EVERY, |material| {
        let capacity = stockpile.capacity()[material];
        let filled = stockpile.stock()[material] / capacity.max(f64::MIN_POSITIVE);
        (capacity > 0.0 && filled > NEAR_CAPACITY_SHARE).then_some(filled)
    })
    .best()
}

fn affords(budget: Materials, cost: Materials) -> bool {
    (budget - cost).amounts().all(|(_, left)| left >= 0.0)
}

fn whole(count: f64) -> u32 {
    let rounded = count.round();
    match rounded >= 0.0 && rounded <= f64::from(MAX_WANT) {
        true => rounded as u32,
        false => MAX_WANT,
    }
}

fn widest(spare: &BTreeMap<AsteroidId, f64>) -> Option<AsteroidId> {
    Ranking::by(spare.keys().copied(), |asteroid| {
        spare.get(&asteroid).copied().filter(|room| *room > 0.0)
    })
    .best()
}

fn expansion(survey: &Survey, commitments: &Commitments, dice: &mut Dice) -> Option<AsteroidId> {
    let from = survey.home()?;
    let occupied = survey.occupied();
    let rated = Ranking::by(
        survey
            .view
            .terrain
            .iter()
            .map(|terrain| terrain.asteroid)
            .filter(|asteroid| !occupied.contains(asteroid))
            .filter(|asteroid| !commitments.claimed(*asteroid) && !commitments.barred(*asteroid))
            .filter(|asteroid| !survey.enemy_asteroids.contains(asteroid)),
        |asteroid| {
            let caps = survey.view.terrain_of(asteroid)?.caps.total();
            Some(caps / (1.0 + survey.between(from, asteroid) / REACH))
        },
    )
    .order();
    let at = dice.below(rated.len().min(CHOICES))?;
    rated.get(at).copied()
}

#[cfg(test)]
mod tests {
    use neumannarch_sim::belt::Belt;
    use neumannarch_sim::roster::{CONSTRUCTOR, SHIPYARD};
    use neumannarch_sim::state::view::View;
    use neumannarch_sim::state::{Batch, Command as Verb, Issued, Seat, State};
    use neumannarch_sim::step::fire::Shots;
    use neumannarch_sim::{Material, TICKS_PER_SECOND, TeamId, Time};
    use std::collections::BTreeMap;

    use super::*;
    use crate::roles::Roles;

    const CLOCK: Time = Time(7 * 60 * TICKS_PER_SECOND as u64);

    const HOME: AsteroidId = AsteroidId(0);

    const STOCK: Materials = Materials::new(1e4, 1e4, 1e4);

    fn drafted(caps: Materials) -> View {
        let reserve = BTreeMap::from([(SHIPYARD, 1), (CONSTRUCTOR, 1)]);
        let seat = |team| Seat::new(TeamId(team), STOCK, reserve.clone());
        let mut state = State::new(
            CLOCK,
            0,
            Belt::GRAVITY,
            Roster::shipped(),
            Belt::from_seed(0),
            vec![seat(0), seat(1)],
        );
        let opening = state.draft().stages()[0];
        let mut batch = Batch::new();
        let placing = Issued {
            seat: opening.seat,
            seq: 0,
            command: Verb::Want {
                asteroid: HOME,
                row: opening.row,
                count: 1,
            },
        };
        assert_eq!(batch.insert(placing), Ok(()));
        state = state.step(&batch).0;
        let mut view = View::of(&state, opening.seat, &Shots::default());
        for terrain in &mut view.terrain {
            if terrain.asteroid == HOME {
                terrain.caps = caps;
            }
        }
        view
    }

    fn shares(view: &View) -> Materials {
        let roster = Roster::shipped();
        let roles = Roles::of(&roster);
        let survey = Survey::of(view, &roster, &roles);
        let plan = Plan::of(
            &survey,
            &Personality::expand(),
            &mut Commitments::default(),
            &mut Dice::new(0),
        );
        plan.wanted_shares(&survey, &Personality::expand())
    }

    #[test]
    fn the_shares_a_pick_is_weighed_by_discount_what_the_asteroids_already_drafted_supply() {
        let metals = Materials::new(1e3, 0.0, 0.0);

        let even = shares(&drafted(Materials::new(1.0, 1.0, 1.0)));
        let mined = shares(&drafted(metals));

        assert!(
            mined[Material::Metals] < even[Material::Metals],
            "metals kept their weight: {} against {}",
            mined[Material::Metals],
            even[Material::Metals]
        );
        assert!(mined[Material::Volatiles] > even[Material::Volatiles]);
        assert!(mined[Material::Energy] > even[Material::Energy]);
    }
}
