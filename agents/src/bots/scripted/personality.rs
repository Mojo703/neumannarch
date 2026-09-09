use neumannarch_sim::roster::Roster;
use neumannarch_sim::{Materials, RowId, Time};

use super::proposal::Reason;
use super::roles::Roles;
use super::survey::Survey;

const PLATING_WORTH: f64 = 20.0;

const ATTACK_RATIO_LASTS: f64 = 2.0 / 3.0;

const BUILD_HORIZON: f64 = 20.0;

#[derive(Clone, Debug, PartialEq)]
pub enum Mix {
    Counters,
    Pinned(Vec<(RowId, f64)>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Personality {
    pub name: &'static str,
    pub seed: u64,
    pub stores: u32,
    pub masons: u32,
    pub claims: usize,
    pub garrison_floor: u32,
    pub attack_floor: u32,
    pub defence_ratio: f64,
    pub attack_ratio: f64,
    pub mix: Mix,
    pub range_taste: f64,
    pub armour_taste: f64,
    pub funding_order: Vec<Reason>,
}

impl Personality {
    pub fn turtle() -> Personality {
        Personality {
            name: "turtle",
            seed: 0x7075_7274_6c65,
            stores: 2,
            masons: 1,
            claims: 1,
            garrison_floor: 3,
            attack_floor: 8,
            defence_ratio: 2.0,
            attack_ratio: 1.1,
            mix: Mix::Counters,
            range_taste: 0.5,
            armour_taste: 1.0,
            funding_order: vec![Reason::Defence, Reason::Economy, Reason::Offence],
        }
    }

    pub fn expand() -> Personality {
        Personality {
            name: "expand",
            seed: 0x6578_7061_6e64,
            stores: 1,
            masons: 3,
            claims: 2,
            garrison_floor: 2,
            attack_floor: 5,
            defence_ratio: 1.2,
            attack_ratio: 0.7,
            mix: Mix::Counters,
            range_taste: 0.0,
            armour_taste: 0.0,
            funding_order: vec![
                Reason::Economy,
                Reason::Expansion,
                Reason::Defence,
                Reason::Offence,
            ],
        }
    }

    pub fn damage_unit_cost(&self, roster: &Roster, weights: &[(RowId, f64)]) -> f64 {
        weights
            .iter()
            .filter_map(|(row, share)| roster.get(*row).map(|row| row.cost.total() * share))
            .sum()
    }

    pub fn order(&self, threatened: bool, army_ahead: bool) -> Vec<Reason> {
        let mut order = self.funding_order.clone();
        if threatened {
            above_economy(&mut order, Reason::Defence);
        }
        if !army_ahead {
            above_economy(&mut order, Reason::Offence);
        }
        order
    }

    pub fn garrison(&self, threat: f64, unit_cost: f64) -> f64 {
        (threat * self.defence_ratio).max(f64::from(self.garrison_floor) * unit_cost)
    }

    pub fn shares(&self, survey: &Survey) -> Vec<(RowId, f64)> {
        self.weights(
            survey.roster,
            survey.roles,
            survey.enemy_plating,
            survey.enemy_range,
        )
    }

    pub fn damage_mix(&self, survey: &Survey) -> Materials {
        self.shares(survey)
            .iter()
            .filter_map(|(row, share)| survey.roster.get(*row).map(|stats| stats.cost * *share))
            .fold(Materials::ZERO, |mix, cost| mix + cost)
    }

    pub fn to_build(&self, survey: &Survey) -> Materials {
        let damage_mix = self.damage_mix(survey);
        let laid_down = survey.build_rate() * BUILD_HORIZON;
        let units = (laid_down / damage_mix.total().max(f64::MIN_POSITIVE)).max(1.0);
        survey.standing_cost() + damage_mix * units
    }

    pub fn demand(&self, survey: &Survey) -> Materials {
        let mix = self.to_build(survey);
        match mix.total() {
            total if total > 0.0 => mix * (survey.build_rate() / total),
            _ => Materials::ZERO,
        }
    }

    pub fn wanted_shares(&self, survey: &Survey) -> Materials {
        let shares = |of: Materials| of * (1.0 / of.total().max(f64::MIN_POSITIVE));
        let drafted = survey
            .held()
            .into_iter()
            .filter_map(|asteroid| survey.view.terrain_of(asteroid))
            .fold(Materials::ZERO, |caps, terrain| caps + terrain.caps);
        let held = shares(drafted);
        let mut wanted = Materials::ZERO;
        for (material, share) in shares(self.to_build(survey)).amounts() {
            wanted[material] = share * (1.0 - held[material]);
        }
        wanted
    }

    pub fn attack_ratio_at(&self, time: Time, length: Time) -> f64 {
        let spent = time.seconds() / (ATTACK_RATIO_LASTS * length.seconds()).max(f64::MIN_POSITIVE);
        self.attack_ratio * (1.0 - spent).max(0.0)
    }

    pub fn weights(
        &self,
        roster: &Roster,
        roles: &Roles,
        plating: f64,
        range: f64,
    ) -> Vec<(RowId, f64)> {
        let scored = match &self.mix {
            Mix::Pinned(pinned) => pinned
                .iter()
                .filter(|(row, _)| roles.army.contains(row))
                .map(|(row, weight)| (*row, weight.max(0.0)))
                .collect(),
            Mix::Counters => self.counters(roster, roles, plating, range),
        };
        share(scored)
    }

    fn counters(
        &self,
        roster: &Roster,
        roles: &Roles,
        plating: f64,
        range: f64,
    ) -> Vec<(RowId, f64)> {
        let rated = |through: f64| -> Vec<(RowId, f64)> {
            roles
                .army
                .iter()
                .filter_map(|id| roster.get(*id).map(|row| (*id, row)))
                .map(|(id, row)| {
                    let cost = row.cost.total().max(f64::MIN_POSITIVE);
                    let reach = row.max_damage_range();
                    let edge = (reach - range) / reach.max(range).max(1.0);
                    let durability = (row.hp.0 + PLATING_WORTH * row.plating.0) / cost;
                    let rate = row.dps_through(through) / cost;
                    (
                        id,
                        rate * (1.0 + self.range_taste * edge).max(0.0)
                            * (1.0 + self.armour_taste * durability),
                    )
                })
                .collect()
        };
        let against = rated(plating);
        if against.iter().any(|(_, weight)| *weight > 0.0) {
            against
        } else {
            rated(0.0)
        }
    }
}

fn above_economy(order: &mut Vec<Reason>, reason: Reason) {
    let Some(economy) = order.iter().position(|held| *held == Reason::Economy) else {
        return;
    };
    let Some(at) = order.iter().position(|held| *held == reason) else {
        return;
    };
    if at < economy {
        return;
    }
    order.remove(at);
    order.insert(economy, reason);
}

fn share(scored: Vec<(RowId, f64)>) -> Vec<(RowId, f64)> {
    let total: f64 = scored.iter().map(|(_, weight)| weight).sum();
    let count = scored.len() as f64;
    scored
        .into_iter()
        .map(|(row, weight)| match total > 0.0 {
            true => (row, weight / total),
            false => (row, 1.0 / count),
        })
        .collect()
}
