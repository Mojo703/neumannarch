use neumannarch_sim::AsteroidId;

use super::commitments::Commitments;
use super::dice::Dice;
use super::personality::Personality;
use super::proposal::{Proposal, Reason};
use super::ranking::Ranking;
use super::survey::{REACH_METERS, Survey};

const CHOICES: usize = 2;

pub struct Expansion;

impl Expansion {
    pub fn proposals(
        survey: &Survey,
        personality: &Personality,
        commitments: &mut Commitments,
        dice: &mut Dice,
    ) -> Vec<Proposal> {
        let Some(row) = survey.roles.masons.first().copied() else {
            return Vec::new();
        };
        let mut proposals: Vec<Proposal> = commitments
            .claimed_asteroids()
            .into_iter()
            .map(|asteroid| {
                let homed = survey.count(asteroid, row).max(1);
                Proposal::at(survey, Reason::Expansion, asteroid, row, homed)
            })
            .collect();
        if commitments.claims() < personality.claims
            && survey.short_of_room(personality.demand(survey))
            && let Some(asteroid) = Expansion::best_free(survey, commitments, dice)
        {
            commitments.claim(asteroid, survey.view.time);
            proposals.push(Proposal::at(survey, Reason::Expansion, asteroid, row, 1));
        }
        proposals
    }

    fn best_free(
        survey: &Survey,
        commitments: &Commitments,
        dice: &mut Dice,
    ) -> Option<AsteroidId> {
        let from = survey.home()?;
        let occupied = survey.occupied();
        let rated = Ranking::by(
            survey
                .view
                .terrain
                .iter()
                .map(|terrain| terrain.asteroid)
                .filter(|asteroid| !occupied.contains(asteroid))
                .filter(|asteroid| {
                    !commitments.claimed(*asteroid) && !commitments.barred(*asteroid)
                })
                .filter(|asteroid| !survey.enemy_asteroids.contains(asteroid)),
            |asteroid| {
                let caps = survey.view.terrain_of(asteroid)?.caps.total();
                Some(caps / (1.0 + survey.between(from, asteroid) / REACH_METERS))
            },
        )
        .order();
        let at = dice.below(rated.len().min(CHOICES))?;
        rated.get(at).copied()
    }
}
