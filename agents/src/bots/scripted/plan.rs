use std::cmp::Ordering;
use std::collections::BTreeMap;

use neumannarch_sim::Posting;
use neumannarch_sim::state::view::View;
use neumannarch_sim::state::{Command, MAX_COMMANDS_PER_TICK};

use super::commitments::Commitments;
use super::defence::Defence;
use super::dice::Dice;
use super::economy::Economy;
use super::expansion::Expansion;
use super::funding::Funding;
use super::offence::Offence;
use super::personality::Personality;
use super::proposal::{Proposal, Reason};
use super::survey::Survey;

#[derive(Clone, Debug, PartialEq)]
pub struct Plan {
    pub(super) wants: BTreeMap<Posting, u32>,
}

impl Plan {
    pub fn of(
        survey: &Survey,
        personality: &Personality,
        commitments: &mut Commitments,
        dice: &mut Dice,
    ) -> Plan {
        match survey.view.draft.ended() {
            None => Plan::drafting(survey, personality),
            Some(_) => Plan::funded(survey, personality, commitments, dice),
        }
    }

    pub fn commands(&self, view: &View) -> Vec<Command> {
        let dropped: Vec<Command> = view
            .plans
            .iter()
            .filter(|(posting, plan)| plan.want > 0 && !self.wants.contains_key(posting))
            .map(|(posting, _)| want(*posting, 0))
            .collect();
        let mut lowered: Vec<Command> = Vec::new();
        let mut raised: Vec<Command> = Vec::new();
        for (posting, count) in &self.wants {
            match count.cmp(&view.want_of(*posting)) {
                Ordering::Less => lowered.push(want(*posting, *count)),
                Ordering::Greater => raised.push(want(*posting, *count)),
                Ordering::Equal => (),
            }
        }
        dropped
            .into_iter()
            .chain(lowered)
            .chain(raised)
            .take(MAX_COMMANDS_PER_TICK)
            .collect()
    }

    fn drafting(survey: &Survey, personality: &Personality) -> Plan {
        let mut wants: BTreeMap<Posting, u32> = BTreeMap::new();
        for (asteroid, rows) in &survey.mine {
            for (row, held) in rows.iter().filter(|(row, _)| survey.is_structure(**row)) {
                if held.present > 0 {
                    wants.insert(survey.posting(*asteroid, *row), held.present);
                }
            }
        }
        let staged = survey
            .view
            .draft
            .running()
            .filter(|stage| stage.seat == survey.view.seat);
        if let Some(stage) = staged
            && let Some(asteroid) = survey.fittest(personality.wanted_shares(survey))
        {
            wants.insert(survey.posting(asteroid, stage.row), 1);
        }
        Plan { wants }
    }

    fn funded(
        survey: &Survey,
        personality: &Personality,
        commitments: &mut Commitments,
        dice: &mut Dice,
    ) -> Plan {
        let order = personality.order(survey.threatened(), commitments.army_ahead);
        let mut proposals: Vec<Proposal> = Vec::new();
        for reason in &order {
            proposals.extend(match reason {
                Reason::Defence => Defence::proposals(survey, personality),
                Reason::Economy => Economy::proposals(survey, personality, commitments),
                Reason::Expansion => Expansion::proposals(survey, personality, commitments, dice),
                Reason::Offence => Offence::proposals(survey, personality, commitments),
            });
        }
        proposals.sort_by_key(|proposal| (ranked(&order, proposal.reason), proposal.posting));
        Plan {
            wants: Funding::over(survey, &proposals),
        }
    }
}

fn ranked(order: &[Reason], reason: Reason) -> usize {
    order
        .iter()
        .position(|held| *held == reason)
        .unwrap_or(order.len())
}

pub(super) fn want(posting: Posting, count: u32) -> Command {
    Command::Want {
        asteroid: posting.asteroid(),
        row: posting.row(),
        count,
    }
}
