use std::collections::BTreeMap;

use neumannarch_sim::state::MAX_WANT;
use neumannarch_sim::{Materials, Posting, RowId};

use super::proposal::Proposal;
use super::survey::Survey;

const HORIZON_SECONDS: f64 = 20.0;

pub struct Funding {
    budget: Materials,
    free: BTreeMap<RowId, u32>,
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
        let mut free: BTreeMap<RowId, u32> = BTreeMap::new();
        for (asteroid, rows) in &survey.mine {
            for (row, held) in rows.iter().filter(|(row, _)| !survey.is_structure(**row)) {
                let standing = held.present + held.arriving;
                let posting = survey.posting(*asteroid, *row);
                let kept = intended.get(&posting).copied().unwrap_or(standing);
                *free.entry(*row).or_default() += standing.saturating_sub(kept).min(held.present);
            }
        }
        let mut budget = survey.view.stockpile.stock() + survey.view.income * HORIZON_SECONDS;
        for (posting, plan) in &survey.view.plans {
            let (Some(building), Some(stats)) = (plan.building, survey.roster.get(posting.row()))
            else {
                continue;
            };
            budget -= stats.cost * (1.0 - building.progress);
        }
        Funding { budget, free }
    }

    pub(super) fn affords(&mut self, survey: &Survey, proposal: &Proposal) -> bool {
        let row = proposal.posting.row();
        let asteroid = proposal.posting.asteroid();
        let short = proposal.count.saturating_sub(survey.count(asteroid, row));
        let sent = match survey.is_structure(row) {
            true => 0,
            false => short.min(self.spare(row)),
        };
        let framed = u32::from(survey.frame_open(asteroid, row) && survey.builds_at(asteroid));
        let building = short.saturating_sub(sent + framed);
        if building > 0 && !survey.builds_at(asteroid) {
            return false;
        }
        let Some(cost) = survey
            .roster
            .get(row)
            .map(|stats| stats.cost * f64::from(building))
        else {
            return false;
        };
        if (self.budget - cost).amounts().any(|(_, left)| left < 0.0) {
            return false;
        }
        self.budget -= cost;
        self.free.insert(row, self.spare(row) - sent);
        true
    }

    fn spare(&self, row: RowId) -> u32 {
        self.free.get(&row).copied().unwrap_or_default()
    }
}

pub(super) fn justified(survey: &Survey, posting: Posting) -> u32 {
    if survey.frame_no_builder_fills(posting) {
        return 0;
    }
    let (asteroid, row) = (posting.asteroid(), posting.row());
    let building = u32::from(survey.frame_open(asteroid, row));
    (survey.count(asteroid, row) + building).min(MAX_WANT)
}
