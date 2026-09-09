use neumannarch_sim::state::MAX_WANT;
use neumannarch_sim::{AsteroidId, Posting, RowId};

use super::survey::Survey;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Reason {
    Defence,
    Economy,
    Expansion,
    Offence,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Proposal {
    pub posting: Posting,
    pub count: u32,
    pub reason: Reason,
}

impl Proposal {
    pub fn at(
        survey: &Survey,
        reason: Reason,
        asteroid: AsteroidId,
        row: RowId,
        count: u32,
    ) -> Proposal {
        Proposal {
            posting: survey.posting(asteroid, row),
            count: count.min(MAX_WANT),
            reason,
        }
    }

    pub fn rounded(
        survey: &Survey,
        reason: Reason,
        asteroid: AsteroidId,
        row: RowId,
        count: f64,
    ) -> Proposal {
        let rounded = count.round();
        let whole = match rounded >= 0.0 && rounded <= f64::from(MAX_WANT) {
            true => rounded as u32,
            false => MAX_WANT,
        };
        Proposal::at(survey, reason, asteroid, row, whole)
    }
}
