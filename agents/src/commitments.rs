use std::collections::BTreeMap;

use neumannarch_sim::roster::{Kind, Roster};
use neumannarch_sim::state::view::View;
use neumannarch_sim::{RockId, Time};

const CLAIM_PATIENCE: f64 = 90.0;

const CLAIM_BAR: f64 = 120.0;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Commitments {
    claims: BTreeMap<RockId, Time>,
    barred: BTreeMap<RockId, Time>,
    pub committed: Option<RockId>,
}

impl Commitments {
    pub fn claimed(&self, rock: RockId) -> bool {
        self.claims.contains_key(&rock)
    }

    pub fn claims(&self) -> usize {
        self.claims.len()
    }

    pub fn barred(&self, rock: RockId) -> bool {
        self.barred.contains_key(&rock)
    }

    pub fn claim(&mut self, rock: RockId, at: Time) {
        self.claims.insert(rock, at);
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
        self.claims.retain(|rock, at| {
            if held.contains(rock) {
                return false;
            }
            if mine.contains(rock) {
                return true;
            }
            if now - at.seconds() >= CLAIM_PATIENCE {
                lapsed.push(*rock);
                return false;
            }
            true
        });
        for rock in lapsed {
            self.barred.insert(rock, view.time);
        }
        self.barred.retain(|_, at| now - at.seconds() < CLAIM_BAR);
    }
}
