use std::collections::BTreeMap;

use neumannarch_sim::{AsteroidId, RowId};

use super::commitments::Commitments;
use super::personality::Personality;
use super::proposal::{Proposal, Reason};
use super::ranking::Ranking;
use super::survey::Survey;

pub struct Offence;

impl Offence {
    pub fn proposals(
        survey: &Survey,
        personality: &Personality,
        commitments: &mut Commitments,
    ) -> Vec<Proposal> {
        let weights = personality.shares(survey);
        let unit_cost = personality.damage_unit_cost(survey.roster, &weights);
        if unit_cost <= 0.0 {
            return Vec::new();
        }
        let sending = match Offence::attacked(survey, personality, commitments, unit_cost) {
            Some(target) if Offence::wave_landed(survey, &weights, target) => {
                commitments.committed = Some(target);
                Offence::wave(survey, personality, &weights, unit_cost, target)
            }
            _ => Vec::new(),
        };
        sending
            .into_iter()
            .chain(Offence::arming(survey, &weights))
            .collect()
    }

    fn wave_landed(survey: &Survey, weights: &[(RowId, f64)], target: AsteroidId) -> bool {
        weights
            .iter()
            .all(|(row, _)| survey.arriving(target, *row) == 0)
    }

    fn attacked(
        survey: &Survey,
        personality: &Personality,
        commitments: &Commitments,
        unit_cost: f64,
    ) -> Option<AsteroidId> {
        let staging = survey.staging()?;
        let target = commitments
            .committed
            .or_else(|| survey.nearest(staging, &survey.enemy_asteroids))?;
        let ratio = personality.attack_ratio_at(survey.view.time, survey.view.length);
        let enough = (ratio * survey.threat_at(target))
            .min(f64::from(personality.attack_floor) * unit_cost)
            .max(0.0);
        let beyond = Offence::beyond_garrisons(survey, personality, unit_cost, target);
        (beyond > 0.0 && beyond >= enough).then_some(target)
    }

    fn beyond_garrisons(
        survey: &Survey,
        personality: &Personality,
        unit_cost: f64,
        target: AsteroidId,
    ) -> f64 {
        Offence::mustering(survey, target)
            .map(|asteroid| {
                let garrison = personality.garrison(survey.threat_at(asteroid), unit_cost);
                (survey.damage_value(asteroid) - garrison).max(0.0)
            })
            .sum()
    }

    fn wave(
        survey: &Survey,
        personality: &Personality,
        weights: &[(RowId, f64)],
        unit_cost: f64,
        target: AsteroidId,
    ) -> Vec<Proposal> {
        let mut sent: BTreeMap<RowId, u32> = BTreeMap::new();
        let mut proposals = Vec::new();
        for asteroid in Offence::mustering(survey, target) {
            let units = personality.garrison(survey.threat_at(asteroid), unit_cost) / unit_cost;
            for (row, share) in weights {
                let garrisoned =
                    Proposal::rounded(survey, Reason::Offence, asteroid, *row, share * units).count;
                let going = survey.standing(asteroid, *row).saturating_sub(garrisoned);
                if going == 0 {
                    continue;
                }
                *sent.entry(*row).or_default() += going;
                let keeping = garrisoned + survey.arriving(asteroid, *row);
                proposals.push(Proposal::at(
                    survey,
                    Reason::Offence,
                    asteroid,
                    *row,
                    keeping,
                ));
            }
        }
        for (row, going) in sent {
            let homed = survey.count(target, row);
            proposals.push(Proposal::at(
                survey,
                Reason::Offence,
                target,
                row,
                homed + going,
            ));
        }
        proposals
    }

    fn mustering(survey: &Survey, target: AsteroidId) -> impl Iterator<Item = AsteroidId> {
        survey
            .occupied()
            .into_iter()
            .filter(move |asteroid| *asteroid != target)
    }

    fn arming(survey: &Survey, weights: &[(RowId, f64)]) -> Vec<Proposal> {
        let yielding = survey.roles.yields_on_completion();
        survey
            .building()
            .into_iter()
            .flat_map(|asteroid| match survey.short_of(asteroid, &yielding) {
                true => Offence::keeping_at(survey, weights, asteroid),
                false => Offence::arming_at(survey, weights, asteroid),
            })
            .collect()
    }

    fn keeping_at(
        survey: &Survey,
        weights: &[(RowId, f64)],
        asteroid: AsteroidId,
    ) -> Vec<Proposal> {
        weights
            .iter()
            .map(|(row, _)| {
                let homed = survey.count(asteroid, *row);
                Proposal::at(survey, Reason::Offence, asteroid, *row, homed)
            })
            .collect()
    }

    fn arming_at(survey: &Survey, weights: &[(RowId, f64)], asteroid: AsteroidId) -> Vec<Proposal> {
        let homed_or_framed =
            |row: RowId| survey.count(asteroid, row) + u32::from(survey.frame_open(asteroid, row));
        let force = f64::from(
            weights
                .iter()
                .map(|(row, _)| homed_or_framed(*row))
                .sum::<u32>()
                + 1,
        );
        let raising = Ranking::by(weights.iter().map(|(row, _)| *row), |row| {
            let share = weights
                .iter()
                .find(|(id, _)| *id == row)
                .map(|(_, share)| *share)?;
            Some(share * force - f64::from(homed_or_framed(row)))
        })
        .best();
        weights
            .iter()
            .map(|(row, _)| {
                let more = u32::from(Some(*row) == raising);
                let count = homed_or_framed(*row) + more;
                Proposal::at(survey, Reason::Offence, asteroid, *row, count)
            })
            .collect()
    }
}
