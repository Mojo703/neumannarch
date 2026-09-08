use super::State;
use crate::ids::{SeatId, TeamId};

#[derive(Clone, Debug, PartialEq)]
pub struct Standings {
    teams: Vec<Team>,
    over: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Team {
    pub team: TeamId,
    pub asteroids: u32,
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
        let seats = (0..self.seats().len())
            .filter_map(|at| u8::try_from(at).ok())
            .map(SeatId)
            .filter(|seat| mine(*seat));
        let mut asteroids: Vec<_> = seats.flat_map(|seat| self.held_by(seat)).collect();
        asteroids.sort_unstable();
        asteroids.dedup();
        Team {
            team,
            asteroids: asteroids.len() as u32,
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
            .map(|team| (team.asteroids, team.value))
            .max_by(|a, b| a.0.cmp(&b.0).then(a.1.total_cmp(&b.1)));
        best.map_or(Vec::new(), |best| {
            self.teams
                .iter()
                .filter(|team| (team.asteroids, team.value) == best)
                .map(|team| team.team)
                .collect()
        })
    }
}
