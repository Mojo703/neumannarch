use std::collections::BTreeMap;

use neumannarch_sim::roster::{Kind, Roster};
use neumannarch_sim::state::view::View;
use neumannarch_sim::{AsteroidId, Time};

const CLAIM_PATIENCE: f64 = 90.0;

const CLAIM_BAR: f64 = 120.0;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Commitments {
    claims: BTreeMap<AsteroidId, Time>,
    barred: BTreeMap<AsteroidId, Time>,
    pub committed: Option<AsteroidId>,
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

    pub fn settle(&mut self, view: &View, roster: &Roster) {
        let mut mine = Vec::new();
        let mut held = Vec::new();
        for own in view.present.iter().filter(|own| own.seat == view.seat) {
            mine.push(own.home);
            if roster
                .get(own.row)
                .is_some_and(|row| row.kind() == Kind::Structure)
            {
                held.push(own.home);
            }
        }
        let now = view.time.seconds();
        let mut lapsed = Vec::new();
        self.claims.retain(|asteroid, at| {
            if held.contains(asteroid) {
                return false;
            }
            if mine.contains(asteroid) {
                return true;
            }
            if now - at.seconds() >= CLAIM_PATIENCE {
                lapsed.push(*asteroid);
                return false;
            }
            true
        });
        for asteroid in lapsed {
            self.barred.insert(asteroid, view.time);
        }
        self.barred.retain(|_, at| now - at.seconds() < CLAIM_BAR);
    }
}
