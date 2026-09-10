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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixture::World;
    use crate::ids::AsteroidId;
    use crate::orbit::body::Gravity;
    use crate::roster::{RAIDER, STORAGE};

    const GRAVITY: Gravity = Gravity::new(4.0e13);

    const MINE: TeamId = TeamId(0);

    const THEIRS: TeamId = TeamId(1);

    fn world() -> World {
        World::ring(GRAVITY, 4, &[MINE, THEIRS])
    }

    fn scored(standings: &Standings, team: TeamId) -> Team {
        *standings
            .teams()
            .iter()
            .find(|scored| scored.team == team)
            .expect("the team stands in the standings")
    }

    #[test]
    fn the_side_holding_the_most_asteroids_leads() {
        let mut world = world();
        world.fix(0, STORAGE, AsteroidId(0));
        world.fix(0, STORAGE, AsteroidId(1));
        world.fix(1, STORAGE, AsteroidId(2));
        world.hold(1, RAIDER, AsteroidId(2), 5.0);

        let standings = world.state.standings();

        assert_eq!(scored(&standings, MINE).asteroids, 2);
        assert_eq!(scored(&standings, THEIRS).asteroids, 1);
        assert_eq!(
            standings.leaders(),
            vec![MINE],
            "the smaller army holding more asteroids lost the match"
        );
    }

    #[test]
    fn an_asteroid_counts_for_a_side_with_a_structure_there() {
        let mut world = world();
        world.fix(0, STORAGE, AsteroidId(0));
        world.hold(1, RAIDER, AsteroidId(1), 5.0);
        world.hold(1, RAIDER, AsteroidId(2), 5.0);

        let standings = world.state.standings();

        assert_eq!(scored(&standings, MINE).asteroids, 1);
        assert_eq!(
            scored(&standings, THEIRS).asteroids,
            0,
            "a side standing units at an asteroid holds it without a structure"
        );
        assert_eq!(standings.leaders(), vec![MINE]);
    }

    #[test]
    fn sides_holding_as_many_asteroids_break_by_total_army_value() {
        let mut world = world();
        world.fix(0, STORAGE, AsteroidId(0));
        world.fix(1, STORAGE, AsteroidId(1));
        world.hold(1, RAIDER, AsteroidId(1), 5.0);

        let standings = world.state.standings();

        assert_eq!(scored(&standings, MINE).asteroids, 1);
        assert_eq!(scored(&standings, THEIRS).asteroids, 1);
        assert_eq!(
            scored(&standings, MINE).value,
            40.0,
            "a side's value is what its own entities cost"
        );
        assert_eq!(scored(&standings, THEIRS).value, 85.0);
        assert_eq!(
            standings.leaders(),
            vec![THEIRS],
            "the tie stood undecided or fell the wrong way"
        );
    }
}
