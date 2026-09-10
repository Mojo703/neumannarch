use std::collections::BTreeMap;

use neumannarch_sim::pattern::{EntityPattern, Kind};
use neumannarch_sim::state::MAX_WANT;
use neumannarch_sim::{Materials, Posting};

use super::proposal::Proposal;
use super::survey::Survey;

const HORIZON_SECONDS: f64 = 20.0;

pub struct Funding {
    budget: Materials,
    free: BTreeMap<EntityPattern, u32>,
}

impl Funding {
    pub fn over(survey: &Survey, proposals: &[Proposal]) -> BTreeMap<Posting, u32> {
        let asked: Vec<&Proposal> = proposals
            .iter()
            .filter(|proposal| !survey.frame_no_builder_fills(proposal.posting))
            .collect();
        let mut intended: BTreeMap<Posting, u32> = BTreeMap::new();
        for proposal in &asked {
            intended.entry(proposal.posting).or_insert(proposal.count);
        }
        let mut funding = Funding::new(survey, &intended);
        let mut wants: BTreeMap<Posting, u32> = BTreeMap::new();
        for proposal in asked {
            if !wants.contains_key(&proposal.posting) && funding.affords(survey, proposal) {
                wants.insert(proposal.posting, proposal.count);
            }
        }
        for posting in survey.postings() {
            let justified = justified(survey, posting);
            if justified > 0 && !wants.contains_key(&posting) {
                wants.insert(posting, justified);
            }
        }
        wants
    }

    pub(super) fn new(survey: &Survey, intended: &BTreeMap<Posting, u32>) -> Funding {
        let mut free: BTreeMap<EntityPattern, u32> = BTreeMap::new();
        for (asteroid, patterns) in &survey.mine {
            let ships = patterns
                .iter()
                .filter(|(pattern, _)| pattern.kind() == Kind::Unit);
            for (pattern, held) in ships {
                let standing = held.present + held.arriving;
                let posting = survey.posting(*asteroid, *pattern);
                let kept = intended.get(&posting).copied().unwrap_or(standing);
                *free.entry(*pattern).or_default() +=
                    standing.saturating_sub(kept).min(held.present);
            }
        }
        let mut budget = survey.view.stockpile.stock() + survey.view.income * HORIZON_SECONDS;
        for (posting, plan) in &survey.view.plans {
            let Some(building) = plan.building else {
                continue;
            };
            budget -= posting.pattern().cost() * (1.0 - building.progress);
        }
        Funding { budget, free }
    }

    pub(super) fn affords(&mut self, survey: &Survey, proposal: &Proposal) -> bool {
        let pattern = proposal.posting.pattern();
        let asteroid = proposal.posting.asteroid();
        let short = proposal
            .count
            .saturating_sub(survey.count(asteroid, pattern));
        let sent = match pattern.kind() {
            Kind::Structure => 0,
            Kind::Unit => short.min(self.spare(pattern)),
        };
        let framed = u32::from(survey.frame_a_builder_fills(proposal.posting));
        let building = short.saturating_sub(sent + framed);
        if building > 0 && !survey.builder_fills(proposal.posting) {
            return false;
        }
        let cost = pattern.cost() * f64::from(building);
        if (self.budget - cost).amounts().any(|(_, left)| left < 0.0) {
            return false;
        }
        self.budget -= cost;
        self.free.insert(pattern, self.spare(pattern) - sent);
        true
    }

    fn spare(&self, pattern: EntityPattern) -> u32 {
        self.free.get(&pattern).copied().unwrap_or_default()
    }
}

pub(super) fn justified(survey: &Survey, posting: Posting) -> u32 {
    if survey.frame_no_builder_fills(posting) {
        return 0;
    }
    let (asteroid, pattern) = (posting.asteroid(), posting.pattern());
    let building = u32::from(survey.frame_open(asteroid, pattern));
    (survey.count(asteroid, pattern) + building).min(MAX_WANT)
}
