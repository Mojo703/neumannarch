use std::collections::BTreeMap;

use neumannarch_sim::{AsteroidId, Time};

use crate::survey::Survey;

const CLAIM_PATIENCE: f64 = 90.0;

const CLAIM_BAR: f64 = 120.0;

const SAVING_BEGINS: f64 = 1.5;

const SAVING_ENDS: f64 = 1.1;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Commitments {
    claims: BTreeMap<AsteroidId, Time>,
    barred: BTreeMap<AsteroidId, Time>,
    pub committed: Option<AsteroidId>,
    pub holding_back_army: bool,
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
        self.holding_back_army = self.saving(survey);
    }

    fn saving(&self, survey: &Survey) -> bool {
        let own = survey.army();
        let enemy = survey.enemy();
        let rising = survey.view.income.total() > survey.view.spend.total();
        match self.holding_back_army {
            false => own > 0.0 && own > SAVING_BEGINS * enemy && rising,
            true => own >= SAVING_ENDS * enemy,
        }
    }
}
