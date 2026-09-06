use super::State;
use crate::ids::{SeatId, TeamId};
use crate::state::Motion;

#[derive(Clone, Debug, PartialEq)]
pub struct Standings {
    teams: Vec<Team>,
    over: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Team {
    pub team: TeamId,
    pub rocks: u32,
    pub value: f64,
    pub alive: bool,
}

impl State {
    pub fn standings(&self) -> Standings {
        let mut teams: Vec<TeamId> = self.seats().iter().map(|seat| seat.team()).collect();
        teams.sort_unstable();
        teams.dedup();
        Standings {
            teams: teams.into_iter().map(|team| self.score(team)).collect(),
            over: self.time() >= self.length(),
        }
    }

    fn score(&self, team: TeamId) -> Team {
        let mine = |seat: SeatId| self[seat].team() == team;
        let mut rocks: Vec<_> = self
            .entities()
            .filter(|entity| mine(entity.seat()))
            .filter(|entity| entity.motion() == Motion::Fixed)
            .map(|entity| entity.home())
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
    pub fn new(teams: Vec<Team>, over: bool) -> Standings {
        Standings { teams, over }
    }

    pub fn teams(&self) -> &[Team] {
        &self.teams
    }

    pub fn over(&self) -> bool {
        self.over
    }

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
