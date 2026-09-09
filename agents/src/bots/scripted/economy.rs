use super::commitments::Commitments;
use super::personality::Personality;
use super::proposal::{Proposal, Reason};
use super::survey::Survey;

const PAYBACK_HORIZON: f64 = 60.0;

const STORE_TRIGGER: f64 = 0.9;

pub struct Economy;

impl Economy {
    pub fn proposals(
        survey: &Survey,
        personality: &Personality,
        commitments: &Commitments,
    ) -> Vec<Proposal> {
        let mut proposals = Economy::extractors(survey, personality);
        proposals.extend(Economy::yards(survey));
        proposals.extend(Economy::stores(survey, personality));
        proposals.extend(Economy::masons(survey, personality, commitments));
        proposals
    }

    fn extractors(survey: &Survey, personality: &Personality) -> Vec<Proposal> {
        let mix = personality.to_build(survey);
        let repaying = survey
            .view
            .length
            .since(survey.view.time)
            .seconds()
            .min(PAYBACK_HORIZON);
        let mut proposals = Vec::new();
        for (material, row) in &survey.roles.extractors {
            let Some(stats) = survey.roster.get(*row).filter(|_| mix[*material] > 0.0) else {
                continue;
            };
            for asteroid in survey.developed() {
                let pulls = stats
                    .extracts_of(*material)
                    .min(survey.spare_cap_at(asteroid, *material));
                if pulls * repaying < stats.cost.total() {
                    continue;
                }
                let standing = survey.count(asteroid, *row);
                proposals.push(Proposal::at(
                    survey,
                    Reason::Economy,
                    asteroid,
                    *row,
                    standing + 1,
                ));
            }
        }
        proposals
    }

    fn yards(survey: &Survey) -> Vec<Proposal> {
        let Some(row) = survey.roles.yards.first().copied() else {
            return Vec::new();
        };
        if survey.build_rate() >= survey.view.income.total() {
            return Vec::new();
        }
        survey
            .building()
            .into_iter()
            .map(|asteroid| {
                let standing = survey.count(asteroid, row);
                Proposal::at(survey, Reason::Economy, asteroid, row, standing + 1)
            })
            .collect()
    }

    fn stores(survey: &Survey, personality: &Personality) -> Vec<Proposal> {
        let (Some(home), Some(row)) = (survey.home(), survey.roles.stores.first().copied()) else {
            return Vec::new();
        };
        let stockpile = &survey.view.stockpile;
        let full = stockpile.stock().total() >= STORE_TRIGGER * stockpile.capacity().total();
        vec![Proposal::at(
            survey,
            Reason::Economy,
            home,
            row,
            personality.stores + u32::from(full),
        )]
    }

    fn masons(
        survey: &Survey,
        personality: &Personality,
        commitments: &Commitments,
    ) -> Vec<Proposal> {
        let (Some(home), Some(row)) = (survey.home(), survey.roles.masons.first().copied()) else {
            return Vec::new();
        };
        let spare = survey.count(home, row).saturating_sub(personality.masons);
        let claiming = personality.funding_order.contains(&Reason::Expansion)
            && commitments.claims() < personality.claims
            && survey.short_of_room(personality.demand(survey));
        let count = personality.masons + u32::from(claiming && spare == 0);
        let mut proposals: Vec<Proposal> = survey
            .building()
            .into_iter()
            .filter(|asteroid| *asteroid != home)
            .map(|asteroid| {
                let standing = survey.count(asteroid, row);
                Proposal::at(survey, Reason::Economy, asteroid, row, standing)
            })
            .collect();
        proposals.push(Proposal::at(survey, Reason::Economy, home, row, count));
        proposals
    }
}
