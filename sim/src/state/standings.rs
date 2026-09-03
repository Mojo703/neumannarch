//! The score: rocks held and army value per team.

use super::State;
use crate::ids::{SeatId, TeamId};
use crate::place::Band;
use crate::state::Motion;

/// The score at one tick, one entry per team in team order.
#[derive(Clone, Debug, PartialEq)]
pub struct Standings {
    teams: Vec<Team>,
    over: bool,
}

/// One team's score.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Team {
    pub team: TeamId,
    /// Rocks where the team has a structure.
    pub rocks: u32,
    /// The cost total of every living entity of the team.
    pub value: f64,
    /// True while any seat of the team is in the match.
    pub alive: bool,
}

impl State {
    /// The score now: rocks held and army value per team, and whether the
    /// clock has run out.
    pub fn standings(&self) -> Standings {
        let mut teams: Vec<TeamId> = self.seats().iter().map(|seat| seat.team()).collect();
        teams.sort_unstable();
        teams.dedup();
        Standings {
            teams: teams.into_iter().map(|team| self.score(team)).collect(),
            over: self.tick() >= self.clock(),
        }
    }

    /// `team`'s rocks held, army value and whether it is still in.
    fn score(&self, team: TeamId) -> Team {
        let mine = |seat: SeatId| self[seat].team() == team;
        let mut rocks: Vec<_> = self
            .entities()
            .filter(|entity| mine(entity.seat()))
            .filter(|entity| entity.motion() == Motion::Fixed && entity.home().band == Band::Inner)
            .map(|entity| entity.home().rock)
            .collect();
        rocks.sort_unstable();
        rocks.dedup();
        Team {
            team,
            rocks: rocks.len() as u32,
            value: self
                .entities()
                .filter(|entity| mine(entity.seat()))
                .map(|entity| self[entity.row()].cost.total())
                .sum(),
            alive: self
                .seats()
                .iter()
                .any(|seat| seat.team() == team && seat.alive()),
        }
    }
}

impl Standings {
    /// Every team's score, in team order.
    pub fn teams(&self) -> &[Team] {
        &self.teams
    }

    /// True once the clock has run out.
    pub fn over(&self) -> bool {
        self.over
    }

    /// The teams holding the most rocks, army value breaking a tie; empty
    /// only when the match has no team.
    pub fn leaders(&self) -> Vec<TeamId> {
        let best = self
            .teams
            .iter()
            .map(|team| (team.rocks, team.value))
            .max_by(|a, b| a.0.cmp(&b.0).then(a.1.total_cmp(&b.1)));
        best.map_or(Vec::new(), |best| {
            self.teams
                .iter()
                .filter(|team| (team.rocks, team.value) == best)
                .map(|team| team.team)
                .collect()
        })
    }
}
