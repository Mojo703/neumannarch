use neumannarch_sim::Material;
use neumannarch_sim::pattern::EntityPattern;

use super::commitments::Commitments;
use super::personality::Personality;
use super::proposal::{Proposal, Reason};
use super::survey::Survey;

const PAYBACK_HORIZON: f64 = 60.0;

const STORE_TRIGGER: f64 = 0.9;

pub struct Economy;

fn yields_on_completion() -> Vec<EntityPattern> {
    vec![
        EntityPattern::Shipyard,
        EntityPattern::Constructor,
        EntityPattern::Storage,
        EntityPattern::Extractor(Material::Metals),
        EntityPattern::Extractor(Material::Volatiles),
        EntityPattern::Extractor(Material::Energy),
    ]
}

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
        Economy::one_at_a_time(survey, proposals)
    }

    fn one_at_a_time(survey: &Survey, proposals: Vec<Proposal>) -> Vec<Proposal> {
        let yielding = yields_on_completion();
        proposals
            .into_iter()
            .filter(|proposal| {
                let (asteroid, pattern) = (proposal.posting.asteroid(), proposal.posting.pattern());
                proposal.count <= survey.count(asteroid, pattern)
                    || !survey.short_of(asteroid, &yielding)
            })
            .collect()
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
        for material in [Material::Metals, Material::Volatiles, Material::Energy] {
            if mix[material] <= 0.0 {
                continue;
            }
            let pattern = EntityPattern::Extractor(material);
            for asteroid in survey.developed() {
                let spare = survey.spare_cap_at(asteroid, material);
                let pulls = pattern.extracts(material).min(spare);
                if pulls * repaying < pattern.cost().total() {
                    continue;
                }
                let standing = survey.count(asteroid, pattern);
                proposals.push(Proposal::at(
                    survey,
                    Reason::Economy,
                    asteroid,
                    pattern,
                    standing + 1,
                ));
            }
        }
        proposals
    }

    fn yards(survey: &Survey) -> Vec<Proposal> {
        let pattern = EntityPattern::Shipyard;
        if survey.build_rate() >= survey.view.income.total() {
            return Vec::new();
        }
        survey
            .building()
            .into_iter()
            .map(|asteroid| {
                let standing = survey.count(asteroid, pattern);
                Proposal::at(survey, Reason::Economy, asteroid, pattern, standing + 1)
            })
            .collect()
    }

    fn stores(survey: &Survey, personality: &Personality) -> Vec<Proposal> {
        let Some(home) = survey.home() else {
            return Vec::new();
        };
        let pattern = EntityPattern::Storage;
        let stockpile = &survey.view.stockpile;
        let full = stockpile.stock().total() >= STORE_TRIGGER * stockpile.capacity().total();
        vec![Proposal::at(
            survey,
            Reason::Economy,
            home,
            pattern,
            personality.stores + u32::from(full),
        )]
    }

    fn masons(
        survey: &Survey,
        personality: &Personality,
        commitments: &Commitments,
    ) -> Vec<Proposal> {
        let Some(home) = survey.home() else {
            return Vec::new();
        };
        let pattern = EntityPattern::Constructor;
        let spare = survey
            .count(home, pattern)
            .saturating_sub(personality.masons);
        let claiming = personality.funding_order.contains(&Reason::Expansion)
            && commitments.claims() < personality.claims
            && survey.short_of_room(personality.demand(survey));
        let count = personality.masons + u32::from(claiming && spare == 0);
        let mut proposals: Vec<Proposal> = survey
            .building()
            .into_iter()
            .filter(|asteroid| *asteroid != home)
            .map(|asteroid| {
                let standing = survey.count(asteroid, pattern);
                Proposal::at(survey, Reason::Economy, asteroid, pattern, standing)
            })
            .collect();
        proposals.push(Proposal::at(survey, Reason::Economy, home, pattern, count));
        proposals
    }
}
