use std::collections::BTreeMap;

use neumannarch_sim::{AsteroidId, Time};

use super::survey::Survey;

const CLAIM_PATIENCE: f64 = 90.0;

const CLAIM_BAR: f64 = 120.0;

pub(super) const AHEAD_BEGINS: f64 = 1.5;

pub(super) const AHEAD_ENDS: f64 = 1.1;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Commitments {
    claims: BTreeMap<AsteroidId, Time>,
    barred: BTreeMap<AsteroidId, Time>,
    pub committed: Option<AsteroidId>,
    pub army_ahead: bool,
}

impl Commitments {
    pub fn claimed(&self, asteroid: AsteroidId) -> bool {
        self.claims.contains_key(&asteroid)
    }

    pub fn claims(&self) -> usize {
        self.claims.len()
    }

    pub fn barred(&self, asteroid: AsteroidId) -> bool {
        self.barred.contains_key(&asteroid)
    }

    pub fn claim(&mut self, asteroid: AsteroidId, at: Time) {
        self.claims.insert(asteroid, at);
    }

    pub fn claimed_asteroids(&self) -> Vec<AsteroidId> {
        self.claims.keys().copied().collect()
    }

    pub fn settle(&mut self, survey: &Survey) {
        let now = survey.view.time.seconds();
        let mut lapsed = Vec::new();
        self.claims.retain(|asteroid, at| {
            if !survey.wanting_more_at(*asteroid) {
                return false;
            }
            if survey.homed_at(*asteroid) > 0 {
                return true;
            }
            if now - at.seconds() >= CLAIM_PATIENCE {
                lapsed.push(*asteroid);
                return false;
            }
            true
        });
        for asteroid in lapsed {
            self.barred.insert(asteroid, survey.view.time);
        }
        self.barred.retain(|_, at| now - at.seconds() < CLAIM_BAR);
        if self
            .committed
            .is_some_and(|asteroid| !survey.enemy_asteroids.contains(&asteroid))
        {
            self.committed = None;
        }
        self.army_ahead = self.ahead(survey);
    }

    fn ahead(&self, survey: &Survey) -> bool {
        let own = survey.army();
        let enemy = survey.enemy();
        match self.army_ahead {
            false => own > 0.0 && own > AHEAD_BEGINS * enemy,
            true => own >= AHEAD_ENDS * enemy,
        }
    }
}
