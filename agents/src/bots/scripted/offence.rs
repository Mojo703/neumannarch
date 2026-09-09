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
        let unit_cost = personality.armed_unit_cost(survey.roster, &weights);
        let Some(staging) = survey.staging().filter(|_| unit_cost > 0.0) else {
            return Vec::new();
        };
        let garrison = personality.garrison(survey.threat_at(staging), unit_cost);
        let sending = match Offence::attacked(survey, personality, commitments, staging, garrison) {
            Some(target) if Offence::wave_landed(survey, &weights, target) => {
                commitments.committed = Some(target);
                Offence::wave(survey, &weights, staging, target, garrison / unit_cost)
            }
            _ => Vec::new(),
        };
        match sending.is_empty() {
            true => Offence::fill(survey, &weights, staging),
            false => sending,
        }
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
        staging: AsteroidId,
        garrison: f64,
    ) -> Option<AsteroidId> {
        let target = commitments
            .committed
            .or_else(|| survey.nearest(staging, &survey.enemy_asteroids))?;
        let weights = personality.shares(survey);
        let unit_cost = personality.armed_unit_cost(survey.roster, &weights);
        let ratio = personality.attack_ratio_at(survey.view.time, survey.view.length);
        let enough = (ratio * survey.threat_at(target))
            .min(f64::from(personality.attack_floor) * unit_cost)
            .max(0.0);
        let beyond = survey.armed_value(staging) - garrison;
        (beyond > 0.0 && beyond >= enough).then_some(target)
    }

    fn wave(
        survey: &Survey,
        weights: &[(RowId, f64)],
        staging: AsteroidId,
        target: AsteroidId,
        units: f64,
    ) -> Vec<Proposal> {
        let mut proposals = Vec::new();
        for (row, share) in weights {
            let stays = Proposal::rounded(survey, Reason::Offence, staging, *row, share * units);
            let sent = survey.count(staging, *row).saturating_sub(stays.count);
            if sent == 0 {
                continue;
            }
            let homed = survey.count(target, *row);
            proposals.push(Proposal::at(
                survey,
                Reason::Offence,
                target,
                *row,
                homed + sent,
            ));
            proposals.push(stays);
        }
        proposals
    }

    fn fill(survey: &Survey, weights: &[(RowId, f64)], staging: AsteroidId) -> Vec<Proposal> {
        let wanted = |row: RowId| survey.want(staging, row).max(survey.count(staging, row));
        let force = f64::from(weights.iter().map(|(row, _)| wanted(*row)).sum::<u32>() + 1);
        let raising = Ranking::by(weights.iter().map(|(row, _)| *row), |row| {
            let share = weights
                .iter()
                .find(|(id, _)| *id == row)
                .map(|(_, share)| *share)?;
            Some(share * force - f64::from(wanted(row)))
        })
        .best();
        weights
            .iter()
            .map(|(row, _)| {
                let more = u32::from(Some(*row) == raising);
                Proposal::at(survey, Reason::Offence, staging, *row, wanted(*row) + more)
            })
            .collect()
    }
}
